//! Writing routes, and the repository a workspace belongs to.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{lock, paths, profile, settings};

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

/// Read, change and write the routes file under a lock, so a change made by one
/// workspace cannot lose a change made by another between its read and its write.
fn mutate(keep: impl Fn(&str) -> bool, add: Option<String>) -> Result<(), String> {
    ensure_root()?;
    let file = paths::routes_file();
    let _guard = lock::Lock::acquire(&file)?;

    let existing = std::fs::read_to_string(&file).unwrap_or_default();
    let mut out: Vec<String> = existing
        .lines()
        .filter(|line| {
            let t = line.trim_end();
            t.is_empty() || t.starts_with('#') || keep(t)
        })
        .map(|l| l.to_string())
        .collect();
    if let Some(line) = add {
        out.push(line);
    }
    let mut body = out.join("\n");
    body.push('\n');
    lock::write_atomic(&file, &body)
}

fn key_of(line: &str) -> &str {
    line.split(['\t', ' ']).next().unwrap_or("")
}

/// One route per path, last write wins.
pub fn write_route(key: &str, profile_name: &str) -> Result<(), String> {
    let owned = key.to_string();
    mutate(
        move |line| key_of(line) != owned,
        Some(format!("{key}\t{profile_name}")),
    )
}

pub fn drop_route(key: &str) -> Result<(), String> {
    let owned = key.to_string();
    mutate(move |line| key_of(line) != owned, None)
}

/// Every route pointing at a profile, so removing one leaves nothing dangling.
pub fn drop_routes_to(profile_name: &str) -> Result<(), String> {
    let owned = profile_name.to_string();
    mutate(
        move |line| match line.split_once(['\t', ' ']) {
            Some((_, rest)) => rest.trim() != owned,
            None => true,
        },
        None,
    )
}

/// The directory a command applies to: the argument if given, otherwise where
/// Conductor says the agent runs, otherwise the working directory.
/// An empty argument means "not given", so a caller can pass a placeholder in
/// order to reach the argument after it. The panel does exactly that when it
/// knows the chat but not the workspace around it.
pub fn target_dir(arg: Option<&String>) -> Result<PathBuf, String> {
    match arg.filter(|p| !p.is_empty()) {
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

/// The same, for reading rather than writing.
///
/// Conductor records a workspace before it finishes making its working tree, so
/// the panel can be asked about a directory that does not exist yet. Refusing
/// there put `no such directory: .../bangkok` in the panel instead of the
/// account. Nothing here needs the directory: routes are matched as paths.
pub fn report_dir(arg: Option<&String>) -> Result<PathBuf, String> {
    match arg.filter(|p| !p.is_empty()) {
        Some(p) => {
            let path = PathBuf::from(p);
            Ok(if path.is_dir() { logical(&path) } else { path })
        }
        None => Ok(paths::workspace_dir()),
    }
}

/// `cd` then `pwd`, which keeps the logical path. Routes recorded from a shell
/// look like this, and matching compares both forms anyway.
fn logical(path: &Path) -> PathBuf {
    Command::new("sh")
        .arg("-c")
        .arg(format!(
            "cd {} && pwd",
            shell_quote(&path.to_string_lossy())
        ))
        .output()
        .ok()
        .and_then(|o| {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if s.is_empty() {
                None
            } else {
                Some(PathBuf::from(s))
            }
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
