//! The account chosen while a workspace is being created.
//!
//! The composer chip is pressed before the workspace exists. Binding the
//! repository was the old answer and it moved every workspace under it; a
//! one-shot is the new one, and the whole difficulty is making sure the right
//! workspace spends it.

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

/// Two workspaces created one after the other, on two accounts, keep them.
///
/// This is the failure as reported: the composer bound the repository, which is
/// a single value for every workspace under it, so the second creation moved the
/// first and everything else in the repository with it.
#[test]
fn each_workspace_keeps_the_account_it_was_created_with() {
    let s = Sandbox::new();
    s.hats(&["install"]).ok();
    s.profile("claude", "work");
    s.profile("claude", "personal");
    let first = s.workspace("repo/ws-work");
    let second = s.workspace("repo/ws-personal");

    s.hats(&["next", "work", "claude"]).ok();
    let got = s.route("claude", "repo/ws-work", &["--session-id=aaa111"]);
    assert!(got.ends_with("/claude/work"), "first workspace: {got}");

    s.hats(&["next", "personal", "claude"]).ok();
    let got = s.route("claude", "repo/ws-personal", &["--session-id=bbb222"]);
    assert!(got.ends_with("/claude/personal"), "second workspace: {got}");

    let out = s.hats(&["which", &first]).out();
    assert!(out.contains("work"), "the first workspace moved:\n{out}");
    let out = s.hats(&["which", &second]).out();
    assert!(
        out.contains("personal"),
        "the second workspace moved:\n{out}"
    );
}

/// The chip is pressed once and can create several workspaces, so the choice
/// holds for all of them rather than the first. Each writes itself a route as it
/// starts, which is what stops a later choice moving it.
#[test]
fn every_workspace_created_after_the_choice_uses_it() {
    let s = Sandbox::new();
    s.hats(&["install"]).ok();
    s.profile("claude", "work");
    s.profile("claude", "personal");
    s.hats(&["assign", "default", "personal"]).ok();

    s.hats(&["next", "work", "claude"]).ok();
    for name in ["ws-a", "ws-b"] {
        s.workspace(name);
        let got = s.route("claude", name, &["--session-id=aaa111"]);
        assert!(
            got.ends_with("/claude/work"),
            "{name} did not use it: {got}"
        );
    }

    /* Another choice replaces it, and the two above keep what they took. */
    s.hats(&["next", "personal", "claude"]).ok();
    s.workspace("ws-c");
    let got = s.route("claude", "ws-c", &["--session-id=bbb222"]);
    assert!(got.ends_with("/claude/personal"), "ws-c: {got}");
    let got = s.route("claude", "ws-a", &["--session-id=ccc333"]);
    assert!(got.ends_with("/claude/work"), "ws-a moved: {got}");
}

/// Conductor starts an agent with the working directory set to `/` before it
/// starts the workspace's own, and more at the repository root. Each of those
/// took the account chosen for a new workspace and left the workspace with none.
#[test]
fn only_a_real_workspace_spends_the_choice() {
    if !sqlite() {
        eprintln!("skipped: sqlite3 is not installed");
        return;
    }
    let s = Sandbox::new();
    s.hats(&["install"]).ok();
    s.profile("claude", "work");
    s.profile("claude", "personal");
    s.hats(&["assign", "default", "personal"]).ok();
    let ws = s.workspace("ws-real");
    let db = database(&s, "ws-real", "sss111", &[("sss111", "sss111", "claude")]);

    s.hats(&["next", "work", "claude"]).ok();

    let elsewhere = s.workspace("not-a-workspace");
    let got = s.route_env(
        "claude",
        "not-a-workspace",
        &["--session-id=aaa111"],
        &[("CONDUCTOR_DB", &db)],
    );
    assert!(
        got.ends_with("/claude/personal"),
        "a directory Conductor does not call a workspace spent the choice: {got} ({elsewhere})"
    );

    let got = s.route_env(
        "claude",
        "ws-real",
        &["--session-id=bbb222"],
        &[("CONDUCTOR_DB", &db)],
    );
    assert!(
        got.ends_with("/claude/work"),
        "the workspace it was chosen for did not get it: {got} ({ws})"
    );
}

