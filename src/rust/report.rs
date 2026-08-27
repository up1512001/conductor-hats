//! What a workspace resolves to: list, status, which, check, json.

use std::path::Path;

use crate::{id, mask, paths, profile, resolve, routes, session, settings, store};

pub fn list(masked: bool) -> Result<(), String> {
    store::ensure_root()?;
    for agent in ["claude", "codex"] {
        let names = paths::profiles(agent);
        if names.is_empty() {
            continue;
        }
        println!("{agent} profiles:");
        for name in &names {
            let label = store::label_for_display(agent, name, masked);
            let shown = if !label.is_empty() {
                label
            } else if profile::signed_in(agent, name) {
                "(signed in, address not cached yet)".into()
            } else {
                "(not signed in)".into()
            };
            println!("  {name:<14} {shown}");
        }
        println!();
    }

    println!("routes:");
    let body = std::fs::read_to_string(paths::routes_file()).unwrap_or_default();
    let mut any = false;
    for line in body.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        println!("  {line}");
        any = true;
    }
    if !any {
        println!("  (none)");
    }
    println!();
    if store::router_installed() {
        println!("router: on");
    } else {
        println!("router: off   (repository bindings still work; hats install turns it on)");
    }
    Ok(())
}

pub fn status(dir: &Path, masked: bool) -> Result<(), String> {
    store::ensure_root()?;
    for agent in ["claude", "codex"] {
        if paths::profiles(agent).is_empty() {
            continue;
        }
        match store::effective_dir(agent, dir) {
            Some(resolved) => {
                let name = store::profile_from_dir(&resolved).unwrap_or(resolved.clone());
                let label = store::label_for_display(agent, &name, masked);
                let suffix = if label.is_empty() {
                    String::new()
                } else {
                    format!("  {label}")
                };
                println!("{agent:<7} {name}{suffix}");
            }
            None => println!("{agent:<7} (default account)"),
        }
    }
    println!("in      {}", dir.display());
    Ok(())
}

pub fn which(dir: &Path, agent: &str) -> Result<(), String> {
    store::ensure_root()?;
    let repo = store::repo_root(dir);
    let var = paths::env_var_for(agent);
    let binding = settings::repo_binding(agent, &repo);

    println!("workspace:  {}", dir.display());
    println!("repository: {}", repo.display());

    let bound_profile = binding.as_deref().and_then(store::profile_from_dir);
    match (&binding, &bound_profile) {
        (Some(b), p) => println!(
            "binding:    {}   (.conductor settings, applies to the whole repository)",
            p.clone().unwrap_or_else(|| b.clone())
        ),
        (None, _) => println!("binding:    (none)"),
    }

    match session::current(agent, dir) {
        session::Current::Chat(live) => match session::pinned(agent, &live) {
            Some(name) => println!("chat:       {name}   (pinned, {live})"),
            None => println!("chat:       (follows the workspace, {live})"),
        },
        session::Current::Idle => println!("chat:       (none active here recently)"),
        session::Current::Ambiguous(n) => {
            println!("chat:       (ambiguous, {n} written at once)")
        }
    }

    match routes::resolve(dir) {
        Some(m) if m.exact => println!("route:      {}   (this workspace)", m.profile),
        Some(m) => println!(
            "route:      {}   (inherited from a parent path or the default)",
            m.profile
        ),
        None => println!("route:      (none)"),
    }

    if !store::router_installed() {
        println!();
        println!("router:     off, so routes are recorded but not applied");
        match (&binding, &bound_profile) {
            (Some(b), p) => println!(
                "effective:  {}   (from the repository binding)",
                p.clone().unwrap_or_else(|| b.clone())
            ),
            (None, _) => println!("effective:  your default account"),
        }
        return Ok(());
    }

    println!();
    match resolve::decide(agent, dir, None, false) {
        Some(name) => {
            let path = paths::profile_dir(agent, &name);
            println!("effective:  {name}");
            println!("            {var}={}", path.display());
        }
        None => match (&binding, &bound_profile) {
            (Some(b), p) => {
                println!(
                    "effective:  {}   (from the repository binding, applied by Conductor)",
                    p.clone().unwrap_or_else(|| b.clone())
                );
                println!("            {var}={b}");
            }
            (None, _) => println!("effective:  your default account (no {var} would be set)"),
        },
    }
    Ok(())
}

