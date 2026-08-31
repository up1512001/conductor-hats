//! What the request parser accepts off the wire, and what it must still refuse.
//!
//! A Cloudflare Tunnel forwards a body-less POST to the origin as `chunked`
//! with no `Content-Length`. Refusing every transfer coding refused every phone
//! that paired through the tunnel, while loopback kept working, because a
//! direct client sends `Content-Length: 0` instead.

use std::io::Write;
use std::net::{TcpListener, TcpStream};

/// Feeds one raw request to the parser exactly as a socket would deliver it.
fn parse(raw: &str) -> Option<super::Request> {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback listener");
    let address = listener.local_addr().expect("its address");
    let raw = raw.to_string();
    let writer = std::thread::spawn(move || {
        let mut client = TcpStream::connect(address).expect("connecting");
        let _ = client.write_all(raw.as_bytes());
        let _ = client.flush();
        std::thread::sleep(std::time::Duration::from_millis(50));
    });
    let (server, _) = listener.accept().expect("accepting");
    let parsed = super::read(&server);
    let _ = writer.join();
    parsed
}

#[test]
fn a_chunked_body_is_read() {
    let parsed = parse(
        "POST /api/send HTTP/1.1\r\nHost: phone.example.com\r\n\
         Transfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n2\r\n, \r\n5\r\nworld\r\n0\r\n\r\n",
    )
    .expect("a chunked request is accepted");
    assert_eq!(parsed.method, "POST");
    assert_eq!(parsed.path, "/api/send");
    assert_eq!(String::from_utf8_lossy(&parsed.body), "hello, world");
}

/// The exact shape the tunnel sends when the page pairs.
#[test]
fn a_body_less_chunked_post_is_accepted() {
    let parsed = parse(
        "POST /api/pair HTTP/1.1\r\nHost: phone.example.com\r\n\
         X-Hats-Token: abc\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\n",
    )
    .expect("the tunnel's pairing request is accepted");
    assert_eq!(parsed.path, "/api/pair");
    assert!(parsed.body.is_empty());
    assert_eq!(
        parsed.headers.get("x-hats-token").map(String::as_str),
        Some("abc")
    );
}

/// Both framings at once is the disagreement a smuggler needs.
#[test]
fn a_request_carrying_both_framings_is_refused() {
    assert!(parse(
        "POST /api/send HTTP/1.1\r\nHost: phone.example.com\r\n\
         Content-Length: 5\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\n",
    )
    .is_none());
}

#[test]
fn a_transfer_coding_this_does_not_implement_is_refused() {
    assert!(parse(
        "POST /api/send HTTP/1.1\r\nHost: phone.example.com\r\n\
         Transfer-Encoding: gzip, chunked\r\n\r\n0\r\n\r\n",
    )
    .is_none());
}

#[test]
fn a_chunked_body_over_the_ceiling_is_refused() {
    let size = super::MAX_BODY + 1;
    let raw = format!(
        "POST /api/send HTTP/1.1\r\nHost: phone.example.com\r\n\
         Transfer-Encoding: chunked\r\n\r\n{size:x}\r\n"
    );
    assert!(parse(&raw).is_none());
}

#[test]
fn a_declared_length_still_works() {
    let parsed = parse(
        "POST /api/send HTTP/1.1\r\nHost: phone.example.com\r\n\
         Content-Length: 2\r\n\r\nhi",
    )
    .expect("a declared length is accepted");
    assert_eq!(String::from_utf8_lossy(&parsed.body), "hi");
}
