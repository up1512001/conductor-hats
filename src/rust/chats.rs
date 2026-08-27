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

use crate::{mask, places, profile, session, store};

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
fn fill_next(chat: &mut Chat) {
    if !chat.next.is_empty() {
        return;
    }
    chat.next = store::effective_dir(&chat.agent, std::path::Path::new(&chat.path))
        .as_deref()
        .and_then(store::profile_from_dir)
        .unwrap_or_default();
}

pub fn collect() -> Vec<Chat> {
    let mut out: Vec<Chat> = places::rows(CHATS)
        .iter()
        .filter_map(|l| parse(l))
        .collect();
    for chat in &mut out {
        fill_next(chat);
    }
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
    let body = serde_json::to_string(&wire).map_err(|e| format!("{e}"))?;
    println!("{body}");
    Ok(())
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
