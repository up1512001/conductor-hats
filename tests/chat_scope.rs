//! A workspace holds many chats, and each runs its own agent process.
//!
//! The panel used to report one account for all of them, because it only ever
//! read the workspace route. Switching from the toolbar then appeared to move
//! every chat, while the running ones stayed exactly where they were.

mod common;

use common::Sandbox;

fn provider(s: &Sandbox, workspace: &str) -> serde_json::Value {
    let out = s.hats(&["json", &s.path(workspace).to_string_lossy()]);
    let state: serde_json::Value = serde_json::from_str(&out.stdout).expect("valid JSON");
    state["providers"][0].clone()
}

#[test]
fn the_live_chat_is_identified_from_its_transcript() {
    let s = Sandbox::new();
    s.profile("claude", "work");
    s.workspace("ws-a");
    s.transcript("ws-a", "abc123");

    s.hats(&["session", &s.path("ws-a").to_string_lossy()])
        .ok()
        .says("abc123");
}

#[test]
fn a_workspace_with_no_recent_chat_reports_none() {
    let s = Sandbox::new();
    s.profile("claude", "work");
    s.workspace("ws-a");

    s.hats(&["session", &s.path("ws-a").to_string_lossy()])
        .ok()
        .says("no chat active");
}

/// The whole point: the workspace and the chat are separate answers.
#[test]
fn a_pinned_chat_reports_its_own_account_not_the_workspaces() {
    let s = Sandbox::new();
    s.hats(&["install"]).ok();
    s.profile("claude", "work");
    s.profile("claude", "personal");
    let ws = s.workspace("ws-a");
    s.transcript("ws-a", "abc123");

    s.hats(&["use", "work", "claude", &ws]).ok();
    s.hats(&["pin", "personal", "claude", "abc123"]).ok();

    let p = provider(&s, "ws-a");
    assert_eq!(p["current"], "work", "the workspace is unchanged");
    assert_eq!(p["chat"], "personal", "the chat carries its own account");
    assert_eq!(p["pinned"], true);
    assert_eq!(p["session"], "abc123");
}

#[test]
fn an_unpinned_chat_follows_the_workspace() {
    let s = Sandbox::new();
    s.hats(&["install"]).ok();
    s.profile("claude", "work");
    let ws = s.workspace("ws-a");
    s.transcript("ws-a", "abc123");
    s.hats(&["use", "work", "claude", &ws]).ok();

    let p = provider(&s, "ws-a");
    assert_eq!(p["chat"], "work");
    assert_eq!(p["pinned"], false);
}

#[test]
fn pinning_beats_the_workspace_route_when_the_agent_starts() {
    let s = Sandbox::new();
    s.profile("claude", "work");
    s.profile("claude", "personal");
    let ws = s.workspace("ws-a");
    s.transcript("ws-a", "abc123");
    s.hats(&["use", "work", "claude", &ws]).ok();
    s.hats(&["pin", "personal", "claude", "abc123"]).ok();

    let got = s.route("claude", "ws-a", &["--session-id=abc123"]);
    assert!(got.ends_with("/claude/personal"), "the pin wins: {got}");

    let other = s.route("claude", "ws-a", &["--session-id=zzz999"]);
    assert!(
        other.ends_with("/claude/work"),
        "another chat still follows the workspace: {other}"
    );
}

/// A pin cannot move a conversation that is already running, and saying so is
/// the difference between this feature and the bug it replaces.
#[test]
fn pinning_says_the_running_conversation_does_not_move() {
    let s = Sandbox::new();
    s.profile("claude", "work");
    s.workspace("ws-a");
    s.transcript("ws-a", "abc123");

    s.hats(&["pin", "work", "claude", "abc123"])
        .ok()
        .says("stays where it is");
}

#[test]
fn unpinning_returns_the_chat_to_the_workspace() {
    let s = Sandbox::new();
    s.hats(&["install"]).ok();
    s.profile("claude", "work");
    s.profile("claude", "personal");
    let ws = s.workspace("ws-a");
    s.transcript("ws-a", "abc123");
    s.hats(&["use", "work", "claude", &ws]).ok();
    s.hats(&["pin", "personal", "claude", "abc123"]).ok();

    s.hats(&["unpin", "claude", "abc123"]).ok();
    let p = provider(&s, "ws-a");
    assert_eq!(p["chat"], "work");
    assert_eq!(p["pinned"], false);
}

#[test]
fn unpinning_a_chat_that_was_not_pinned_is_not_an_error() {
    let s = Sandbox::new();
    s.workspace("ws-a");
    s.transcript("ws-a", "abc123");

    s.hats(&["unpin", "claude", "abc123"])
        .ok()
        .says("was not pinned");
}

#[test]
fn pinning_refuses_a_profile_that_does_not_exist() {
    let s = Sandbox::new();
    s.workspace("ws-a");
    s.transcript("ws-a", "abc123");

    s.hats(&["pin", "ghost", "claude", "abc123"]).failed();
}

#[test]
fn pinning_refuses_a_session_id_that_is_not_one() {
    let s = Sandbox::new();
    s.profile("claude", "work");
    s.workspace("ws-a");

    s.hats(&["pin", "work", "claude", "../../escape"]).failed();
}

/// Two chats written at the same moment are both plausibly on screen, so
/// choosing between them would silently pin the wrong conversation.
#[test]
fn two_equally_fresh_chats_are_refused_rather_than_guessed_between() {
    let s = Sandbox::new();
    s.profile("claude", "work");
    s.workspace("ws-a");
    s.transcript("ws-a", "aaa111");
    s.transcript("ws-a", "bbb222");

    s.hats(&["session", &s.path("ws-a").to_string_lossy()])
        .ok()
        .says("ambiguous");

    let p = provider(&s, "ws-a");
    assert_eq!(
        p["session"], "",
        "no chat is claimed when two are equally fresh"
    );
}
