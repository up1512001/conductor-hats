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
    /* The copy is the only thing this may touch. Rebuilding it reads the real
     * Conductor and must never disturb it: quitting that closes every agent
     * running inside it, which is how this was found. */
    if same_bundle(&opts.app, &opts.src) {
        return Err(format!(
            "refusing to rebuild {} onto itself: that is the real Conductor",
            opts.src.display()
        ));
    }
    if let Some(pid) = own_ancestor(&opts.app.join("Contents/MacOS/")) {
        return Err(format!(
            "refusing: this is running inside {} (pid {pid}).\n\
             Quitting it would kill the agent asking for the rebuild. Run this \
             from a terminal outside that application.",
            opts.app.display()
        ));
    }

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
        println!("    {}", scrubbed_launch(&opts.app));
    }

    println!(
        "\nLook for the account button next to \"Open in\", and the account chip in\n\
         the New Workspace composer. If neither is there, the anchors moved in this\n\
         release: the button falls back to floating at the top right, so nothing at\n\
         all means the injection itself did not take."
    );
    Ok(())
}

/// The launch command to hand someone who is not relaunching now.
///
/// Built from `LEAKED` rather than written out, because it was written out and
/// drifted: the hint named two variables while the relaunch above scrubbed six.
/// Following the short version from a routed shell leaves `CLAUDE_CONFIG_DIR`
/// set, which pins the whole copy to one account no matter what its routes and
/// bindings say.
fn scrubbed_launch(app: &Path) -> String {
    let scrub = LEAKED
        .iter()
        .map(|key| format!("-u {key}"))
        .collect::<Vec<String>>()
        .join(" ");
    format!("env {scrub} open -a '{}'", app.display())
}

/// Quits the copy, and nothing else.
///
/// It used to ask LaunchServices, `quit app id "com.conductor.dev"`. The copy is
/// Conductor with one string rewritten, so LaunchServices resolved that id back
/// to the original and quit that instead: the real Conductor, and every agent
/// running inside it, including whichever one had just asked for the rebuild.
///
/// Nothing is resolved by name or identifier now. The process list is read, and
/// only processes whose executable is inside this bundle are signalled.
fn quit(app: &Path) {
    let inner = app.join("Contents/MacOS/");
    let mut pids = pids_under(&inner);
    if pids.is_empty() {
        println!("    not running");
        return;
    }
    for pid in &pids {
        let _ = Command::new("kill").arg("-TERM").arg(pid).status();
    }
    for _ in 0..10 {
        pids = pids_under(&inner);
        if pids.is_empty() {
            println!("    quit");
            return;
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
    println!("    still running, asking harder");
    for pid in &pids {
        let _ = Command::new("kill").arg("-KILL").arg(pid).status();
    }
    std::thread::sleep(std::time::Duration::from_secs(2));
}

fn same_bundle(a: &Path, b: &Path) -> bool {
    let real = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    real(a) == real(b)
}

/// This process's own ancestry, if any of it lives inside the bundle.
///
/// A belt beside the brace: whatever else goes wrong, an agent cannot be made to
/// quit the application it is running inside.
fn own_ancestor(inner: &Path) -> Option<u32> {
    let mine = pids_under(inner);
    if mine.is_empty() {
        return None;
    }
    let mut pid = std::process::id();
    for _ in 0..12 {
        if mine.iter().any(|p| p.parse::<u32>().ok() == Some(pid)) {
            return Some(pid);
        }
        let out = Command::new("ps")
            .args(["-o", "ppid=", "-p", &pid.to_string()])
            .output()
            .ok()?;
        let parent: u32 = String::from_utf8_lossy(&out.stdout).trim().parse().ok()?;
        if parent <= 1 {
            return None;
        }
        pid = parent;
    }
    None
}

/// The pids whose executable path sits inside this bundle.
///
/// Compared as a plain prefix rather than matched with `pkill -f`, whose pattern
/// is a regular expression: every `.` in an application path is a wildcard, and
/// the paths of the copy and the original differ by very little.
fn pids_under(inner: &Path) -> Vec<String> {
    let want = inner.to_string_lossy().to_string();
    let Ok(out) = Command::new("ps").args(["-eo", "pid=,comm="]).output() else {
        return Vec::new();
    };
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|line| {
            let line = line.trim_start();
            let (pid, comm) = line.split_once(char::is_whitespace)?;
            comm.trim_start()
                .starts_with(&want)
                .then(|| pid.to_string())
        })
        .collect()
}

#[cfg(test)]
#[path = "repatch_tests.rs"]
mod tests;
