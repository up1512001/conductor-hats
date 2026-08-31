//! One coherent mobile snapshot and the settings hats can apply safely.

use std::collections::BTreeMap;

use crate::{
    chats, conductor_session, mask, mobile_catalog, paths, profile, remote, remote_control,
    remote_create, session, transcript,
};

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

/// Which sections the socket has decided are stale. Anything left out of the
/// message is unchanged, and the phone keeps what it already holds.
#[derive(Clone, Copy)]
pub struct Want {
    pub chats: bool,
    pub active: bool,
    pub accounts: bool,
}

impl Want {
    pub fn any(&self) -> bool {
        self.chats || self.active || self.accounts
    }
}

#[derive(serde::Serialize)]
struct Snapshot {
    r#type: &'static str,
    stamp: String,
    source: String,
    /// Absent rather than null when unchanged. `active` is null when no chat is
    /// open, so the two cases have to stay distinguishable on the wire.
    #[serde(skip_serializing_if = "Option::is_none")]
    chats: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    active: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    accounts: Option<BTreeMap<String, Vec<Account>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    models: Option<BTreeMap<String, Vec<String>>>,
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

fn models(
    value: &serde_json::Value,
    mut found: BTreeMap<String, Vec<String>>,
) -> BTreeMap<String, Vec<String>> {
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

fn apply_titles(value: &mut serde_json::Value, titles: &BTreeMap<String, String>) {
    for chat in value.as_array_mut().into_iter().flatten() {
        let session = chat
            .get("session")
            .and_then(|item| item.as_str())
            .unwrap_or("");
        if let Some(title) = titles
            .get(session)
            .filter(|value| !value.eq_ignore_ascii_case("new chat"))
            .filter(|value| !value.eq_ignore_ascii_case("untitled"))
        {
            chat["title"] = serde_json::Value::String(title.clone());
        }
    }
}

/// One message carrying only the sections the socket found stale.
///
/// Nothing here decides what is stale; that is `mobile_stamp`. Reading a section
/// costs a query and a file walk, so a section the phone already has is not read
/// at all, not read and then discarded.
pub fn snapshot(selected: Option<&str>, want: Want, stamp: String) -> Result<String, String> {
    let catalog = mobile_catalog::current();
    let mut listed = None;
    let mut models_for = None;
    if want.chats {
        let mut chats: serde_json::Value = serde_json::from_str(&chats::json_string()?)
            .map_err(|e| format!("chat snapshot: {e}"))?;
        apply_titles(&mut chats, &catalog.titles);
        models_for = Some(models(&chats, catalog.models));
        listed = Some(chats);
    }
    let active = want.active.then(|| {
        let found = selected.and_then(|raw| {
            let route = conductor_session::route(raw)?;
            let transcript =
                serde_json::from_str(&transcript::as_json(&route.session, 180)).ok()?;
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
        serde_json::to_value(found).unwrap_or(serde_json::Value::Null)
    });
    serde_json::to_string(&Snapshot {
        r#type: "snapshot",
        stamp,
        source: crate::source::active()
            .map(|source| source.label().to_string())
            .unwrap_or_else(|| "Conductor".into()),
        chats: listed,
        active,
        accounts: want.accounts.then(accounts),
        models: models_for,
    })
    .map_err(|e| format!("mobile snapshot: {e}"))
}

pub fn choose_account(chat: &str, name: &str) -> Result<(), String> {
    let route = conductor_session::route(chat).ok_or("that chat is no longer open")?;
    session::pin(name, &route.agent, &route.router_session)
}

#[cfg(test)]
#[path = "mobile_state_tests.rs"]
mod tests;
