//! Where everything lives on disk, and the two environment variables Conductor
//! sets when it spawns an agent.

use std::path::{Path, PathBuf};

pub fn accounts_root() -> PathBuf {
    std::env::var_os("CONDUCTOR_ACCOUNTS_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".conductor-accounts"))
}

pub fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default()
}

pub fn routes_file() -> PathBuf {
    accounts_root().join("routes")
}

pub fn session_dir() -> PathBuf {
    accounts_root().join("sessions")
}

pub fn profile_dir(agent: &str, profile: &str) -> PathBuf {
    accounts_root().join(agent).join(profile)
}

/// Resolved, because routes are compared as strings and macOS puts $TMPDIR under
/// the /var to /private/var symlink.
pub fn normalize(dir: &Path) -> PathBuf {
    dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf())
}

/// The directory Conductor would run the agent in. Inside a Conductor session the
/// app says so; from a terminal the working directory is right.
pub fn workspace_dir() -> PathBuf {
    if let Some(p) = std::env::var_os("CONDUCTOR_WORKSPACE_PATH") {
        if !p.is_empty() {
            return normalize(Path::new(&p));
        }
    }
    normalize(&std::env::current_dir().unwrap_or_default())
}

/// Conductor passes `--session-id=`, and `--resume=` when picking a conversation
/// back up. Either identifies the session a pin belongs to.
pub fn session_id(args: &[String]) -> Option<String> {
    for flag in ["--session-id=", "--resume="] {
        if let Some(v) = args.iter().find_map(|a| a.strip_prefix(flag)) {
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

pub fn env_var_for(agent: &str) -> &'static str {
    match agent {
        "codex" => "CODEX_HOME",
        _ => "CLAUDE_CONFIG_DIR",
    }
}

pub fn first_line(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let line = text.lines().next()?.trim();
    if line.is_empty() {
        None
    } else {
        Some(line.to_string())
    }
}

pub fn profiles(agent: &str) -> Vec<String> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(accounts_root().join(agent)) else {
        return out;
    };
    for entry in entries.flatten() {
        if entry.path().is_dir() {
            if let Some(name) = entry.file_name().to_str() {
                out.push(name.to_string());
            }
        }
    }
    out.sort();
    out
}
