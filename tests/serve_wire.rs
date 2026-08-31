//! What the socket puts on the wire, and what it refuses to take off it.
//!
//! Kept apart from `serve.rs`, which is about who is allowed to read the screen
//! at all. This is about the shape and size of what a permitted reader gets.

mod common;

/// The socket sends the sections that moved, not the whole world.
///
/// One stamp over everything meant `places::revision()`, which covers the
/// write-ahead log, resent the chat list and the open transcript on any
/// Conductor write anywhere on the machine.
#[test]
fn the_socket_sends_only_the_sections_that_changed() {
    let root = common::repo();
    let stamp = std::fs::read_to_string(root.join("src/rust/mobile_stamp.rs")).unwrap();
    let socket = std::fs::read_to_string(root.join("src/rust/mobile_socket.rs")).unwrap();
    let state = std::fs::read_to_string(root.join("src/rust/mobile_state.rs")).unwrap();
    let client = std::fs::read_to_string(root.join("src/mobile/socket.ts")).unwrap();
    for needle in [
        "pub chats: String",
        "pub active: String",
        "pub accounts: String",
        "revision == previous.revision && selected == previous.selected",
        "selected.and_then(id::session)",
    ] {
        assert!(stamp.contains(needle), "stamp is missing {needle:?}");
    }
    for needle in [
        "mobile_stamp::read(selected.as_deref(), &sections)",
        "next.chats != sections.chats",
        "next.active != sections.active",
    ] {
        assert!(socket.contains(needle), "socket is missing {needle:?}");
    }
    assert!(
        state.contains("skip_serializing_if = \"Option::is_none\""),
        "unchanged sections are still serialised"
    );
    assert!(
        client.contains("\"active\" in update"),
        "the client cannot tell an absent section from a null one"
    );
}

/// A section is recorded as delivered only once it is on the wire.
#[test]
fn a_dropped_send_does_not_mark_a_section_delivered() {
    let socket = std::fs::read_to_string(common::repo().join("src/rust/mobile_socket.rs")).unwrap();
    let body = socket
        .split("if want.any() {")
        .nth(1)
        .expect("the send block");
    let send = body.find("if !send(").expect("the send call");
    let record = body.find("sections = next;").expect("the section record");
    assert!(
        send < record,
        "sections are recorded before the snapshot is sent"
    );
}

/// Compression runs one way only.
///
/// The Mac compresses what it sends because a snapshot is JSON over a tunnel to
/// a phone. It must never decompress what it receives: that would let a paired
/// but hostile client hand over a small frame that expands into an enormous one,
/// and the incoming size caps would be measuring the wrong number.
#[test]
fn the_socket_compresses_outward_and_never_inward() {
    let socket = std::fs::read_to_string(common::repo().join("src/rust/mobile_socket.rs")).unwrap();
    let client = std::fs::read_to_string(common::repo().join("src/mobile/socket.ts")).unwrap();
    assert!(
        socket.contains("GzEncoder"),
        "outgoing frames are not compressed"
    );
    assert!(
        !socket.contains("GzDecoder") && !socket.contains("read::Gz"),
        "the Mac decompresses something a client sent"
    );
    assert!(
        socket.contains("max_message_size(Some(128 * 1024))"),
        "the incoming size cap was dropped"
    );
    assert!(
        client.contains("binaryType = \"arraybuffer\""),
        "the phone cannot receive a compressed frame"
    );
    assert!(
        client.contains("DecompressionStream(\"gzip\")"),
        "the phone does not decompress"
    );
}
