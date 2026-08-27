//! The screen a phone reads, served from this machine.
//!
//! Loopback by default and on purpose. The way out is a tunnel that connects
//! outward and authenticates at the edge, not a port opened on a laptop that
//! moves between networks. `--host` exists for trying it on a LAN and says so.
//!
//! Read-only. Every route here answers a question about Conductor's state and
//! none of them change it, which is what makes the first version of this safe
//! enough to leave running.

use std::net::TcpListener;

use crate::{chats, http, places, store, transcript};

const PAGE: &str = include_str!("../mobile/index.html");

/// The cheapest question that changes when anything else does.
///
/// `pragma data_version` would be the right primitive, and it is unavailable:
/// it only moves on a long-lived connection, and every query here is a fresh
/// `sqlite3`. Linking a SQLite library to get one would mean a C dependency.
/// Measured, this costs about 5.5 ms including the process start.
const PROBE: &str = "select (select max(rowid) from session_messages) || ':' || \
     (select count(*) from sessions where status is not null and status != 'idle')";

fn probe() -> String {
    places::rows(PROBE).first().cloned().unwrap_or_default()
}

fn handle(mut stream: std::net::TcpStream) {
    let Some(request) = http::read(&stream) else {
        return;
    };
    if request.method != "GET" {
        http::send(
            &mut stream,
            "405 Method Not Allowed",
            "application/json",
            b"{\"error\":\"read only\"}",
        );
        return;
    }

    match request.path.as_str() {
        "/" => http::send(
            &mut stream,
            "200 OK",
            "text/html; charset=utf-8",
            PAGE.as_bytes(),
        ),
        "/api/chats" => match chats::json_string() {
            Ok(body) => http::json(&mut stream, &body),
            Err(e) => http::send(
                &mut stream,
                "500 Internal Server Error",
                "application/json",
                format!("{{\"error\":{}}}", quoted(&e)).as_bytes(),
            ),
        },
        "/api/chat" => {
            let session = http::param(&request.query, "session").unwrap_or_default();
            let limit = http::param(&request.query, "limit")
                .and_then(|l| l.parse().ok())
                .unwrap_or(60);
            http::json(&mut stream, &transcript::as_json(&session, limit));
        }
        "/api/events" => events(&mut stream),
        _ => http::not_found(&mut stream),
    }
}

fn quoted(text: &str) -> String {
    serde_json::to_string(text).unwrap_or_else(|_| "\"error\"".into())
}

/// Holds the connection open and writes when Conductor's state moves.
///
/// A tick is sent every fifteen seconds even when nothing has changed, because
/// an idle stream through a tunnel is a stream something in the middle will
/// eventually close.
fn events(stream: &mut std::net::TcpStream) {
    if !http::open_stream(stream) {
        return;
    }
    let mut last = String::new();
    let mut idle = 0;
    loop {
        let now = probe();
        if now != last {
            last = now.clone();
            if !http::event(stream, &format!("{{\"state\":{}}}", quoted(&now))) {
                return;
            }
            idle = 0;
        } else {
            idle += 1;
            if idle >= 30 && !http::event(stream, "{\"tick\":true}") {
                return;
            }
            if idle >= 30 {
                idle = 0;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
}

pub fn run(host: Option<&str>, port: u16) -> Result<(), String> {
    store::ensure_root()?;
    let host = host.unwrap_or("127.0.0.1");
    let bind = format!("{host}:{port}");
    let listener = TcpListener::bind(&bind).map_err(|e| format!("{bind}: {e}"))?;

    println!("hats serve, read-only, on http://{bind}");
    if host != "127.0.0.1" && host != "localhost" {
        println!();
        println!("Bound beyond loopback, so anything on this network can read it,");
        println!("and there is no password on it. For anywhere else, put a tunnel");
        println!("in front and keep this on 127.0.0.1. See docs/mobile.md.");
    }
    println!();
    println!("  /            the screen");
    println!("  /api/chats   every open chat and its account");
    println!("  /api/chat    one conversation, ?session=<id>&limit=<n>");
    println!("  /api/events  a stream that fires when Conductor's state moves");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                std::thread::spawn(move || handle(stream));
            }
            Err(e) => eprintln!("hats serve: {e}"),
        }
    }
    Ok(())
}