/// A dozen workspaces are open at any time and each respawns its agent on a
/// resume, a model switch or a generator restart. Any of those would have taken
/// the account chosen for a workspace that did not exist yet.
#[test]
fn a_workspace_that_already_existed_cannot_spend_the_choice() {
    if !sqlite() {
        eprintln!("skipped: sqlite3 is not installed");
        return;
    }
    let s = Sandbox::new();
    s.hats(&["install"]).ok();
    s.profile("claude", "work");
    s.profile("claude", "personal");
    s.hats(&["assign", "default", "personal"]).ok();

    let old = s.workspace("ws-old");
    let db = s.path("conductor.db").to_string_lossy().to_string();
    let sql = format!(
        "create table workspaces (id text, workspace_path text, state text, active_session_id text);\n\
         insert into workspaces values ('w-old', '{old}', 'ready', null);\n\
         create table sessions (id text, claude_session_id text, agent_type text);\n"
    );
    let out = std::process::Command::new("sqlite3")
        .arg(&db)
        .arg(&sql)
        .output()
        .expect("building the fixture database");
    assert!(out.status.success(), "sqlite3 refused the fixture");

    s.hats_env(&["next", "work", "claude"], &[("CONDUCTOR_DB", &db)])
        .ok();

    let got = s.route_env(
        "claude",
        "ws-old",
        &["--session-id=aaa111"],
        &[("CONDUCTOR_DB", &db)],
    );
    assert!(
        got.ends_with("/claude/personal"),
        "a workspace that already existed took the choice: {got}"
    );

    /* The one that appears afterwards is the one it was for. */
    let fresh = s.workspace("ws-new");
    let add = format!("insert into workspaces values ('w-new', '{fresh}', 'ready', null);");
    std::process::Command::new("sqlite3")
        .arg(&db)
        .arg(&add)
        .output()
        .expect("adding the new workspace");

    let got = s.route_env(
        "claude",
        "ws-new",
        &["--session-id=bbb222"],
        &[("CONDUCTOR_DB", &db)],
    );
    assert!(
        got.ends_with("/claude/work"),
        "the workspace created for it did not get it: {got}"
    );
}

/// The tick in the New Workspace view has to follow the choice, or pressing an
/// account looks like it did nothing and the account gets set again afterwards,
/// from inside the chat, which is the long way round.
#[test]
fn the_new_workspace_view_shows_the_account_it_will_use() {
    if !sqlite() {
        eprintln!("skipped: sqlite3 is not installed");
        return;
    }
    let s = Sandbox::new();
    s.hats(&["install"]).ok();
    s.profile("claude", "work");
    s.profile("claude", "personal");
    /* The default is deliberately not either choice below, so nothing passes by
     * coincidence. */
    s.hats(&["assign", "default", "personal"]).ok();

    let ws = s.workspace("ws-old");
    let db = database(&s, "ws-old", "sss111", &[("sss111", "sss111", "claude")]);
    let repo = s.workspace("repo-root");
    assert_eq!(
        account_of(&s, &db, &ws),
        "personal",
        "the fixture is not as assumed"
    );

    s.hats_env(&["next", "work", "claude"], &[("CONDUCTOR_DB", &db)])
        .ok();
    assert_eq!(
        account_of(&s, &db, &repo),
        "work",
        "the New Workspace view ignores the choice, so pressing an account looks inert"
    );
    /* An existing workspace is not the one being created, and must not move. */
    assert_eq!(
        account_of(&s, &db, &ws),
        "personal",
        "a workspace that already existed was moved by the choice"
    );

    s.hats_env(&["next", "personal", "claude"], &[("CONDUCTOR_DB", &db)])
        .ok();
    assert_eq!(
        account_of(&s, &db, &repo),
        "personal",
        "choosing a second time does not show"
    );
}

fn account_of(s: &Sandbox, db: &str, path: &str) -> String {
    let out = s.hats_env(&["json", path], &[("CONDUCTOR_DB", db)]).out();
    let state: serde_json::Value = serde_json::from_str(&out).expect("valid JSON");
    state["providers"][0]["current"]
        .as_str()
        .unwrap_or("")
        .to_string()
}
