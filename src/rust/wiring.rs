//! Turning routing on and off, and checking it end to end.

use std::path::Path;

use crate::{manage, paths, profile, resolve, settings, store};

const CLAUDE_KEY: &str = "claude_code_executable_path";
const CODEX_KEY: &str = "codex_executable_path";

/// The routers are this binary under another name, so install writes symlinks
/// rather than copying anything: one artifact, two entry points.
pub fn install() -> Result<(), String> {
    store::ensure_root()?;
    let bin = manage::install_bin();
    if bin.is_symlink() {
        let _ = std::fs::remove_file(&bin);
    }
    std::fs::create_dir_all(&bin).map_err(|e| format!("{}: {e}", bin.display()))?;

    let me = std::env::current_exe().map_err(|e| format!("locating this binary: {e}"))?;
    let deployed = bin.join("hats");
    if me != deployed {
        std::fs::copy(&me, &deployed).map_err(|e| format!("{}: {e}", deployed.display()))?;
    }
    for name in ["claude-router", "codex-router"] {
        let link = bin.join(name);
        let _ = std::fs::remove_file(&link);
        std::os::unix::fs::symlink(&deployed, &link)
            .map_err(|e| format!("{}: {e}", link.display()))?;
    }

    let settings_file = settings::conductor_settings();
    settings::set_key(
        &settings_file,
        CLAUDE_KEY,
        &bin.join("claude-router").to_string_lossy(),
    )?;
    settings::set_key(
        &settings_file,
        CODEX_KEY,
        &bin.join("codex-router").to_string_lossy(),
    )?;

    println!("Installed to {}", bin.display());
    println!("Wrote to {}:", settings_file.display());
    println!(
        "  {CLAUDE_KEY} = \"{}\"",
        bin.join("claude-router").display()
    );
    println!(
        "  {CODEX_KEY}       = \"{}\"",
        bin.join("codex-router").display()
    );

    let command = settings::commands_dir().join("account.md");
    if let Some(dir) = command.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Some(source) = command_source() {
        let _ = std::fs::remove_file(&command);
        if std::fs::copy(&source, &command).is_ok() {
            println!("  {}  (adds /account inside Conductor)", command.display());
        }
    }
    println!();
    println!("Re-run install after pulling changes; doctor warns when the copy is stale.");
    Ok(())
}

/// Shipped beside the binary in a release, or in the checkout during development.
/// Where `commands/account.md` sits relative to the binary.
///
/// Beside it in a release tarball, one level up in an install. From a cargo
/// build it is two levels up, past target/release, and missing that case meant
/// `install` from a checkout quietly deployed no slash command at all: the panel
/// worked and `/account` was simply absent, with nothing said.
fn command_source() -> Option<std::path::PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let base = exe.parent()?;
    [
        base.join("commands/account.md"),
        base.join("../commands/account.md"),
        base.join("../../commands/account.md"),
    ]
    .into_iter()
    .find(|candidate| candidate.is_file())
}

pub fn uninstall() -> Result<(), String> {
    let settings_file = settings::conductor_settings();
    settings::unset_key(&settings_file, CLAUDE_KEY)?;
    settings::unset_key(&settings_file, CODEX_KEY)?;
    let _ = std::fs::remove_file(settings::commands_dir().join("account.md"));
    println!("Router off. Restart Conductor for it to take effect.");
    println!("Your profiles and routes are untouched; hats install turns it back on.");
    Ok(())
}

pub fn doctor(dir: &Path) -> Result<(), String> {
    store::ensure_root()?;
    let mut ok = true;

    println!("version:  {}", env!("CARGO_PKG_VERSION"));
    println!("root:     {}", paths::accounts_root().display());

    match resolve::agent_binary("claude") {
        Some(b) => println!("claude:   {}", b.display()),
        None => {
            println!("claude:   NOT FOUND");
            ok = false;
        }
    }

    if store::router_installed() {
        println!("router:   on, via {CLAUDE_KEY}");
        let wired =
            settings::get_key(&settings::conductor_settings(), CLAUDE_KEY).unwrap_or_default();
        let bin = manage::install_bin();
        if !wired.starts_with(&bin.to_string_lossy().to_string()) {
            println!(
                "warn:     Conductor points at {wired}, outside {}",
                paths::accounts_root().display()
            );
            println!("          re-run 'hats install' so it cannot be deleted");
            ok = false;
        }
    } else {
        println!("router:   off (repository bindings still work; hats install turns it on)");
    }

    for name in paths::profiles("claude") {
        if !profile::signed_in("claude", &name) {
            println!("warn:     profile '{name}' is not signed in");
            ok = false;
        }
    }

    let mut seen: Vec<(String, String)> = Vec::new();
    for agent in ["claude", "codex"] {
        for name in paths::profiles(agent) {
            let Some(email) = profile::label(agent, &name) else {
                continue;
            };
            if seen.iter().any(|(a, e)| a == agent && e == &email) {
                println!("warn:     {agent} profiles share the address {email}");
                println!("          one live token per account, so they will sign each other out");
                ok = false;
            } else {
                seen.push((agent.to_string(), email));
            }
        }
    }

    let routes = std::fs::read_to_string(paths::routes_file()).unwrap_or_default();
    for line in routes.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        let Some((key, rest)) = t.split_once(['\t', ' ']) else {
            continue;
        };
        let routed = rest.trim();
        if routed.is_empty() {
            continue;
        }
        if !paths::profile_dir("claude", routed).is_dir()
            && !paths::profile_dir("codex", routed).is_dir()
        {
            println!("warn:     route {key} points at missing profile '{routed}'");
            ok = false;
        }
    }

    if store::router_installed() {
        match resolve::decide("claude", dir, None, false) {
            Some(name) => println!("dry run:  {} -> {name}", dir.display()),
            None => println!(
                "dry run:  {} -> default account (no route, no binding)",
                dir.display()
            ),
        }
    }

    if ok {
        println!("OK");
    }
    Ok(())
}
