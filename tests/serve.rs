//! Who is allowed to read the screen.
//!
//! A tunnel is transport, not authorisation. Cloudflare Access in front is the
//! plan, and a policy that quietly stops enforcing is a thing that happens, so
//! this holds a lock of its own.

mod common;

use common::Sandbox;

fn assert_pairing_link(link: &str, origin: &str) {
    let (address, token) = link.trim().split_once("#token=").expect("fragment token");
    let route = address
        .strip_prefix(&format!("{origin}/"))
        .expect("pairing origin and path");
    assert_eq!(route.len(), 64, "expected a 64-character route: {link}");
    assert!(
        route.chars().all(|c| c.is_ascii_hexdigit()),
        "route: {route}"
    );
    assert_eq!(token.len(), 64, "expected a 64-character token: {link}");
    assert!(
        token.chars().all(|c| c.is_ascii_hexdigit()),
        "token: {token}"
    );
}

#[test]
fn a_secret_is_made_once_and_kept() {
    let s = Sandbox::new();
    s.hats(&["install"]).ok();

    let first = s
        .hats(&["serve", "--pair", "--origin", "https://phone.example.com"])
        .ok()
        .out();
    let again = s
        .hats(&["serve", "--pair", "--origin", "https://phone.example.com"])
        .ok()
        .out();
    assert_eq!(first, again, "the secret changed between runs");

    assert_pairing_link(&first, "https://phone.example.com");
}

/// Readable by its owner and nobody else on the machine.
#[test]
fn the_secret_is_not_world_readable() {
    use std::os::unix::fs::PermissionsExt;

    let s = Sandbox::new();
    s.hats(&["install"]).ok();
    s.hats(&["serve", "--pair", "--origin", "https://phone.example.com"])
        .ok();

    let mode = std::fs::metadata(s.accounts().join("serve-pairing"))
        .expect("the secret file")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o600, "secret is mode {mode:o}");
}

/// The secret belongs after the `#`. In a query string it reaches the server it
/// authenticates to, every proxy in between, and their logs.
#[test]
fn the_pairing_link_carries_the_secret_in_the_fragment() {
    let s = Sandbox::new();
    s.hats(&["install"]).ok();

    let url = s
        .hats(&["serve", "--pair", "--origin", "https://phone.example.com"])
        .ok()
        .out();
    let link = url.lines().last().unwrap_or_default().trim();
    assert_pairing_link(link, "https://phone.example.com");
    assert!(!link.contains('?'), "the secret is in the query: {link}");
}

#[test]
fn a_plain_http_public_origin_is_refused() {
    let s = Sandbox::new();
    let run = s.hats(&["serve", "--pair", "--origin", "http://phone.example.com"]);
    run.failed().says("must start with https://");
}

#[test]
fn a_loopback_address_is_never_turned_into_a_phone_qr() {
    let s = Sandbox::new();
    s.hats(&["remote", "mobile-origin", "https://localhost:8787"])
        .failed()
        .says("reachable from the phone");
}

#[test]
fn the_panel_can_save_an_origin_and_create_a_pairing_link() {
    let s = Sandbox::new();
    let saved = s
        .hats(&["remote", "mobile-origin", "https://phone.example.com"])
        .ok()
        .json();
    assert_eq!(saved["origin"], "https://phone.example.com");

    let pairing_url = s
        .hats(&["serve", "--pair", "--origin", "https://phone.example.com"])
        .ok()
        .out();
    let pairing = s.hats(&["remote", "mobile-status"]).ok().json();
    assert!(
        pairing["pairing"]["url"]
            .as_str()
            .unwrap_or("")
            .starts_with("https://phone.example.com/"),
        "wrong pairing URL: {pairing}"
    );
    assert_eq!(pairing["pairing"]["path"].as_str().unwrap_or("").len(), 65);
    assert_pairing_link(
        pairing["pairing"]["url"].as_str().unwrap_or(""),
        "https://phone.example.com",
    );
    assert_eq!(
        pairing["pairing"]["url"].as_str().unwrap_or(""),
        pairing_url.trim()
    );
    assert!(pairing["pairing"]["expires_at"].as_u64().unwrap_or(0) > 0);
    assert_eq!(pairing["service"]["running"], false);
    assert_eq!(pairing["service"]["address"], "127.0.0.1:8787");

    let status = s.hats(&["remote", "mobile-status"]).ok().json();
    assert_eq!(status["origin"], "https://phone.example.com");
    assert_eq!(status["pairing"]["url"], pairing["pairing"]["url"]);
}

