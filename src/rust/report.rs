//! What a workspace resolves to: list, status, which, check, json.

use std::path::Path;

use crate::{mask, paths, profile, resolve, routes, settings, store};

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
        println!("router: off   (repository bindings still work; conductor-acct install turns it on)");
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
                let suffix = if label.is_empty() { String::new() } else { format!("  {label}") };
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

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

pub fn json(dir: &Path) -> Result<(), String> {
    store::ensure_root()?;
    let repo = store::repo_root(dir);
    print!(
        "{{\"workspace\":\"{}\",\"repo\":\"{}\",\"enabled\":{},\"providers\":[",
        escape(&dir.to_string_lossy()),
        escape(&repo.to_string_lossy()),
        store::router_installed()
    );
    for (i, agent) in ["claude", "codex"].iter().enumerate() {
        if i > 0 {
            print!(",");
        }
        let current = store::effective_dir(agent, dir)
            .as_deref()
            .and_then(store::profile_from_dir)
            .unwrap_or_default();
        print!("{{\"agent\":\"{agent}\",\"current\":\"{}\",\"accounts\":[", escape(&current));
        for (j, name) in paths::profiles(agent).iter().enumerate() {
            if j > 0 {
                print!(",");
            }
            print!(
                "{{\"name\":\"{}\",\"email\":\"{}\",\"active\":{},\"signedIn\":{}}}",
                escape(name),
                escape(&profile::label(agent, name).unwrap_or_default()),
                *name == current,
                profile::signed_in(agent, name)
            );
        }
        print!("]}}");
    }
    println!("]}}");
    Ok(())
}

pub fn mask_one(address: &str) {
    let masked = mask::email(address);
    if !masked.is_empty() {
        println!("{masked}");
    }
}
