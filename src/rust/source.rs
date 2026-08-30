//! One Conductor application database selected by a workspace it owns.

use std::path::{Path, PathBuf};

use crate::{id, places};

#[derive(Clone, Eq, PartialEq)]
pub struct Source {
    database: PathBuf,
    label: String,
}

fn label(path: &Path) -> String {
    match path
        .parent()
        .and_then(Path::file_name)
        .and_then(|value| value.to_str())
        .unwrap_or("")
    {
        "com.conductor.app" => "Conductor".into(),
        "com.conductor.dev" | "com.conductor.dep" => "Conductor Dev".into(),
        _ => "Conductor (local)".into(),
    }
}

fn from_database(database: PathBuf) -> Source {
    Source {
        label: label(&database),
        database,
    }
}

pub fn for_workspace(workspace: &str) -> Result<Source, String> {
    let workspace = id::session(workspace)
        .filter(|value| value.len() <= 36)
        .ok_or("open a workspace before configuring mobile access")?;
    let sql = format!("select 1 from workspaces where id='{workspace}' limit 1");
    let mut owners = places::databases()
        .into_iter()
        .filter(|database| !places::query(database, &sql).is_empty());
    let owner = owners
        .next()
        .ok_or("the open workspace does not belong to a readable Conductor database")?;
    if owners.next().is_some() {
        return Err("the open workspace belongs to more than one Conductor app".into());
    }
    Ok(from_database(owner))
}

pub fn active() -> Option<Source> {
    let configured = std::env::var_os("CONDUCTOR_DB").map(PathBuf::from)?;
    places::databases()
        .into_iter()
        .find(|database| database == &configured)
        .map(from_database)
}

pub fn from_key(key: &str) -> Option<Source> {
    places::databases()
        .into_iter()
        .find(|database| database.to_string_lossy() == key)
        .map(from_database)
}

impl Source {
    pub fn key(&self) -> String {
        self.database.to_string_lossy().to_string()
    }

    pub fn database(&self) -> &Path {
        &self.database
    }

    pub fn label(&self) -> &str {
        &self.label
    }
}
