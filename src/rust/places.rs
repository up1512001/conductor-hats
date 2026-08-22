//! Where Conductor thinks its workspaces and repositories are.
//!
//! Conductor's webview routes in memory, so `location` carries no workspace id
//! and the panel cannot ask which workspace is on screen. It matches the visible
//! chrome against this list instead, which is why the list has to exist: without
//! it nothing matches, the panel resolves to no target, and every account row is
//! disabled with "Open a workspace to choose its account" under it. That is what
//! losing these two commands in the port to Rust did.
//!
//! Read-only, through the stock `sqlite3`, across every Conductor copy: a
//! patched copy keeps its own database, and the panel runs in both.

use std::path::{Path, PathBuf};
use std::process::Command;

const WORKSPACES: &str = "select workspace_path from workspaces \
     where workspace_path is not null and state != 'archived'";

const REPOS: &str = "select root_path from repos where root_path is not null";

fn databases() -> Vec<PathBuf> {
    if let Some(one) = std::env::var_os("CONDUCTOR_DB") {
        let path = PathBuf::from(one);
        return if path.is_file() {
            vec![path]
        } else {
            Vec::new()
        };
    }
    let home = std::env::var_os("HOME").unwrap_or_default();
    let support = PathBuf::from(home).join("Library/Application Support");
    let Ok(entries) = std::fs::read_dir(&support) else {
        return Vec::new();
    };
    let mut found: Vec<PathBuf> = entries
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with("com.conductor"))
        .map(|e| e.path().join("conductor.db"))
        .filter(|p| p.is_file())
        .collect();
    found.sort();
    found
}

/// `mode=ro` rather than a copy: the database is usually open in a running
/// Conductor, and read-only is the whole of what is needed here.
fn query(db: &Path, sql: &str) -> Vec<String> {
    let uri = format!("file:{}?mode=ro", db.display());
    let Ok(out) = Command::new("sqlite3").arg(uri).arg(sql).output() else {
        return Vec::new();
    };
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect()
}

fn basename(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// One `name<TAB>path` per line, which is what the panel parses.
pub fn run(kind: &str) -> Result<(), String> {
    let sql = match kind {
        "workspaces" => WORKSPACES,
        "repos" => REPOS,
        other => return Err(format!("unknown place '{other}'")),
    };
    let dbs = databases();
    if dbs.is_empty() {
        return Ok(());
    }
    if Command::new("sqlite3").arg("-version").output().is_err() {
        return Err("sqlite3 is not on PATH, so the panel cannot tell which \
                    workspace is on screen"
            .into());
    }

    let mut lines: Vec<String> = dbs
        .iter()
        .flat_map(|db| query(db, sql))
        .filter(|path| !basename(path).is_empty())
        .map(|path| format!("{}\t{}", basename(&path), path))
        .collect();
    lines.sort();
    lines.dedup();
    for line in lines {
        println!("{line}");
    }
    Ok(())
}
