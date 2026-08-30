//! One coherent mobile snapshot and the settings hats can apply safely.

use std::collections::BTreeMap;

use crate::{
    chats, conductor_session, mask, mobile_catalog, paths, places, profile, remote, remote_control,
    remote_create, session, transcript,
};

const PROBE: &str = "select (select max(rowid) from session_messages) || ':' || \
     (select count(*) from sessions where status is not null and status != 'idle')";

#[derive(serde::Serialize)]
struct Account {
    name: String,
    label: String,
    signed_in: bool,
}

#[derive(serde::Serialize)]
struct Active {
    session: String,
    transcript: serde_json::Value,
    outbox: serde_json::Value,
    controls: serde_json::Value,
    creates: serde_json::Value,
}

#[derive(serde::Serialize)]
struct Snapshot {
    r#type: &'static str,
    stamp: String,
    chats: serde_json::Value,
    active: Option<Active>,
    accounts: BTreeMap<String, Vec<Account>>,
    models: BTreeMap<String, Vec<String>>,
    source: String,
}

pub fn stamp() -> String {
    format!(
        "{}:{}:{}:{}:{}:{}",
        places::revision(),
        places::rows(PROBE).first().cloned().unwrap_or_default(),
        remote::stamp(),
        remote_control::stamp(),
        remote_create::stamp(),
        mobile_catalog::stamp()
    )
}

fn accounts() -> BTreeMap<String, Vec<Account>> {
    ["claude", "codex"]
        .into_iter()
        .map(|agent| {
            let list = paths::profiles(agent)
                .into_iter()
                .map(|name| Account {
                    label: profile::label(agent, &name)
                        .filter(|value| !value.is_empty())
                        .map(|value| mask::email(&value))
                        .unwrap_or_else(|| name.clone()),
                    signed_in: profile::signed_in(agent, &name),
                    name,
                })
                .collect();
            (agent.to_string(), list)
        })
        .collect()
}

fn models(value: &serde_json::Value) -> BTreeMap<String, Vec<String>> {
    let mut found = mobile_catalog::current().models;
    for chat in value.as_array().into_iter().flatten() {
        let agent = chat.get("agent").and_then(|v| v.as_str()).unwrap_or("");
        let model = chat.get("model").and_then(|v| v.as_str()).unwrap_or("");
        if agent.is_empty() || model.is_empty() {
            continue;
        }
        let models = found.entry(agent.to_string()).or_default();
        if !models.iter().any(|value| value == model) {
            models.push(model.to_string());
        }
    }
    found
}

pub fn snapshot(selected: Option<&str>) -> Result<String, String> {
    let chats: serde_json::Value =
        serde_json::from_str(&chats::json_string()?).map_err(|e| format!("chat snapshot: {e}"))?;
    let active = selected.and_then(|raw| {
        let route = conductor_session::route(raw)?;
        let transcript = serde_json::from_str(&transcript::as_json(&route.session, 180)).ok()?;
        let outbox = serde_json::from_str(&remote::pending_json(&route.session).ok()?).ok()?;
        let controls = remote_control::pending_json(&route.session).ok()?;
        let creates = remote_create::pending_json(&route.session);
        Some(Active {
            session: route.session,
            transcript,
            outbox,
            controls,
            creates,
        })
    });
    serde_json::to_string(&Snapshot {
        r#type: "snapshot",
        stamp: stamp(),
        source: crate::source::active()
            .map(|source| source.label().to_string())
            .unwrap_or_else(|| "Conductor".into()),
        models: models(&chats),
        accounts: accounts(),
        chats,
        active,
    })
    .map_err(|e| format!("mobile snapshot: {e}"))
}

pub fn choose_account(chat: &str, name: &str) -> Result<(), String> {
    let route = conductor_session::route(chat).ok_or("that chat is no longer open")?;
    session::pin(name, &route.agent, &route.router_session)
}
