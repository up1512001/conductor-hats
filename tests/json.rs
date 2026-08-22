//! The panel reads `hats json`, and the address shown in it comes out of the
//! provider's own state file. Both used to be handled by looking for quotes and
//! colons in a string, which is right until a value contains one.

mod common;

use common::Sandbox;

#[test]
fn an_address_is_read_from_the_field_not_the_first_match() {
    let s = Sandbox::new();
    s.credentialed("claude", "work");
    std::fs::write(
        s.accounts().join("claude/work/.claude.json"),
        r#"{
  "note": "the key \"emailAddress\" is documented elsewhere",
  "oauthAccount": { "emailAddress": "real@example.test" }
}"#,
    )
    .unwrap();

    s.hats(&["login", "work", "claude"]).ok();
    assert_eq!(
        s.read("accounts/claude/work/.label").trim(),
        "real@example.test",
        "the real address is cached"
    );
}

#[test]
fn a_corrupt_state_file_yields_no_address_rather_than_a_wrong_one() {
    let s = Sandbox::new();
    s.bare("claude", "work");
    std::fs::write(
        s.accounts().join("claude/work/.claude.json"),
        r#"{"oauthAccount": {"emailAddress": "trunc"#,
    )
    .unwrap();

    s.hats(&["login", "work", "claude"]).ok();
    assert!(
        !s.exists("accounts/claude/work/.label"),
        "nothing was cached"
    );
}

#[test]
fn json_stays_parseable_when_a_path_contains_a_quote() {
    let s = Sandbox::new();
    s.profile("claude", "work");
    let odd = s.workspace("say \"hi\"");

    let out = s.hats(&["json", &odd]);
    serde_json::from_str::<serde_json::Value>(&out.stdout)
        .unwrap_or_else(|e| panic!("output must parse as JSON ({e}):\n{}", out.stdout));
}

#[test]
fn json_keeps_the_field_names_the_panel_expects() {
    let s = Sandbox::new();
    s.profile("claude", "work");

    let out = s.hats(&["json", &s.workspace("ws-a")]);
    let state: serde_json::Value = serde_json::from_str(&out.stdout).expect("valid JSON");

    let keys = |value: &serde_json::Value| {
        let mut names: Vec<String> = value
            .as_object()
            .expect("an object")
            .keys()
            .map(String::from)
            .collect();
        names.sort();
        names
    };
    assert_eq!(keys(&state), ["enabled", "providers", "repo", "workspace"]);
    let provider = &state["providers"][0];
    assert_eq!(
        keys(provider),
        ["accounts", "agent", "chat", "current", "pinned", "session"]
    );
    assert_eq!(
        keys(&provider["accounts"][0]),
        ["active", "email", "name", "signedIn"]
    );
}

#[test]
fn an_address_with_a_backslash_survives_the_round_trip() {
    let s = Sandbox::new();
    s.profile_with("claude", "work", r"back\slash@example.test");

    let out = s.hats(&["json", &s.workspace("ws-a")]);
    let state: serde_json::Value = serde_json::from_str(&out.stdout)
        .unwrap_or_else(|e| panic!("output must parse as JSON ({e}):\n{}", out.stdout));
    assert_eq!(
        state["providers"][0]["accounts"][0]["email"],
        r"back\slash@example.test"
    );
}
