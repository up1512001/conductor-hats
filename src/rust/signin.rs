//! Signing a copy in to Conductor, the way the real app signs in.
//!
//! Conductor signs in through the browser, and the web page hands the code back
//! by navigating to a `conductor://auth` link it builds in script and never
//! shows. macOS opens that link with the real Conductor, which rejects a sign-in
//! it never started, and the copy waits on its sign-in screen for ever. The web
//! app only accepts the production channel, so the scheme cannot change.
//!
//! So for the length of one sign-in the copy becomes the app that opens
//! `conductor://`, and the previous handler gets it back afterwards. The boot
//! guard runs `--begin` when the copy opens the sign-in page and `--end` once
//! the code is exchanged; a detached `--expire` restores it if `--end` never
//! comes. Run by hand, `hats signin` does the same around a keypress.

use std::io::BufRead;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::{devapp, patch, urlhandler};

const SCHEME: &str = "conductor";
const REAL_ID: &str = "com.conductor.app";
const WINDOW: Duration = Duration::from_secs(600);

pub fn run(app: &Path, rest: &[String]) -> Result<(), String> {
    let flag = |f: &str| rest.iter().any(|a| a == f);
    if flag("--restore") {
        let _ = std::fs::remove_file(state_file());
        return restore(REAL_ID);
    }
    if flag("--end") {
        return end(None);
    }
    if let Some(at) = rest.iter().position(|a| a == "--expire") {
        std::thread::sleep(WINDOW);
        return end(rest.get(at + 1).map(String::as_str));
    }
    let app = if flag("--begin") {
        calling_app().unwrap_or_else(|| app.to_path_buf())
    } else {
        app.to_path_buf()
    };
    let id = devapp::bundle_id(&app).ok_or_else(|| format!("no copy at {}", app.display()))?;
    if id == REAL_ID {
        return Err(format!("{} is the real Conductor", app.display()));
    }
    let before = match std::fs::read_to_string(state_file()) {
        Ok(saved) => saved
            .split_whitespace()
            .next()
            .unwrap_or(REAL_ID)
            .to_string(),
        Err(_) => urlhandler::default_handler(SCHEME)
            .filter(|h| h != &id)
            .unwrap_or_else(|| REAL_ID.to_string()),
    };
    let nonce = format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    );
    std::fs::write(state_file(), format!("{before} {nonce}\n"))
        .map_err(|e| format!("{}: {e}", state_file().display()))?;
    urlhandler::set_default_handler(SCHEME, &id)?;

    if flag("--begin") {
        expire_later(&nonce);
        println!("conductor:// opens {id} until this sign-in finishes");
        return Ok(());
    }

    let _ = Command::new("open").arg("-a").arg(&app).status();
    println!(
        "conductor:// now opens {}.\n\n\
         Click sign in in that copy's window and finish in the browser.\n\
         Press Enter once it is signed in; {before} gets conductor:// back\n\
         then, or after {} minutes. If this is interrupted:\n\n  \
         hats signin --restore\n",
        app.display(),
        WINDOW.as_secs() / 60
    );
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = std::io::stdin().lock().read_line(&mut String::new());
        let _ = tx.send(());
    });
    let _ = rx.recv_timeout(WINDOW);
    end(Some(&nonce))
}

/// Hands `conductor://` back, unless a newer sign-in has started since.
fn end(nonce: Option<&str>) -> Result<(), String> {
    let Ok(saved) = std::fs::read_to_string(state_file()) else {
        return Ok(());
    };
    let mut parts = saved.split_whitespace();
    let before = parts.next().unwrap_or(REAL_ID).to_string();
    if nonce.is_some() && parts.next() != nonce {
        return Ok(());
    }
    let _ = std::fs::remove_file(state_file());
    restore(&before)
}

fn restore(id: &str) -> Result<(), String> {
    urlhandler::set_default_handler(SCHEME, id)?;
    println!("conductor:// opens {id} again");
    Ok(())
}

/// Restores the handler later from a process of its own, outside the process
/// group of the shell Conductor ran us in, so it outlives that command.
fn expire_later(nonce: &str) {
    let Ok(me) = std::env::current_exe() else {
        return;
    };
    let _ = Command::new(me)
        .args(["signin", "--expire", nonce])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn();
}

fn state_file() -> PathBuf {
    patch::dirs_accounts_root().join("signin-handler")
}

/// The application bundle whose process started this one, if any.
fn calling_app() -> Option<PathBuf> {
    let mut pid = std::process::id();
    for _ in 0..12 {
        let out = Command::new("ps")
            .args(["-o", "ppid=,comm=", "-p", &pid.to_string()])
            .output()
            .ok()?;
        let line = String::from_utf8_lossy(&out.stdout).trim().to_string();
        let (ppid, comm) = line.split_once(char::is_whitespace)?;
        let comm = comm.trim();
        if let Some(at) = comm.find(".app/Contents/") {
            return Some(PathBuf::from(&comm[..at + 4]));
        }
        pid = ppid.trim().parse().ok()?;
        if pid <= 1 {
            return None;
        }
    }
    None
}
