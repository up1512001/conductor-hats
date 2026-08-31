//! Durable run-setting commands applied through Conductor's own visible controls.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::{auth, conductor_session, id, lock, paths, remote_control_result, remote_scan};

pub use crate::remote_control_result::applied;

const LEASE: Duration = Duration::from_secs(60);

/// A next-message setting is no longer intent after fifteen minutes.
const EXPIRY: Duration = Duration::from_secs(15 * 60);

/// How many times the panel may fail to apply one setting before it is dropped.
const ATTEMPTS: u32 = 2;

#[derive(Clone, serde::Deserialize, serde::Serialize)]
pub struct Control {
    pub id: String,
    pub session: String,
    pub setting: String,
    pub value: String,
    pub before: String,
    pub created_at: u64,
    pub lease: String,
    pub lease_until: u64,
    #[serde(default)]
    pub attempts: u32,
    #[serde(default)]
    pub done: bool,
    #[serde(default)]
    pub marker: u64,
    #[serde(default)]
    pub result: String,
    #[serde(default)]
    pub error: String,
}

fn root() -> PathBuf {
    paths::accounts_root().join("remote-controls")
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn directory(session: &str) -> Result<PathBuf, String> {
    let session = id::session(session).ok_or("invalid chat id")?;
    if conductor_session::route(session).is_none() {
        return Err("that chat is no longer open".into());
    }
    let path = root().join(session);
    std::fs::create_dir_all(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    private(&root())?;
    private(&path)?;
    Ok(path)
}

fn private(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
        .map_err(|e| format!("{}: {e}", path.display()))
}

fn files(dir: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("json"))
        .collect();
    found.sort();
    found
}

/// Whether this setting still stands: recent enough, and not repeatedly refused.
fn live(control: &Control) -> bool {
    control.created_at + EXPIRY.as_millis() as u64 > now()
        && (control.done || control.attempts < ATTEMPTS)
}

fn read(path: &Path) -> Option<Control> {
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

fn live_read(path: &Path) -> Option<Control> {
    let item = read(path);
    if item.as_ref().is_some_and(live) {
        item
    } else {
        let _ = std::fs::remove_file(path);
        None
    }
}

fn write(path: &Path, control: &Control) -> Result<(), String> {
    use std::os::unix::fs::OpenOptionsExt;
    let body = serde_json::to_vec(control).map_err(|e| format!("control: {e}"))?;
    let temp = path.with_extension(format!("{}.tmp", std::process::id()));
    let _ = std::fs::remove_file(&temp);
    let result = (|| -> std::io::Result<()> {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temp)?;
        file.write_all(&body)?;
        file.sync_all()?;
        std::fs::rename(&temp, path)
    })();
    if let Err(error) = result {
        let _ = std::fs::remove_file(&temp);
        return Err(format!("{}: {error}", path.display()));
    }
    Ok(())
}

pub fn enqueue(session: &str, setting: &str, value: &str, before: &str) -> Result<Control, String> {
    if !remote_control_result::valid(setting, value) {
        return Err("invalid run setting".into());
    }
    let dir = directory(session)?;
    let _guard = lock::Lock::acquire(&dir.join("queue"))?;
    for path in files(&dir) {
        if let Some(existing) = read(&path) {
            if existing.setting == setting {
                std::fs::remove_file(path).map_err(|e| format!("replacing control: {e}"))?;
            }
        }
    }
    let control = Control {
        id: auth::random_hex(16)?,
        session: session.to_string(),
        setting: setting.to_string(),
        value: value.to_string(),
        before: before.to_string(),
        created_at: now(),
        lease: String::new(),
        lease_until: 0,
        attempts: 0,
        done: false,
        marker: conductor_session::workspace_marker(session).unwrap_or(0),
        result: String::new(),
        error: String::new(),
    };
    write(&dir.join(format!("{}.json", control.id)), &control)?;
    Ok(control)
}

pub fn claim(session: &str) -> Result<Option<Control>, String> {
    let dir = directory(session)?;
    let _guard = lock::Lock::acquire(&dir.join("queue"))?;
    for path in files(&dir) {
        let Some(mut control) = live_read(&path) else {
            continue;
        };
        if control.done {
            continue;
        }
        if control.lease_until > now() {
            return Ok(None);
        }
        control.lease = auth::random_hex(16)?;
        control.lease_until = now() + LEASE.as_millis() as u64;
        write(&path, &control)?;
        return Ok(Some(control));
    }
    Ok(None)
}

fn path(session: &str, id: &str) -> Result<PathBuf, String> {
    if id.len() != 32 || !id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("invalid control id".into());
    }
    Ok(directory(session)?.join(format!("{id}.json")))
}

