//! Re-signing a patched copy.
//!
//! codesign, PlistBuddy, xattr and security ship with macOS, so shelling out to
//! them costs a user nothing. A Python or Node runtime would not.

use std::path::Path;
use std::process::Command;

fn run(cmd: &str, args: &[&str]) -> Result<Vec<u8>, String> {
    let out = Command::new(cmd)
        .args(args)
        .output()
        .map_err(|e| format!("{cmd}: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(format!("{cmd} failed: {}", err.trim().chars().take(300).collect::<String>()));
    }
    Ok(out.stdout)
}

/// The original entitlements, which must be carried across: without allow-jit the
/// WebView cannot run and the app dies on launch.
fn entitlements(app: &Path) -> Option<std::path::PathBuf> {
    let out = Command::new("codesign")
        .args(["-d", "--entitlements", "-", "--xml"])
        .arg(app)
        .output()
        .ok()?;
    if !out.status.success() || !String::from_utf8_lossy(&out.stdout).contains("allow-jit") {
        return None;
    }
    let path = std::env::temp_dir().join("conductor-hats-entitlements.plist");
    std::fs::write(&path, &out.stdout).ok()?;
    Some(path)
}

pub fn resign(app: &Path) -> Result<bool, String> {
    let ents = entitlements(app);
    let app_str = app.to_string_lossy().to_string();
    let mut args: Vec<String> = vec![
        "-f".into(),
        "-s".into(),
        "-".into(),
        "--options".into(),
        "runtime".into(),
    ];
    if let Some(path) = &ents {
        args.push("--entitlements".into());
        args.push(path.to_string_lossy().to_string());
    }
    args.push(app_str);
    let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
    run("codesign", &borrowed)?;

    let _ = run("xattr", &["-cr", &app.to_string_lossy()]);
    drop_stale_keychain_items(app);

    let ok = Command::new("codesign")
        .args(["--verify", "--strict"])
        .arg(app)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    Ok(ok)
}

/// Re-signing orphans the copy's keychain items, and macOS then prompts for the
/// login password. Clearing them first avoids that. Scoped to the copy's own
/// identifier.
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
    for _ in 0..20 {
        let gone = Command::new("security")
            .args(["delete-generic-password", "-s", &service])
            .output()
            .map(|o| !o.status.success())
            .unwrap_or(true);
        if gone {
            break;
        }
    }
}
