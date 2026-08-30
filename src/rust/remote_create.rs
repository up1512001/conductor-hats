//! Durable requests to open a new chat through Conductor's own New chat action.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::{auth, conductor_session, id, lock, paths, remote_scan};

const LEASE: Duration = Duration::from_secs(60);
const EXPIRY: Duration = Duration::from_secs(15 * 60);
const ATTEMPTS: u32 = 4;
const MAX_PENDING: usize = 10;

#[derive(Clone, serde::Deserialize, serde::Serialize)]
pub struct Item {
    pub id: String,
    pub session: String,
    pub workspace: String,
    pub marker: u64,
    pub created_at: u64,
    pub lease: String,
    pub lease_until: u64,
    pub attempts: u32,
    pub done: bool,
    pub result: String,
    pub error: String,
}

fn root() -> PathBuf {
    paths::accounts_root().join("remote-create")
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn private(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::create_dir_all(path).map_err(|e| format!("{}: {e}", path.display()))?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
        .map_err(|e| format!("{}: {e}", path.display()))
}

fn files() -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(root())
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("json"))
        .collect();
    found.sort();
    found
}

fn read(path: &Path) -> Option<Item> {
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

fn path(id: &str) -> Result<PathBuf, String> {
    if id.len() != 32 || !id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("invalid new-chat request id".into());
    }
    Ok(root().join(format!("{id}.json")))
}

fn write(item: &Item) -> Result<(), String> {
    private(&root())?;
    let body = serde_json::to_string(item).map_err(|e| format!("new-chat request: {e}"))?;
    auth::write_private(&path(&item.id)?, &body)
}

fn fresh(item: &Item) -> bool {
    item.created_at + EXPIRY.as_millis() as u64 > now()
}

fn fresh_read(file: &Path) -> Option<Item> {
    let item = read(file);
    if item.as_ref().is_some_and(fresh) {
        item
    } else {
        let _ = std::fs::remove_file(file);
        None
    }
}

pub fn enqueue(session: &str) -> Result<Item, String> {
    let route = conductor_session::route(session).ok_or("that chat is no longer open")?;
    let marker = conductor_session::workspace_marker(&route.session)
        .ok_or("could not read the workspace's current chats")?;
    private(&root())?;
    let _guard = lock::Lock::acquire(&root().join("queue"))?;
    let queued: Vec<Item> = files().iter().filter_map(|file| fresh_read(file)).collect();
    if let Some(existing) = queued
        .iter()
        .find(|item| item.workspace == route.workspace_id && fresh(item) && !item.done)
    {
        return Ok(existing.clone());
    }
    if queued
        .iter()
        .filter(|item| fresh(item) && !item.done)
        .count()
        >= MAX_PENDING
    {
        return Err("ten new chats are already waiting for Conductor".into());
    }
    let item = Item {
        id: auth::random_hex(16)?,
        session: route.session,
        workspace: route.workspace_id,
        marker,
        created_at: now(),
        lease: String::new(),
        lease_until: 0,
        attempts: 0,
        done: false,
        result: String::new(),
        error: String::new(),
    };
    write(&item)?;
    Ok(item)
}

pub fn claim(session: &str) -> Result<Option<Item>, String> {
    let session = id::session(session).ok_or("invalid chat id")?;
    private(&root())?;
    let _guard = lock::Lock::acquire(&root().join("queue"))?;
    let mut queued: Vec<(PathBuf, Item)> = files()
        .into_iter()
        .filter_map(|file| read(&file).map(|item| (file, item)))
        .collect();
    queued.sort_by_key(|(_, item)| (item.created_at, item.id.clone()));
    for (file, mut item) in queued {
        if !fresh(&item) {
            let _ = std::fs::remove_file(file);
            continue;
        }
        if item.done || item.session != session {
            continue;
        }
        if item.lease_until > now() {
            return Ok(None);
        }
        item.lease = auth::random_hex(16)?;
        item.lease_until = now() + LEASE.as_millis() as u64;
        write(&item)?;
        return Ok(Some(item));
    }
    Ok(None)
}

pub fn check(item: &Item) -> Result<Option<String>, String> {
    let _guard = lock::Lock::acquire(&root().join("queue"))?;
    let current = read(&path(&item.id)?).ok_or("that new-chat request is no longer active")?;
    if !auth::same(&current.lease, &item.lease) {
        return Err("that new-chat lease is no longer active".into());
    }
    Ok(conductor_session::created_since(
        &current.session,
        current.marker,
    ))
}

pub fn finish(item: &Item, completed: bool) -> Result<(), String> {
    let _guard = lock::Lock::acquire(&root().join("queue"))?;
    let file = path(&item.id)?;
    let Some(mut current) = read(&file) else {
        return Ok(());
    };
    if !auth::same(&current.lease, &item.lease) {
        return Err("that new-chat lease is no longer active".into());
    }
    current.lease.clear();
    current.lease_until = 0;
    if completed {
        current.result = conductor_session::created_since(&current.session, current.marker)
            .ok_or("Conductor has not recorded the new chat yet")?;
        current.done = true;
    } else {
        current.attempts += 1;
        if current.attempts >= ATTEMPTS {
            current.done = true;
            current.error = "Conductor did not open a new chat after four attempts".into();
        }
    }
    write(&current)
}

pub fn ack(id: &str) -> Result<(), String> {
    let _guard = lock::Lock::acquire(&root().join("queue"))?;
    let file = path(id)?;
    let Some(item) = read(&file) else {
        return Ok(());
    };
    if !item.done {
        return Err("that new-chat request is still active".into());
    }
    std::fs::remove_file(file).map_err(|e| format!("acknowledging new chat: {e}"))
}

pub fn pending_json(session: &str) -> serde_json::Value {
    let items = files()
        .into_iter()
        .filter_map(|file| fresh_read(&file))
        .filter(|item| item.session == session);
    serde_json::Value::Array(
        items
            .map(|item| {
                serde_json::json!({
                    "id": item.id,
                    "state": if item.done { "done" } else if item.lease_until > now() { "creating" } else { "waiting" },
                    "result": item.result,
                    "error": item.error,
                })
            })
            .collect(),
    )
}

pub fn sessions() -> Vec<(u64, String)> {
    let mut found: Vec<(u64, String)> = files()
        .into_iter()
        .filter_map(|file| fresh_read(&file))
        .filter(|item| !item.done && conductor_session::route(&item.session).is_some())
        .map(|item| (item.created_at, item.session))
        .collect();
    found.sort();
    found
}

pub fn purge() -> usize {
    files()
        .into_iter()
        .filter(|file| {
            read(file)
                .map(|item| !fresh(&item) || conductor_session::route(&item.session).is_none())
                .unwrap_or(true)
        })
        .filter(|file| std::fs::remove_file(file).is_ok())
        .count()
}

pub fn stamp() -> String {
    remote_scan::metadata_stamp(files().into_iter())
}
