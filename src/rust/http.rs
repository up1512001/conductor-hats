//! Just enough HTTP/1.1 to serve one screen to one phone.
//!
//! Written on `std::net` rather than pulled from a crate. hats has three
//! dependencies and the reason is a binary that needs no C toolchain and
//! cross-compiles cleanly; an async runtime and a web framework would be two
//! hundred more crates for GET, Server-Sent Events, and nothing else.
//!
//! Server-Sent Events rather than websockets, for the same reason and one more:
//! a browser cannot set headers on a websocket handshake, so a token has to go
//! in the URL or be exchanged for a ticket first. An event stream is a GET.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;

pub struct Request {
    pub method: String,
    pub path: String,
    pub query: String,
}

/// Reads the request line and drains the headers.
///
/// Everything after the first line is discarded: this serves a fixed set of
/// paths to one client and has no use for the rest. The headers are read anyway
/// so the socket is left where a response belongs.
pub fn read(stream: &TcpStream) -> Option<Request> {
    let mut reader = BufReader::new(stream.try_clone().ok()?);
    let mut line = String::new();
    if reader.read_line(&mut line).ok()? == 0 {
        return None;
    }
    let mut parts = line.split_whitespace();
    let method = parts.next()?.to_string();
    let target = parts.next()?.to_string();

    loop {
        let mut header = String::new();
        match reader.read_line(&mut header) {
            Ok(0) => break,
            Ok(_) if header.trim().is_empty() => break,
            Ok(_) => continue,
            Err(_) => return None,
        }
    }

    let (path, query) = match target.split_once('?') {
        Some((p, q)) => (p.to_string(), q.to_string()),
        None => (target, String::new()),
    };
    Some(Request {
        method,
        path,
        query,
    })
}

/// One query parameter, percent-decoded.
pub fn param(query: &str, want: &str) -> Option<String> {
    query.split('&').find_map(|pair| {
        let (key, value) = pair.split_once('=')?;
        (key == want).then(|| decode(value))
    })
}

fn decode(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("");
                match u8::from_str_radix(hex, 16) {
                    Ok(byte) => {
                        out.push(byte);
                        i += 3;
                    }
                    Err(_) => {
                        out.push(bytes[i]);
                        i += 1;
                    }
                }
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            byte => {
                out.push(byte);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).to_string()
}

/// Headers sent with everything.
///
/// `no-store` because every answer here is a live reading of Conductor's state.
/// The frame and sniffing headers cost nothing and close the two ways a page
/// like this is usually turned against its owner.
const COMMON: &str = "Cache-Control: no-store\r\n\
     X-Content-Type-Options: nosniff\r\n\
     X-Frame-Options: DENY\r\n\
     Referrer-Policy: no-referrer\r\n";

pub fn send(stream: &mut TcpStream, status: &str, kind: &str, body: &[u8]) {
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\n{COMMON}\
         Connection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(body);
    let _ = stream.flush();
}

pub fn json(stream: &mut TcpStream, body: &str) {
    send(stream, "200 OK", "application/json", body.as_bytes());
}

pub fn not_found(stream: &mut TcpStream) {
    send(
        stream,
        "404 Not Found",
        "application/json",
        b"{\"error\":\"not found\"}",
    );
}

/// Opens an event stream and hands back the socket to write events on.
pub fn open_stream(stream: &mut TcpStream) -> bool {
    let head = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\n{COMMON}\
         Connection: keep-alive\r\n\r\n"
    );
    stream.write_all(head.as_bytes()).is_ok()
}

/// One event. Returns false once the phone has gone away, which is how the
/// thread serving it learns to stop.
pub fn event(stream: &mut TcpStream, data: &str) -> bool {
    let frame = format!("data: {data}\n\n");
    stream.write_all(frame.as_bytes()).is_ok() && stream.flush().is_ok()
}
