//! Writing routes, and the repository a workspace belongs to.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{paths, profile, settings};

pub fn ensure_root() -> Result<(), String> {
    let root = paths::accounts_root();
    for sub in ["claude", "codex", "sessions"] {
        std::fs::create_dir_all(root.join(sub)).map_err(|e| format!("{}: {e}", root.display()))?;
    }
    let config = root.join("config");
    if !config.is_file() {
        let _ = std::fs::write(
            &config,
            "# Reserved for future settings. Accounts are chosen per workspace and live in\n\
             # the routes file next to this one.\n",
        );
    }
    let routes = paths::routes_file();
    if !routes.is_file() {
        let _ = std::fs::write(
            &routes,
            "# <workspace-or-repo-path><TAB><profile>     longest matching prefix wins\n\
             # default<TAB><profile>                      fallback when nothing matches\n",
        );
    }
    Ok(())
}

/// One route per path, last write wins.
pub fn write_route(key: &str, profile_name: &str) -> Result<(), String> {
    ensure_root()?;
    let file = paths::routes_file();
    let existing = std::fs::read_to_string(&file).unwrap_or_default();
    let mut out: Vec<String> = existing
        .lines()
        .filter(|line| {
            let t = line.trim_end();
            if t.is_empty() || t.starts_with('#') {
                return true;
            }
            t.split(['\t', ' ']).next().unwrap_or("") != key
        })
        .map(|l| l.to_string())
        .collect();
    out.push(format!("{key}\t{profile_name}"));
    let mut body = out.join("\n");
    body.push('\n');
    std::fs::write(&file, body).map_err(|e| format!("{}: {e}", file.display()))
}

pub fn drop_route(key: &str) -> Result<(), String> {
    let file = paths::routes_file();
    if !file.is_file() {
        return Ok(());
    }
    let existing = std::fs::read_to_string(&file).unwrap_or_default();
    let kept: Vec<&str> = existing
        .lines()
        .filter(|line| {
            let t = line.trim_end();
            if t.is_empty() || t.starts_with('#') {
                return true;
            }
            t.split(['\t', ' ']).next().unwrap_or("") != key
        })
        .collect();
    let mut body = kept.join("\n");
    body.push('\n');
    std::fs::write(&file, body).map_err(|e| format!("{}: {e}", file.display()))
}

/// Every route pointing at a profile, so removing one leaves nothing dangling.
pub fn drop_routes_to(profile_name: &str) -> Result<(), String> {
    let file = paths::routes_file();
    if !file.is_file() {
        return Ok(());
    }
    let existing = std::fs::read_to_string(&file).unwrap_or_default();
    let kept: Vec<&str> = existing
        .lines()
        .filter(|line| {
            let t = line.trim_end();
            if t.is_empty() || t.starts_with('#') {
                return true;
            }
            match t.split_once(['\t', ' ']) {
                Some((_, rest)) => rest.trim() != profile_name,
                None => true,
            }
        })
        .collect();
    let mut body = kept.join("\n");
    body.push('\n');
    std::fs::write(&file, body).map_err(|e| format!("{}: {e}", file.display()))
}

/// The directory a command applies to: the argument if given, otherwise where
/// Conductor says the agent runs, otherwise the working directory.
pub fn target_dir(arg: Option<&String>) -> Result<PathBuf, String> {
    match arg {
        Some(p) => {
            let path = PathBuf::from(p);
            if !path.is_dir() {
                return Err(format!("no such directory: {p}"));
            }
            Ok(logical(&path))
        }
        None => Ok(paths::workspace_dir()),
    }
}

/// `cd` then `pwd`, which keeps the logical path. Routes recorded from a shell
/// look like this, and matching compares both forms anyway.
fn logical(path: &Path) -> PathBuf {
    Command::new("sh")
        .arg("-c")
        .arg(format!("cd {} && pwd", shell_quote(&path.to_string_lossy())))
        .output()
        .ok()
        .and_then(|o| {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if s.is_empty() { None } else { Some(PathBuf::from(s)) }
        })
        .unwrap_or_else(|| path.to_path_buf())
}

pub fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Conductor exports the repository root; a plain checkout falls back to git.
pub fn repo_root(start: &Path) -> PathBuf {
    if let Some(root) = std::env::var_os("CONDUCTOR_ROOT_PATH") {
        if !root.is_empty() {
            return PathBuf::from(root);
        }
    }
    let out = Command::new("git")
        .args(["-C"])
        .arg(start)
        .args(["rev-parse", "--path-format=absolute", "--git-common-dir"])
        .output();
    if let Ok(out) = out {
        if out.status.success() {
            let common = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !common.is_empty() {
                if let Some(parent) = Path::new(&common).parent() {
                    return parent.to_path_buf();
                }
            }
        }
    }
    start.to_path_buf()
}

/// What this path ends up on, whichever mechanism gets there first. The router
/// never exports a repository binding, because Conductor applies those itself.
pub fn effective_dir(agent: &str, dir: &Path) -> Option<String> {
    if router_installed() {
        if let Some(found) = crate::resolve::decide(agent, dir, None, false) {
            let path = paths::profile_dir(agent, &found);
            if path.is_dir() {
                return Some(path.to_string_lossy().to_string());
            }
        }
    }
    settings::repo_binding(agent, &repo_root(dir))
}

pub fn router_installed() -> bool {
    std::fs::read_to_string(settings::conductor_settings())
        .map(|s| s.contains("claude-router"))
        .unwrap_or(false)
}

pub fn profile_from_dir(dir: &str) -> Option<String> {
    let root = paths::accounts_root();
    let prefix = format!("{}/", root.to_string_lossy());
    let rest = dir.strip_prefix(&prefix)?;
    let mut parts = rest.split('/');
    let _agent = parts.next()?;
    let name = parts.next()?;
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

pub fn label_for_display(agent: &str, name: &str, masked: bool) -> String {
    match profile::label(agent, name) {
        Some(l) if masked => crate::mask::email(&l),
        Some(l) => l,
        None => String::new(),
    }
}
