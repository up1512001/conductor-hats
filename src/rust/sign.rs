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

/// A directory this process is known to have created.
///
/// `create_dir` fails when the name already exists, so a symlink planted in a
/// shared temporary directory cannot be followed, and two concurrent patches
/// cannot write over each other's entitlements. Removed on drop, which covers
/// success, failure, early return and unwind alike.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Result<Self, String> {
        let base = std::env::temp_dir();
        for attempt in 0..64u32 {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_nanos())
                .unwrap_or(0);
            let path = base.join(format!("hats-{}-{nanos}-{attempt}", std::process::id()));
            match std::fs::create_dir(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(format!("{}: {e}", path.display())),
            }
        }
        Err("could not create a private temporary directory".into())
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Reads an application's entitlements to a file inside `scratch`.
///
/// These must survive re-signing: without `allow-jit` the WebView cannot run and
/// the app dies on launch, and `conductor-runtime` is a Bun executable that
/// JIT-compiles JavaScript, so signing it bare produces "Sidecar terminated
/// unexpectedly" the moment it needs to compile.
fn entitlements(app: &Path, name: &str, scratch: &Scratch) -> Option<PathBuf> {
    let out = Command::new("codesign")
        .args(["-d", "--entitlements", "-", "--xml"])
        .arg(app)
        .output()
        .ok()?;
    if !out.status.success() || out.stdout.is_empty() {
        return None;
    }
    let path = scratch.0.join(format!("{name}.plist"));
    std::fs::write(&path, &out.stdout).ok()?;
    Some(path)
}

fn codesign(target: &Path, ents: Option<&Path>) -> Result<(), String> {
    let mut args: Vec<String> = vec![
        "-f".into(),
        "-s".into(),
        "-".into(),
        "--options".into(),
        "runtime".into(),
    ];
    if let Some(path) = ents {
        args.push("--entitlements".into());
        args.push(path.to_string_lossy().to_string());
    }
    args.push(target.to_string_lossy().to_string());
    let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
    run("codesign", &borrowed).map(|_| ())
}

/// Signs the outer bundle only, for a copy whose contents are already signed.
pub fn resign(app: &Path) -> Result<(), String> {
    let scratch = Scratch::new()?;
    let ents = entitlements(app, "outer", &scratch);
    codesign(app, ents.as_deref())?;
    let _ = run("xattr", &["-cr", &app.to_string_lossy()]);
    drop_keychain_items(app);
    verify(app)
}

/// Signs every inner Mach-O with its own original entitlements, then the bundle,
/// so the outer seal covers the final bytes of everything it contains.
pub fn resign_bundle(dst: &Path, pristine: &Path) -> Result<(), String> {
    let scratch = Scratch::new()?;
    let bin = dst.join("Contents/Resources/bin");
    if bin.is_dir() {
        for inner in mach_o_files(&bin) {
            let rel = inner.strip_prefix(dst).unwrap_or(&inner);
            let original = pristine.join(rel);
            let ents = entitlements(&original, "inner", &scratch);
            let source = match &ents {
                Some(_) => "its original entitlements",
                None => "no entitlements, because none could be read from the original",
            };
            codesign(&inner, ents.as_deref()).map_err(|e| {
                format!(
                    "signing {} with {source} failed: {e}\n\
                     Retrying without them would produce an application that is signed \
                     but dies the moment the WebView needs to compile, so nothing was \
                     replaced.",
                    rel.display()
                )
            })?;
        }
    }
    let ents = entitlements(pristine, "outer", &scratch);
    codesign(dst, ents.as_deref())
        .map_err(|e| format!("signing the bundle {} failed: {e}", dst.display()))?;
    let _ = run("xattr", &["-cr", &dst.to_string_lossy()]);
    drop_keychain_items(dst);
    verify(dst)
}

/// Verification is the contract, not a remark: a command that signs and then
/// reports a bad signature must not exit 0, or a broken application reads as
/// installed.
pub fn verify(app: &Path) -> Result<(), String> {
    let out = Command::new("codesign")
        .args(["--verify", "--strict", "--verbose=2"])
        .arg(app)
        .output()
        .map_err(|e| format!("codesign: {e}"))?;
    if out.status.success() {
        return Ok(());
    }
    let detail: String = String::from_utf8_lossy(&out.stderr)
        .trim()
        .chars()
        .take(400)
        .collect();
    Err(format!(
        "signature verification failed for {}:\n{detail}",
        app.display()
    ))
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

/// Deletes what the copy keeps in the keychain, on every signature.
///
/// An ad-hoc signature carries no stable identity, so a re-signed copy looks like
/// a different application: macOS then blocks the copy from reading what the
/// previous build stored and puts up "Conductor Dev wants to use your confidential
/// information", asking for the login password. There is no third option while the
/// signature is ad-hoc. Either the items go, and the copy starts signed out, or
/// they stay and that dialog appears on launch.
///
/// Removing them is the choice here, because the dialog cannot be answered
/// safely by habit and a copy that asks for a keychain password is a copy that
/// teaches a bad reflex. A signing identity that stays the same between patches
/// removes both; see docs/dev-conductor.md.
///
/// Scoped to the copy's own service name, so the real Conductor's credentials are
/// never in range.
pub fn drop_keychain_items(app: &Path) {
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
        println!(
            "    keychain: cleared {removed} item(s) for {service}, so the copy starts signed out"
        );
    }
}
