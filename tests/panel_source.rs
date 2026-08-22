//! Structure and style rules for the panel, read from the source it is built
//! from. Each one was a visible defect before it was a rule.

mod common;

use std::path::Path;

fn read_all(dir: &Path, extension: &str) -> String {
    let mut out = String::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    let mut paths: Vec<_> = entries.flatten().map(|e| e.path()).collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            out.push_str(&read_all(&path, extension));
        } else if path.extension().map(|e| e == extension).unwrap_or(false) {
            out.push_str(&std::fs::read_to_string(&path).unwrap_or_default());
            out.push('\n');
        }
    }
    out
}

fn source() -> String {
    read_all(&common::repo().join("src/panel"), "ts")
}

fn styles() -> String {
    read_all(&common::repo().join("src/panel"), "scss")
}

fn must(body: &str, needles: &[&str], what: &str) {
    for needle in needles {
        assert!(body.contains(needle), "{what}: missing {needle:?}");
    }
}

/// Sealing pointer events on the capture phase stopped the click before it ever
/// reached the row that was clicked: every account row went inert and the panel
/// stopped opening at all. The seal has to be on the bubble phase, after the
/// panel's own handlers have run.
#[test]
fn the_panel_seals_pointer_events_on_the_bubble_phase() {
    let file = std::fs::read_to_string(common::repo().join("src/panel/attach.ts")).unwrap();
    let body: String = file
        .lines()
        .skip_while(|l| !l.starts_with("export function seal("))
        .take_while(|l| !l.starts_with('}'))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        body.contains("false);"),
        "the listener must be non-capturing:\n{body}"
    );
    assert!(
        !body.contains("true)"),
        "nothing in seal may capture:\n{body}"
    );
}

#[test]
fn every_clickable_thing_says_it_is_clickable() {
    let css = styles();
    must(
        &css,
        &[
            ".cma-btn",
            ".cma-chip",
            ".cma-card",
            ".cma-signout",
            ".cma-back",
            ".cma-add",
            ".cma-go",
            ".cma-act",
        ],
        "a clickable class is missing",
    );

    let defaults = css
        .lines()
        .filter(|l| l.contains("cursor: default"))
        .count();
    let excused = css
        .lines()
        .collect::<Vec<_>>()
        .windows(4)
        .filter(|w| {
            w[3].contains("cursor: default")
                && w.iter()
                    .any(|l| l.contains(":disabled") || l.contains("cma-ghost"))
        })
        .count();
    assert_eq!(
        defaults, excused,
        "the arrow cursor belongs only on unclickable things"
    );
}

/// Nothing under the pointer may move once the panel is open. Four things make
/// that true, and each one was a jump before it was there.
#[test]
fn the_panel_cannot_shift_once_it_is_open() {
    must(
        &source(),
        &[
            "if (panel && panel.pos)",
            "\"cma-tickslot\"",
            "btn.hidden = true",
        ],
        "layout stability",
    );
    must(
        &styles(),
        &[
            "width: 300px",
            "overflow-y: auto",
            "flex: 1",
            ".cma-tickslot",
        ],
        "layout stability",
    );
}

/// A dot next to a label is decoration standing in for a word. The label says it.
#[test]
fn there_are_no_status_dots() {
    assert!(!source().contains("cma-dot"), "no dot element");
    assert!(!styles().contains("cma-dot"), "no dot rule");
}

/// Profile names are lower case on disk and capitalised only for display.
#[test]
fn display_names_are_capitalised_without_touching_the_cli() {
    let src = source();
    must(
        &src,
        &["export function cap(", "cap(account.name)", "cap(cur)"],
        "display casing",
    );
    assert_eq!(
        src.matches("applyAccount(state, provider.agent, account.name, scope)")
            .count(),
        1,
        "writes must send the raw name"
    );
    assert_eq!(
        src.matches("logout ${account.name}").count(),
        1,
        "as must sign-out"
    );
}

/// The wireframe is a drill-down: providers first, then that provider's accounts
/// with a sign-out each and one "Add new account" at the foot.
#[test]
fn the_panel_is_a_two_level_drill_down() {
    must(
        &source(),
        &[
            "export function rootView(",
            "export function providerView(",
            "el(\"button\", \"cma-back\")",
            "\"Add new account\"",
            "export function confirmSignOut(",
            "panel.view.level === \"provider\"",
        ],
        "the drill-down",
    );
}

/// The panel signs an account out and touches nothing else. Deleting a profile
/// outright stays in the terminal, where an accidental click cannot reach it.
#[test]
fn the_panel_signs_out_and_deletes_nothing() {
    let src = source();
    must(
        &src,
        &[
            "acct(`logout ${account.name}",
            "Nothing else changes",
            "sessions and transcripts are untouched",
            "icon(\"signout\"",
            "if (account.signedIn) {",
            "icon(\"signin\"",
        ],
        "sign-out semantics",
    );
    assert!(
        !src.contains("acct(`remove "),
        "the panel must never remove a profile"
    );
    assert!(!src.contains("trash:"), "no bin glyph");
}

/// Sign-out costs a browser round trip to undo, so it asks in a dialog with a
/// scrim rather than a control that arms on a first click.
#[test]
fn sign_out_asks_in_a_dialog() {
    must(
        &source(),
        &[
            "export function dialog(",
            "\"alertdialog\"",
            "if (e.key === \"Escape\") {",
            "if (e.target === scrim) shut()",
            "\"Signs \"",
            "if (!panel || openDialog()) return",
        ],
        "the confirmation dialog",
    );
    must(&styles(), &[".cma-scrim"], "the confirmation dialog");
}

/// The control lives inside the row's border, divided from the selectable area,
/// rather than floating in the gutter beside it.
#[test]
fn sign_out_sits_inside_the_row() {
    must(
        &styles(),
        &[
            ".cma-row2",
            "border-left: 1px solid var(--border)",
            "align-self: stretch",
        ],
        "the row layout",
    );
}

#[test]
fn the_panel_never_renders_a_full_address() {
    let src = source();
    must(
        &src,
        &[
            "maskEmail(account.email)",
            "account.email ? maskEmail(account.email) : cap(account.name)",
            "\"Signed in as \" + maskEmail(email)",
        ],
        "masking",
    );
    assert!(
        !src.lines()
            .any(|l| l.contains("title =") && l.contains("account.email")),
        "no address in a title attribute"
    );
}
