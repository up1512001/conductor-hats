//! Per-section change detection for the phone, in one database round trip.
//!
//! The socket used to compute a single stamp over everything and resend the
//! whole snapshot whenever any part of it moved. `places::revision()` covers the
//! write-ahead log, which changes on any Conductor write anywhere, so an agent
//! streaming in an unrelated workspace made a phone re-download the chat list
//! and the open transcript several times a second: 263 KB measured, three times
//! a second, over a tunnel to a phone.
//!
//! Each section carries its own stamp here, so only what moved is sent. The
//! database probe is a single `sqlite3` invocation for both sections, and it is
//! skipped entirely while the revision is unchanged, so an idle phone costs no
//! query at all rather than one every 350 ms.

use crate::{
    id, mobile_catalog, paths, places, remote, remote_control, remote_create, remote_scan,
};

/// Counts and high-water marks rather than contents. A stamp only has to change
/// when the section does; it never travels to the phone.
const SESSIONS: &str = "(select coalesce(max(rowid),0)||':'||count(*)||':'\
     ||replace(coalesce(max(updated_at),''),'|','')||':'\
     ||coalesce(sum(unread_count),0)||':'\
     ||cast(coalesce(sum(context_used_percent),0) as int)||':'\
     ||coalesce(sum(case when status is not null and status!='idle' then 1 else 0 end),0) \
   from sessions)";

#[derive(Clone, Default, PartialEq, Eq)]
pub struct Sections {
    revision: String,
    selected: String,
    sessions: String,
    messages: String,
    pub chats: String,
    pub active: String,
    pub accounts: String,
}

/// Two scalars in one query: the chat list, and the open chat's transcript.
///
/// The session id is checked before it reaches the statement. `id::session`
/// admits only `[A-Za-z0-9_-]`, so it cannot carry a quote, and the phone
/// chooses this value.
fn probe(selected: &str) -> (String, String) {
    let messages = if selected.is_empty() {
        "''".to_string()
    } else {
        format!(
            "(select coalesce(max(rowid),0)||':'||count(*)||':'\
             ||coalesce(sum(case when cancelled_at is null then 1 else 0 end),0) \
           from session_messages where session_id='{selected}')"
        )
    };
    let row = places::rows(&format!("select {SESSIONS}, {messages}"))
        .into_iter()
        .next()
        .unwrap_or_default();
    match row.split_once('|') {
        Some((sessions, messages)) => (sessions.to_string(), messages.to_string()),
        None => (row, String::new()),
    }
}

/// Whichever profiles exist and whether any has just signed in or out.
fn accounts() -> String {
    let roots = ["claude", "codex"].into_iter().flat_map(|agent| {
        std::iter::once(paths::accounts_root().join(agent)).chain(
            paths::profiles(agent)
                .into_iter()
                .map(move |name| paths::profile_dir(agent, &name)),
        )
    });
    remote_scan::metadata_stamp(roots)
}

pub fn read(selected: Option<&str>, previous: &Sections) -> Sections {
    let selected = selected.and_then(id::session).unwrap_or("").to_string();
    let revision = places::revision();
    let catalog = mobile_catalog::stamp();
    let queues = format!(
        "{}:{}:{}",
        remote::stamp(),
        remote_control::stamp(),
        remote_create::stamp()
    );
    /* The queues and the catalog are filesystem stamps and cost nothing. Only
     * the database probe is worth avoiding, and nothing in it can have moved
     * while the file and its write-ahead log are byte for byte the same. */
    let (sessions, messages) = if revision == previous.revision && selected == previous.selected {
        (previous.sessions.clone(), previous.messages.clone())
    } else {
        probe(&selected)
    };
    Sections {
        chats: format!("{sessions}|{catalog}"),
        active: format!("{selected}|{messages}|{queues}"),
        accounts: accounts(),
        revision,
        selected,
        sessions,
        messages,
    }
}
