//! Turning arguments into commands.
//!
//! The surface matches the shell CLI it replaces, because the test suite is the
//! contract and runs against either implementation.

use crate::{id, manage, paths, report, session, store, wiring};

pub fn agent_of(arg: Option<&String>) -> Result<String, String> {
    id::agent(arg.map(String::as_str).unwrap_or("claude")).map(str::to_string)
}

/// `use <profile> [agent] [path]`, and the same shape for bind.
fn profile_agent_path(rest: &[String]) -> Result<(String, String, Option<String>), String> {
    let name = rest
        .first()
        .cloned()
        .ok_or("usage: hats use <profile> [agent] [path]")?;
    let agent = agent_of(rest.get(1)).unwrap_or_else(|_| "claude".into());
    let skip = if rest
        .get(1)
        .map(|a| a == "claude" || a == "codex")
        .unwrap_or(false)
    {
        2
    } else {
        1
    };
    Ok((name, agent, rest.get(skip).cloned()))
}

pub fn is_account_command(cmd: &str) -> bool {
    matches!(
        cmd,
        "init"
            | "list"
            | "mask"
            | "status"
            | "check"
            | "json"
            | "which"
            | "use"
            | "bind"
            | "unbind"
            | "assign"
            | "unassign"
            | "add"
            | "login"
            | "logout"
            | "remove"
            | "sessions"
            | "session"
            | "pin"
            | "unpin"
            | "install"
            | "uninstall"
            | "doctor"
    )
}

pub fn run(cmd: &str, rest: &[String]) -> Result<(), String> {
    let masked = rest.iter().any(|a| a == "--mask");
    let positional: Vec<String> = rest
        .iter()
        .filter(|a| !a.starts_with("--"))
        .cloned()
        .collect();

    match cmd {
        "init" => store::ensure_root().map(|_| {
            println!("Initialised {}", paths::accounts_root().display());
            println!("Next: hats add <profile>");
        }),
        "list" => report::list(masked),
        "mask" => {
            report::mask_one(positional.first().map(String::as_str).unwrap_or(""));
            Ok(())
        }
        "status" => store::target_dir(positional.first()).and_then(|d| report::status(&d, masked)),
        "check" => store::target_dir(positional.first()).and_then(|d| report::check(&d)),
        "json" => store::target_dir(positional.first()).and_then(|d| report::json(&d)),
        "which" => {
            let dir = store::target_dir(positional.first())?;
            let agent = agent_of(positional.get(1)).unwrap_or_else(|_| "claude".into());
            report::which(&dir, &agent)
        }
        "use" => {
            let (name, agent, path) = profile_agent_path(&positional)?;
            let dir = store::target_dir(path.as_ref())?;
            manage::use_route(&name, &agent, &dir)
        }
        "bind" => {
            let (name, agent, path) = profile_agent_path(&positional)?;
            let dir = store::target_dir(path.as_ref())?;
            manage::bind(&name, &agent, &store::repo_root(&dir))
        }
        "unbind" => {
            let agent = agent_of(positional.first()).unwrap_or_else(|_| "claude".into());
            let skip = if positional
                .first()
                .map(|a| a == "claude" || a == "codex")
                .unwrap_or(false)
            {
                1
            } else {
                0
            };
            let dir = store::target_dir(positional.get(skip))?;
            manage::unbind(&agent, &store::repo_root(&dir))
        }
        "assign" => {
            let first = positional
                .first()
                .cloned()
                .ok_or("usage: hats assign <profile> [path] | assign default <profile>")?;
            if first == "default" {
                let name = positional
                    .get(1)
                    .cloned()
                    .ok_or("usage: hats assign default <profile>")?;
                manage::assign("default", &name, "claude")
            } else {
                let dir = store::target_dir(positional.get(1))?;
                manage::assign(&dir.to_string_lossy(), &first, "claude")
            }
        }
        "unassign" => {
            let first = positional.first().cloned().unwrap_or_default();
            if first == "default" {
                manage::unassign("default")
            } else {
                let dir = store::target_dir(positional.first())?;
                manage::unassign(&dir.to_string_lossy())
            }
        }
        "add" => manage::add(
            positional
                .first()
                .ok_or("usage: hats add <profile> [agent]")?,
            &agent_of(positional.get(1))?,
        ),
        "login" => manage::login(
            positional
                .first()
                .ok_or("usage: hats login <profile> [agent]")?,
            &agent_of(positional.get(1))?,
        ),
        "logout" => manage::logout(
            positional
                .first()
                .ok_or("usage: hats logout <profile> [agent]")?,
            &agent_of(positional.get(1))?,
        ),
        "remove" => manage::remove(
            positional
                .first()
                .ok_or("usage: hats remove <profile> [agent] [--force]")?,
            &agent_of(positional.get(1))?,
            rest.iter().any(|a| a == "--force"),
        ),
        "sessions" => manage::sessions(positional.first().map(String::as_str) == Some("clear")),
        "session" => {
            let dir = store::target_dir(positional.first())?;
            let agent = agent_of(positional.get(1)).unwrap_or_else(|_| "claude".into());
            match session::current(&agent, &dir) {
                session::Current::Chat(s) => println!("{s}"),
                session::Current::Idle => println!("(no chat active here recently)"),
                session::Current::Ambiguous(n) => {
                    println!("(ambiguous: {n} chats written at once)")
                }
            }
            Ok(())
        }
        "pin" => {
            let name = positional
                .first()
                .cloned()
                .ok_or("usage: hats pin <profile> [agent] [session]")?;
            let agent = agent_of(positional.get(1)).unwrap_or_else(|_| "claude".into());
            let skip = if positional
                .get(1)
                .map(|a| a == "claude" || a == "codex")
                .unwrap_or(false)
            {
                2
            } else {
                1
            };
            let dir = paths::workspace_dir();
            let target = session::target(&agent, &dir, positional.get(skip))?;
            session::pin(&name, &agent, &target)
        }
        "unpin" => {
            let agent = agent_of(positional.first()).unwrap_or_else(|_| "claude".into());
            let skip = if positional
                .first()
                .map(|a| a == "claude" || a == "codex")
                .unwrap_or(false)
            {
                1
            } else {
                0
            };
            let dir = paths::workspace_dir();
            let target = session::target(&agent, &dir, positional.get(skip))?;
            session::unpin(&agent, &target)
        }
        "install" => wiring::install(),
        "uninstall" => wiring::uninstall(),
        "doctor" => store::target_dir(positional.first()).and_then(|d| wiring::doctor(&d)),
        other => Err(format!("unknown command '{other}'")),
    }
}
