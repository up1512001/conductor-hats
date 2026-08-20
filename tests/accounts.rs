//! Signed-in state, and the one-account-one-profile rule.

mod common;

use common::Sandbox;

/// Read from the credentials, never from the cached address. A profile that had
/// working credentials but no address yet read as signed out for ever.
#[test]
fn signed_in_is_read_from_the_credentials_not_the_label() {
    let s = Sandbox::new();
    s.credentialed("claude", "quiet");

    s.hats(&["json", &s.workspace("ws-a")])
        .says(r#""name":"quiet","email":"","active":false,"signedIn":true"#);
    s.hats(&["list"])
        .says("(signed in, address not cached yet)");

    s.profile_with("claude", "loud", "loud@example.test");
    s.hats(&["json", &s.workspace("ws-a")])
        .says(r#""email":"loud@example.test""#);
}

#[test]
fn a_profile_with_nothing_is_not_signed_in() {
    let s = Sandbox::new();
    s.bare("claude", "empty");

    s.hats(&["json", &s.workspace("ws-a")])
        .says(r#""name":"empty","email":"","active":false,"signedIn":false"#);
    s.hats(&["doctor"]).says("profile 'empty' is not signed in");
}

/// Two profiles on one account is not two accounts: the provider keeps one live
/// token per account, so the second sign-in revokes the first and the pair take
/// turns logging each other out. The symptom is an account asking to sign in
/// minutes after it did.
#[test]
fn two_profiles_on_one_address_are_flagged() {
    let s = Sandbox::new();
    s.profile_with("claude", "hello", "same@example.test");
    s.profile_with("claude", "personal", "same@example.test");
    s.profile_with("claude", "work", "other@example.test");

    s.hats(&["doctor"])
        .says("share the address same@example.test")
        .says("sign each other out")
        .silent_about("other@example.test");
}

/// The README says any number of accounts, so the claim gets a test rather than
/// a hope. Nothing in the design caps it: profiles are directories, routes are
/// lines in a file, and each config directory gets its own keychain item. Five
/// is more than anyone has asked for and enough to catch a two-account
/// assumption.
#[test]
fn any_number_of_accounts_route_independently() {
    let s = Sandbox::new();
    let names = ["alpha", "bravo", "charlie", "delta", "echo"];
    for name in names {
        s.profile_with("claude", name, &format!("{name}@example.test"));
        s.hats(&["use", name, "claude", &s.workspace(&format!("ws-{name}"))])
            .ok();
    }

    for name in names {
        let want = s
            .accounts()
            .join("claude")
            .join(name)
            .to_string_lossy()
            .to_string();
        assert_eq!(
            s.route("claude", &format!("ws-{name}"), &[]),
            want,
            "ws-{name}"
        );
    }

    let listed = s.hats(&["json", &s.workspace("ws-alpha")]);
    for name in names {
        listed.says(&format!(r#""name":"{name}""#));
    }

    let routes = s.read("accounts/routes");
    let lines = routes.lines().filter(|l| l.contains("/ws-")).count();
    assert_eq!(lines, 5, "one route each:\n{routes}");
}
