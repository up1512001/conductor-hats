//! Building an isolated Conductor copy that is safe to modify.
//!
//! The copy gets its own bundle identifier, so its own Application Support
//! directory and its own keychain items. The identifier is compiled into the
//! binaries, not just Info.plist, so changing the plist alone leaves the copy
//! writing to the real app's database.
//!
//! `com.conductor.dev` is the same length as `com.conductor.app`, so every
//! substitution is byte for byte with no offsets or length fields to repair.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::sign;

const OLD_ID: &[u8] = b"com.conductor.app";
const PRODUCT: &[u8] = b"Conductor";
const REAL_SCHEME: &str = "conductor";
const COPY_SCHEME: &str = "conductor-dev";
const LSREGISTER: &str = "/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister";

pub struct Options {
    pub src: PathBuf,
    pub dst: PathBuf,
    pub id: String,
    pub force: bool,
}

/// Occurrences of the identifier are chosen by what surrounds them, never
/// replaced blindly. It appears four times in the Tauri binary and only one is
/// the config string that builds the Application Support path. Two live inside
/// the code signature, which codesign rewrites anyway, and one sits in an encoded
/// string table where editing raw bytes corrupts what comes back out: a blind
/// replace produced a data directory called `com.conductor.dep`.
fn patch_bytes(
    path: &Path,
    new_id: &[u8],
    pick: impl Fn(&[u8], usize) -> bool,
) -> Result<usize, String> {
    if !path.is_file() {
        return Ok(0);
    }
    let mut data = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut hits = Vec::new();
    let mut at = 0;
    while let Some(found) = find(&data[at..], OLD_ID) {
        let s = at + found;
        if pick(&data, s) {
            hits.push(s);
        }
        at = s + OLD_ID.len();
    }
    for s in &hits {
        data[*s..*s + OLD_ID.len()].copy_from_slice(new_id);
    }
    if !hits.is_empty() {
        std::fs::write(path, &data).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(hits.len())
}

/// The identifier field of Tauri's embedded config.
///
/// Up to 0.92 the dev URL followed it directly, `com.conductor.apphttp://...`.
/// 0.93 pads the string pool with NULs and moves the dev URL away, so that
/// anchor matched nothing and the copy kept the real app's data directory. The
/// product name sits in front of it in both layouts, padded to eight bytes.
fn is_tauri_config(d: &[u8], s: usize) -> bool {
    if starts_with_at(d, s + OLD_ID.len(), b"http://localhost:1420") {
        return true;
    }
    let before = &d[s.saturating_sub(16)..s];
    match find(before, PRODUCT) {
        Some(at) => before[at + PRODUCT.len()..].iter().all(|b| *b == 0),
        None => false,
    }
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn starts_with_at(data: &[u8], at: usize, what: &[u8]) -> bool {
    data.get(at..at + what.len())
        .map(|s| s == what)
        .unwrap_or(false)
}

pub fn build(opts: &Options) -> Result<(), String> {
    let new_id = opts.id.as_bytes();
    if new_id.len() != OLD_ID.len() {
        return Err(format!(
            "identifier must be {} bytes to substitute in place, got {}",
            OLD_ID.len(),
            new_id.len()
        ));
    }
    if !opts.src.is_dir() {
        return Err(format!("not found: {}", opts.src.display()));
    }
    if opts.dst.exists() {
        if !opts.force {
            return Err(format!(
                "{} exists (pass --force to rebuild)",
                opts.dst.display()
            ));
        }
        std::fs::remove_dir_all(&opts.dst).map_err(|e| format!("removing the old copy: {e}"))?;
    }

    println!("==> Copying {}", opts.src.display());
    let ok = Command::new("cp")
        .arg("-R")
        .arg(&opts.src)
        .arg(&opts.dst)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !ok {
        let _ = std::fs::remove_dir_all(&opts.dst);
        return Err("copying the app failed".into());
    }

    if let Err(e) = rewrite(opts, new_id) {
        let _ = std::fs::remove_dir_all(&opts.dst);
        return Err(format!(
            "{e}\nRemoved the half-built {}: it still pointed at the real \
             Conductor's data directory and keychain.",
            opts.dst.display()
        ));
    }
    Ok(())
}

/// Turns a fresh copy into the isolated one, in place.
///
/// Any failure leaves the copy sharing the real app's identifier, database and
/// keychain, so the caller deletes it rather than leave it for `patch` to sign.
fn rewrite(opts: &Options, new_id: &[u8]) -> Result<(), String> {
    println!("==> Rewriting Info.plist");
    let plist = opts.dst.join("Contents/Info.plist");
    plist_set(&plist, "CFBundleIdentifier", &opts.id)?;
    plist_set(&plist, "CFBundleName", "Conductor Dev")?;
    let _ = plist_set(&plist, "CFBundleDisplayName", "Conductor Dev");

    claim_scheme(&opts.dst)?;

    println!("==> Patching the identifier inside the binaries");
    let binary = opts.dst.join("Contents/MacOS/conductor");

    let n = patch_bytes(&binary, new_id, is_tauri_config)?;
    if n != 1 {
        return Err(format!(
            "expected exactly 1 tauri config identifier, found {n}"
        ));
    }
    println!("    tauri config identifier: 1");
    crate::updater::disable(&binary)?;

    let n = patch_bytes(&binary, new_id, |d, s| {
        s > 0 && d[s - 1] == 0x12 && starts_with_at(d, s + OLD_ID.len(), b".")
    })?;
    if n != 1 {
        return Err(format!(
            "expected exactly 1 keychain service prefix, found {n}"
        ));
    }
    println!("    keychain service prefix: 1");

    let runtime = opts
        .dst
        .join("Contents/Resources/bin/.internal/conductor-runtime");
    let n = patch_bytes(&runtime, new_id, |d, s| {
        let from = s.saturating_sub(40);
        find(&d[from..s], b"__CFBundleIdentifier === \"").is_some()
    })?;
    println!("    runtime bundle check: {n}");

    println!("==> Re-signing ad-hoc");
    sign::resign_bundle(&opts.dst, &opts.src)?;

    println!("==> Verifying");
    println!("    signature:  valid (ad-hoc)");
    println!("    identifier: {}", opts.id);
    println!("    data dir:   ~/Library/Application Support/{}", opts.id);
    println!(
        "\nDone. {}\n\nIt starts empty: its own database, its own login, its own agent\n\
         binaries. Your real Conductor is untouched and both can run at once.\n\n  \
         open '{}'\n\nTo remove:  rm -rf '{}'",
        opts.dst.display(),
        opts.dst.display(),
        opts.dst.display()
    );
    Ok(())
}

/// Moves the copy's URL scheme from `conductor` to `conductor-dev`.
///
/// Both apps claiming `conductor://` leaves macOS to pick which one gets every
/// deep link, so links meant for the real Conductor could open the copy. The
/// copy's sign-in still comes back on `conductor://`; `hats signin` points that
/// scheme at the copy for one sign-in. The real app, patched with `--i-know`, keeps its
/// scheme. Returns whether anything changed.
pub fn claim_scheme(app: &Path) -> Result<bool, String> {
    let plist = app.join("Contents/Info.plist");
    let key = "CFBundleURLTypes:0:CFBundleURLSchemes:0";
    let id = plist_get(&plist, "CFBundleIdentifier").unwrap_or_default();
    if id.is_empty() || id.as_bytes() == OLD_ID {
        return Ok(false);
    }
    if plist_get(&plist, key).as_deref() != Some(REAL_SCHEME) {
        return Ok(false);
    }
    plist_set(&plist, key, COPY_SCHEME)?;
    let _ = plist_set(
        &plist,
        "CFBundleURLTypes:0:CFBundleURLName",
        &format!("{id} {COPY_SCHEME}"),
    );
    println!("    url scheme: {REAL_SCHEME} -> {COPY_SCHEME}");
    Ok(true)
}

/// Re-reads the copy's Info.plist into LaunchServices, so a scheme it claimed
/// before the rewrite stops routing deep links to it. Only after the last
/// signature: registering makes macOS assess the bundle, and a `codesign`
/// racing that assessment fails with "Operation not permitted".
pub fn register(app: &Path) {
    let _ = Command::new(LSREGISTER).arg("-f").arg(app).status();
}

/// The bundle identifier in the app's Info.plist.
pub fn bundle_id(app: &Path) -> Option<String> {
    plist_get(&app.join("Contents/Info.plist"), "CFBundleIdentifier").filter(|s| !s.is_empty())
}

/// The first URL scheme the app claims, as `verify` reports it.
pub fn url_scheme(app: &Path) -> Option<String> {
    plist_get(
        &app.join("Contents/Info.plist"),
        "CFBundleURLTypes:0:CFBundleURLSchemes:0",
    )
}

fn plist_get(plist: &Path, key: &str) -> Option<String> {
    let out = Command::new("/usr/libexec/PlistBuddy")
        .arg("-c")
        .arg(format!("Print :{key}"))
        .arg(plist)
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn plist_set(plist: &Path, key: &str, value: &str) -> Result<(), String> {
    let ok = Command::new("/usr/libexec/PlistBuddy")
        .arg("-c")
        .arg(format!("Set :{key} {value}"))
        .arg(plist)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if ok {
        Ok(())
    } else {
        Err(format!("could not set {key} in {}", plist.display()))
    }
}
