//! Exact read-only Conductor identity and run state for one mobile chat.

use crate::{id, places};

use std::collections::HashMap;

#[derive(Clone, serde::Serialize)]
pub struct Route {
    pub session: String,
    pub router_session: String,
    pub workspace_id: String,
    pub repository_id: String,
    pub agent: String,
}

fn safe(raw: &str) -> Option<&str> {
    id::session(raw)
}

fn parsed(row: &str) -> Option<Route> {
    let values: Vec<&str> = row.split('|').collect();
    if values.len() != 5 || safe(values[0]).is_none() || safe(values[2]).is_none() {
        return None;
    }
    Some(Route {
        session: values[0].to_string(),
        router_session: values[1].to_string(),
        workspace_id: values[2].to_string(),
        repository_id: values[3].to_string(),
        agent: values[4].to_string(),
    })
}

fn select(sessions: &str) -> String {
    format!(
        "select s.id, coalesce(nullif(s.claude_session_id,''),s.id), \
         w.id, coalesce(w.repository_id,''), coalesce(s.agent_type,'claude') \
         from sessions s join workspaces w on w.id=s.workspace_id \
         where s.id in ({sessions})"
    )
}

pub fn route(session: &str) -> Option<Route> {
    let session = safe(session)?;
    places::rows(&select(&format!("'{session}'")))
        .into_iter()
        .find_map(|row| parsed(&row))
}

/// The oldest queued route owned by the same Conductor copy as `workspace`.
pub fn first_in_workspace(workspace: &str, sessions: &[String]) -> Option<Route> {
    let valid: Vec<&str> = sessions.iter().filter_map(|value| safe(value)).collect();
    if valid.is_empty() {
        return None;
    }
    let ids = valid
        .iter()
        .map(|value| format!("'{value}'"))
        .collect::<Vec<String>>()
        .join(",");
    let found: HashMap<String, Route> = places::rows_in_workspace(workspace, &select(&ids))
        .into_iter()
        .filter_map(|row| parsed(&row))
        .map(|route| (route.session.clone(), route))
        .collect();
    valid
        .into_iter()
        .find_map(|session| found.get(session).cloned())
}

pub fn setting(session: &str, setting: &str) -> Option<String> {
    let session = safe(session)?;
    let expression = match setting {
        "model" => "coalesce(model,'')",
        "effort" => "coalesce(nullif(claude_effort_level,''),codex_thinking_level,'')",
        "permission" => "coalesce(permission_mode,'')",
        "fast" => "coalesce(fast_mode,0)",
        _ => return None,
    };
    let sql = format!("select {expression} from sessions where id='{session}' limit 1");
    places::rows(&sql).into_iter().next()
}

pub fn workspace_marker(session: &str) -> Option<u64> {
    let route = route(session)?;
    let sql = format!(
        "select coalesce(max(rowid),0) from sessions where workspace_id='{}'",
        route.workspace_id
    );
    places::rows(&sql).into_iter().next()?.parse().ok()
}

pub fn created_since(session: &str, marker: u64) -> Option<String> {
    let route = route(session)?;
    let sql = format!(
        "select id from sessions where workspace_id='{}' and rowid>{marker} order by rowid desc limit 1",
        route.workspace_id
    );
    places::rows(&sql)
        .into_iter()
        .find(|value| safe(value).is_some())
}

pub fn created_model_since(session: &str, marker: u64, model: &str) -> Option<String> {
    let route = route(session)?;
    let sql = format!(
        "select id, coalesce(model,'') from sessions where workspace_id='{}' \
         and rowid>{marker} order by rowid desc",
        route.workspace_id
    );
    places::rows(&sql).into_iter().find_map(|row| {
        let (session, found) = row.split_once('|')?;
        (safe(session).is_some() && found == model).then(|| session.to_string())
    })
}
