//! What a partial snapshot puts on the wire.
//!
//! Absent and null mean different things here. A section left out is one the
//! phone already holds, while `active: null` means no chat is open. Collapsing
//! the two would either strand the phone on a chat it has closed, or make it
//! throw away a transcript it was still reading.

use super::{snapshot, Want};

fn body(want: Want, selected: Option<&str>) -> serde_json::Value {
    serde_json::from_str(&snapshot(selected, want, "stamp".into()).expect("a snapshot"))
        .expect("valid JSON")
}

#[test]
fn an_unchanged_section_is_absent_rather_than_empty() {
    let quiet = body(
        Want {
            chats: false,
            active: false,
            accounts: false,
        },
        None,
    );
    let map = quiet.as_object().expect("an object");
    for section in ["chats", "active", "accounts", "models"] {
        assert!(!map.contains_key(section), "{section} was sent unchanged");
    }
    assert_eq!(quiet["type"], "snapshot");
    assert_eq!(quiet["stamp"], "stamp");
}

#[test]
fn no_open_chat_is_a_null_active_not_an_absent_one() {
    let open = body(
        Want {
            chats: false,
            active: true,
            accounts: false,
        },
        None,
    );
    let map = open.as_object().expect("an object");
    assert!(map.contains_key("active"), "active was dropped entirely");
    assert!(open["active"].is_null(), "active should be null: {open}");
}

/// A chat id the phone made up must not reach the transcript reader.
#[test]
fn a_rejected_session_id_yields_no_active_chat() {
    let hostile = body(
        Want {
            chats: false,
            active: true,
            accounts: false,
        },
        Some("'; drop table sessions --"),
    );
    assert!(hostile["active"].is_null(), "{hostile}");
}
