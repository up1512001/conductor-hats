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
     where w.state != 'archived' and s.agent_type = ";

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

fn ask(db: &Path, sql: &str, mode: &str) -> Option<Vec<String>> {
    let uri = format!("file:{}?{mode}", db.display());
    let out = Command::new("sqlite3").arg(uri).arg(sql).output().ok()?;
    if !out.status.success() {
        return None;
    }
    Some(
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(str::to_string)
            .collect(),
    )
}

/// Read-only rather than a copy: the database is usually open in a running
/// Conductor, and reading is the whole of what is needed here. The database is
/// also 1.6 GB, so copying it to read two columns is not an option.
///
/// `mode=ro` alone is not enough. Conductor's database is in WAL mode, and
/// opening one of those read-only fails outright unless its `-shm` file already
/// exists, which it does not while Conductor is closed or between a quit and the
/// next launch. The failure is silent in the worst way: sqlite3 exits non-zero
/// with an empty result, so every caller reads it as "Conductor knows of no
/// workspaces" and the panel quietly falls back to guessing from the screen.
///
/// So a refusal is retried as `immutable=1`, which reads the file without the
/// shared-memory index. It is only reached when there is no live index to share,
/// and a possibly stale answer beats an empty one that reads as fact.
fn query(db: &Path, sql: &str) -> Vec<String> {
    if let Some(rows) = ask(db, sql, "mode=ro") {
        return rows;
    }
    ask(db, sql, "immutable=1").unwrap_or_default()
}

/// Single quotes doubled, which is all SQL string quoting is. Paths arrive from
/// the caller rather than from Conductor, so they are not ids and cannot be
/// checked like one.
fn quoted(text: &str) -> String {
    format!("'{}'", text.replace('\'', "''"))
}

/// Whether Conductor calls this directory a workspace.
///
/// The account chosen while creating one must not be spent by anything else, and
/// plenty else starts an agent: Conductor runs one with the working directory set
/// to `/` before the workspace's own, and others at the repository root. Both
/// would swallow the choice and leave the new workspace with nothing.
pub fn is_workspace(dir: &Path) -> bool {
    let dbs = databases();
    if dbs.is_empty() {
        /* No Conductor to ask, so nothing else is starting agents either and
         * there is nothing to protect the choice from. */
        return true;
    }
    /* Compared after resolving both sides rather than as strings. On macOS the
     * temporary directory is `/var/...`, a symlink to `/private/var/...`, and
     * which spelling reaches here depends on who resolved it last. */
    let want = real(dir);
    dbs.iter()
        .flat_map(|db| query(db, WORKSPACES))
        .any(|path| real(Path::new(&path)) == want)
}

fn real(dir: &Path) -> PathBuf {
    std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf())
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