/// Deliberately terse: an agent reads this at the top of every session.
pub fn check(dir: &Path) -> Result<(), String> {
    store::ensure_root()?;
    for agent in ["claude", "codex"] {
        if paths::profiles(agent).is_empty() {
            continue;
        }
        if let Some(resolved) = store::effective_dir(agent, dir) {
            if let Some(name) = store::profile_from_dir(&resolved) {
                println!("ACCOUNT {agent} {name}");
                return Ok(());
            }
        }
    }
    if paths::profiles("claude").is_empty() {
        println!("NO_PROFILES");
    } else {
        println!("NEEDS_ACCOUNT");
    }
    Ok(())
}

/// The panel reads this, so the field names are a contract. Serialised rather
/// than formatted: a profile name or an address containing a quote or a
/// backslash used to produce JSON the panel could not parse.
#[derive(serde::Serialize)]
struct Account {
    name: String,
    email: String,
    active: bool,
    #[serde(rename = "signedIn")]
    signed_in: bool,
}

/// `current` is the workspace, which is what the toolbar control sets. `chat` is
/// what the conversation on screen actually resolves to, which differs whenever
/// that chat carries a pin. Reporting only the first made every chat in a
/// workspace claim the same account.
#[derive(serde::Serialize)]
struct Provider {
    agent: String,
    current: String,
    session: String,
    chat: String,
    pinned: bool,
    accounts: Vec<Account>,
}

#[derive(serde::Serialize)]
struct State {
    workspace: String,
    repo: String,
    enabled: bool,
    providers: Vec<Provider>,
}

/// `given` is the chat the caller already knows it is looking at.
///
/// The panel reads it out of the components the toolbar sits inside, which is
/// exact. Detecting it here is a fallback for callers with no window to read:
/// the terminal, and `/account`.
pub fn json(dir: &Path, given: Option<&str>) -> Result<(), String> {
    store::ensure_root()?;
    let named = given.and_then(id::session);
    let mut providers = Vec::new();
    for agent in ["claude", "codex"] {
        let current = store::effective_dir(agent, dir)
            .as_deref()
            .and_then(store::profile_from_dir)
            .unwrap_or_default();
        /* A workspace created moments ago has no route yet, because nothing has
         * started an agent in it. It still has an account: the one chosen while
         * it was being created, which the first spawn will take. */
        let current = crate::pending::for_display(agent, dir).unwrap_or(current);
        let live = match named {
            Some(id) => id.to_string(),
            None => match session::current(agent, dir) {
                session::Current::Chat(id) => id,
                _ => String::new(),
            },
        };
        let pin = if live.is_empty() {
            None
        } else {
            session::pinned(agent, &live)
        };
        let chat = pin.clone().unwrap_or_else(|| current.clone());

        let accounts = paths::profiles(agent)
            .into_iter()
            .map(|name| Account {
                email: profile::label(agent, &name).unwrap_or_default(),
                active: name == current,
                signed_in: profile::signed_in(agent, &name),
                name,
            })
            .collect();
        providers.push(Provider {
            agent: agent.to_string(),
            current,
            session: live,
            pinned: pin.is_some(),
            chat,
            accounts,
        });
    }

    let state = State {
        workspace: dir.to_string_lossy().to_string(),
        repo: store::repo_root(dir).to_string_lossy().to_string(),
        enabled: store::router_installed(),
        providers,
    };
    let body = serde_json::to_string(&state).map_err(|e| format!("serialising state: {e}"))?;
    println!("{body}");
    Ok(())
}

pub fn mask_one(address: &str) {
    let masked = mask::email(address);
    if !masked.is_empty() {
        println!("{masked}");
    }
}
