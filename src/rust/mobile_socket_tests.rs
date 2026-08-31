//! What goes on the wire when a snapshot is too big to send as text.

use super::{packed, PACK_OVER};
use std::io::Read;

fn unpacked(body: &[u8]) -> String {
    let mut out = String::new();
    flate2::read::GzDecoder::new(body)
        .read_to_string(&mut out)
        .expect("a gzip stream the browser could also read");
    out
}

#[test]
fn a_small_frame_stays_text() {
    assert!(packed("{\"type\":\"accepted\"}").is_none());
    assert!(packed(&"x".repeat(PACK_OVER - 1)).is_none());
}

#[test]
fn a_large_frame_round_trips_through_gzip() {
    let body = format!(
        "{{\"type\":\"snapshot\",\"chats\":\"{}\"}}",
        "ab".repeat(PACK_OVER)
    );
    let sent = packed(&body).expect("a large frame is compressed");
    assert!(
        sent.len() < body.len() / 2,
        "compression gained nothing: {} -> {}",
        body.len(),
        sent.len()
    );
    assert_eq!(
        unpacked(&sent),
        body,
        "the phone would decode something else"
    );
}

/// Real transcripts carry unicode. A byte-sliced frame would decode to mojibake
/// or fail outright on the phone.
#[test]
fn a_frame_of_multibyte_text_survives() {
    let body = "«思考» ".repeat(PACK_OVER);
    let sent = packed(&body).expect("a large frame is compressed");
    assert_eq!(unpacked(&sent), body);
}
