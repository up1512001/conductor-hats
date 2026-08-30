//! The injected bridge sends and changes state through Conductor's visible UI.

mod common;

fn source() -> String {
    std::fs::read_to_string(common::repo().join("src/panel/remote.ts")).unwrap()
}

fn controls() -> String {
    std::fs::read_to_string(common::repo().join("src/panel/controls.ts")).unwrap()
}

fn must(body: &str, needles: &[&str], feature: &str) {
    for needle in needles {
        assert!(body.contains(needle), "{feature} is missing {needle:?}");
    }
}

#[test]
fn remote_messages_use_conductors_real_composer() {
    must(
        &source(),
        &[
            "data-testid=\"composer-input\"",
            "data-testid=\"composer-send-button\"",
            "document.execCommand(\"insertText\"",
            "live.send.click()",
            "remote confirm ",
        ],
        "remote composer delivery",
    );
}

#[test]
fn remote_delivery_preserves_a_laptop_draft() {
    must(
        &source(),
        &[
            "!empty(ui.editor)",
            "!empty(live.editor)",
            "remote release ",
            "still.session !== item.session",
        ],
        "remote draft protection",
    );
}

#[test]
fn remote_delivery_opens_the_exact_project_workspace_and_chat() {
    must(
        &source(),
        &[
            "remote next",
            "q(scopeWorkspace)",
            "current.workspace !== scopeWorkspace",
            "route.repository_id",
            "route.workspace_id",
            "activeTabType=session&sessionId=",
            "history.pushState",
            "PopStateEvent",
            "ui && !empty(ui.editor)",
        ],
        "remote chat navigation",
    );
}

#[test]
fn remote_run_settings_use_conductors_visible_controls() {
    must(
        &controls(),
        &[
            "controlScope()",
            "button.click()",
            "choice.click()",
            "remote control-",
            "remote control-check",
        ],
        "remote run settings",
    );
}

/// A queued model change has to reach Conductor before the message sent with it.
///
/// The panel used to make that true by calling two commands in order, which only
/// held as long as nobody reordered them. One command answers now, and it is the
/// Rust side that refuses to hand over a message while a run setting is waiting.
#[test]
fn remote_run_settings_are_applied_before_the_queued_message() {
    let cli = std::fs::read_to_string(common::repo().join("src/rust/remote_cli.rs")).unwrap();
    let take = cli.find("\"take\" =>").expect("the take command");
    let body = &cli[take..];
    let control = body.find("remote_control::claim").expect("control claim");
    let message = body.find("remote::claim").expect("message claim");
    assert!(
        control < message,
        "the message can outrun its model settings"
    );
    assert!(
        body[control..message].contains("Some(_) => None"),
        "a message is handed over while a run setting is still waiting"
    );
    assert!(
        source().contains("remote take "),
        "the panel no longer asks for both in one call"
    );
}

#[test]
fn remote_new_chats_use_conductors_own_action() {
    must(
        &source(),
        &[
            "newChatButton()",
            "new chat(?:, same files)?",
            "metaKey: true",
            "remote create-check ",
            "createCommand(\"complete\"",
            "createCommand(\"release\"",
        ],
        "remote new-chat creation",
    );
}
