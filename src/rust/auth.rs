//! Pairing a browser and keeping its session private.
//!
//! The browser receives a short-lived, one-use token in a URL fragment. A
//! fragment is never sent in the HTTP request, proxy logs or referrers. The page
//! trades it over a request header for a separate persistent session cookie,
//! which authenticates the same-origin WebSocket handshake without putting a
//! credential in its URL. Both secrets are 256 random bits and files are mode
//! `0600`.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::{lock, mobile_catalog, mobile_scope, paths, source};

pub const COOKIE: &str = "__Host-hats_session";
const PAIR_TTL: Duration = Duration::from_secs(10 * 60);

fn session_path() -> PathBuf {
    paths::accounts_root().join("serve-session")
}

fn pairing_path() -> PathBuf {
    paths::accounts_root().join("serve-pairing")
}

#[derive(serde::Serialize)]
pub struct PairingInfo {
    pub origin: String,
    pub path: String,
    pub url: String,
    pub expires_at: u64,
}

#[derive(serde::Deserialize, serde::Serialize)]
struct PairingRecord {
    token: String,
    route: String,
}

/// Cryptographically random bytes in a printable form.
pub fn random_hex(bytes: usize) -> Result<String, String> {
    let mut source =
        std::fs::File::open("/dev/urandom").map_err(|e| format!("/dev/urandom: {e}"))?;
    let mut raw = vec![0u8; bytes];
    source
        .read_exact(&mut raw)
        .map_err(|e| format!("/dev/urandom: {e}"))?;
    Ok(raw.iter().map(|b| format!("{b:02x}")).collect())
}

fn valid(secret: &str) -> bool {
    secret.len() == 64 && secret.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Writes a secret without ever leaving a permissive version on disk.
pub(crate) fn write_private(path: &Path, value: &str) -> Result<(), String> {
    let parent = path.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    let tmp = path.with_file_name(format!(
        ".{}.{}.tmp",
        path.file_name().unwrap_or_default().to_string_lossy(),
        std::process::id()
    ));
    let _ = std::fs::remove_file(&tmp);
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let result = (|| -> std::io::Result<()> {
        let mut file = options.open(&tmp)?;
        writeln!(file, "{value}")?;
        file.sync_all()?;
        std::fs::rename(&tmp, path)
    })();
    if let Err(e) = result {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("{}: {e}", path.display()));
    }
    Ok(())
}

fn read_secret(path: &Path) -> Option<String> {
    let found = paths::first_line(path)?;
    valid(&found).then_some(found)
}

fn read_pairing(path: &Path) -> Option<PairingRecord> {
    let record = serde_json::from_slice::<PairingRecord>(&std::fs::read(path).ok()?).ok()?;
    (valid(&record.token) && valid(&record.route)).then_some(record)
}

fn fresh(path: &Path) -> bool {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .and_then(|at| {
            SystemTime::now()
                .duration_since(at)
                .map_err(std::io::Error::other)
        })
        .map(|age| age < PAIR_TTL)
        .unwrap_or(false)
}

/// The long-lived browser session key, stable until explicitly revoked.
pub fn session() -> Result<String, String> {
    let path = session_path();
    let _guard = lock::Lock::acquire(&path)?;
    if let Some(found) = read_secret(&path) {
        set_private_mode(&path)?;
        return Ok(found);
    }
    let made = random_hex(32)?;
    write_private(&path, &made)?;
    Ok(made)
}

/// The current ten-minute token and its unguessable public path.
fn pairing() -> Result<PairingRecord, String> {
    let path = pairing_path();
    let _guard = lock::Lock::acquire(&path)?;
    if fresh(&path) {
        if let Some(found) = read_pairing(&path) {
            set_private_mode(&path)?;
            return Ok(found);
        }
    }
    let made = PairingRecord {
        token: random_hex(32)?,
        route: random_hex(32)?,
    };
    write_private(
        &path,
        &serde_json::to_string(&made).map_err(|e| format!("pairing: {e}"))?,
    )?;
    Ok(made)
}

