//! Conductor's live model catalog, published by the injected panel.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::time::UNIX_EPOCH;

use crate::{auth, id, lock, paths, source};

const AGENTS: [&str; 2] = ["claude", "codex"];
const MAX_CATALOG: usize = 32 * 1024;
const MAX_MODELS: usize = 64;
const MAX_VALUE: usize = 120;

#[derive(Clone, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Catalog {
    pub models: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub titles: BTreeMap<String, String>,
}

#[derive(Eq, PartialEq, serde::Deserialize, serde::Serialize)]
struct Record {
    source: String,
    catalog: Catalog,
}

fn path() -> PathBuf {
    paths::accounts_root().join("serve-catalog")
}

fn safe(value: &str, limit: usize) -> bool {
    !value.is_empty() && value.len() <= limit && !value.chars().any(char::is_control)
}

fn cleaned(raw: &str) -> Result<Catalog, String> {
    if raw.len() > MAX_CATALOG {
        return Err("the Conductor model catalog is too large".into());
    }
    let incoming: Catalog =
        serde_json::from_str(raw).map_err(|_| "invalid Conductor model catalog")?;
    if incoming
        .models
        .keys()
        .any(|agent| !AGENTS.contains(&agent.as_str()))
    {
        return Err("the Conductor model catalog names an unsupported agent".into());
    }
    let mut models = BTreeMap::new();
    for agent in AGENTS {
        let mut seen = BTreeSet::new();
        let values = incoming
            .models
            .get(agent)
            .into_iter()
            .flatten()
            .filter(|value| safe(value, MAX_VALUE) && seen.insert((*value).clone()))
            .take(MAX_MODELS)
            .cloned()
            .collect::<Vec<_>>();
        if !values.is_empty() {
            models.insert(agent.to_string(), values);
        }
    }
    if models.is_empty() {
        return Err("the Conductor model catalog is empty".into());
    }
    if AGENTS.iter().any(|agent| !models.contains_key(*agent)) {
        return Err("the Conductor model catalog is incomplete".into());
    }
    let titles = incoming
        .titles
        .into_iter()
        .filter(|(session, title)| id::session(session).is_some() && safe(title, 240))
        .collect();
    Ok(Catalog { models, titles })
}

pub fn publish(owner: &source::Source, raw: &str) -> Result<bool, String> {
    let record = Record {
        source: owner.key(),
        catalog: cleaned(raw)?,
    };
    let file = path();
    let _guard = lock::Lock::acquire(&file)?;
    if read_record().as_ref() == Some(&record) {
        return Ok(false);
    }
    let body = serde_json::to_string(&record).map_err(|e| format!("model catalog: {e}"))?;
    auth::write_private(&file, &body)?;
    Ok(true)
}

fn read_record() -> Option<Record> {
    serde_json::from_slice(&std::fs::read(path()).ok()?).ok()
}

pub fn current() -> Catalog {
    let Some(owner) = source::active() else {
        return Catalog::default();
    };
    read_record()
        .filter(|record| record.source == owner.key())
        .map(|record| record.catalog)
        .unwrap_or_default()
}

pub fn stamp() -> String {
    std::fs::metadata(path())
        .ok()
        .and_then(|metadata| {
            let changed = metadata.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
            Some(format!("{}:{}", metadata.len(), changed.as_nanos()))
        })
        .unwrap_or_default()
}

pub fn clear() {
    let file = path();
    let Ok(_guard) = lock::Lock::acquire(&file) else {
        return;
    };
    let _ = std::fs::remove_file(file);
}
