//! Every chat at once, and the account each is on.
//!
//! The panel answers this for the chat in front of you. When several agents are
//! running, the question becomes which of them is on what, and that is a list.

mod common;

use common::Sandbox;

fn sqlite() -> bool {
    std::process::Command::new("sqlite3")
        .arg("-version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// One workspace, three chats: a visible pair, and one Conductor keeps hidden.
fn database(s: &Sandbox) -> String {
    let db = s.path("conductor.db");
    let live = s.path("ws-live").to_string_lossy().to_string();
    let gone = s.path("ws-archived").to_string_lossy().to_string();
    let project = s.path("project").to_string_lossy().to_string();
    let sql = format!(
        "create table repos (id text, name text, root_path text);\n\
         insert into repos values ('r1','Project One','{project}');\n\
         create table workspaces (id text, directory_name text, workspace_path text, state text, active_session_id text, repository_id text, created_at text);\n\
         insert into workspaces values ('w1','ws-live','{live}','ready','s1','r1','2026-01-02');\n\
         insert into workspaces values ('w2','ws-archived','{gone}','archived','s3','r1','2026-01-01');\n\
         create table sessions (id text, claude_session_id text, agent_type text, status text, \
             unread_count integer, title text, context_used_percent real, is_hidden integer, \
             workspace_id text, updated_at text, context_token_count integer, model text, \
             permission_mode text, claude_effort_level text, codex_thinking_level text, \
             agent_personality text, fast_mode integer, created_at text);\n\
         insert into sessions values ('s1','s1','claude','working',2,'Live one',12.5,0,'w1','2026-08-27T10:00:00Z',1200,'claude-sonnet-4-6','default','high','','concise',1,'2026-08-27T10:00:00Z');\n\
         insert into sessions values ('s2','s2','claude','idle',0,'Live two',3.0,0,'w1','2026-08-27T09:00:00Z',300,'claude-sonnet-4-6','plan','medium','','',0,'2026-08-27T09:00:00Z');\n\
         insert into sessions values ('s4','s4','claude','idle',0,'Hidden',0,1,'w1','2026-08-27T08:00:00Z',0,'','','','','',0,'2026-08-27T08:00:00Z');\n\
         insert into sessions values ('s3','s3','claude','idle',0,'Archived',0,0,'w2','2026-08-27T07:00:00Z',0,'','','','','',0,'2026-08-27T07:00:00Z');\n"
    );
    let out = std::process::Command::new("sqlite3")
        .arg(&db)
        .arg(&sql)
        .output()
        .expect("building the fixture database");
    assert!(out.status.success(), "sqlite3 refused the fixture");
    db.to_string_lossy().to_string()
}

fn listing(s: &Sandbox, db: &str, args: &[&str]) -> String {
    let mut argv = vec!["chats"];
    argv.extend_from_slice(args);
    s.hats_env(&argv, &[("CONDUCTOR_DB", db)]).out()
}

#[test]
fn it_lists_visible_chats_and_hides_the_rest() {
    if !sqlite() {
        eprintln!("skipped: sqlite3 is not installed");
        return;
    }
    let s = Sandbox::new();
    s.hats(&["install"]).ok();
    s.profile("claude", "work");
    s.workspace("ws-live");
    s.workspace("ws-archived");
    let db = database(&s);

    let out = listing(&s, &db, &[]);
    assert!(
        out.contains("Live one"),
        "the newest chat is missing:\n{out}"
    );
    assert!(
        out.contains("Live two"),
        "a visible chat is missing:\n{out}"
    );
    assert!(!out.contains("Hidden"), "a hidden chat was listed:\n{out}");
    assert!(
        !out.contains("Archived"),
        "a chat in an archived workspace was listed:\n{out}"
    );
    assert!(out.contains("(2 unread)"), "unread is not reported:\n{out}");
}

/// The whole point of two columns: a chat that has been pointed somewhere new
/// is still running on the old account until it restarts.
#[test]
fn it_separates_what_a_chat_runs_on_from_what_it_will_run_on() {
    if !sqlite() {
        eprintln!("skipped: sqlite3 is not installed");
        return;
    }
    let s = Sandbox::new();
    s.hats(&["install"]).ok();
    s.profile("claude", "work");
    s.profile("claude", "personal");
    let ws = s.workspace("ws-live");
    s.workspace("ws-archived");
    let db = database(&s);
    s.hats(&["use", "personal", "claude", &ws]).ok();

    /* Starting an agent records what it took; pinning decides the next one. */
    s.route("claude", "ws-live", &["--session-id=s1"]);
    s.hats(&["pin", "work", "claude", "s1"]).ok();

    let out = listing(&s, &db, &[]);
    let row = out
        .lines()
        .find(|l| l.contains("Live one"))
        .unwrap_or_default();
    assert!(
        row.contains("personal") && row.contains("work"),
        "the row does not carry both accounts:\n{row}"
    );
    assert!(
        out.contains("will change account when reopened"),
        "nothing said the chat is about to move:\n{out}"
    );
}

/// A chat with no pin follows its workspace, so that is what it will start on.
#[test]
fn an_unpinned_chat_reports_the_workspaces_account_as_next() {
    if !sqlite() {
        eprintln!("skipped: sqlite3 is not installed");
        return;
    }
    let s = Sandbox::new();
    s.hats(&["install"]).ok();
    s.profile("claude", "personal");
    let ws = s.workspace("ws-live");
    s.workspace("ws-archived");
    let db = database(&s);
    s.hats(&["use", "personal", "claude", &ws]).ok();

    let out = listing(&s, &db, &[]);
    let row = out
        .lines()
        .find(|l| l.contains("Live two"))
        .unwrap_or_default();
    assert!(row.contains("personal"), "next account is missing:\n{row}");
}

/// The JSON form is what anything drawing a screen reads, so its shape is a
/// contract rather than a convenience.
#[test]
fn the_json_form_carries_the_fields_a_screen_needs() {
    if !sqlite() {
        eprintln!("skipped: sqlite3 is not installed");
        return;
    }
    let s = Sandbox::new();
    s.hats(&["install"]).ok();
    s.profile("claude", "work");
    s.workspace("ws-live");
    s.workspace("ws-archived");
    let db = database(&s);

    let out = listing(&s, &db, &["--json"]);
    let parsed: serde_json::Value = serde_json::from_str(&out).expect("valid JSON");
    let first = parsed[0].clone();
    for field in [
        "project",
        "project_path",
        "repository_id",
        "workspace",
        "workspace_id",
        "path",
        "session",
        "agent",
        "status",
        "unread",
        "title",
        "context",
        "context_tokens",
        "model",
        "permission",
        "effort",
        "personality",
        "fast",
        "updated_at",
        "pending",
        "on",
        "next",
    ] {
        assert!(!first[field].is_null(), "{field} is missing from the JSON");
    }
    assert_eq!(first["project"], "Project One");
    assert_eq!(first["repository_id"], "r1");
    assert_eq!(first["workspace_id"], "w1");
    assert_eq!(parsed.as_array().map(|a| a.len()), Some(2), "wrong count");
}

/// Conductor lists the newest workspace first and keeps chats in the order they
/// were started. Ordering by last activity reshuffled the list under the reader
/// every time an agent wrote a line.
#[test]
fn chats_are_listed_in_conductors_own_order() {
    if !sqlite() {
        eprintln!("skipped: sqlite3 is not installed");
        return;
    }
    let s = Sandbox::new();
    let db = database(&s);
    let out = listing(&s, &db, &["--json"]);
    let first = out.find("Live two").expect("the oldest chat");
    let second = out.find("Live one").expect("the newest chat");
    assert!(
        first < second,
        "chats are not in the order they were started:\n{out}"
    );
}
