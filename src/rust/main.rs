//! hats: run any number of Claude Code or Codex accounts in Conductor.
//!
//! Carries the panel inside it, so patching needs no Python, Node or brotli
//! command installed.

mod args;
mod auth;
mod chats;
mod chats_table;
mod cli;
mod conductor_session;
mod debug;
mod devapp;
mod edit;
mod help;
mod http;
mod id;
mod lock;
mod macho;
mod manage;
mod mask;
mod mobile_catalog;
mod mobile_page;
mod mobile_scope;
mod mobile_service;
mod mobile_socket;
mod mobile_stamp;
mod mobile_state;
mod origin;
mod panel_login;
mod patch;
mod paths;
mod pending;
mod places;
mod profile;
mod remote;
mod remote_cli;
mod remote_control;
mod remote_control_result;
mod remote_create;
mod remote_scan;
mod repatch;
mod report;
mod resolve;
mod router;
mod routes;
mod serve;
mod session;
mod settings;
mod sign;
mod source;
mod store;
mod transcript;
mod verify;
mod wiring;

use std::path::{Path, PathBuf};

use args::{parse, Args};

const REAL_APP: &str = "/Applications/Conductor.app";
pub const DEV_APP: &str = "/Applications/Conductor Dev.app";
const DEV_ID: &str = "com.conductor.dev";

pub fn binary_in(app: &Path) -> PathBuf {
    app.join("Contents/MacOS/conductor")
}

/// Patching the real Conductor costs it notarization and its keychain access, so
/// it takes an explicit flag.
fn guard(app: &Path, i_know: bool) -> Result<(), String> {
    let same = app
        .canonicalize()
        .ok()
        .zip(Path::new(REAL_APP).canonicalize().ok())
        .map(|(a, b)| a == b)
        .unwrap_or(false);
    if same && !i_know {
        return Err("refusing to patch your real Conductor.\n\
             Build a copy first:  hats dev-app\n\
             Then:                hats patch\n\
             Override with --i-know if you really mean it."
            .into());
    }
    Ok(())
}

/// The pristine copy of the frontend, taken before anything is written to it.
fn ensure_backup(binary: &Path, app: &Path) -> Result<PathBuf, String> {
    let backup = patch::backup_path(app);
    if !backup.is_file() {
        if let Some(dir) = backup.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        std::fs::copy(binary, &backup).map_err(|e| format!("taking a backup: {e}"))?;
        println!("    backup   {}", backup.display());
    }
    Ok(backup)
}

pub(crate) fn cmd_patch_app(app: &Path, i_know: bool) -> Result<(), String> {
    let binary = binary_in(app);
    if !binary.is_file() {
        return Err(format!("no Conductor binary at {}", binary.display()));
    }
    guard(app, i_know)?;

    let backup = ensure_backup(&binary, app)?;
    for report in patch::inject(&binary, &backup)? {
        println!("    target   {}", report.key);
        println!(
            "    {} bytes plain, {} of {} used, {} bytes of headroom left over",
            report.plain, report.now, report.was, report.headroom
        );
    }

    sign::resign(app)?;
    println!("    signature valid");
    Ok(())
}

/// Inject something other than the panel, for working out why a patched copy
/// paints nothing: a no-op script says whether the injection itself is at fault,
/// and a reporter injected into the entry module says what the frontend threw.
fn cmd_patch(args: &Args) -> Result<(), String> {
    if args.scripts.is_empty() {
        return cmd_patch_app(&args.app, args.i_know);
    }
    let binary = binary_in(&args.app);
    if !binary.is_file() {
        return Err(format!("no Conductor binary at {}", binary.display()));
    }
    guard(&args.app, args.i_know)?;

    let mut edits = Vec::new();
    for (key, prepend, path) in &args.scripts {
        let script =
            std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        edits.push(edit::Edit {
            key: key.clone(),
            prepend: *prepend,
            checked: false,
            script,
        });
    }
    let backup = ensure_backup(&binary, &args.app)?;
    let reports = edit::apply(&binary, &backup, &edits)?;
    for (report, edit) in reports.iter().zip(&edits) {
        println!(
            "    {} {} -> {} bytes, {} headroom",
            if edit.prepend {
                "prepended to"
            } else {
                "appended to"
            },
            report.key,
            report.now,
            report.headroom
        );
    }

    sign::resign(&args.app)?;
    println!("    signature valid");
    Ok(())
}

fn cmd_revert(args: &Args) -> Result<(), String> {
    let binary = binary_in(&args.app);
    let backup = patch::backup_path(&args.app);
    if !backup.is_file() {
        return Err(format!("no backup at {}", backup.display()));
    }
    std::fs::copy(&backup, &binary).map_err(|e| format!("restoring: {e}"))?;
    println!("restored {}", binary.display());
    sign::resign(&args.app)?;
    println!("signature valid");
    Ok(())
}

/// The name this binary was invoked by. It answers to several through symlinks,
/// and reports itself as whichever was used.
fn invoked_as() -> String {
    std::env::args()
        .next()
        .and_then(|a| a.rsplit('/').next().map(str::to_string))
        .unwrap_or_else(|| "hats".into())
}

/// Conductor spawns the router by path, so the same binary answers to
/// `claude-router` and `codex-router` through symlinks install.sh creates.
fn router_agent() -> Option<&'static str> {
    let arg0 = std::env::args().next()?;
    let name = arg0.rsplit('/').next()?.to_string();
    match name.as_str() {
        "claude-router" => Some("claude"),
        "codex-router" => Some("codex"),
        _ => None,
    }
}

fn main() {
    if let Some(agent) = router_agent() {
        router::run(agent, std::env::args().skip(1).collect());
    }

    let argv: Vec<String> = std::env::args().skip(1).collect();
    let (cmd, rest) = argv
        .split_first()
        .map(|(c, r)| (c.as_str(), r.to_vec()))
        .unwrap_or(("help", Vec::new()));
    let args = parse(&rest);

    let result = match cmd {
        "dev-app" => devapp::build(&devapp::Options {
            src: args.src.clone(),
            dst: args.app.clone(),
            id: args.id.clone(),
            force: args.force,
        }),
        "patch" => cmd_patch(&args),
        "revert" => cmd_revert(&args),
        "repatch" => repatch::run(&repatch::Options {
            app: args.app.clone(),
            src: args.src.clone(),
            id: args.id.clone(),
            rebuild: args.rebuild,
            launch: args.launch,
        }),
        "workspaces" | "repos" => places::run(cmd),
        "resolve" | "resolve-repo" => {
            places::resolve(cmd, rest.first().map(String::as_str).unwrap_or(""))
        }
        "assets" => patch::list(&args.app, args.pattern.as_deref(), args.dump),
        "verify" => verify::run(&args.app),
        "debug" => debug::run(&rest),
        "log" => debug::write(&rest),
        "reset-keychain" => {
            sign::drop_keychain_items(&args.app);
            Ok(())
        }
        "claude-router" => router::run("claude", rest),
        "codex-router" => router::run("codex", rest),
        other if cli::is_account_command(other) => cli::run(other, &rest),
        "panel" => {
            print!("{}", patch::PANEL);
            Ok(())
        }
        "guard" => {
            print!("{}", patch::GUARD);
            Ok(())
        }
        "version" | "--version" | "-V" => {
            println!("{} {}", invoked_as(), env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        "help" | "-h" | "--help" => {
            help::usage();
            Ok(())
        }
        other => Err(format!("unknown command '{other}' (try --help)")),
    };

    if let Err(e) = result {
        eprintln!("hats: {e}");
        std::process::exit(1);
    }
}
