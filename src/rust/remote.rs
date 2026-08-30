//! Durable messages travelling from the phone into Conductor's real composer.
//!
//! The server writes only under hats' own root. The injected panel claims the
//! oldest item for the chat it is visibly attached to, types it into Conductor's
//! composer and presses Conductor's send button. Delivery is acknowledged only
//! after the same user message appears in Conductor's read-only database view.
//! That makes a panel reload between the click and acknowledgement recoverable.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::{auth, conductor_session, id, lock, paths, places};

pub use crate::remote_scan::{counts, purge, stamp};

const MAX_MESSAGE: usize = 64 * 1024;
const MAX_PENDING: usize = 50;
const LEASE: Duration = Duration::from_secs(90);

#[derive(Clone, serde::Deserialize, serde::Serialize)]
pub struct Item {
    pub id: String,
    pub session: String,
    pub message: String,
    pub created_at: u64,
    pub seen_before: usize,
    pub lease: String,
    pub lease_until: u64,
}

#[derive(serde::Serialize)]
struct Public<'a> {
    id: &'a str,
    session: &'a str,
    message: &'a str,
    created_at: u64,
    state: &'a str,
}

fn root() -> PathBuf {
    paths::accounts_root().join("remote")
}

fn session_dir(session: &str) -> Result<PathBuf, String> {
    let session = id::session(session).ok_or("invalid chat id")?;
    let dir = root().join(session);
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    set_directory_mode(&root())?;
    set_directory_mode(&dir)?;
    Ok(dir)
}

fn set_directory_mode(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
            .map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(())
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn files(dir: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|v| v.to_str()) == Some("json"))
        .collect();
    found.sort();
    found
}

pub(crate) fn read(path: &Path) -> Option<Item> {
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

fn item_path(dir: &Path, item: &str) -> Result<PathBuf, String> {
    if item.len() != 32 || !item.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("invalid queue item id".into());
    }
    Ok(dir.join(format!("{item}.json")))
}

/// Replaces one private queue file atomically.
fn write(path: &Path, item: &Item) -> Result<(), String> {
    let body = serde_json::to_vec(item).map_err(|e| format!("queue item: {e}"))?;
    let tmp = path.with_file_name(format!(
        ".{}.{}.tmp",
        path.file_name().unwrap_or_default().to_string_lossy(),
        std::process::id()
    ));
    let _ = std::fs::remove_file(&tmp);
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let result = (|| -> std::io::Result<()> {
        let mut file = options.open(&tmp)?;
        file.write_all(&body)?;
        file.sync_all()?;
        std::fs::rename(&tmp, path)
    })();
    if let Err(e) = result {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("{}: {e}", path.display()));
    }
    Ok(())
}

pub(crate) fn delivered(item: &Item) -> bool {
    places::user_message_count(&item.session, &item.message) > item.seen_before
}

/// Whether the chat this item is for exists in the Conductor copy being served.
///
/// A queue is shared between every Conductor copy on the machine, while only the
/// panel injected into one of them drains it. An item for a chat in another copy
/// is therefore claimed by nobody: it sat at "Delivering" for as long as the
/// queue lived, indistinguishable from one merely waiting its turn. Items like
/// this exist because `hats serve` used to read every copy at once, so a phone
/// could queue against a chat its own panel could never open.
pub(crate) fn reachable(item: &Item) -> bool {
    conductor_session::route(&item.session).is_some()
}

