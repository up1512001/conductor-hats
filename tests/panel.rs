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

/// The boot guard the binary carries, spliced ahead of Conductor's entry chunk.
fn guard() -> String {
    let s = Sandbox::new();
    s.hats(&["guard"]).ok().stdout.clone()
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

/// Both scripts are spliced into ES modules, where strict mode applies and
/// `node --check` alone does not: it parses its input as a script, which allows
/// what a module forbids.
#[test]
fn both_scripts_parse_as_modules() {
    let Some(node) = node() else {
        eprintln!("skipped: node is not installed");
        return;
    };
    for (name, text) in [("account-ui.mjs", bundle()), ("boot-guard.mjs", guard())] {
        let s = Sandbox::new();
        let file = s.path(name);
        std::fs::write(&file, text).unwrap();
        let out = std::process::Command::new(node)
            .arg("--check")
            .arg(&file)
            .output()
            .expect("node --check");
        assert!(
            out.status.success(),
            "{name} does not parse as a module:\n{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

/// Conductor 0.82 renders nothing while its minimum client version query is
/// unsettled, and in a patched copy it never settles. The guard answers that one
/// request so the check fails, which Conductor handles by carrying on.
#[test]
fn the_guard_answers_only_the_version_check() {
    let dist = guard();
    assert!(
        dist.contains("/minimum-client-version"),
        "the guard does not name the request it answers"
    );
    assert!(
        dist.contains("plugin:http|fetch"),
        "the guard does not watch the HTTP plugin"
    );
    assert!(
        dist.contains("ipc://"),
        "the guard would answer requests that are not Tauri commands"
    );
    assert!(
        !dist.contains("__conductorHats"),
        "the guard should carry none of the panel: it is injected separately"
    );
}

/// The panel offers one thing per level: providers, then that provider's
/// accounts. The segmented control that once sat between them is gone and stays
/// gone; what a choice applies to is said in words instead.
#[test]
fn the_panel_offers_no_scope_switch() {
    let dist = bundle();
    for gone in ["cma-scope", "cma-seg"] {
        assert!(
            !dist.contains(gone),
            "the panel still carries {gone:?}, which belongs to the removed scope switch"
        );
    }
}

/// The toolbar belongs to the chat it was pressed in. Choosing an account pins
/// that chat and writes no route, so the other chats in the workspace are left
/// where they are.
#[test]
fn choosing_an_account_sets_the_open_chat_alone() {
    let dist = bundle();
    assert!(
        dist.contains("pin ${profile} ${agent} ${session}"),
        "choosing an account does not pin the chat on screen"
    );
    assert!(
        dist.contains("function effective("),
        "the label does not read the account the chat will use"
    );
}

/// Nothing in the panel binds a repository any more.
///
/// A binding is one value for every workspace under it, so choosing Work while
/// creating one workspace and Personal while creating the next left both on
/// Personal, and moved every other workspace in that repository too. The
/// composer writes a one-shot instead, spent by the workspace it was chosen for.
#[test]
fn the_panel_never_binds_a_repository() {
    let dist = bundle();
    assert!(
        !dist.contains("bind ${profile}"),
        "the panel can still bind a repository"
    );
    assert!(
        dist.contains("next ${profile} ${agent}"),
        "the composer does not record a choice for the next workspace"
    );
}

/// With no chat and no workspace, the toolbar in the New Workspace view still
/// has something to mean: the workspace about to be made. It must reach that
/// through the one-shot and never through a repository binding, which would move
/// every workspace under it.
#[test]
fn the_new_workspace_view_offers_the_next_workspace_not_a_binding() {
    let dist = bundle();
    assert!(
        dist.contains("No chat here yet, so this applies to the workspace you create next."),
        "the panel does not say what a choice would do there"
    );
    assert!(
        !dist.contains("bind ${profile}"),
        "the panel can still bind a repository, which moves every workspace in it"
    );
    assert!(
        dist.contains("next ${profile}"),
        "the panel cannot set the account for the workspace being created"
    );
}

/// Setting every chat at once is still reachable, and it has to clear the open
/// chat's pin as well as write the route: a pin beats a route, so the route
/// alone would leave that chat behind.
#[test]
fn the_whole_workspace_action_clears_the_pin_too() {
    let dist = bundle();
    assert!(
        dist.contains("unpin ${agent} ${session}"),
        "the workspace-wide choice leaves the open chat pinned against it"
    );
    assert!(
        dist.contains("for every chat here"),
        "there is no way to set every chat in the workspace"
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
