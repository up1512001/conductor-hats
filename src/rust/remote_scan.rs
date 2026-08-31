//! Cheap queue metadata that never reads message text into command output.

use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use crate::{id, paths};

fn root() -> PathBuf {
    paths::accounts_root().join("remote")
}

fn files(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("json"))
        .collect()
}

pub fn counts() -> std::collections::HashMap<String, usize> {
    let mut out = std::collections::HashMap::new();
    let Ok(sessions) = std::fs::read_dir(root()) else {
        return out;
    };
    for entry in sessions.flatten().filter(|entry| entry.path().is_dir()) {
        let session = entry.file_name().to_string_lossy().to_string();
        if id::session(&session).is_some() {
            let count = files(&entry.path()).len();
            if count > 0 {
                out.insert(session, count);
            }
        }
    }
    out
}

pub fn stamp() -> String {
    let Ok(sessions) = std::fs::read_dir(root()) else {
        return "0:0".into();
    };
    metadata_stamp(
        sessions
            .flatten()
            .filter(|entry| entry.path().is_dir())
            .flat_map(|entry| files(&entry.path())),
    )
}

pub(crate) fn metadata_stamp(paths: impl Iterator<Item = PathBuf>) -> String {
    let mut count = 0usize;
    let mut newest = 0u128;
    for path in paths {
        count += 1;
        let changed = std::fs::metadata(path)
            .and_then(|meta| meta.modified())
            .and_then(|at| at.duration_since(UNIX_EPOCH).map_err(std::io::Error::other))
            .map(|age| age.as_nanos())
            .unwrap_or(0);
        newest = newest.max(changed);
    }
    format!("{count}:{newest}")
}

pub fn sessions() -> Vec<(u64, String)> {
    let Ok(sessions) = std::fs::read_dir(root()) else {
        return Vec::new();
    };
    let mut found: Vec<(u64, String)> = sessions
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .flat_map(|entry| files(&entry.path()))
        .filter_map(|path| {
            serde_json::from_slice::<serde_json::Value>(&std::fs::read(path).ok()?).ok()
        })
        .filter_map(|item| {
            let session = item.get("session")?.as_str()?.to_string();
            let created = item.get("created_at")?.as_u64()?;
            id::session(&session)?;
            Some((created, session))
        })
        .collect();
    found.sort();
    found
}

/// Drops every queued item whose chat no longer exists in this Conductor copy.
///
/// Deleting somebody's unsent words is not something to do quietly, so this is
/// its own command rather than a side effect of listing the queue.
pub fn purge() -> Result<usize, String> {
    let mut dropped = 0;
    for dir in std::fs::read_dir(root()).into_iter().flatten().flatten() {
        for path in files(&dir.path()) {
            if let Some(item) = crate::remote::read(&path) {
                if !crate::remote::delivered(&item) && crate::remote::reachable(&item) {
                    continue;
                }
                if std::fs::remove_file(&path).is_ok() {
                    dropped += 1;
                }
            }
        }
    }
    Ok(dropped + crate::remote_control::purge())
}
