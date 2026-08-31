//! The loopback service started explicitly by the Mobile access button.
//!
//! The child gets its own process group and no inherited standard streams, so
//! Conductor's short shell command can finish without taking the listener with
//! it. Runtime files prove that the recorded PID is this hats binary serving the
//! expected loopback address; a stale PID is never treated as a live service.

use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use crate::{auth, lock, paths, source, store};

const BIND: &str = "127.0.0.1:8787";
static CLIENTS: AtomicUsize = AtomicUsize::new(0);

#[derive(serde::Serialize)]
pub struct Status {
    pub running: bool,
    pub address: &'static str,
    pub connections: usize,
    pub source: String,
}

fn root() -> PathBuf {
    paths::accounts_root().join("serve-runtime")
}

fn file(name: &str) -> PathBuf {
    root().join(name)
}

fn private_dir(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::create_dir_all(path).map_err(|e| format!("{}: {e}", path.display()))?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
        .map_err(|e| format!("{}: {e}", path.display()))
}

fn private_log() -> Result<File, String> {
    use std::os::unix::fs::OpenOptionsExt;
    OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(file("error.log"))
        .map_err(|e| format!("starting the mobile service log: {e}"))
}

fn pid() -> Option<u32> {
    paths::first_line(&file("pid"))?.parse().ok()
}

fn owned(process: u32) -> bool {
    if process <= 1 {
        return false;
    }
    let Some(binary) = paths::first_line(&file("binary")) else {
        return false;
    };
    let Ok(output) = Command::new("ps")
        .args(["-o", "command=", "-p", &process.to_string()])
        .output()
    else {
        return false;
    };
    if !output.status.success() {
        return false;
    }
    let command = String::from_utf8_lossy(&output.stdout);
    command.contains(&binary) && (command.contains(" serve") || command.ends_with(" serve\n"))
}

fn matches(process: u32) -> bool {
    paths::first_line(&file("ready")).as_deref() == Some(&process.to_string())
        && paths::first_line(&file("address")).as_deref() == Some(BIND)
        && owned(process)
}

fn clear(process: u32) {
    if pid() != Some(process) {
        return;
    }
    for name in ["pid", "binary", "address", "ready", "clients", "source"] {
        let _ = std::fs::remove_file(file(name));
    }
}

fn problem() -> String {
    let body = std::fs::read_to_string(file("error.log")).unwrap_or_default();
    let lines: Vec<&str> = body.lines().rev().take(3).collect();
    let text = lines.into_iter().rev().collect::<Vec<_>>().join(" ");
    if text.trim().is_empty() {
        "the local mobile service exited before it was ready".into()
    } else {
        text.trim().chars().take(500).collect()
    }
}

pub fn status() -> Status {
    let running = pid().is_some_and(matches);
    let source = paths::first_line(&file("source"))
        .and_then(|key| source::from_key(&key))
        .map(|found| found.label().to_string())
        .unwrap_or_default();
    Status {
        running,
        address: BIND,
        connections: if running {
            paths::first_line(&file("clients"))
                .and_then(|value| value.parse().ok())
                .unwrap_or(0)
        } else {
            0
        },
        source,
    }
}

pub fn status_for(want: &source::Source) -> Status {
    let mut found = status();
    if paths::first_line(&file("source")).as_deref() != Some(&want.key()) {
        found.running = false;
        found.connections = 0;
    }
    found.source = want.label().to_string();
    found
}

pub fn register(address: &str) -> Result<Runtime, String> {
    private_dir(&root())?;
    let process = std::process::id();
    let binary = std::env::current_exe().map_err(|e| format!("locating hats: {e}"))?;
    auth::write_private(&file("pid"), &process.to_string())?;
    auth::write_private(&file("binary"), &binary.to_string_lossy())?;
    auth::write_private(&file("address"), address)?;
    if let Some(source) = source::active() {
        auth::write_private(&file("source"), &source.key())?;
    }
    auth::write_private(&file("ready"), &process.to_string())?;
    CLIENTS.store(0, Ordering::SeqCst);
    auth::write_private(&file("clients"), "0")?;
    Ok(Runtime(process))
}

/// An authenticated browser currently holding a WebSocket connection.
pub struct Client;

fn write_clients(count: usize) {
    let Ok(_guard) = lock::Lock::acquire(&file("client-count")) else {
        return;
    };
    let _ = auth::write_private(&file("clients"), &count.to_string());
}

pub fn connected() -> Client {
    let count = CLIENTS.fetch_add(1, Ordering::SeqCst) + 1;
    write_clients(count);
    Client
}

impl Drop for Client {
    fn drop(&mut self) {
        let count = CLIENTS
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |value| {
                Some(value.saturating_sub(1))
            })
            .unwrap_or(0)
            .saturating_sub(1);
        write_clients(count);
    }
}

fn halt(process: u32) -> Result<(), String> {
    let target = process.to_string();
    let sent = Command::new("kill")
        .args(["-TERM", &target])
        .status()
        .map_err(|e| format!("stopping the mobile service: {e}"))?;
    if !sent.success() {
        return Err("the mobile service could not be stopped".into());
    }
    for _ in 0..40 {
        if !owned(process) {
            clear(process);
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    if matches(process) {
        let _ = Command::new("kill").args(["-KILL", &target]).status();
    }
    clear(process);
    Ok(())
}

/// Stops only the exact hats listener recorded in the private runtime files.
pub fn stop() -> Result<Status, String> {
    private_dir(&root())?;
    let _guard = lock::Lock::acquire(&file("launch"))?;
    let Some(process) = pid() else {
        return Ok(status());
    };
    if matches(process) {
        halt(process)?;
    } else {
        clear(process);
    }
    Ok(status())
}

pub fn stop_for(source: &source::Source) -> Result<Status, String> {
    if paths::first_line(&file("source")).as_deref() != Some(&source.key()) {
        return Ok(status_for(source));
    }
    stop()?;
    Ok(status_for(source))
}

pub fn start(source: &source::Source) -> Result<Status, String> {
    use std::os::unix::process::CommandExt;

    store::ensure_root()?;
    private_dir(&root())?;
    let _guard = lock::Lock::acquire(&file("launch"))?;
    if status().running && paths::first_line(&file("source")).as_deref() == Some(&source.key()) {
        return Ok(status_for(source));
    }
    if let Some(process) = pid().filter(|process| matches(*process)) {
        halt(process)?;
    }
    if let Some(stale) = pid() {
        clear(stale);
    }

    let binary = std::env::current_exe().map_err(|e| format!("locating hats: {e}"))?;
    let errors = private_log()?;
    let mut child = Command::new(&binary)
        .args(["serve", "--host", "127.0.0.1", "--port", "8787"])
        .env("CONDUCTOR_DB", source.database())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::from(errors))
        .process_group(0)
        .spawn()
        .map_err(|e| format!("starting the local mobile service: {e}"))?;
    let process = child.id();
    auth::write_private(&file("pid"), &process.to_string())?;
    auth::write_private(&file("binary"), &binary.to_string_lossy())?;

    for _ in 0..50 {
        if matches(process) {
            return Ok(status_for(source));
        }
        if child
            .try_wait()
            .map_err(|e| format!("checking the mobile service: {e}"))?
            .is_some()
        {
            let reason = problem();
            clear(process);
            return Err(reason);
        }
        std::thread::sleep(Duration::from_millis(40));
    }

    let _ = child.kill();
    let _ = child.wait();
    clear(process);
    Err("the local mobile service did not become ready within two seconds".into())
}

pub struct Runtime(u32);

impl Drop for Runtime {
    fn drop(&mut self) {
        clear(self.0);
    }
}
