//! Commands that change something: routes, bindings, profiles, the router.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{paths, profile, resolve, settings, store};

pub fn install_bin() -> PathBuf {
    paths::accounts_root().join("bin")
}

pub fn use_route(name: &str, agent: &str, dir: &Path) -> Result<(), String> {
    profile::require(agent, name)?;
    store::write_route(&dir.to_string_lossy(), name)?;
    let label = profile::label(agent, name).unwrap_or_default();
    let suffix = if label.is_empty() { String::new() } else { format!(" ({label})") };
    let leaf = dir.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    println!("{leaf} now uses '{name}'{suffix}");
    println!("  {}", dir.display());
    println!();
    println!("Open a new chat in this workspace for it to take effect. A chat that is");
    println!("already running keeps the account its agent process started on.");
    Ok(())
}

pub fn assign(key: &str, name: &str, agent: &str) -> Result<(), String> {
    profile::require(agent, name)?;
    store::write_route(key, name)?;
    if key == "default" {
        println!("default account is now '{name}'");
    } else {
        println!("{key} now uses '{name}'");
    }
    Ok(())
}

pub fn unassign(key: &str) -> Result<(), String> {
    store::drop_route(key)?;
    println!("dropped the route for {key}");
    Ok(())
}

pub fn bind(name: &str, agent: &str, repo: &Path) -> Result<(), String> {
    profile::require(agent, name)?;
    let dir = paths::profile_dir(agent, name);
    let file = settings::set_repo_binding(agent, repo, &dir.to_string_lossy())?;
    println!("{} now uses '{name}' for every workspace", repo.display());
    println!("  {}", file.display());
    println!();
    println!("That file is machine local. Add it to .gitignore.");
    Ok(())
}

pub fn unbind(agent: &str, repo: &Path) -> Result<(), String> {
    settings::clear_repo_binding(agent, repo)?;
    println!("dropped the {agent} binding for {}", repo.display());
    Ok(())
}

pub fn logout(name: &str, agent: &str) -> Result<(), String> {
    profile::require(agent, name)?;
    let dir = paths::profile_dir(agent, name);
    let binary = resolve::agent_binary(agent).ok_or("could not locate the real agent binary")?;
    let args: &[&str] = if agent == "codex" { &["logout"] } else { &["auth", "logout"] };
    let _ = Command::new(&binary)
        .args(args)
        .env(paths::env_var_for(agent), &dir)
        .status();
    let _ = std::fs::remove_file(dir.join(".label"));
    println!(
        "Signed out of '{name}'. The profile directory is still there; \
         conductor-acct remove {name} deletes it."
    );
    Ok(())
}

pub fn remove(name: &str, agent: &str) -> Result<(), String> {
    profile::require(agent, name)?;
    store::ensure_root()?;
    let _ = logout(name, agent);
    let dir = paths::profile_dir(agent, name);
    std::fs::remove_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    store::drop_routes_to(name)?;
    println!("Removed the {agent} profile '{name}' and any routes pointing at it.");
    Ok(())
}

/// Shares everything except credentials, so skills, plugins, hooks and
/// transcripts stay common across accounts.
const SHARED: [&str; 7] = [
    "projects",
    "skills",
    "plugins",
    "commands",
    "agents",
    "settings.json",
    "CLAUDE.md",
];

pub fn add(name: &str, agent: &str) -> Result<(), String> {
    profile::valid_name(name)?;
    store::ensure_root()?;
    let dir = paths::profile_dir(agent, name);
    if dir.is_dir() {
        println!("Profile '{name}' already exists at {}", dir.display());
    } else {
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        println!("Created {}", dir.display());
    }
    if agent == "claude" {
        let source = paths::home().join(".claude");
        for item in SHARED {
            let from = source.join(item);
            let to = dir.join(item);
            if !from.exists() || to.exists() {
                continue;
            }
            if std::os::unix::fs::symlink(&from, &to).is_ok() {
                println!("  linked {item} -> {}", from.display());
            }
        }
    }
    login(name, agent)
}

pub fn login(name: &str, agent: &str) -> Result<(), String> {
    profile::require(agent, name)?;
    let dir = paths::profile_dir(agent, name);
    let binary = resolve::agent_binary(agent).ok_or("could not locate the real agent binary")?;
    println!("Signing in to '{name}' using {}", binary.display());
    println!("  {}={}", paths::env_var_for(agent), dir.display());
    println!();

    let args: &[&str] = if agent == "codex" { &["login"] } else { &["auth", "login"] };
    let _ = Command::new(&binary)
        .args(args)
        .env(paths::env_var_for(agent), &dir)
        .status();

    println!();
    match profile::refresh_label(agent, name) {
        Some(email) => {
            println!("Profile '{name}' is now {email}");
            if let Some(clash) = profile::with_email(agent, &email, name) {
                warn_duplicate(agent, name, &clash, &email);
            }
        }
        None => println!(
            "Signed in. Could not read the account email; the picker will show the profile name only."
        ),
    }
    Ok(())
}

/// A warning rather than a refusal: the address is only knowable after the OAuth
/// round trip, so refusing now would leave the profile in a state the message
/// denies.
fn warn_duplicate(agent: &str, name: &str, clash: &str, email: &str) {
    eprintln!();
    eprintln!("Warning: '{clash}' is already signed in to {email} on {agent}.");
    eprintln!();
    eprintln!("One account cannot be two accounts. The provider keeps a single live token per");
    eprintln!("account, so whichever of '{name}' and '{clash}' signed in last holds it and the");
    eprintln!("other is now signed out. They will keep logging each other out.");
    eprintln!();
    eprintln!("Keep one and drop the other:");
    eprintln!("  conductor-acct remove {clash} {agent}");
}

pub fn sessions(clear: bool) -> Result<(), String> {
    let dir = paths::session_dir();
    if clear {
        for agent in ["claude", "codex"] {
            let _ = std::fs::remove_dir_all(dir.join(agent));
        }
        println!("cleared session pins");
        return Ok(());
    }
    for agent in ["claude", "codex"] {
        let Ok(entries) = std::fs::read_dir(dir.join(agent)) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if let Some(pinned) = paths::first_line(&entry.path()) {
                println!("{agent:<7} {name} -> {pinned}");
            }
        }
    }
    Ok(())
}
