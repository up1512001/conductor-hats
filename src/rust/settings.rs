//! Editing TOML by line rather than by parser.
//!
//! Conductor's settings file and a repository's `.conductor/settings.local.toml`
//! both belong to their owners, so the rest of the file is preserved exactly.
//! Top-level keys must land above the first `[table]` header or the file stops
//! being valid TOML.

use std::path::{Path, PathBuf};

use crate::paths;

pub fn conductor_settings() -> PathBuf {
    std::env::var_os("CONDUCTOR_ACCT_SETTINGS_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| paths::home().join(".conductor/settings.toml"))
}

pub fn commands_dir() -> PathBuf {
    std::env::var_os("CONDUCTOR_ACCT_COMMANDS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| paths::home().join(".claude/commands"))
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

fn write(path: &Path, body: &str) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    std::fs::write(path, body).map_err(|e| format!("{}: {e}", path.display()))
}

pub fn set_key(path: &Path, key: &str, value: &str) -> Result<(), String> {
    let existing = read(path);
    let line = format!("{key} = \"{value}\"");
    let mut out: Vec<String> = Vec::new();
    let mut placed = false;

    for raw in existing.lines() {
        let trimmed = raw.trim_start();
        if trimmed.starts_with(&format!("{key} ")) || trimmed.starts_with(&format!("{key}=")) {
            continue;
        }
        if !placed && trimmed.starts_with('[') {
            out.push(line.clone());
            out.push(String::new());
            placed = true;
        }
        out.push(raw.to_string());
    }
    if !placed {
        out.push(line);
    }
    let mut body = out.join("\n");
    if !body.ends_with('\n') {
        body.push('\n');
    }
    write(path, &body)
}

pub fn unset_key(path: &Path, key: &str) -> Result<(), String> {
    if !path.is_file() {
        return Ok(());
    }
    let existing = read(path);
    let kept: Vec<&str> = existing
        .lines()
        .filter(|raw| {
            let t = raw.trim_start();
            !(t.starts_with(&format!("{key} ")) || t.starts_with(&format!("{key}=")))
        })
        .collect();
    let mut body = kept.join("\n");
    if !body.ends_with('\n') {
        body.push('\n');
    }
    write(path, &body)
}

pub fn get_key(path: &Path, key: &str) -> Option<String> {
    for raw in read(path).lines() {
        let t = raw.trim_start();
        if t.starts_with(&format!("{key} ")) || t.starts_with(&format!("{key}=")) {
            let value = t.split_once('=')?.1.trim();
            return Some(value.trim_matches('"').to_string());
        }
    }
    None
}

/// A repository binding, which Conductor applies itself when it spawns the agent,
/// so the router never sees it.
pub fn repo_binding(agent: &str, repo: &Path) -> Option<String> {
    let var = paths::env_var_for(agent);
    for name in ["settings.local.toml", "settings.toml"] {
        let file = repo.join(".conductor").join(name);
        if !file.is_file() {
            continue;
        }
        let mut in_env = false;
        for raw in read(&file).lines() {
            let t = raw.trim();
            if t.starts_with('[') {
                in_env = t == "[environment_variables]" || t == "[environment_variables.local]";
                continue;
            }
            if in_env {
                if let Some((k, v)) = t.split_once('=') {
                    if k.trim() == var {
                        return Some(v.trim().trim_matches('"').to_string());
                    }
                }
            }
        }
    }
    None
}

pub fn set_repo_binding(agent: &str, repo: &Path, dir: &str) -> Result<PathBuf, String> {
    let var = paths::env_var_for(agent);
    let file = repo.join(".conductor/settings.local.toml");
    let existing = read(&file);

    let mut out: Vec<String> = Vec::new();
    let mut in_env = false;
    let mut wrote = false;
    let mut saw_table = false;

    for raw in existing.lines() {
        let t = raw.trim();
        if t.starts_with('[') {
            if in_env && !wrote {
                out.push(format!("{var} = \"{dir}\""));
                wrote = true;
            }
            in_env = t == "[environment_variables]";
            if in_env {
                saw_table = true;
            }
            out.push(raw.to_string());
            continue;
        }
        if in_env {
            if let Some((k, _)) = t.split_once('=') {
                if k.trim() == var {
                    out.push(format!("{var} = \"{dir}\""));
                    wrote = true;
                    continue;
                }
            }
        }
        out.push(raw.to_string());
    }
    if in_env && !wrote {
        out.push(format!("{var} = \"{dir}\""));
    }
    if !saw_table {
        if !out.is_empty() && !out.last().map(|l| l.trim().is_empty()).unwrap_or(true) {
            out.push(String::new());
        }
        out.push("[environment_variables]".into());
        out.push(format!("{var} = \"{dir}\""));
    }
    let mut body = out.join("\n");
    if !body.ends_with('\n') {
        body.push('\n');
    }
    write(&file, &body)?;
    Ok(file)
}

pub fn clear_repo_binding(agent: &str, repo: &Path) -> Result<(), String> {
    let var = paths::env_var_for(agent);
    let file = repo.join(".conductor/settings.local.toml");
    if !file.is_file() {
        return Ok(());
    }
    let kept: Vec<String> = read(&file)
        .lines()
        .filter(|raw| {
            raw.trim()
                .split_once('=')
                .map(|(k, _)| k.trim() != var)
                .unwrap_or(true)
        })
        .map(|l| l.to_string())
        .collect();
    let mut body = kept.join("\n");
    if !body.ends_with('\n') {
        body.push('\n');
    }
    write(&file, &body)
}
