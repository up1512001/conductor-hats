//! Conductor's settings file belongs to Conductor. Install and uninstall add and
//! remove two top-level keys and must leave everything else byte for byte, which
//! means understanding which table a key sits in rather than matching its name
//! anywhere in the file.

mod common;

use common::Sandbox;

const LOOKALIKE: &str = "\
# Conductor settings, hand edited.
theme = \"dark\"

[editor]
claude_code_executable_path = \"/somebody/elses/claude\"
font = \"Berkeley Mono\"

[telemetry]
enabled = false
";

fn given(s: &Sandbox, body: &str) {
    std::fs::write(s.settings(), body).expect("a settings file");
}

fn settings(s: &Sandbox) -> String {
    std::fs::read_to_string(s.settings()).unwrap_or_default()
}

#[test]
fn uninstall_leaves_a_lookalike_key_in_another_table_alone() {
    let s = Sandbox::new();
    given(&s, LOOKALIKE);
    s.hats(&["uninstall"]).ok();

    let body = settings(&s);
    assert!(
        body.contains("claude_code_executable_path = \"/somebody/elses/claude\""),
        "the editor table keeps its key:\n{body}"
    );
    assert!(body.contains("font = \"Berkeley Mono\""), "{body}");
}

#[test]
fn install_then_uninstall_restores_the_file() {
    let s = Sandbox::new();
    given(&s, LOOKALIKE);
    s.hats(&["install"]).ok();
    s.hats(&["uninstall"]).ok();

    assert_eq!(settings(&s), LOOKALIKE, "the file is back as it was");
}

#[test]
fn install_writes_its_keys_above_the_first_table() {
    let s = Sandbox::new();
    given(&s, LOOKALIKE);
    s.hats(&["install"]).ok();

    let body = settings(&s);
    let key = body
        .lines()
        .position(|l| l.starts_with("claude_code_executable_path"));
    let table = body.lines().position(|l| l.starts_with('['));
    assert!(
        key < table,
        "the key must land above the first table:\n{body}"
    );
}

#[test]
fn a_commented_key_is_not_mistaken_for_a_real_one() {
    let s = Sandbox::new();
    given(
        &s,
        "# claude_code_executable_path = \"/commented/out\"\ntheme = \"dark\"\n",
    );
    s.hats(&["uninstall"]).ok();

    assert!(
        settings(&s).contains("# claude_code_executable_path"),
        "the comment survives"
    );
}

#[test]
fn reinstalling_does_not_duplicate_the_key() {
    let s = Sandbox::new();
    s.hats(&["install"]).ok();
    s.hats(&["install"]).ok();

    let body = settings(&s);
    assert_eq!(
        body.lines()
            .filter(|l| l.starts_with("claude_code_executable_path"))
            .count(),
        1,
        "{body}"
    );
}

#[test]
fn a_quoted_path_with_spaces_is_read_back_whole() {
    let s = Sandbox::new();
    let dir = s.workspace("App Support/bin");
    given(
        &s,
        &format!("claude_code_executable_path = \"{dir}/claude-router\"\n"),
    );

    s.hats(&["doctor", &s.workspace("ws-a")])
        .says("App Support");
}