pub fn finish(control: &Control, completed: bool) -> Result<(), String> {
    let file = path(&control.session, &control.id)?;
    let dir = file.parent().ok_or("invalid control path")?;
    let _guard = lock::Lock::acquire(&dir.join("queue"))?;
    let Some(mut current) = read(&file) else {
        return Ok(());
    };
    if !auth::same(&current.lease, &control.lease) {
        return Err("that control lease is no longer active".into());
    }
    if completed {
        let result = remote_control_result::applied_session(&current)
            .ok_or("Conductor has not recorded the run setting yet")?;
        if result == current.session {
            std::fs::remove_file(file).map_err(|e| format!("completing control: {e}"))
        } else {
            current.lease.clear();
            current.lease_until = 0;
            current.done = true;
            current.result = result;
            write(&file, &current)
        }
    } else {
        current.lease.clear();
        current.lease_until = 0;
        current.attempts += 1;
        if current.attempts >= ATTEMPTS {
            current.done = true;
            current.error = format!(
                "Conductor did not apply {}={} after {ATTEMPTS} attempts",
                current.setting, current.value
            );
        }
        write(&file, &current)
    }
}

pub fn sessions() -> Vec<(u64, String)> {
    let Ok(sessions) = std::fs::read_dir(root()) else {
        return Vec::new();
    };
    let mut found: Vec<(u64, String)> = sessions
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .flat_map(|entry| files(&entry.path()))
        .filter_map(|path| live_read(&path))
        .filter(|control| live(control) && !control.done && id::session(&control.session).is_some())
        .map(|control| (control.created_at, control.session))
        .collect();
    found.sort();
    found
}

/// Drops every queued run setting that has expired or been refused too often.
pub fn purge() -> usize {
    let mut dropped = 0;
    for dir in std::fs::read_dir(root()).into_iter().flatten().flatten() {
        for path in files(&dir.path()) {
            let stale = read(&path).map(|control| !live(&control)).unwrap_or(true);
            if stale && std::fs::remove_file(&path).is_ok() {
                dropped += 1;
            }
        }
    }
    dropped
}

pub fn pending_json(session: &str) -> Result<serde_json::Value, String> {
    let dir = directory(session)?;
    let _guard = lock::Lock::acquire(&dir.join("queue"))?;
    let mut items = Vec::new();
    for path in files(&dir) {
        if let Some(item) = live_read(&path) {
            items.push(item);
        }
    }
    items.sort_by_key(|item| item.created_at);
    Ok(serde_json::Value::Array(
        items
            .into_iter()
            .map(|item| {
                serde_json::json!({
                    "setting": item.setting,
                    "value": item.value,
                    "id": item.id,
                    "error": item.error,
                    "result": item.result,
                    "state": if item.done && item.error.is_empty() { "done" } else if item.done { "failed" } else if item.lease_until > now() { "applying" } else { "waiting" },
                })
            })
            .collect(),
    ))
}

pub fn ack(session: &str, id: &str) -> Result<(), String> {
    let dir = directory(session)?;
    let _guard = lock::Lock::acquire(&dir.join("queue"))?;
    let file = path(session, id)?;
    let Some(item) = read(&file) else {
        return Ok(());
    };
    if !item.done {
        return Err("that run setting is still active".into());
    }
    std::fs::remove_file(file).map_err(|e| format!("acknowledging run setting: {e}"))
}

pub fn stamp() -> String {
    let paths = std::fs::read_dir(root())
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .flat_map(|entry| files(&entry.path()));
    remote_scan::metadata_stamp(paths)
}
