//! Every chat Conductor has open, and the account each one is on.
//!
//! The panel answers this for the chat in front of you. This answers it for all
//! of them at once, which is the question that gets asked when several agents
//! are running and it is no longer obvious which is spending what.
//!
//! Two accounts per chat, and the difference matters. `on` is what the process
//! already running took when it spawned and cannot be changed. `next` is what
//! the process after it will take. They differ exactly when a chat has been
//! pointed somewhere new and not yet restarted.

use crate::{mask, places, profile, routes, session, settings, store};

#[derive(serde::Serialize)]
struct Wire<'a> {
    workspace: &'a str,
    path: &'a str,
    session: &'a str,
    agent: &'a str,
    status: &'a str,
    unread: i64,
    title: &'a str,
    context: f64,
    /// The account the running process took, empty when it has not started.
    on: &'a str,
    /// The account the next process will take.
    next: &'a str,
}

/// Non-archived workspaces, visible chats, newest activity first.
///
/// `is_hidden` covers the chats Conductor keeps but does not show, which would
/// otherwise pad the list with rows nobody recognises.
const CHATS: &str = "select w.directory_name, w.workspace_path, s.id, \
     coalesce(nullif(s.claude_session_id,''), s.id), coalesce(s.agent_type,'claude'), \
     coalesce(s.status,''), coalesce(s.unread_count,0), \
     replace(coalesce(nullif(s.title,''),'Untitled'), '|', ' '), \
     coalesce(s.context_used_percent,0) \
   from sessions s join workspaces w on w.id = s.workspace_id \
   where w.state != 'archived' and coalesce(s.is_hidden,0) = 0 \
     and w.workspace_path is not null \
   order by s.updated_at desc";

pub struct Chat {
    pub workspace: String,
    pub path: String,
    pub session: String,
    pub agent: String,
    pub status: String,
    pub unread: i64,
    pub title: String,
    pub context: f64,
    /// The account the running process took. Empty when it has not started, or
    /// when it started before hats began recording this.
    pub on: String,
    /// The account the next process will take.
    pub next: String,
}

/// Splits on the separator `sqlite3` writes between columns. Titles are the only
/// free text here and the query has already had the separator removed from them.
fn parse(line: &str) -> Option<Chat> {
    let f: Vec<&str> = line.split('|').collect();
    if f.len() < 9 {
        return None;
    }
    let agent = f[4].to_string();
    let router_id = f[3].to_string();
    let on = session::started(&agent, &router_id).unwrap_or_default();
    let next = session::pinned(&agent, &router_id).unwrap_or_default();
    Some(Chat {
        workspace: f[0].to_string(),
        path: f[1].to_string(),
        session: f[2].to_string(),
        agent,
        status: f[5].to_string(),
        unread: f[6].parse().unwrap_or(0),
        title: f[7].to_string(),
        context: f[8].parse().unwrap_or(0.0),
        on,
        next,
    })
}

/// A chat with no pin follows its workspace, so the workspace's answer is what
/// its next process will take.
///
/// Deliberately not `store::effective_dir`. That goes through `resolve::decide`,
/// which is the router's decision and therefore writes: it can spend the account
/// chosen for a new workspace and record a route as a side effect. A listing
/// must not do that, and this one is served to a phone on a poll, which would
/// have done it every second.
///
/// So the read-only layers only, in the router's own order: an exact route, then
/// a repository binding, then a parent route or the default. Resolved once per
/// workspace rather than once per chat, which is also what took the listing from
/// 1.9 seconds across 161 chats down to something a phone can poll.
fn workspace_account(agent: &str, path: &str) -> String {
    let dir = std::path::Path::new(path);
    let found = routes::resolve(dir);
    if let Some(m) = &found {
        if m.exact {
            return m.profile.clone();
        }
    }
    if let Some(bound) = settings::repo_binding(agent, &store::repo_root(dir)) {
        if let Some(name) = store::profile_from_dir(&bound) {
            return name;
        }
    }
    found.map(|m| m.profile).unwrap_or_default()
}

fn fill_next(chats: &mut [Chat]) {
    let mut known: std::collections::HashMap<(String, String), String> =
        std::collections::HashMap::new();
    for chat in chats.iter_mut() {
        if !chat.next.is_empty() {
            continue;
        }
        let key = (chat.agent.clone(), chat.path.clone());
        let answer = known
            .entry(key)
            .or_insert_with(|| workspace_account(&chat.agent, &chat.path));
        chat.next.clone_from(answer);
    }
}

pub fn collect() -> Vec<Chat> {
    let mut out: Vec<Chat> = places::rows(CHATS)
        .iter()
        .filter_map(|l| parse(l))
        .collect();
    fill_next(&mut out);
    out
}

fn shown(agent: &str, name: &str, masked: bool) -> String {
    if name.is_empty() {
        return "-".into();
    }
    match (masked, profile::label(agent, name)) {
        (true, Some(email)) if !email.is_empty() => mask::email(&email),
        _ => name.to_string(),
    }
}

/// The same list as JSON, which is what anything drawing a screen wants.
pub fn as_json() -> Result<(), String> {
    println!("{}", json_string()?);
    Ok(())
}

pub fn json_string() -> Result<String, String> {
    store::ensure_root()?;
    let chats = collect();
    let wire: Vec<Wire> = chats
        .iter()
        .map(|c| Wire {
            workspace: &c.workspace,
            path: &c.path,
            session: &c.session,
            agent: &c.agent,
            status: &c.status,
            unread: c.unread,
            title: &c.title,
            context: c.context,
            on: &c.on,
            next: &c.next,
        })
        .collect();
    serde_json::to_string(&wire).map_err(|e| format!("{e}"))
}

pub fn run(masked: bool) -> Result<(), String> {
    store::ensure_root()?;
    let chats = collect();
    if chats.is_empty() {
        println!("No chats. Conductor's database is unreadable, or it has none open.");
        return Ok(());
    }

    println!(
        "{:<20} {:<8} {:<9} {:>6}  {:<10} {:<10} TITLE",
        "WORKSPACE", "AGENT", "STATUS", "CTX", "ON", "NEXT"
    );
    for c in &chats {
        let unread = if c.unread > 0 {
            format!(" ({} unread)", c.unread)
        } else {
            String::new()
        };
        println!(
            "{:<20} {:<8} {:<9} {:>5.0}%  {:<10} {:<10} {}{}",
            c.workspace,
            c.agent,
            c.status,
            c.context,
            shown(&c.agent, &c.on, masked),
            shown(&c.agent, &c.next, masked),
            c.title,
            unread
        );
    }

    let moving = chats
        .iter()
        .filter(|c| !c.on.is_empty() && !c.next.is_empty() && c.on != c.next)
        .count();
    if moving > 0 {
        println!();
        println!(
            "{moving} chat(s) will change account when reopened. A running \
             conversation keeps the account it started on."
        );
    }
    Ok(())
}
