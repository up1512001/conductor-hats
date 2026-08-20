//! Which account a workspace gets, and the order the layers win in.

mod common;

use common::Sandbox;

fn claude_dir(s: &Sandbox, name: &str) -> String {
    s.accounts()
        .join("claude")
        .join(name)
        .to_string_lossy()
        .to_string()
}

#[test]
fn use_routes_a_single_workspace() {
    let s = Sandbox::new();
    s.profile("claude", "work");
    s.profile("claude", "personal");
    s.hats(&["use", "work", "claude", &s.workspace("ws-a")])
        .ok();
    s.hats(&["use", "personal", "claude", &s.workspace("ws-b")])
        .ok();

    assert_eq!(s.route("claude", "ws-a", &[]), claude_dir(&s, "work"));
    assert_eq!(s.route("claude", "ws-b", &[]), claude_dir(&s, "personal"));
}

#[test]
fn use_is_idempotent() {
    let s = Sandbox::new();
    s.profile("claude", "work");
    s.profile("claude", "personal");
    let ws = s.workspace("ws-a");
    for name in ["work", "personal", "work"] {
        s.hats(&["use", name, "claude", &ws]).ok();
    }

    let routes = s.read("accounts/routes");
    let written = routes
        .lines()
        .filter(|l| l.starts_with(&format!("{ws}\t")))
        .count();
    assert_eq!(written, 1, "one route per workspace:\n{routes}");
    assert_eq!(s.route("claude", "ws-a", &[]), claude_dir(&s, "work"));
}

#[test]
fn a_route_covers_children_but_not_a_name_prefix_sibling() {
    let s = Sandbox::new();
    s.profile("claude", "work");
    s.workspace("ws-a/nested");
    s.workspace("ws-abc");
    s.hats(&["assign", "work", &s.workspace("ws-a")]).ok();

    assert_eq!(
        s.route("claude", "ws-a/nested", &[]),
        claude_dir(&s, "work")
    );
    assert_eq!(s.route("claude", "ws-abc", &[]), "");
}

#[test]
fn the_longest_matching_route_wins() {
    let s = Sandbox::new();
    s.profile("claude", "work");
    s.profile("claude", "personal");
    s.hats(&["assign", "work", &s.workspace("ws-a")]).ok();
    s.hats(&["assign", "personal", &s.workspace("ws-a/inner")])
        .ok();

    assert_eq!(
        s.route("claude", "ws-a/inner", &[]),
        claude_dir(&s, "personal")
    );
}

#[test]
fn the_default_route_is_the_fallback() {
    let s = Sandbox::new();
    s.profile("claude", "personal");
    s.hats(&["assign", "default", "personal"]).ok();

    assert_eq!(s.route("claude", "ws-b", &[]), claude_dir(&s, "personal"));
}

#[test]
fn a_repository_binding_is_left_alone() {
    let s = Sandbox::new();
    s.profile("claude", "work");
    s.workspace("ws-a");
    let bound = claude_dir(&s, "work");
    assert_eq!(router_saw(&s, "ws-a", &bound, &[]), bound);
}

/// The router with a binding already in the environment, which is how Conductor
/// applies one.
fn router_saw(s: &Sandbox, workspace: &str, bound: &str, args: &[&str]) -> String {
    let mut cmd = s.router("claude", workspace);
    cmd.env("CLAUDE_CONFIG_DIR", bound);
    s.config_dir_from("claude", cmd, args)
}

#[test]
fn a_workspace_route_beats_a_repository_binding() {
    let s = Sandbox::new();
    s.profile("claude", "work");
    s.profile("claude", "personal");
    s.hats(&["use", "personal", "claude", &s.workspace("ws-a")])
        .ok();

    let got = router_saw(&s, "ws-a", &claude_dir(&s, "work"), &[]);
    assert_eq!(got, claude_dir(&s, "personal"));
}

#[test]
fn an_inherited_route_yields_to_a_repository_binding() {
    let s = Sandbox::new();
    s.profile("claude", "work");
    s.profile("claude", "personal");
    s.workspace("ws-a/inner");
    s.hats(&["assign", "personal", &s.workspace("ws-a")]).ok();

    let bound = claude_dir(&s, "work");
    assert_eq!(router_saw(&s, "ws-a/inner", &bound, &[]), bound);
}

#[test]
fn conductor_account_overrides_everything() {
    let s = Sandbox::new();
    s.profile("claude", "work");
    s.profile("claude", "personal");
    s.hats(&["use", "personal", "claude", &s.workspace("ws-a")])
        .ok();

    let mut cmd = s.router("claude", "ws-a");
    cmd.env("CONDUCTOR_ACCOUNT", "work");
    assert_eq!(
        s.config_dir_from("claude", cmd, &[]),
        claude_dir(&s, "work")
    );
}

#[test]
fn argv_is_forwarded_untouched() {
    let s = Sandbox::new();
    s.profile("claude", "work");
    s.hats(&["use", "work", "claude", &s.workspace("ws-a")])
        .ok();

    let out = s
        .router("claude", "ws-a")
        .args([
            "--output-format",
            "stream-json",
            "--session-id=abc123",
            "--model",
            "opus",
        ])
        .output()
        .expect("running the router");
    let argv = String::from_utf8_lossy(&out.stdout)
        .lines()
        .find_map(|l| l.strip_prefix("ARGV="))
        .unwrap_or_default()
        .to_string();
    assert_eq!(
        argv,
        "--output-format stream-json --session-id=abc123 --model opus"
    );
}

#[test]
fn nothing_configured_changes_nothing() {
    let s = Sandbox::new();
    assert_eq!(s.route("claude", "ws-a", &[]), "");
}
