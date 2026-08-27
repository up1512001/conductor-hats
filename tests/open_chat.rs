//! Which chat Conductor has open, why guessing at it was not enough, and the
//! blast radius of getting it wrong.
//!
//! The panel could not reliably tell which conversation was on screen, so a
//! toolbar choice fell back to the workspace, or worse to the repository, and
//! moved every chat at once.

mod common;

use common::Sandbox;

fn sqlite() -> bool {
    std::process::Command::new("sqlite3")
        .arg("-version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Conductor's own record of the open chat: the workspace row points at a
/// session, and the session names the id the agent was started with.
fn database(s: &Sandbox, workspace: &str, active: &str, sessions: &[(&str, &str, &str)]) -> String {
    let db = s.path("conductor.db");
    let path = s.path(workspace).to_string_lossy().to_string();
    let mut sql = String::from(
        "create table workspaces (id text, workspace_path text, state text, active_session_id text);\n\
         create table sessions (id text, claude_session_id text, agent_type text);\n",
    );
    sql.push_str(&format!(
        "insert into workspaces values ('w-1', '{path}', 'ready', '{active}');\n"
    ));
    for (id, claude, agent) in sessions {
        sql.push_str(&format!(
            "insert into sessions values ('{id}', '{claude}', '{agent}');\n"
        ));
    }
    let out = std::process::Command::new("sqlite3")
        .arg(&db)
        .arg(&sql)
        .output()
        .expect("building the fixture database");
    assert!(out.status.success(), "sqlite3 refused the fixture");
    db.to_string_lossy().to_string()
}

fn session_in(s: &Sandbox, workspace: &str, db: &str) -> String {
    let path = s.path(workspace).to_string_lossy().to_string();
    s.hats_env(&["session", &path], &[("CONDUCTOR_DB", db)])
        .out()
        .trim()
        .to_string()
}

/// The bug this exists to end: two chats answering at once left the toolbar with
/// no chat to act on, so it fell back to the workspace and moved both of them.
/// Conductor knows which one is open and never has to guess.
#[test]
fn the_open_chat_comes_from_conductor_not_from_timestamps() {
    if !sqlite() {
        eprintln!("skipped: sqlite3 is not installed");
        return;
    }
    let s = Sandbox::new();
    s.profile("claude", "work");
    s.workspace("ws-a");
    s.transcript("ws-a", "aaa111");
    s.transcript("ws-a", "bbb222");

    let db = database(&s, "ws-a", "bbb222", &[("bbb222", "bbb222", "claude")]);
    assert_eq!(session_in(&s, "ws-a", &db), "bbb222");
}

/// A workspace nobody has typed in for an hour still has a chat on screen, and
/// the toolbar in it still has to say the right account.
#[test]
fn an_idle_workspace_still_knows_which_chat_is_open() {
    if !sqlite() {
        eprintln!("skipped: sqlite3 is not installed");
        return;
    }
    let s = Sandbox::new();
    s.profile("claude", "work");
    s.workspace("ws-a");

    let db = database(&s, "ws-a", "ccc333", &[("ccc333", "ccc333", "claude")]);
    assert_eq!(session_in(&s, "ws-a", &db), "ccc333");
}

/// A workspace showing a Codex chat has no Claude chat open. Answering with the
/// Codex one would pin the wrong agent's conversation.
#[test]
fn a_codex_chat_is_not_offered_as_the_claude_one() {
    if !sqlite() {
        eprintln!("skipped: sqlite3 is not installed");
        return;
    }
    let s = Sandbox::new();
    s.profile("claude", "work");
    s.workspace("ws-a");

    let db = database(&s, "ws-a", "ddd444", &[("ddd444", "ddd444", "codex")]);
    assert!(
        session_in(&s, "ws-a", &db).contains("no chat"),
        "a Codex chat was reported as the Claude one"
    );
}

/// The two ids are different namespaces. A conversation resumed after a
/// compaction carries a `claude_session_id` of its own, and that is the one on
/// the command line the router reads: pinning Conductor's would write a file
/// nothing ever looks up.
#[test]
fn the_pinned_id_is_the_one_the_router_will_see() {
    if !sqlite() {
        eprintln!("skipped: sqlite3 is not installed");
        return;
    }
    let s = Sandbox::new();
    s.profile("claude", "work");
    s.workspace("ws-a");

    let db = database(&s, "ws-a", "eee555", &[("eee555", "fff666", "claude")]);
    assert_eq!(session_in(&s, "ws-a", &db), "fff666");
}

/// The toolbar belongs to the chat it was pressed in. Binding a repository from
/// it writes one value that every workspace in that repository inherits, so the
/// last account chosen wins everywhere: the exact failure this was reported as.
#[test]
fn a_repository_binding_is_one_value_for_every_workspace_in_it() {
    let s = Sandbox::new();
    s.hats(&["install"]).ok();
    s.profile("claude", "work");
    s.profile("claude", "personal");
    let a = s.workspace("repo/ws-a");
    let b = s.workspace("repo/ws-b");
    let repo = s.path("repo").to_string_lossy().to_string();

    s.hats(&["bind", "personal", "claude", &repo]).ok();
    s.hats(&["bind", "work", "claude", &repo]).ok();

    for ws in [&a, &b] {
        let out = s.hats(&["which", ws]).out();
        assert!(
            out.contains("work"),
            "the last binding did not reach every workspace:\n{out}"
        );
    }
}

/// Panel logging is off until asked for, and it records decisions rather than
/// anything typed.
#[test]
fn panel_logging_is_off_until_it_is_turned_on() {
    let s = Sandbox::new();
    s.hats(&["debug", "status"]).ok().says("off");
    s.hats(&["log", "ignored while off"]).ok();
    s.hats(&["debug", "read"]).ok().silent_about("ignored");

    s.hats(&["debug", "on"]).ok();
    s.hats(&["log", "scope=workspace fiberId=none"]).ok();
    s.hats(&["debug", "read"]).ok().says("scope=workspace");

    s.hats(&["debug", "off"]).ok();
    s.hats(&["debug", "status"]).ok().says("off");
}
