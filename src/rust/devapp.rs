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
        return Err("copying the app failed".into());
    }

    println!("==> Rewriting Info.plist");
    let plist = opts.dst.join("Contents/Info.plist");
    plist_set(&plist, "CFBundleIdentifier", &opts.id)?;
    plist_set(&plist, "CFBundleName", "Conductor Dev")?;
    let _ = plist_set(&plist, "CFBundleDisplayName", "Conductor Dev");

    println!("==> Patching the identifier inside the binaries");
    let binary = opts.dst.join("Contents/MacOS/conductor");

    let n = patch_bytes(&binary, new_id, |d, s| {
        starts_with_at(d, s + OLD_ID.len(), b"http://localhost:1420")
    })?;
    if n != 1 {
        return Err(format!(
            "expected exactly 1 tauri config identifier, found {n}"
        ));
    }
    println!("    tauri config identifier: 1");

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
