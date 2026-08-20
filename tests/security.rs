//! Containment: no identifier may become a path component that escapes its root.
//!
//! Every case here was reachable before validation was centralised. `remove`
//! with a traversal name deleted an arbitrary directory, and a crafted
//! --session-id wrote a pin file anywhere the user could write.

mod common;

use common::Sandbox;

#[test]
fn remove_refuses_a_traversal_name() {
    let s = Sandbox::new();
    s.workspace("victim");
    std::fs::write(s.path("victim/important.txt"), "keep").unwrap();

    s.hats(&["remove", "../../victim", "claude"])
        .failed()
        .says("invalid profile name");
    assert!(s.exists("victim/important.txt"), "the directory survives");
}

#[test]
fn remove_refuses_an_absolute_name() {
    let s = Sandbox::new();
    let victim = s.workspace("victim");

    s.hats(&["remove", &victim, "claude"]).failed();
    assert!(s.exists("victim"), "the directory survives");
}

#[test]
fn a_traversal_name_is_refused_by_every_command() {
    let s = Sandbox::new();
    s.profile("claude", "work");
    for command in ["use", "bind", "login", "logout", "remove"] {
        s.hats(&[command, "../escape", "claude"]).failed();
    }
}

#[test]
fn a_crafted_session_id_writes_no_pin_outside_the_sessions_directory() {
    let s = Sandbox::new();
    s.profile("claude", "work");
    s.hats(&["use", "work", "claude", &s.workspace("ws-a")])
        .ok();

    s.route("claude", "ws-a", &["--session-id=../../../../pwned"]);
    assert!(!s.exists("pwned"), "no pin escaped the sessions directory");
}

#[test]
fn a_valid_session_id_still_pins() {
    let s = Sandbox::new();
    s.profile("claude", "work");
    s.profile("claude", "personal");
    let ws = s.workspace("ws-a");
    s.hats(&["use", "work", "claude", &ws]).ok();
    s.route("claude", "ws-a", &["--session-id=abc123"]);

    s.hats(&["use", "personal", "claude", &ws]).ok();
    let got = s.route("claude", "ws-a", &["--session-id=abc123"]);
    assert!(
        got.ends_with("/claude/work"),
        "a running session keeps its account: {got}"
    );
}

#[test]
fn a_corrupt_route_is_skipped_rather_than_followed() {
    let s = Sandbox::new();
    s.profile("claude", "work");
    let ws = s.workspace("ws-a");
    std::fs::write(
        s.accounts().join("routes"),
        format!("{ws}\t../../../../etc\n"),
    )
    .unwrap();

    assert_eq!(s.route("claude", "ws-a", &[]), "");
}

#[test]
fn the_conductor_account_override_is_validated() {
    let s = Sandbox::new();
    s.profile("claude", "work");
    let mut cmd = s.router("claude", "ws-a");
    cmd.env("CONDUCTOR_ACCOUNT", "../../../../etc");
    assert_eq!(
        s.config_dir_from("claude", cmd, &[]),
        "",
        "a traversal override is ignored"
    );
}

/// A command that signs must never report a bad signature and still exit 0: a
/// broken application would read as installed.
#[test]
fn patching_a_bogus_app_fails_loudly() {
    let s = Sandbox::new();
    let app = s.workspace("Fake.app/Contents/MacOS");
    std::fs::write(format!("{app}/conductor"), "not a mach-o binary at all").unwrap();

    s.hats(&[
        "patch",
        "--app",
        &s.path("Fake.app").to_string_lossy(),
        "--i-know",
    ])
    .failed()
    .silent_about("signature valid");
}

#[test]
fn reverting_without_a_backup_fails_loudly() {
    let s = Sandbox::new();
    let app = s.workspace("Fake.app/Contents/MacOS");
    std::fs::write(format!("{app}/conductor"), "nothing").unwrap();

    s.hats(&["revert", "--app", &s.path("Fake.app").to_string_lossy()])
        .failed();
}

/// A sign-out that failed means the provider still holds a live session.
/// Deleting the local profile then throws away the only record of it.
#[test]
fn remove_refuses_when_sign_out_fails() {
    let s = Sandbox::new();
    s.signed_in("claude", "work");
    s.failing_agent();

    s.hats(&["remove", "work", "claude"])
        .failed()
        .says("exited with status 3");
    assert!(
        s.exists("accounts/claude/work"),
        "the profile is still there"
    );
}

#[test]
fn remove_force_deletes_and_warns() {
    let s = Sandbox::new();
    s.signed_in("claude", "work");
    s.failing_agent();

    s.hats(&["remove", "work", "claude", "--force"])
        .ok()
        .says("may still consider this account signed in");
    assert!(!s.exists("accounts/claude/work"), "the profile is gone");
}

#[test]
fn logout_reports_a_refusal_rather_than_claiming_success() {
    let s = Sandbox::new();
    s.signed_in("claude", "work");
    s.failing_agent();

    s.hats(&["logout", "work", "claude"])
        .failed()
        .says("signing 'work' out failed");
}

#[test]
fn logging_out_of_a_signed_out_profile_is_not_an_error() {
    let s = Sandbox::new();
    s.profile("claude", "work");

    s.hats(&["logout", "work", "claude"])
        .ok()
        .says("nothing to sign out of");
}

#[test]
fn removing_a_signed_out_profile_still_works() {
    let s = Sandbox::new();
    s.profile("claude", "work");
    s.hats(&["use", "work", "claude", &s.workspace("ws-a")])
        .ok();

    s.hats(&["remove", "work", "claude"]).ok();
    assert!(!s.exists("accounts/claude/work"), "the profile is gone");
}
