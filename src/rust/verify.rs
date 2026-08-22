//! Checking a patched copy end to end, so a Conductor update says what it broke.
//!
//! Conductor ships roughly weekly and each release can move something the
//! injection depends on. When the copy came up blank on 0.82 every one of these
//! was checked by hand, badly and slowly. Doing it in one command means the next
//! release is a question with an answer rather than an afternoon.

use std::path::Path;
use std::process::Command;

use crate::{binary_in, macho::MachO, patch, sign};

/// The identifier a copy must no longer carry, or it fights the real app over
/// the same data directory and keychain items.
const REAL_ID: &str = "com.conductor.app";

/// Without this the WebView cannot run and the window paints nothing.
const REQUIRED_ENTITLEMENT: &str = "com.apple.security.cs.allow-jit";

struct Report {
    ok: bool,
}

impl Report {
    fn pass(&mut self, label: &str, detail: &str) {
        println!("  ok    {label:<28} {detail}");
    }

    fn fail(&mut self, label: &str, detail: &str) {
        println!("  FAIL  {label:<28} {detail}");
        self.ok = false;
    }

    fn note(&self, label: &str, detail: &str) {
        println!("  note  {label:<28} {detail}");
    }
}

fn entitlements(app: &Path) -> String {
    Command::new("codesign")
        .args(["-d", "--entitlements", "-", "--xml"])
        .arg(app)
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default()
}

fn contains(path: &Path, needle: &str) -> bool {
    let Ok(data) = std::fs::read(path) else {
        return false;
    };
    data.windows(needle.len()).any(|w| w == needle.as_bytes())
}

pub fn run(app: &Path) -> Result<(), String> {
    println!("app:      {}", app.display());
    if !app.is_dir() {
        return Err(format!("{} is not there", app.display()));
    }
    let binary = binary_in(app);
    let mut r = Report { ok: true };

    let backup = patch::backup_path(app);
    let pristine = if backup.is_file() {
        match (std::fs::read(&binary), std::fs::read(&backup)) {
            (Ok(a), Ok(b)) if a == b => {
                r.note("patched", "no, identical to its pristine backup");
                true
            }
            (Ok(_), Ok(_)) => {
                r.pass("patched", "yes, differs from its pristine backup");
                false
            }
            _ => {
                r.fail("patched", "could not read the binary or its backup");
                false
            }
        }
    } else {
        r.note("patched", "unknown, no backup has been taken");
        false
    };

    let macho = MachO::open(&binary)?;
    match patch::pick_bundle(macho.assets()) {
        Ok(asset) => {
            r.pass("bundle found", &asset.key);
            match macho
                .data
                .get(asset.offset..asset.offset + asset.length)
                .ok_or_else(|| "the asset map points outside the file".to_string())
                .and_then(patch::decompress)
            {
                Ok(plain) => {
                    let text = String::from_utf8_lossy(&plain);
                    r.pass("bundle decompresses", &format!("{} bytes", plain.len()));
                    if pristine {
                        r.note("panel present", "not expected, this copy is unpatched");
                    } else if text.contains(patch::MARKER) {
                        r.pass("panel present", "the marker is in the bundle");
                    } else {
                        r.fail("panel present", "the marker is missing from the bundle");
                    }
                    /* Only meaningful once patched: the panel is appended last,
                     * so a patched bundle must close on its IIFE. An untouched
                     * one ends however the bundler left it, which on 0.82 is a
                     * //# debugId comment. */
                    let tail = text.trim_end();
                    let last: String = tail
                        .chars()
                        .rev()
                        .take(48)
                        .collect::<Vec<_>>()
                        .into_iter()
                        .rev()
                        .collect();
                    if pristine {
                        r.note("bundle tail", &last);
                    } else if tail.ends_with("})();") {
                        r.pass("bundle ends cleanly", "closes on the panel's IIFE");
                    } else {
                        r.fail("bundle ends cleanly", &format!("ends with {last:?}"));
                    }
                }
                Err(e) => r.fail("bundle decompresses", &e),
            }
        }
        Err(e) => r.fail("bundle found", e.lines().next().unwrap_or("no match")),
    }

    match patch::asset_text(&macho, "/index.html").and_then(|html| {
        let entry = patch::pick_entry(&html)?;
        let text = patch::asset_text(&macho, &entry)?;
        Ok((entry, text))
    }) {
        Ok((entry, text)) => {
            if pristine {
                r.note("boot guard", "not expected, this copy is unpatched");
            } else if text.contains(patch::GUARD_MARKER) {
                r.pass("boot guard", &format!("present in {entry}"));
            } else {
                r.fail(
                    "boot guard",
                    &format!("missing from {entry}, so 0.82 paints nothing"),
                );
            }
        }
        Err(e) => r.fail("boot guard", e.lines().next().unwrap_or("entry not found")),
    }

    match sign::verify(app) {
        Ok(()) => r.pass("signature", "valid"),
        Err(e) => r.fail("signature", e.lines().next().unwrap_or("invalid")),
    }

    let ents = entitlements(app);
    if ents.contains(REQUIRED_ENTITLEMENT) {
        r.pass("entitlements", "allow-jit survived re-signing");
    } else {
        r.fail(
            "entitlements",
            "allow-jit is missing, so the WebView will paint nothing",
        );
    }

    if contains(&binary, REAL_ID) {
        r.fail(
            "identifier rewritten",
            "the copy still carries com.conductor.app",
        );
    } else {
        r.pass(
            "identifier rewritten",
            "no trace of the real app's identifier",
        );
    }

    let runtime = app.join("Contents/Resources/bin/.internal/conductor-runtime");
    if runtime.is_file() {
        if contains(&runtime, REAL_ID) {
            r.fail(
                "runtime rewritten",
                "conductor-runtime still checks the real identifier",
            );
        } else {
            r.pass(
                "runtime rewritten",
                "conductor-runtime agrees with the copy",
            );
        }
    }

    println!();
    if r.ok {
        println!("OK");
        Ok(())
    } else {
        Err("this copy will not work as patched; the failures above say why".into())
    }
}
