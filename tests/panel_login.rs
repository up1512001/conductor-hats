//! The non-interactive sign-in commands used only by the injected panel.

mod common;

use std::path::{Path, PathBuf};
use std::time::Duration;

use common::Sandbox;

fn agent(s: &Sandbox) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let path = s.path("login-agent");
    std::fs::write(
        &path,
        r#"#!/bin/sh
printf '%s\n' 'Approve at https://claude.ai/oauth/authorize?panel=test'
IFS= read -r code
if [ "$code" != "approved-code" ]; then
  printf '%s\n' 'provider rejected the code' >&2
  exit 4
fi
printf '%s\n' '{"token":"private"}' > "$CLAUDE_CONFIG_DIR/.credentials.json"
printf '%s\n' '{"oauthAccount":{"emailAddress":"panel@example.test"}}' > "$CLAUDE_CONFIG_DIR/.claude.json"
"#,
    )
    .unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path
}

fn start(s: &Sandbox, binary: &Path) -> String {
    s.hats_env(
        &["login-start", "personal", "claude"],
        &[(
            "CONDUCTOR_ACCOUNTS_CLAUDE_BIN",
            binary.to_str().unwrap_or_default(),
        )],
    )
    .ok()
    .stdout
    .trim()
    .to_string()
}

fn wait_status(s: &Sandbox, want: &str) -> String {
    for _ in 0..100 {
        let out = s
            .hats(&["login-status", "personal", "claude"])
            .ok()
            .stdout
            .trim()
            .to_string();
        if out.starts_with(want) {
            return out;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("login status did not become {want:?}");
}

#[test]
fn the_panel_can_complete_a_login_without_a_terminal() {
    use std::os::unix::fs::PermissionsExt;

    let s = Sandbox::new();
    let binary = agent(&s);
    let url = start(&s, &binary);
    assert_eq!(
        url, "https://claude.ai/oauth/authorize?panel=test",
        "login-start must print only the URL"
    );
    assert!(s.accounts().join("claude/personal").is_dir());
    let fifo = s.accounts().join("login/claude/personal/stdin");
    assert_eq!(
        std::fs::metadata(fifo).unwrap().permissions().mode() & 0o777,
        0o600
    );
    s.hats(&["login-code", "personal", "claude", "approved-code"])
        .ok()
        .says("submitted");
    assert_eq!(wait_status(&s, "ok"), "ok panel@example.test");
}

#[test]
fn a_provider_failure_is_reported_and_cleaned_up() {
    let s = Sandbox::new();
    let binary = agent(&s);
    start(&s, &binary);
    s.hats(&["login-code", "personal", "claude", "wrong"]).ok();
    assert!(wait_status(&s, "error").contains("provider rejected the code"));
    assert!(!s.accounts().join("login/claude/personal").exists());
}

#[test]
fn cancellation_stops_only_the_owned_login_process() {
    let s = Sandbox::new();
    let binary = agent(&s);
    start(&s, &binary);
    s.hats(&["login-cancel", "personal", "claude"])
        .ok()
        .says("cancelled");
    assert_eq!(
        s.hats(&["login-status", "personal", "claude"])
            .ok()
            .stdout
            .trim(),
        "idle"
    );
}

#[test]
fn login_state_never_accepts_a_path_component() {
    let s = Sandbox::new();
    s.hats(&["login-start", "../escape", "claude"])
        .failed()
        .says("invalid profile name");
    assert!(!s.path("escape").exists());
}
