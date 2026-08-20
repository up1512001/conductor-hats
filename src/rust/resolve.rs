//! Which account a spawn gets, and which binary to hand it to.

use std::path::{Path, PathBuf};

use crate::{id, paths, routes};

/// Highest precedence first:
///
///   1. `CONDUCTOR_ACCOUNT`, a one-off override for a single spawn
///   2. the session pin, so a running conversation never changes account: an
///      account change under a live `--resume` would break it
///   3. a route naming this exact workspace, which outranks a repository binding
///   4. a repository binding, which Conductor has already applied to the
///      environment and which has therefore answered the question
///   5. a parent-directory route, then the default
///
/// `env_bound` is true when the agent's config directory is already set, which is
/// how a repository binding arrives.
pub fn decide(agent: &str, dir: &Path, session: Option<&str>, env_bound: bool) -> Option<String> {
    if let Some(forced) = std::env::var_os("CONDUCTOR_ACCOUNT") {
        let forced = forced.to_string_lossy().to_string();
        if !forced.is_empty() {
            return id::profile_or_none(&forced).map(str::to_string);
        }
    }

    if let Some(session) = session.and_then(id::session) {
        if let Some(pinned) = paths::first_line(&pin_path(agent, session)) {
            if let Some(valid) = id::profile_or_none(&pinned) {
                return Some(valid.to_string());
            }
        }
    }

    let found = routes::resolve(dir);
    if let Some(m) = &found {
        if m.exact {
            remember(agent, session, &m.profile);
            return Some(m.profile.clone());
        }
    }
    if env_bound {
        return None;
    }
    let profile = found.map(|m| m.profile)?;
    remember(agent, session, &profile);
    Some(profile)
}

fn pin_path(agent: &str, session: &str) -> PathBuf {
    paths::session_dir().join(agent).join(session)
}

fn remember(agent: &str, session: Option<&str>, profile: &str) {
    let Some(session) = session.and_then(id::session) else {
        return;
    };
    let dir = paths::session_dir().join(agent);
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let _ = std::fs::write(dir.join(session), format!("{profile}\n"));
}

/// The real agent, never this binary. An override for the tests, then a pinned
/// path, then the copy Conductor ships, then `$PATH`.
pub fn agent_binary(agent: &str) -> Option<PathBuf> {
    let override_var = match agent {
        "codex" => "CONDUCTOR_ACCOUNTS_CODEX_BIN",
        _ => "CONDUCTOR_ACCOUNTS_CLAUDE_BIN",
    };
    if let Some(p) = std::env::var_os(override_var) {
        let p = PathBuf::from(p);
        if is_executable(&p) {
            return Some(p);
        }
    }

    if let Some(pinned) = paths::first_line(&paths::accounts_root().join(format!("{agent}-bin"))) {
        let p = PathBuf::from(pinned);
        if is_executable(&p) {
            return Some(p);
        }
    }

    let bundled_dir = std::env::var_os("CONDUCTOR_AGENT_BIN_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            paths::home().join("Library/Application Support/com.conductor.app/bin")
        });
    let bundled = bundled_dir.join(agent);
    if is_executable(&bundled) {
        return Some(bundled);
    }

    which(agent)
}

fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

/// Skips anything that resolves back to this binary, so a `hats` on `$PATH`
/// named after the agent cannot make the router call itself.
fn which(agent: &str) -> Option<PathBuf> {
    let self_path = std::env::current_exe().ok();
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(agent);
        if !is_executable(&candidate) {
            continue;
        }
        let same = self_path
            .as_ref()
            .and_then(|s| s.canonicalize().ok())
            .zip(candidate.canonicalize().ok())
            .map(|(a, b)| a == b)
            .unwrap_or(false);
        if !same {
            return Some(candidate);
        }
    }
    None
}
