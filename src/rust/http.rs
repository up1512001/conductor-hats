//! Just enough HTTP/1.1 to serve one screen to one phone.
//!
//! Written on `std::net` rather than pulled from a framework. hats keeps a few
//! pure-Rust dependencies so the binary needs no C toolchain and
//! cross-compiles cleanly. The private cookie exchanged during pairing also
//! authenticates a same-origin WebSocket upgrade without exposing a credential
//! in its URL.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;

const MAX_HEAD: usize = 32 * 1024;
const MAX_BODY: usize = 64 * 1024;

pub struct Request {
    pub method: String,
    pub path: String,
    pub query: String,
    /// Lower-cased header names to values. Only two matter here, the cookie and
    /// the one the page pairs with, but keeping them all costs nothing.
    pub headers: std::collections::HashMap<String, String>,
    pub body: Vec<u8>,
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
    if line.len() > MAX_HEAD {
        return None;
    }
    let mut parts = line.split_whitespace();
    let method = parts.next()?.to_string();
    let target = parts.next()?.to_string();

    let mut headers = std::collections::HashMap::new();
    let mut head_size = line.len();
    loop {
        let mut header = String::new();
        match reader.read_line(&mut header) {
            Ok(0) => break,
            Ok(_) if header.trim().is_empty() => break,
            Ok(_) => {
                head_size += header.len();
                if head_size > MAX_HEAD {
                    return None;
                }
                if let Some((name, value)) = header.split_once(':') {
                    headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_string());
                }
            }
            Err(_) => return None,
        }
    }

    if headers.contains_key("transfer-encoding") {
        return None;
    }
    let length = headers
        .get("content-length")
        .map(|value| value.parse::<usize>().ok())
        .unwrap_or(Some(0))?;
    if length > MAX_BODY {
        return None;
    }
    let mut body = vec![0u8; length];
    reader.read_exact(&mut body).ok()?;

    let (path, query) = match target.split_once('?') {
        Some((p, q)) => (p.to_string(), q.to_string()),
        None => (target, String::new()),
    };
    Some(Request {
        method,
        path,
        query,
        headers,
        body,
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
const COMMON: &str = "Cache-Control: private, no-store, no-transform\r\n\
     X-Content-Type-Options: nosniff\r\n\
     X-Frame-Options: DENY\r\n\
     Referrer-Policy: no-referrer\r\n\
     Cross-Origin-Opener-Policy: same-origin\r\n\
     Cross-Origin-Resource-Policy: same-origin\r\n\
     Permissions-Policy: camera=(), microphone=(), geolocation=()\r\n\
     Content-Security-Policy: default-src 'self'; connect-src 'self'; img-src 'self' data:; script-src 'self'; style-src 'self'; base-uri 'none'; frame-ancestors 'none'; form-action 'none'\r\n";

pub fn send(stream: &mut TcpStream, status: &str, kind: &str, body: &[u8]) {
    send_with(stream, status, kind, body, "");
}

/// The same, with one extra header. Used for `Set-Cookie`, which is the only
/// thing here that varies per response.
pub fn send_with(stream: &mut TcpStream, status: &str, kind: &str, body: &[u8], extra: &str) {
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\n{COMMON}{extra}\
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

/// Completes an authenticated WebSocket handshake on the existing socket.
pub fn websocket(stream: &mut TcpStream, accept: &str) -> bool {
    let head = format!(
        "HTTP/1.1 101 Switching Protocols\r\n\
         Upgrade: websocket\r\nConnection: Upgrade\r\n\
         Sec-WebSocket-Accept: {accept}\r\n{COMMON}\r\n"
    );
    stream.write_all(head.as_bytes()).is_ok() && stream.flush().is_ok()
}
