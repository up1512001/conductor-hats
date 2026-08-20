//! Validating identifiers before they become path components.
//!
//! A profile name, a session id and a profile read back out of the routes file
//! all end up joined onto a filesystem root. An unchecked `..` in any of them
//! escapes that root, so validation lives at every boundary rather than at the
//! one the CLI happens to use: the same value arrives from argv, from the
//! environment, and from state written by an earlier run.

use std::path::Path;

/// Letters, digits, `-` and `_`, up to 64 characters. Deliberately narrower than
/// the filesystem allows, because a name is an identifier and not a path.
pub fn profile(name: &str) -> Result<&str, String> {
    if name.is_empty() {
        return Err("profile names cannot be empty".into());
    }
    if name.chars().count() > 64 {
        return Err("profile names are limited to 64 characters".into());
    }
    if !name.chars().all(ok_char) {
        return Err(format!(
            "invalid profile name '{}': letters, digits, - and _ only",
            name.escape_debug()
        ));
    }
    Ok(name)
}

fn ok_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-' || c == '_'
}

/// A value read back from disk or the environment, where a bad one means the
/// state is corrupt rather than mistyped. Dropped rather than reported, so the
/// router keeps failing open.
pub fn profile_or_none(name: &str) -> Option<&str> {
    profile(name).ok()
}

/// Session identifiers name pin files, so they are held to the same charset.
/// Conductor's are UUIDs; anything else is not one and gets no pin.
pub fn session(raw: &str) -> Option<&str> {
    if raw.is_empty() || raw.chars().count() > 128 || !raw.chars().all(ok_char) {
        return None;
    }
    Some(raw)
}

pub fn agent(name: &str) -> Result<&'static str, String> {
    match name {
        "claude" => Ok("claude"),
        "codex" => Ok("codex"),
        other => Err(format!(
            "unknown agent '{other}' (expected claude or codex)"
        )),
    }
}

/// The last check before a destructive operation.
///
/// Validation upstream should make this unreachable, so reaching it means a
/// boundary was missed. Both sides are resolved first, which catches a symlink
/// pointing out of the root as well as a `..` that survived.
pub fn contained(root: &Path, path: &Path) -> Result<(), String> {
    let root_real = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let path_real = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    if path_real == root_real || !path_real.starts_with(&root_real) {
        return Err(format!(
            "refusing to touch {}: outside {}",
            path_real.display(),
            root_real.display()
        ));
    }
    Ok(())
}