fn pairing_expiry() -> u64 {
    std::fs::metadata(pairing_path())
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|made| made.checked_add(PAIR_TTL))
        .and_then(|expires| expires.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

/// Uses the pairing token exactly once, atomically across server threads.
pub fn consume_pairing(offered: &str) -> Result<bool, String> {
    let path = pairing_path();
    let _guard = lock::Lock::acquire(&path)?;
    let accepted = fresh(&path)
        && read_pairing(&path)
            .map(|expected| same(offered, &expected.token))
            .unwrap_or(false);
    if accepted {
        std::fs::remove_file(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(accepted)
}

/// Invalidates every paired browser and creates a new pairing route and token.
fn revoke() -> Result<PairingRecord, String> {
    let session_path = session_path();
    {
        let _guard = lock::Lock::acquire(&session_path)?;
        write_private(&session_path, &random_hex(32)?)?;
    }
    let pair_path = pairing_path();
    {
        let _guard = lock::Lock::acquire(&pair_path)?;
        let _ = std::fs::remove_file(&pair_path);
    }
    pairing()
}

/// Invalidates every browser and removes any unconsumed pairing link.
pub fn invalidate_all() -> Result<(), String> {
    let session_path = session_path();
    {
        let _guard = lock::Lock::acquire(&session_path)?;
        write_private(&session_path, &random_hex(32)?)?;
    }
    let pair_path = pairing_path();
    {
        let _guard = lock::Lock::acquire(&pair_path)?;
        let _ = std::fs::remove_file(&pair_path);
    }
    mobile_catalog::clear();
    mobile_scope::clear();
    Ok(())
}

fn info(origin: String, pairing: PairingRecord) -> PairingInfo {
    let path = format!("/{}", pairing.route);
    PairingInfo {
        url: format!("{origin}{path}#token={}", pairing.token),
        origin,
        path,
        expires_at: pairing_expiry(),
    }
}

pub fn active_pairing() -> Result<Option<PairingInfo>, String> {
    let Some(origin) = crate::origin::configured() else {
        return Ok(None);
    };
    let path = pairing_path();
    let _guard = lock::Lock::acquire(&path)?;
    if !fresh(&path) {
        return Ok(None);
    }
    Ok(read_pairing(&path).map(|pairing| info(origin, pairing)))
}

pub fn active_pairing_for(source: &source::Source) -> Result<Option<PairingInfo>, String> {
    if !mobile_scope::matches(source) {
        return Ok(None);
    }
    active_pairing()
}

pub fn mobile_pair(source: &source::Source, revoking: bool) -> Result<PairingInfo, String> {
    let origin = crate::origin::configured()
        .ok_or("set the public HTTPS address before creating a pairing code")?;
    let changed = !mobile_scope::matches(source);
    let pairing = pairing_for(&origin, revoking || changed)?;
    mobile_scope::bind(source)?;
    Ok(pairing)
}

pub fn pairing_for(origin: &str, revoking: bool) -> Result<PairingInfo, String> {
    let origin = crate::origin::normalized(origin)?;
    let pairing = if revoking { revoke()? } else { pairing()? };
    Ok(info(origin, pairing))
}

pub fn is_pairing_path(path: &str) -> bool {
    path.strip_prefix('/').is_some_and(valid)
}

pub fn route_matches(path: &str) -> bool {
    let Some(offered) = path.strip_prefix('/').filter(|route| valid(route)) else {
        return false;
    };
    let path = pairing_path();
    let Ok(_guard) = lock::Lock::acquire(&path) else {
        return false;
    };
    fresh(&path)
        && read_pairing(&path)
            .map(|pairing| same(offered, &pairing.route))
            .unwrap_or(false)
}

pub(crate) fn set_private_mode(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = std::fs::Permissions::from_mode(0o600);
        std::fs::set_permissions(path, permissions)
            .map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(())
}

/// Constant-work comparison for equal-length secrets.
pub fn same(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let mut differences = (a.len() ^ b.len()) as u8;
    let width = a.len().max(b.len());
    for index in 0..width {
        differences |= a.get(index).copied().unwrap_or(0) ^ b.get(index).copied().unwrap_or(0);
    }
    differences == 0
}

/// Pulls one cookie out of a `Cookie:` header.
pub fn cookie(header: &str, want: &str) -> Option<String> {
    header.split(';').find_map(|pair| {
        let (name, value) = pair.trim().split_once('=')?;
        (name == want).then(|| value.trim().to_string())
    })
}

pub fn set_cookie(value: &str) -> String {
    format!("{COOKIE}={value}; Path=/; HttpOnly; SameSite=Strict; Secure; Max-Age=2592000")
}
