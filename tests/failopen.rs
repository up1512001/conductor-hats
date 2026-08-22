//! Failing open is the whole safety story: a broken install costs the routing,
//! never the agent. The state it has to survive is unreadable or nonsense on
//! disk, and the loop guard has to tell a real loop from an inherited one.

mod common;

use common::Sandbox;

fn claude_dir(s: &Sandbox, name: &str) -> String {
    s.accounts()
        .join("claude")
        .join(name)
        .to_string_lossy()
        .to_string()
}

/// Failing open is the whole safety story: a broken install costs the routing,
#[test]
fn the_router_fails_open_on_an_unreadable_routes_file() {
    use std::os::unix::fs::PermissionsExt;
    let s = Sandbox::new();
    s.profile("claude", "work");
    s.hats(&["use", "work", "claude", &s.workspace("ws-a")])
        .ok();

    let routes = s.accounts().join("routes");
    let mut perms = std::fs::metadata(&routes).unwrap().permissions();
    perms.set_mode(0o000);
    std::fs::set_permissions(&routes, perms).unwrap();

    let got = s.route("claude", "ws-a", &["--model", "opus"]);

    let mut perms = std::fs::metadata(&routes).unwrap().permissions();
    perms.set_mode(0o644);
    std::fs::set_permissions(&routes, perms).unwrap();

    assert_eq!(got, "", "no config dir is better than a wrong one");
}

#[test]
fn the_router_fails_open_on_a_nonsense_routes_file() {
    let s = Sandbox::new();
    s.profile("claude", "work");
    std::fs::write(
        s.accounts().join("routes"),
        b"this is not ( a route\n\x00\x01 garbage\n",
    )
    .unwrap();

    let out = s
        .router("claude", "ws-a")
        .args(["--model", "opus"])
        .output()
        .expect("running the router");
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("ARGV=--model opus"),
        "the agent still starts"
    );
}

#[test]
fn the_router_fails_open_with_no_accounts_root_at_all() {
    let s = Sandbox::new();
    std::fs::remove_dir_all(s.accounts()).unwrap();

    let out = s
        .router("claude", "ws-a")
        .args(["--model", "opus"])
        .output()
        .expect("running the router");
    assert!(String::from_utf8_lossy(&out.stdout).contains("ARGV=--model opus"));
}

#[test]
fn a_real_loop_exits_seventy() {
    let s = Sandbox::new();
    let status = s
        .router("claude", "ws-a")
        .env("CONDUCTOR_ACCOUNTS_DEPTH", "2")
        .output()
        .expect("running the router")
        .status;
    assert_eq!(status.code(), Some(70));
}

/// Launching Conductor from inside a routed session leaves the depth variable
/// set, and a flag-based guard refused every agent because of it.
#[test]
fn one_inherited_generation_is_tolerated() {
    let s = Sandbox::new();
    s.profile("claude", "work");
    s.hats(&["use", "work", "claude", &s.workspace("ws-a")])
        .ok();

    let out = s
        .router("claude", "ws-a")
        .args(["--model", "opus"])
        .env("CONDUCTOR_ACCOUNTS_ROUTING", "claude")
        .env("CONDUCTOR_ACCOUNTS_DEPTH", "1")
        .output()
        .expect("running the router");
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(text.contains("ARGV=--model opus"), "the agent still starts");
    assert!(
        text.contains(&format!("CLAUDE_CONFIG_DIR={}", claude_dir(&s, "work"))),
        "and is still routed:\n{text}"
    );
}

#[test]
fn a_route_to_a_missing_profile_is_ignored() {
    let s = Sandbox::new();
    s.profile("claude", "work");
    s.hats(&["use", "work", "claude", &s.workspace("ws-a")])
        .ok();
    std::fs::remove_dir_all(s.accounts().join("claude/work")).unwrap();

    assert_eq!(s.route("claude", "ws-a", &["--model", "opus"]), "");
}

#[test]
fn a_session_pin_survives_a_route_change() {
    let s = Sandbox::new();
    s.profile("claude", "work");
    s.profile("claude", "personal");
    std::fs::create_dir_all(s.accounts().join("sessions/claude")).unwrap();
    std::fs::write(s.accounts().join("sessions/claude/sess1"), "personal\n").unwrap();
    s.hats(&["use", "work", "claude", &s.workspace("ws-a")])
        .ok();

    assert_eq!(
        s.route("claude", "ws-a", &["--session-id=sess1"]),
        claude_dir(&s, "personal"),
        "a running session keeps its account"
    );
    assert_eq!(
        s.route("claude", "ws-a", &["--session-id=sess2"]),
        claude_dir(&s, "work"),
        "a new session gets the new route"
    );
}

#[test]
fn resume_reuses_the_session_pin() {
    let s = Sandbox::new();
    s.profile("claude", "personal");
    std::fs::create_dir_all(s.accounts().join("sessions/claude")).unwrap();
    std::fs::write(s.accounts().join("sessions/claude/sess1"), "personal\n").unwrap();

    assert_eq!(
        s.route("claude", "ws-a", &["--resume=sess1"]),
        claude_dir(&s, "personal")
    );
}