#[cfg(unix)]
#[test]
fn the_saved_public_origin_is_owner_only() {
    use std::os::unix::fs::PermissionsExt;

    let s = Sandbox::new();
    s.hats(&["remote", "mobile-origin", "https://phone.example.com"])
        .ok();
    let mode = std::fs::metadata(s.accounts().join("serve-origin"))
        .unwrap()
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o600);
}

#[test]
fn a_saved_origin_is_used_by_the_public_serve_commands() {
    let s = Sandbox::new();
    s.hats(&["remote", "mobile-origin", "https://phone.example.com"])
        .ok();
    let link = s.hats(&["serve", "--pair"]).ok().out();
    assert_pairing_link(&link, "https://phone.example.com");
}

#[test]
fn only_the_active_random_path_opens_the_pairing_page() {
    let source = std::fs::read_to_string(common::repo().join("src/rust/serve.rs")).unwrap();
    assert!(source.contains("auth::is_pairing_path(path)"));
    assert!(source.contains("auth::route_matches(path) || allowed(&request)"));
}

#[test]
fn revoking_changes_both_browser_and_pairing_secrets() {
    let s = Sandbox::new();
    let args = ["serve", "--revoke", "--origin", "https://phone.example.com"];
    let first = s.hats(&args).ok().out();
    let before = std::fs::read_to_string(s.accounts().join("serve-session")).unwrap_or_default();

    let second = s
        .hats(&["serve", "--revoke", "--origin", "https://phone.example.com"])
        .ok()
        .out();
    let after = std::fs::read_to_string(s.accounts().join("serve-session")).unwrap_or_default();
    assert_ne!(first, second, "the pairing token survived revocation");
    assert_ne!(before, after, "the browser session survived revocation");
}

#[test]
fn a_live_socket_rechecks_the_session_so_revoke_disconnects_it() {
    let body = std::fs::read_to_string(common::repo().join("src/rust/mobile_socket.rs")).unwrap();
    assert!(body.contains("auth::session()"));
    assert!(body.contains("auth::same(&credential, &expected)"));
    assert!(body.contains("Message::Ping"));
    assert!(body.contains("header(request, \"origin\")"));
    assert!(body.contains("max_message_size(Some(128 * 1024))"));
}

#[test]
fn the_mobile_client_is_typed_websocket_only_and_csp_clean() {
    let root = common::repo();
    let app = std::fs::read_to_string(root.join("src/mobile/app.ts")).unwrap();
    let socket = std::fs::read_to_string(root.join("src/mobile/socket.ts")).unwrap();
    let render = std::fs::read_to_string(root.join("src/mobile/render.ts")).unwrap();
    let page = std::fs::read_to_string(root.join("src/mobile/index.html")).unwrap();
    let http = std::fs::read_to_string(root.join("src/rust/http.rs")).unwrap();
    assert!(socket.contains("new WebSocket(address())"));
    assert!(!app.contains("EventSource"));
    assert!(!app.contains("/api/chats"));
    assert!(!render.contains("style="));
    assert!(!app.contains(".style."));
    assert!(page.contains("/logo.png"), "the icon is not same-origin");
    assert!(
        !page.contains("http://") && !page.contains("https://"),
        "the page fetches something off-origin"
    );
    assert!(page.contains("/mobile.js"));
    assert!(http.contains("private, no-store, no-transform"));
    assert!(http.contains("script-src 'self'"));
    assert!(!root.join("src/mobile/app.js").exists());
}

#[test]
fn one_websocket_carries_updates_and_commands_in_both_directions() {
    let root = common::repo();
    let socket = std::fs::read_to_string(root.join("src/rust/mobile_socket.rs")).unwrap();
    let panel = std::fs::read_to_string(root.join("src/panel/remote.ts")).unwrap();
    let app = std::fs::read_to_string(root.join("src/mobile/app.ts")).unwrap();
    for needle in [
        "mobile_state::snapshot",
        "send(&mut socket, &snapshot)",
        "\"send\" =>",
        "remote::enqueue(chat, message)",
        "\"accepted\"",
    ] {
        assert!(socket.contains(needle), "socket is missing {needle:?}");
    }
    for needle in ["live.send.click()", "remote confirm "] {
        assert!(
            panel.contains(needle),
            "desktop bridge is missing {needle:?}"
        );
    }
    assert!(app.contains("transport.send({ type: \"send\""));
    assert!(app.contains("snapshot(value: MobileSnapshot)"));
    /* Change detection moved out of mobile_state when the snapshot was split
     * into sections, but it is still the cheap filesystem revision that decides
     * whether the database is worth asking at all. */
    let stamp = std::fs::read_to_string(root.join("src/rust/mobile_stamp.rs")).unwrap();
    assert!(stamp.contains("places::revision()"));
}
