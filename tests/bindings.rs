//! Repository bindings, which Conductor applies itself and the router never sees.

mod common;

use common::Sandbox;

fn binding_file(s: &Sandbox) -> String {
    s.read("repo/.conductor/settings.local.toml")
}

#[test]
fn bind_writes_repository_settings() {
    let s = Sandbox::new();
    s.profile("claude", "work");
    s.hats(&["bind", "work", "claude", &s.workspace("repo")])
        .ok();

    let body = binding_file(&s);
    assert!(body.contains("[environment_variables]"), "{body}");
    let want = format!(
        "CLAUDE_CONFIG_DIR = \"{}\"",
        s.accounts().join("claude/work").display()
    );
    assert!(body.contains(&want), "{body}");
}

#[test]
fn bind_preserves_the_rest_of_the_file() {
    let s = Sandbox::new();
    s.profile("claude", "work");
    std::fs::write(
        s.path("repo/.conductor/settings.local.toml"),
        "[scripts]\nsetup = \"pnpm install\"\n\n[environment_variables]\nDATABASE_URL = \"postgres://localhost/dev\"\n",
    )
    .unwrap();
    s.hats(&["bind", "work", "claude", &s.workspace("repo")])
        .ok();

    let body = binding_file(&s);
    assert!(body.contains("setup = \"pnpm install\""), "{body}");
    assert!(
        body.contains("DATABASE_URL = \"postgres://localhost/dev\""),
        "{body}"
    );
    assert!(body.contains("CLAUDE_CONFIG_DIR"), "{body}");
    assert_eq!(
        body.lines()
            .filter(|l| l.trim() == "[environment_variables]")
            .count(),
        1,
        "no duplicate table:\n{body}"
    );
}

#[test]
fn bind_replaces_rather_than_appends() {
    let s = Sandbox::new();
    s.profile("claude", "work");
    s.profile("claude", "personal");
    let repo = s.workspace("repo");
    s.hats(&["bind", "work", "claude", &repo]).ok();
    s.hats(&["bind", "personal", "claude", &repo]).ok();

    let body = binding_file(&s);
    assert_eq!(
        body.lines()
            .filter(|l| l.contains("CLAUDE_CONFIG_DIR"))
            .count(),
        1,
        "{body}"
    );
    assert!(body.contains("claude/personal"), "{body}");
}

/// The router never exports a repository binding, because Conductor applies the
/// repository's [environment_variables] itself. Reporting only the dry run
/// therefore hid the binding the moment the router was installed, and the New
/// Workspace chip read "default account" for a repository that was firmly bound.
#[test]
fn a_repository_binding_is_reported_while_the_router_is_on() {
    let s = Sandbox::new();
    s.profile("claude", "work");
    s.hats(&["install"]).ok();
    let repo = s.workspace("repo");
    s.hats(&["bind", "work", "claude", &repo]).ok();

    s.hats(&["json", &repo]).says(r#""current":"work""#);
    s.hats(&["status", &repo]).says("work");
    s.hats(&["which", &repo]).says("effective:  work");
    s.hats(&["check", &repo]).says("ACCOUNT claude work");
}

#[test]
fn a_workspace_route_still_beats_a_binding_in_json() {
    let s = Sandbox::new();
    s.profile("claude", "work");
    s.profile("claude", "personal");
    s.hats(&["install"]).ok();
    s.hats(&["bind", "work", "claude", &s.workspace("repo")])
        .ok();
    s.hats(&["use", "personal", "claude", &s.workspace("repo/ws")])
        .ok();

    s.hats(&["json", &s.workspace("repo/ws")])
        .says(r#""current":"personal""#);
}

#[test]
fn unbind_leaves_the_rest_of_the_file() {
    let s = Sandbox::new();
    s.profile("claude", "work");
    std::fs::write(
        s.path("repo/.conductor/settings.local.toml"),
        "[environment_variables]\nDATABASE_URL = \"postgres://localhost/dev\"\n",
    )
    .unwrap();
    let repo = s.workspace("repo");
    s.hats(&["bind", "work", "claude", &repo]).ok();
    s.hats(&["unbind", "claude", &repo]).ok();

    let body = binding_file(&s);
    assert!(body.contains("DATABASE_URL"), "{body}");
    assert!(!body.contains("CLAUDE_CONFIG_DIR"), "{body}");
}

/// A repository binding outranks the account chosen while a workspace was being
/// created.
///
/// The creation choice is one global value that is never cleared, so a choice
/// made for one repository was still being spent days later on the first
/// workspace created in an unrelated one, overriding that repository's own
/// binding. Conductor delivers a binding by setting the config directory before
/// the router runs, so the router leaving an already-set value alone is what
/// respecting the binding means.
#[test]
fn a_repository_binding_outranks_a_creation_choice() {
    let s = Sandbox::new();
    s.hats(&["install"]).ok();
    s.profile("claude", "personal");
    s.profile("claude", "work");
    s.hats(&["next", "personal", "claude"]).ok();

    let bound = s
        .accounts()
        .join("claude/work")
        .to_string_lossy()
        .to_string();
    let got = s.route_env(
        "claude",
        "ws-a",
        &["--model", "opus"],
        &[("CLAUDE_CONFIG_DIR", &bound)],
    );
    assert_eq!(
        got, bound,
        "the creation choice overrode the repository binding"
    );
}

/// The choice still applies where no binding has answered the question.
#[test]
fn a_creation_choice_still_applies_without_a_binding() {
    let s = Sandbox::new();
    s.hats(&["install"]).ok();
    s.profile("claude", "personal");
    s.hats(&["next", "personal", "claude"]).ok();

    let got = s.route("claude", "ws-b", &["--model", "opus"]);
    assert!(
        got.ends_with("claude/personal"),
        "the creation choice was lost: {got}"
    );
}
