//! The injected panel, judged against the artifact that actually gets injected.
//!
//! Behaviour is asserted against the built bundle rather than the source it came
//! from, because a broken bundle found the other way costs a patch, a launch and
//! a click.

mod common;

use common::Sandbox;

/// The panel the binary carries, which is the one `patch` injects.
fn bundle() -> String {
    let s = Sandbox::new();
    s.hats(&["panel"]).ok().stdout.clone()
}

fn node() -> Option<&'static str> {
    let found = std::process::Command::new("node")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    found.then_some("node")
}

#[test]
fn the_built_panel_parses() {
    let Some(node) = node() else {
        eprintln!("skipped: node is not installed");
        return;
    };
    let s = Sandbox::new();
    let file = s.path("account-ui.js");
    std::fs::write(&file, bundle()).unwrap();

    let out = std::process::Command::new(node)
        .arg("--check")
        .arg(&file)
        .output()
        .expect("node --check");
    assert!(
        out.status.success(),
        "node --check is unhappy:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn the_built_panel_is_one_self_contained_iife() {
    let dist = bundle();
    let head = &dist[..dist.len().min(400)];
    assert!(
        head.contains("(() => {"),
        "no IIFE in the first 400 bytes:\n{head}"
    );
}

#[test]
fn the_built_panel_carries_the_behaviour() {
    let dist = bundle();
    for needle in [
        "__conductorHats",
        ".cma-panel{position:fixed",
        "logout ",
        "pointerdown",
        "m4.7144",
        "M22.2819",
        "maskEmail",
    ] {
        assert!(dist.contains(needle), "the bundle is missing {needle:?}");
    }
    assert!(
        !dist.contains("acct(`remove "),
        "the panel must never remove a profile"
    );
    assert!(
        !dist.contains("acct(\"remove "),
        "the panel must never remove a profile"
    );
}

const MASK_CASES: [&str; 8] = [
    "someone.long@example.com",
    "joe@mail.example.com",
    "ab@x.test",
    "a@b.test",
    "first.last@example.com",
    "someone.else@example.org",
    "noatsign",
    "x@y",
];

#[test]
fn masking_never_reveals_a_whole_part() {
    let s = Sandbox::new();
    for address in MASK_CASES {
        let masked = s.hats(&["mask", address]).ok().stdout.trim().to_string();
        assert!(
            masked.contains("**"),
            "{address} is not masked at all: {masked}"
        );

        for part in address.split('@') {
            let head = part.split('.').next().unwrap_or_default();
            if head.len() <= 2 {
                continue;
            }
            assert!(!masked.contains(head), "leaked {head:?} in {masked}");
        }
    }
}

/// The panel cannot shell out once per row, so the rule exists twice. A test is
/// cheaper than a refactor and catches the only thing that actually matters.
#[test]
fn the_cli_and_the_panel_mask_identically() {
    let Some(node) = node() else {
        eprintln!("skipped: node is not installed");
        return;
    };
    let s = Sandbox::new();
    let file = s.path("account-ui.js");
    std::fs::write(&file, bundle()).unwrap();

    for address in MASK_CASES {
        let from_cli = s.hats(&["mask", address]).ok().stdout.trim().to_string();
        let out = std::process::Command::new(node)
            .arg("-e")
            .arg(
                r#"var fs = require("fs");
var src = fs.readFileSync(process.argv[1], "utf8");
var fns = src.match(/function maskPart[\s\S]*?\n  }\n/)[0] +
          src.match(/function maskEmail[\s\S]*?\n  }\n/)[0];
eval(fns);
process.stdout.write(maskEmail(process.argv[2]));"#,
            )
            .arg(&file)
            .arg(address)
            .output()
            .expect("running the panel's masker");
        let from_panel = String::from_utf8_lossy(&out.stdout).to_string();
        assert_eq!(
            from_cli, from_panel,
            "the two maskers disagree on {address}"
        );
    }
}

#[test]
fn masking_is_opt_in_for_the_terminal() {
    let s = Sandbox::new();
    s.profile_with("claude", "work", "person@example.com");

    s.hats(&["list"]).says("person@example.com");
    s.hats(&["list", "--mask"])
        .says("pe**n@ex**e.com")
        .silent_about("person@example.com");
}
