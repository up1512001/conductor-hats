//! Rebuilding the personalized copy after a Conductor update, in one command.
//!
//! Conductor ships a new frontend bundle roughly weekly, which drops the injected
//! panel. Two steps in the recovery fail quietly if done by hand.
//!
//! The UI backup is keyed by app name, not by version, so after an update it
//! holds the *previous* Conductor's binary and patching a freshly rebuilt copy
//! against it silently reinstates the old version. It is dropped on rebuild.
//!
//! Launching with `open` from inside a routed agent session leaks
//! CONDUCTOR_ACCOUNTS_ROUTING into the app, which hands it to every agent it
//! spawns, and the loop guard then refuses them with exit code 70. The
//! environment is scrubbed before launching.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{devapp, patch};

const LEAKED: [&str; 6] = [
    "CONDUCTOR_ACCOUNTS_ROUTING",
    "CONDUCTOR_ACCOUNTS_DEPTH",
    "CONDUCTOR_WORKSPACE_PATH",
    "CONDUCTOR_ROOT_PATH",
    "CONDUCTOR_ACCOUNT",
    "CLAUDE_CONFIG_DIR",
];

pub struct Options {
    pub app: PathBuf,
    pub src: PathBuf,
    pub id: String,
    pub rebuild: bool,
    pub launch: bool,
}

pub fn run(opts: &Options) -> Result<(), String> {
    println!("==> Quitting {}", opts.app.display());
    quit(&opts.app);

    if opts.rebuild {
        println!("==> Dropping the stale UI backup");
        let backup = patch::backup_path(&opts.app);
        let _ = std::fs::remove_file(&backup);

        println!("==> Rebuilding the copy from the current Conductor");
        devapp::build(&devapp::Options {
            src: opts.src.clone(),
            dst: opts.app.clone(),
            id: opts.id.clone(),
            force: true,
        })?;
    }

    println!("==> Injecting the account panel");
    crate::cmd_patch_app(&opts.app, false)?;

    if opts.launch {
        println!("==> Relaunching with a scrubbed environment");
        let mut cmd = Command::new("open");
        for key in LEAKED {
            cmd.env_remove(key);
        }
        cmd.arg("-a").arg(&opts.app);
        let ok = cmd.status().map(|s| s.success()).unwrap_or(false);
        println!("    {}", if ok { "launched" } else { "launch failed" });
    } else {
        println!("==> Not relaunching");
        println!("    Scrub the environment when you do, or agents get exit code 70:");
        println!(
            "    env -u CONDUCTOR_ACCOUNTS_ROUTING -u CONDUCTOR_ACCOUNTS_DEPTH open -a '{}'",
            opts.app.display()
        );
    }

    println!(
        "\nLook for the account button next to \"Open in\", and the account chip in\n\
         the New Workspace composer. If neither is there, the anchors moved in this\n\
         release: the button falls back to floating at the top right, so nothing at\n\
         all means the injection itself did not take."
    );
    Ok(())
}

fn quit(app: &Path) {
    let ident = Command::new("/usr/libexec/PlistBuddy")
        .args(["-c", "Print :CFBundleIdentifier"])
        .arg(app.join("Contents/Info.plist"))
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "com.conductor.dev".into());
    let _ = Command::new("osascript")
        .arg("-e")
        .arg(format!("quit app id \"{ident}\""))
        .status();

    let inner = app.join("Contents/MacOS/");
    for _ in 0..10 {
        if !running(&inner) {
            println!("    quit");
            return;
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
    println!("    still running, asking harder");
    let _ = Command::new("pkill")
        .arg("-f")
        .arg(inner.to_string_lossy().to_string())
        .status();
    std::thread::sleep(std::time::Duration::from_secs(2));
}

fn running(inner: &Path) -> bool {
    Command::new("pgrep")
        .arg("-f")
        .arg(inner.to_string_lossy().to_string())
        .output()
        .map(|o| !o.stdout.is_empty())
        .unwrap_or(false)
}
