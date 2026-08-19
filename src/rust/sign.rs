//! Re-signing a patched copy.
//!
//! codesign, PlistBuddy, xattr and security ship with macOS, so shelling out to
//! them costs a user nothing. A Python or Node runtime would not.

use std::path::{Path, PathBuf};
use std::process::Command;

fn run(cmd: &str, args: &[&str]) -> Result<Vec<u8>, String> {
    let out = Command::new(cmd)
        .args(args)
        .output()
        .map_err(|e| format!("{cmd}: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let head: String = err.trim().chars().take(300).collect();
        return Err(format!("{cmd} failed: {head}"));
    }
    Ok(out.stdout)
}

/// Reads an application's entitlements to a temporary file.
///
/// These must survive re-signing: without `allow-jit` the WebView cannot run and
/// the app dies on launch, and `conductor-runtime` is a Bun executable that
/// JIT-compiles JavaScript, so signing it bare produces "Sidecar terminated
/// unexpectedly" the moment it needs to compile.
fn entitlements(app: &Path, suffix: &str) -> Option<PathBuf> {
    let out = Command::new("codesign")
        .args(["-d", "--entitlements", "-", "--xml"])
        .arg(app)
        .output()
        .ok()?;
    if !out.status.success() || out.stdout.is_empty() {
        return None;
    }
    let path = std::env::temp_dir().join(format!("conductor-hats-{suffix}.plist"));
    std::fs::write(&path, &out.stdout).ok()?;
    Some(path)
}

fn codesign(target: &Path, ents: Option<&Path>) -> Result<(), String> {
    let mut args: Vec<String> = vec!["-f".into(), "-s".into(), "-".into(), "--options".into(), "runtime".into()];
    if let Some(path) = ents {
        args.push("--entitlements".into());
        args.push(path.to_string_lossy().to_string());
    }
    args.push(target.to_string_lossy().to_string());
    let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
    run("codesign", &borrowed).map(|_| ())
}

/// Signs the outer bundle only, for a copy whose contents are already signed.
pub fn resign(app: &Path) -> Result<bool, String> {
    let ents = entitlements(app, "outer");
    codesign(app, ents.as_deref())?;
    let _ = run("xattr", &["-cr", &app.to_string_lossy()]);
    drop_stale_keychain_items(app);
    Ok(verify(app))
}

/// Signs every inner Mach-O with its own original entitlements, then the bundle,
/// so the outer seal covers the final bytes of everything it contains.
pub fn resign_bundle(dst: &Path, pristine: &Path) -> Result<(), String> {
    let bin = dst.join("Contents/Resources/bin");
    if bin.is_dir() {
        for inner in mach_o_files(&bin) {
            let rel = inner.strip_prefix(dst).unwrap_or(&inner);
            let original = pristine.join(rel);
            let ents = entitlements(&original, "inner");
            if codesign(&inner, ents.as_deref()).is_err() {
                let _ = codesign(&inner, None);
            }
        }
    }
    let ents = entitlements(pristine, "outer");
    codesign(dst, ents.as_deref())?;
    let _ = run("xattr", &["-cr", &dst.to_string_lossy()]);
    drop_stale_keychain_items(dst);
    Ok(())
}

fn verify(app: &Path) -> bool {
    Command::new("codesign")
        .args(["--verify", "--strict"])
        .arg(app)
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn mach_o_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(root) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(mach_o_files(&path));
        } else if is_mach_o(&path) {
            out.push(path);
        }
    }
    out
}

fn is_mach_o(path: &Path) -> bool {
    use std::io::Read;
    let Ok(mut f) = std::fs::File::open(path) else {
        return false;
    };
    let mut magic = [0u8; 4];
    if f.read_exact(&mut magic).is_err() {
        return false;
    }
    matches!(
        u32::from_le_bytes(magic),
        0xfeed_facf | 0xfeed_face | 0xcafe_babe | 0xbeba_feca
    )
}

/// An ad-hoc signature carries no stable identity, so every rebuild looks like a
/// different application to the keychain and macOS asks for the login password to
/// release the previous build's items. Scoped to the copy's own service name, so
/// the real Conductor's credentials are never in range.
fn drop_stale_keychain_items(app: &Path) {
    let plist = app.join("Contents/Info.plist");
    let ident = Command::new("/usr/libexec/PlistBuddy")
        .args(["-c", "Print :CFBundleIdentifier"])
        .arg(&plist)
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    if ident.is_empty() || ident == "com.conductor.app" {
        return;
    }
    let service = format!("{ident}.production.settings");
    let mut removed = 0;
    while removed < 20 {
        let gone = Command::new("security")
            .args(["delete-generic-password", "-s", &service])
            .output()
            .map(|o| !o.status.success())
            .unwrap_or(true);
        if gone {
            break;
        }
        removed += 1;
    }
    if removed > 0 {
        println!("    keychain: cleared {removed} stale item(s) for {service}");
    }
}
