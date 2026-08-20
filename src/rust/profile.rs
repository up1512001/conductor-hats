//! Accounts on disk: their addresses, and whether they hold credentials.

use std::path::Path;
use std::process::Command;

use crate::paths;

/// The cached address, written after a sign-in and removed on sign-out.
pub fn label(agent: &str, profile: &str) -> Option<String> {
    paths::first_line(&paths::profile_dir(agent, profile).join(".label"))
}

/// Reads the address out of the agent's own state and caches it.
pub fn refresh_label(agent: &str, profile: &str) -> Option<String> {
    let dir = paths::profile_dir(agent, profile);
    let (file, key) = match agent {
        "codex" => (dir.join("auth.json"), "email"),
        _ => (dir.join(".claude.json"), "emailAddress"),
    };
    let text = std::fs::read_to_string(&file).ok()?;
    let found = extract(&text, key)?;
    let _ = std::fs::write(dir.join(".label"), format!("{found}\n"));
    Some(found)
}

/// The first `"key": "value"` in a blob, without depending on a JSON parser for
/// one field.
fn extract(text: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    let at = text.find(&needle)?;
    let rest = &text[at + needle.len()..];
    let colon = rest.find(':')?;
    let after = &rest[colon + 1..];
    let open = after.find('"')?;
    let tail = &after[open + 1..];
    let close = tail.find('"')?;
    let value = &tail[..close];
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

/// Whether a profile actually holds credentials.
///
/// Claude Code resolves them as `$CLAUDE_CONFIG_DIR/.credentials.json`, then a
/// keychain item whose service name carries the first 8 hex of sha256 of the
/// config directory. Both are checked, in that order.
///
/// Not inferred from `.label`: that is a cached address, only written once the
/// agent has recorded one, so a profile with working credentials read as signed
/// out for ever.
pub fn signed_in(agent: &str, profile: &str) -> bool {
    let dir = paths::profile_dir(agent, profile);
    if !dir.is_dir() {
        return false;
    }
    match agent {
        "codex" => non_empty(&dir.join("auth.json")),
        _ => {
            if non_empty(&dir.join(".credentials.json")) {
                return true;
            }
            keychain_has(&keychain_service(&dir))
        }
    }
}

fn non_empty(path: &Path) -> bool {
    std::fs::metadata(path).map(|m| m.len() > 0).unwrap_or(false)
}

fn keychain_service(dir: &Path) -> String {
    format!("Claude Code-credentials-{}", sha256_prefix(&dir.to_string_lossy()))
}

fn keychain_has(service: &str) -> bool {
    Command::new("security")
        .args(["find-generic-password", "-s", service])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// First 8 hex characters of sha256, via shasum, which ships with macOS.
fn sha256_prefix(input: &str) -> String {
    use std::io::Write;
    let Ok(mut child) = Command::new("shasum")
        .args(["-a", "256"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
    else {
        return String::new();
    };
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(input.as_bytes());
    }
    let Ok(out) = child.wait_with_output() else {
        return String::new();
    };
    String::from_utf8_lossy(&out.stdout).chars().take(8).collect()
}

/// Which other profile already holds this address. One live token per account, so
/// a pair sharing one sign each other out.
pub fn with_email(agent: &str, email: &str, skip: &str) -> Option<String> {
    if email.is_empty() {
        return None;
    }
    paths::profiles(agent)
        .into_iter()
        .find(|p| p != skip && label(agent, p).as_deref() == Some(email))
}

pub fn require(agent: &str, profile: &str) -> Result<(), String> {
    if paths::profile_dir(agent, profile).is_dir() {
        Ok(())
    } else {
        Err(format!(
            "no such {agent} profile '{profile}' (run: conductor-acct add {profile} {agent})"
        ))
    }
}

pub fn valid_name(name: &str) -> Result<(), String> {
    if !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        Ok(())
    } else {
        Err("profile names may only contain letters, digits, - and _".into())
    }
}
