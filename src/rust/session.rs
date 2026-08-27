//! Which chat is live in a workspace, and pinning one to an account.
//!
//! A workspace holds many chats and each runs its own long lived agent process,
//! so "the account this workspace uses" is not the same question as "the account
//! this chat is on". The router has always honoured a per session pin; nothing
//! could set one deliberately, and the panel could not see one.
//!
//! The agent writes a transcript per chat under `projects/<encoded workspace>`,
//! and every profile shares that directory through a symlink, so the newest
//! transcript names the chat being typed in whichever account is answering.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::{id, paths, profile};

/// A chat is only "current" if it has been written to recently. Beyond this the
/// workspace is idle and there is nothing to pin.
const FRESH: Duration = Duration::from_secs(300);

/// Two chats written within this of each other are both plausibly in view, and
/// picking the newer would be a guess.
const AMBIGUOUS_WITHIN: Duration = Duration::from_secs(2);

pub fn pin_path(agent: &str, session: &str) -> PathBuf {
    paths::session_dir().join(agent).join(session)
}

/// Claude Code's own encoding: the absolute path with every separator replaced
/// by a dash.
fn encode(dir: &Path) -> String {
    dir.to_string_lossy().replace('/', "-")
}

/// Every directory a transcript for this workspace could be in. Profiles share
/// one `projects` through a symlink, but a profile added before that was true
/// may still have its own.
fn transcript_dirs(agent: &str, dir: &Path) -> Vec<PathBuf> {
    let encoded = encode(dir);
    let mut out = vec![paths::home().join(".claude/projects").join(&encoded)];
    for name in paths::profiles(agent) {
        out.push(
            paths::profile_dir(agent, &name)
                .join("projects")
                .join(&encoded),
        );
    }
    out
}

struct Transcript {
    session: String,
    at: SystemTime,
}

fn transcripts(agent: &str, dir: &Path) -> Vec<Transcript> {
    let mut out: Vec<Transcript> = Vec::new();
    for base in transcript_dirs(agent, dir) {
        let Ok(entries) = std::fs::read_dir(&base) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e != "jsonl").unwrap_or(true) {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let Some(session) = id::session(stem) else {
                continue;
            };
            let Ok(at) = entry.metadata().and_then(|m| m.modified()) else {
                continue;
            };
            if out.iter().any(|t| t.session == session) {
                continue;
            }
            out.push(Transcript {
                session: session.to_string(),
                at,
            });
        }
    }
    out.sort_by_key(|t| std::cmp::Reverse(t.at));
    out
}

/// Why there is no current chat, so the caller can say which rather than going
/// quiet.
pub enum Current {
    Chat(String),
    Idle,
    Ambiguous(usize),
}

/// The chat on screen in this workspace.
///
/// Conductor's own record comes first: it stores the selected chat against the
/// workspace, so there is nothing to infer. It is right while the workspace is
/// idle, right when two chats answer at once, and right for a conversation whose
/// transcript is named something else after a compaction.
///
/// The scan below stays as the fallback, for a Conductor with no database to
/// read, no `sessions` table, or no `sqlite3` to read it with. It infers the chat
/// from transcript timestamps, and refuses to choose when two were written within
/// a couple of seconds of each other: both are plausibly on screen, and picking
/// one would silently pin the wrong conversation.
pub fn current(agent: &str, dir: &Path) -> Current {
    if let Some(open) = crate::places::active_session(agent, dir) {
        return Current::Chat(open);
    }
    let found = transcripts(agent, dir);
    let Some(newest) = found.first() else {
        return Current::Idle;
    };
    let age = SystemTime::now()
        .duration_since(newest.at)
        .unwrap_or(Duration::ZERO);
    if age > FRESH {
        return Current::Idle;
    }
    let rivals = found
        .iter()
        .filter(|t| newest.at.duration_since(t.at).unwrap_or(Duration::ZERO) <= AMBIGUOUS_WITHIN)
        .count();
    if rivals > 1 {
        return Current::Ambiguous(rivals);
    }
    Current::Chat(newest.session.clone())
}

/// Where the account a chat actually started on is recorded.
///
/// Separate from the pin, and it has to be. The pin says what the next process
/// will use; a running conversation took its account when it spawned and cannot
/// be moved. With one file for both, choosing an account made the panel report
/// the new one immediately while the conversation on screen carried on under the
/// old, which is the misreporting this whole feature exists to end.
fn started_path(agent: &str, session: &str) -> PathBuf {
    paths::session_dir()
        .join(agent)
        .join("started")
        .join(session)
}

/// The account the agent for this chat took when it spawned.
pub fn started(agent: &str, session: &str) -> Option<String> {
    let found = paths::first_line(&started_path(agent, session))?;
    id::profile_or_none(&found).map(str::to_string)
}

/// Written by the router as it hands an agent its account, and by nothing else.
pub fn record_started(agent: &str, session: &str, name: &str) {
    let Some(session) = id::session(session) else {
        return;
    };
    let Some(name) = id::profile_or_none(name) else {
        return;
    };
    let path = started_path(agent, session);
    if let Some(parent) = path.parent() {
        if std::fs::create_dir_all(parent).is_err() {
            return;
        }
    }
    let _ = crate::lock::write_atomic(&path, &format!("{name}\n"));
}

pub fn pinned(agent: &str, session: &str) -> Option<String> {
    let found = paths::first_line(&pin_path(agent, session))?;
    id::profile_or_none(&found).map(str::to_string)
}

/// Pins a chat to a profile.
///
/// The agent process for a running chat took its config directory at spawn and
/// no longer reads anything, so this cannot move the conversation that is on
/// screen. It decides the next process Conductor starts for that chat.
pub fn pin(name: &str, agent: &str, session: &str) -> Result<(), String> {
    profile::require(agent, name)?;
    let session = id::session(session).ok_or("that is not a session id")?;
    let path = pin_path(agent, session);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    crate::lock::write_atomic(&path, &format!("{name}\n"))?;
    println!("This chat now uses '{name}'.");
    println!();
    println!("The agent already running took its account when it started, so the");
    println!("conversation on screen stays where it is. Reopen the chat, or start a new");
    println!("one, and it comes up on '{name}'.");
    Ok(())
}

pub fn unpin(agent: &str, session: &str) -> Result<(), String> {
    let session = id::session(session).ok_or("that is not a session id")?;
    let path = pin_path(agent, session);
    if !path.is_file() {
        println!("That chat was not pinned; it follows the workspace.");
        return Ok(());
    }
    std::fs::remove_file(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    println!("Unpinned. That chat follows the workspace again from its next start.");
    Ok(())
}

/// Resolves the session a command should act on: the argument if given, else the
/// chat currently being written to.
pub fn target(agent: &str, dir: &Path, arg: Option<&String>) -> Result<String, String> {
    if let Some(given) = arg {
        return id::session(given)
            .map(str::to_string)
            .ok_or_else(|| format!("'{given}' is not a session id"));
    }
    match current(agent, dir) {
        Current::Chat(s) => Ok(s),
        Current::Idle => Err(
            "no chat has been active in this workspace recently, so there is none to pin.\n\
             Open the chat you mean, send a message, and try again."
                .into(),
        ),
        Current::Ambiguous(n) => Err(format!(
            "{n} chats in this workspace were written to at the same moment, so which one \
             you mean is a guess.\nPass the session id explicitly."
        )),
    }
}