/// Adds one mobile message without touching Conductor's database.
pub fn enqueue(session: &str, message: &str) -> Result<Item, String> {
    let session = id::session(session).ok_or("invalid chat id")?;
    if conductor_session::route(session).is_none() {
        return Err("that chat is not open in this Conductor app".into());
    }
    let message = message.trim();
    if message.is_empty() {
        return Err("the message is empty".into());
    }
    if message.len() > MAX_MESSAGE || message.contains('\0') {
        return Err("the message is larger than 64 KiB or contains a null byte".into());
    }
    let dir = session_dir(session)?;
    let _guard = lock::Lock::acquire(&dir.join("queue"))?;
    let queued: Vec<Item> = files(&dir).iter().filter_map(|path| read(path)).collect();
    if queued.len() >= MAX_PENDING {
        return Err("this chat already has 50 messages waiting".into());
    }
    let created_at = queued
        .iter()
        .map(|item| item.created_at)
        .max()
        .map(|latest| now().max(latest + 1))
        .unwrap_or_else(now);
    let present = places::user_message_count(session, message);
    let ahead = queued
        .iter()
        .filter(|item| item.message == message && present <= item.seen_before)
        .count();
    let item = Item {
        id: auth::random_hex(16)?,
        session: session.to_string(),
        message: message.to_string(),
        created_at,
        seen_before: present + ahead,
        lease: String::new(),
        lease_until: 0,
    };
    write(&item_path(&dir, &item.id)?, &item)?;
    Ok(item)
}

/// Claims the oldest undelivered message for one visible chat.
pub fn claim(session: &str) -> Result<Option<Item>, String> {
    let dir = session_dir(session)?;
    let _guard = lock::Lock::acquire(&dir.join("queue"))?;
    let mut queued: Vec<(PathBuf, Item)> = files(&dir)
        .into_iter()
        .filter_map(|path| read(&path).map(|item| (path, item)))
        .collect();
    queued.sort_by_key(|(_, item)| (item.created_at, item.id.clone()));
    for (path, mut item) in queued {
        if delivered(&item) {
            let _ = std::fs::remove_file(&path);
            continue;
        }
        if !reachable(&item) {
            continue;
        }
        if item.lease_until > now() {
            return Ok(None);
        }
        item.lease = auth::random_hex(16)?;
        item.lease_until = now() + LEASE.as_millis() as u64;
        write(&path, &item)?;
        return Ok(Some(item));
    }
    Ok(None)
}

/// Removes a claimed item only when Conductor has actually recorded it.
pub fn confirm(session: &str, item: &str, lease: &str) -> Result<bool, String> {
    let dir = session_dir(session)?;
    let _guard = lock::Lock::acquire(&dir.join("queue"))?;
    let path = item_path(&dir, item)?;
    let Some(found) = read(&path) else {
        return Ok(true);
    };
    if !auth::same(&found.lease, lease) {
        return Err("that queue lease is no longer active".into());
    }
    if !delivered(&found) {
        return Ok(false);
    }
    std::fs::remove_file(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(true)
}

/// Gives a failed claim back immediately rather than waiting for its lease.
pub fn release(session: &str, item: &str, lease: &str) -> Result<(), String> {
    let dir = session_dir(session)?;
    let _guard = lock::Lock::acquire(&dir.join("queue"))?;
    let path = item_path(&dir, item)?;
    let Some(mut found) = read(&path) else {
        return Ok(());
    };
    if !auth::same(&found.lease, lease) {
        return Err("that queue lease is no longer active".into());
    }
    found.lease.clear();
    found.lease_until = 0;
    write(&path, &found)
}

pub fn pending(session: &str) -> Result<Vec<Item>, String> {
    let dir = session_dir(session)?;
    let _guard = lock::Lock::acquire(&dir.join("queue"))?;
    let mut out = Vec::new();
    for path in files(&dir) {
        if let Some(item) = read(&path) {
            if delivered(&item) {
                let _ = std::fs::remove_file(path);
            } else {
                out.push(item);
            }
        }
    }
    out.sort_by_key(|item| (item.created_at, item.id.clone()));
    Ok(out)
}

pub fn pending_json(session: &str) -> Result<String, String> {
    let items = pending(session)?;
    let wire: Vec<Public> = items
        .iter()
        .map(|item| Public {
            id: &item.id,
            session: &item.session,
            message: &item.message,
            created_at: item.created_at,
            state: if item.lease_until > now() {
                "sending"
            } else if !reachable(item) {
                "unavailable"
            } else {
                "waiting"
            },
        })
        .collect();
    serde_json::to_string(&wire).map_err(|e| format!("{e}"))
}
