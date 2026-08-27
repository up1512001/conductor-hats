//! What a choice in the panel actually writes.
//!
//! Read from the built bundle rather than the source, because the bundle is what
//! is injected: a fix that never reached it is the failure this repository has
//! already had twice.

mod common;

use common::repo;

fn bundle() -> String {
    std::fs::read_to_string(repo().join("dist/account-ui.js")).expect("run `pnpm build` first")
}

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

/// The toolbar names the account and shows whose it is. Inside the panel the
/// heading already says which provider, so a mark on every row was noise.
#[test]
fn the_toolbar_shows_the_provider_beside_the_account() {
    let dist = bundle();
    assert!(
        dist.contains("cma-mark"),
        "the toolbar carries no provider mark"
    );
    assert!(
        dist.contains("agentShowing"),
        "nothing decides which provider the label is naming"
    );
}

/// Switching chats must not cost a `resolve` spawn: the workspace has not
/// changed, and two process spawns between the switch and the new label is what
/// made it feel slow.
#[test]
fn a_workspace_is_resolved_once_and_remembered() {
    let dist = bundle();
    assert!(
        dist.contains("resolve "),
        "the panel no longer resolves a workspace by id"
    );
    assert!(
        dist.contains("new Map"),
        "the resolved workspace is not kept between reads"
    );
}

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
