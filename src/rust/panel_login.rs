//! A browser login driven by the injected panel instead of a terminal.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime};

use crate::{manage, paths, profile, resolve, store};

const LOGIN_TTL: Duration = Duration::from_secs(5 * 60);

fn state(agent: &str, name: &str) -> PathBuf {
    paths::accounts_root().join("login").join(agent).join(name)
}

fn private_dir(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::create_dir_all(path).map_err(|e| format!("{}: {e}", path.display()))?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
        .map_err(|e| format!("{}: {e}", path.display()))
}

fn private_file(path: &Path) -> Result<std::fs::File, String> {
    use std::os::unix::fs::OpenOptionsExt;
    OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
        .map_err(|e| format!("{}: {e}", path.display()))
}

fn write_state(path: &Path, value: &str) -> Result<(), String> {
    let mut file = private_file(path)?;
    writeln!(file, "{value}").map_err(|e| format!("{}: {e}", path.display()))
}

fn pid(path: &Path) -> Option<u32> {
    paths::first_line(&path.join("pid"))?.parse().ok()
}

fn owned_process(path: &Path, process: u32) -> bool {
    if process <= 1 {
        return false;
    }
    let Some(binary) = paths::first_line(&path.join("binary")) else {
        return false;
    };
    let Ok(output) = Command::new("ps")
        .args(["-o", "command=", "-p", &process.to_string()])
        .output()
    else {
        return false;
    };
    output.status.success() && String::from_utf8_lossy(&output.stdout).contains(&binary)
}

fn cancel_state(path: &Path) -> Result<(), String> {
    if let Some(process) = pid(path).filter(|process| owned_process(path, *process)) {
        let _ = Command::new("kill")
            .args(["-TERM", &process.to_string()])
            .status();
    }
    if path.exists() {
        std::fs::remove_dir_all(path).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(())
}

fn expired(path: &Path) -> bool {
    std::fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .and_then(|made| {
            SystemTime::now()
                .duration_since(made)
                .map_err(std::io::Error::other)
        })
        .map(|age| age >= LOGIN_TTL)
        .unwrap_or(true)
}

fn output(path: &Path) -> String {
    std::fs::read_to_string(path.join("out")).unwrap_or_default()
}

fn login_url(body: &str) -> Option<String> {
    for (start, _) in body.match_indices("https://") {
        let tail = &body[start..];
        let end = tail
            .find(|c: char| c.is_whitespace() || c == '\x1b')
            .unwrap_or(tail.len());
        let found = tail[..end].trim_end_matches([')', ']', '}', ',', ';', '\'', '"']);
        if found.contains("oauth") || found.contains("auth.openai.com") {
            return Some(found.to_string());
        }
    }
    None
}

fn problem(body: &str) -> String {
    let lines: Vec<&str> = body.lines().rev().take(3).collect();
    let text = lines
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if text.is_empty() {
        "sign-in ended without credentials".into()
    } else {
        text.chars().take(500).collect()
    }
}

pub fn start(name: &str, agent: &str) -> Result<(), String> {
    profile::valid_name(name)?;
    store::ensure_root()?;
    let path = state(agent, name);
    cancel_state(&path)?;
    manage::prepare_profile(name, agent, false)?;
    private_dir(&path)?;

    let fifo = path.join("stdin");
    let made = Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .map(|status| status.success())
        .unwrap_or(false);
    if !made {
        cancel_state(&path)?;
        return Err("could not create the private sign-in channel".into());
    }
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&fifo, std::fs::Permissions::from_mode(0o600))
        .map_err(|e| format!("{}: {e}", fifo.display()))?;
    let input = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&fifo)
        .map_err(|e| format!("{}: {e}", fifo.display()))?;
    let out = private_file(&path.join("out"))?;
    let binary = resolve::agent_binary(agent).ok_or("could not locate the real agent binary")?;
    write_state(&path.join("binary"), &binary.to_string_lossy())?;
    let args: &[&str] = if agent == "codex" {
        &["login"]
    } else {
        &["auth", "login"]
    };
    let mut command = Command::new(&binary);
    command
        .args(args)
        .env(paths::env_var_for(agent), paths::profile_dir(agent, name))
        .stdin(Stdio::from(input))
        .stdout(Stdio::from(
            out.try_clone().map_err(|e| format!("login output: {e}"))?,
        ))
        .stderr(Stdio::from(out));
    for key in [
        "CONDUCTOR_ACCOUNTS_ROUTING",
        "CONDUCTOR_ACCOUNTS_DEPTH",
        "CONDUCTOR_ACCOUNT",
    ] {
        command.env_remove(key);
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("could not run {}: {e}", binary.display()))?;
    write_state(&path.join("pid"), &child.id().to_string())?;

    for _ in 0..100 {
        if let Some(url) = login_url(&output(&path)) {
            println!("{url}");
            return Ok(());
        }
        if child.try_wait().ok().flatten().is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let reason = problem(&output(&path));
    cancel_state(&path)?;
    Err(format!("sign-in did not produce an approval URL: {reason}"))
}

pub fn code(name: &str, agent: &str, code: &str) -> Result<(), String> {
    profile::valid_name(name)?;
    if code.is_empty() || code.len() > 8192 || code.contains(['\0', '\n', '\r']) {
        return Err("the sign-in code is empty or invalid".into());
    }
    let path = state(agent, name);
    let Some(process) = pid(&path) else {
        return Err(format!("no sign-in in progress for '{name}'"));
    };
    if !owned_process(&path, process) || expired(&path) {
        cancel_state(&path)?;
        return Err(format!("no active sign-in for '{name}'"));
    }
    use std::os::unix::fs::FileTypeExt;
    let fifo = path.join("stdin");
    if !std::fs::metadata(&fifo)
        .map(|metadata| metadata.file_type().is_fifo())
        .unwrap_or(false)
    {
        return Err(format!("no private sign-in channel for '{name}'"));
    }
    let mut input = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&fifo)
        .map_err(|e| format!("{}: {e}", fifo.display()))?;
    writeln!(input, "{code}").map_err(|e| format!("submitting sign-in code: {e}"))?;
    println!("submitted");
    Ok(())
}

pub fn status(name: &str, agent: &str) -> Result<(), String> {
    profile::valid_name(name)?;
    let path = state(agent, name);
    if !path.is_dir() {
        println!("idle");
        return Ok(());
    }
    if pid(&path)
        .filter(|process| owned_process(&path, *process))
        .is_some()
        && !expired(&path)
    {
        println!("pending");
        return Ok(());
    }
    let reason = problem(&output(&path));
    cancel_state(&path)?;
    if let Some(email) = profile::refresh_label(agent, name) {
        println!("ok {email}");
    } else if profile::signed_in(agent, name) {
        println!("ok");
    } else {
        println!("error {reason}");
    }
    Ok(())
}

pub fn cancel(name: &str, agent: &str) -> Result<(), String> {
    profile::valid_name(name)?;
    cancel_state(&state(agent, name))?;
    println!("cancelled");
    Ok(())
}
