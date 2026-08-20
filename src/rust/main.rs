//! hats: run any number of Claude Code or Codex accounts in Conductor.
//!
//! Carries the panel inside it, so patching needs no Python, Node or brotli
//! command installed.

mod cli;
mod devapp;
mod macho;
mod manage;
mod mask;
mod patch;
mod paths;
mod profile;
mod repatch;
mod report;
mod resolve;
mod router;
mod routes;
mod settings;
mod sign;
mod store;
mod wiring;

use std::path::{Path, PathBuf};

const REAL_APP: &str = "/Applications/Conductor.app";
const DEV_APP: &str = "/Applications/Conductor Dev.app";
const DEV_ID: &str = "com.conductor.dev";

fn usage() {
    println!(
        "hats {}

  hats dev-app [--force]               build an isolated Conductor copy
  hats patch [--app PATH] [--i-know]   inject the account panel into a copy
  hats revert [--app PATH]             restore the copy's original frontend
  hats repatch [--keep-app|--no-launch] rebuild and re-inject after an update
  hats assets [--app PATH] [PATTERN]   list the frontend assets in a binary
  hats panel                           print the panel this binary carries
  hats version

Patching rewrites a signed application, so it works on a copy by default:
  {DEV_APP}
Passing --i-know allows patching {REAL_APP}, which costs it notarization and
its keychain access.",
        env!("CARGO_PKG_VERSION")
    );
}

struct Args {
    app: PathBuf,
    src: PathBuf,
    id: String,
    i_know: bool,
    force: bool,
    rebuild: bool,
    launch: bool,
    pattern: Option<String>,
}

fn parse(rest: &[String]) -> Args {
    let mut args = Args {
        app: env_path("CONDUCTOR_DEV_APP", DEV_APP),
        src: env_path("CONDUCTOR_APP", REAL_APP),
        id: std::env::var("CONDUCTOR_DEV_ID").unwrap_or_else(|_| DEV_ID.into()),
        i_know: false,
        force: false,
        rebuild: true,
        launch: true,
        pattern: None,
    };
    let mut it = rest.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--app" => {
                if let Some(v) = it.next() {
                    args.app = PathBuf::from(v);
                }
            }
            "--src" => {
                if let Some(v) = it.next() {
                    args.src = PathBuf::from(v);
                }
            }
            "--i-know" => args.i_know = true,
            "--force" => args.force = true,
            "--keep-app" => args.rebuild = false,
            "--no-launch" => args.launch = false,
            other => args.pattern = Some(other.to_string()),
        }
    }
    args
}

fn env_path(key: &str, fallback: &str) -> PathBuf {
    std::env::var_os(key).map(PathBuf::from).unwrap_or_else(|| PathBuf::from(fallback))
}

fn binary_in(app: &Path) -> PathBuf {
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
        return Err(
            "refusing to patch your real Conductor.\n\
             Build a copy first:  hats dev-app\n\
             Then:                hats patch\n\
             Override with --i-know if you really mean it."
                .into(),
        );
    }
    Ok(())
}

pub(crate) fn cmd_patch_app(app: &Path, i_know: bool) -> Result<(), String> {
    let binary = binary_in(app);
    if !binary.is_file() {
        return Err(format!("no Conductor binary at {}", binary.display()));
    }
    guard(app, i_know)?;

    let backup = patch::backup_path(app);
    if !backup.is_file() {
        if let Some(dir) = backup.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        std::fs::copy(&binary, &backup).map_err(|e| format!("taking a backup: {e}"))?;
        println!("    backup   {}", backup.display());
    }

    let report = patch::inject(&binary, &backup, patch::PANEL)?;
    println!("    target   {}", report.key);
    println!("    {} compressed -> {} bytes", report.was, report.plain);
    println!("    + {} bytes of panel -> {} compressed", patch::PANEL.len(), report.now);
    println!("    {} bytes of headroom left over", report.headroom);

    let valid = sign::resign(app)?;
    println!("    signature {}", if valid { "valid" } else { "INVALID" });
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
    let valid = sign::resign(&args.app)?;
    println!("signature {}", if valid { "valid" } else { "INVALID" });
    Ok(())
}

fn cmd_assets(args: &Args) -> Result<(), String> {
    let macho = macho::MachO::open(&binary_in(&args.app))?;
    let mut shown = 0;
    for asset in macho.assets() {
        if let Some(p) = &args.pattern {
            if !asset.key.contains(p.as_str()) {
                continue;
            }
        }
        println!("{:>10}  {}", asset.length, asset.key);
        shown += 1;
    }
    println!("{shown} assets");
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
        "patch" => cmd_patch_app(&args.app, args.i_know),
        "revert" => cmd_revert(&args),
        "repatch" => repatch::run(&repatch::Options {
            app: args.app.clone(),
            src: args.src.clone(),
            id: args.id.clone(),
            rebuild: args.rebuild,
            launch: args.launch,
        }),
        "assets" => cmd_assets(&args),
        "claude-router" => router::run("claude", rest),
        "codex-router" => router::run("codex", rest),
        other if cli::is_account_command(other) => cli::run(other, &rest),
        "panel" => {
            print!("{}", patch::PANEL);
            Ok(())
        }
        "version" | "--version" | "-V" => {
            println!("{} {}", invoked_as(), env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        "help" | "-h" | "--help" => {
            usage();
            Ok(())
        }
        other => Err(format!("unknown command '{other}' (try --help)")),
    };

    if let Err(e) = result {
        eprintln!("hats: {e}");
        std::process::exit(1);
    }
}
