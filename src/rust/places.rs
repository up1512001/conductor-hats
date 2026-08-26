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

/// The chat Conductor has open in a workspace, named the way the router will see
/// it.
///
/// Conductor records the selected chat itself, in `workspaces.active_session_id`,
/// which is exact: it changes the moment another chat is clicked, and it holds
/// while the workspace sits idle. Guessing from transcript timestamps can do
/// neither.
///
/// The two ids are not the same namespace. Conductor's `sessions.id` is what it
/// passes as `--session-id` when it starts a chat, so the two usually agree, but
/// a conversation resumed after a compaction carries a `claude_session_id` of
/// its own and that is the one on the command line the router reads. Pinning the
/// other would write a file nothing ever looks up.
const ACTIVE: &str = "select coalesce(nullif(s.claude_session_id, ''), s.id) \
     from workspaces w join sessions s on s.id = w.active_session_id \
     where s.agent_type = ";

/// Ids come from the frontend, so they are checked before being put in a query
/// rather than trusted. Conductor's are UUIDs.
fn is_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 36 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

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

/// Single quotes doubled, which is all SQL string quoting is. Paths arrive from
/// the caller rather than from Conductor, so they are not ids and cannot be
/// checked like one.
fn quoted(text: &str) -> String {
    format!("'{}'", text.replace('\'', "''"))
}

/// The open chat in this workspace for one agent, or None when Conductor knows
/// of none, the database is unreadable, or this build has no `sessions` table.
///
/// Filtered by agent: a workspace showing a Codex chat has no Claude chat open,
/// and answering with the Codex one would pin the wrong agent's conversation.
pub fn active_session(agent: &str, dir: &Path) -> Option<String> {
    let sql = format!(
        "{ACTIVE}{} and w.workspace_path = {}",
        quoted(agent),
        quoted(&dir.to_string_lossy())
    );
    databases()
        .iter()
        .flat_map(|db| query(db, &sql))
        .find(|id| is_id(id))
}

fn basename(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// The path of one workspace or repository, by id.
///
/// The panel reads the id out of the frontend, which is exact, where matching a
/// name against what is on screen is a guess that a repository can win.
pub fn resolve(kind: &str, id: &str) -> Result<(), String> {
    if !is_id(id) {
        return Err(format!("not an id: {id}"));
    }
    let sql = match kind {
        "resolve" => format!("select workspace_path from workspaces where id = '{id}'"),
        "resolve-repo" => format!("select root_path from repos where id = '{id}'"),
        other => return Err(format!("unknown lookup '{other}'")),
    };
    for db in databases() {
        if let Some(path) = query(&db, &sql).into_iter().find(|p| !p.is_empty()) {
            println!("{path}");
            return Ok(());
        }
    }
    Ok(())
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
