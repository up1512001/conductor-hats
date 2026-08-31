//! The authenticated Conductor screen served from this Mac.
//!
//! It binds loopback by default. A named outbound tunnel supplies the stable
//! public HTTPS origin; no router port or laptop firewall opening is needed.
//! Reads come from Conductor's databases. Mobile writes stop in hats' private
//! durable queue until the injected panel submits them through Conductor's real
//! composer and the database confirms delivery.

use std::net::TcpListener;
use std::time::Duration;

use crate::{
    auth, chats, http, mobile_page, mobile_scope, mobile_service, mobile_socket, origin, remote,
    store, transcript,
};

fn allowed(request: &http::Request) -> bool {
    let Some(offered) = credential(request) else {
        return false;
    };
    auth::session()
        .map(|expected| auth::same(&offered, &expected))
        .unwrap_or(false)
}

fn credential(request: &http::Request) -> Option<String> {
    request
        .headers
        .get("cookie")
        .and_then(|header| auth::cookie(header, auth::COOKIE))
}

fn error(stream: &mut std::net::TcpStream, status: &str, message: &str) {
    let quoted = serde_json::to_string(message).unwrap_or_else(|_| "\"error\"".into());
    http::send(
        stream,
        status,
        "application/json",
        format!("{{\"error\":{quoted}}}").as_bytes(),
    );
}

fn pair(stream: &mut std::net::TcpStream, request: &http::Request) {
    let offered = request
        .headers
        .get("x-hats-token")
        .map(String::as_str)
        .unwrap_or("");
    match auth::consume_pairing(offered) {
        Ok(true) => match auth::session() {
            Ok(session) => http::send_with(
                stream,
                "200 OK",
                "application/json",
                b"{\"paired\":true}",
                &format!("Set-Cookie: {}\r\n", auth::set_cookie(&session)),
            ),
            Err(problem) => error(stream, "500 Internal Server Error", &problem),
        },
        Ok(false) => error(
            stream,
            "401 Unauthorized",
            "pairing token expired or already used",
        ),
        Err(problem) => error(stream, "500 Internal Server Error", &problem),
    }
}

fn enqueue(stream: &mut std::net::TcpStream, request: &http::Request) {
    let input = serde_json::from_slice::<serde_json::Value>(&request.body);
    let Ok(input) = input else {
        error(stream, "400 Bad Request", "the request body is not JSON");
        return;
    };
    let session = input
        .get("session")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    let message = input
        .get("message")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    match remote::enqueue(session, message) {
        Ok(item) => http::send(
            stream,
            "202 Accepted",
            "application/json",
            format!("{{\"queued\":true,\"id\":\"{}\"}}", item.id).as_bytes(),
        ),
        Err(problem) => error(stream, "400 Bad Request", &problem),
    }
}

fn protected(stream: &mut std::net::TcpStream, request: &http::Request) {
    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/api/chats") => match chats::json_string() {
            Ok(body) => http::json(stream, &body),
            Err(problem) => error(stream, "500 Internal Server Error", &problem),
        },
        ("GET", "/api/chat") => {
            let session = http::param(&request.query, "session").unwrap_or_default();
            let limit = http::param(&request.query, "limit")
                .and_then(|value| value.parse().ok())
                .unwrap_or(160);
            http::json(stream, &transcript::as_json(&session, limit));
        }
        ("GET", "/api/outbox") => {
            let session = http::param(&request.query, "session").unwrap_or_default();
            match remote::pending_json(&session) {
                Ok(body) => http::json(stream, &body),
                Err(problem) => error(stream, "400 Bad Request", &problem),
            }
        }
        ("GET", "/api/health") => http::json(stream, "{\"ok\":true}"),
        ("POST", "/api/messages") => enqueue(stream, request),
        ("GET", _) => http::not_found(stream),
        _ => error(stream, "405 Method Not Allowed", "method not allowed"),
    }
}

fn handle(mut stream: std::net::TcpStream) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(20)));
    let Some(request) = http::read(&stream) else {
        error(
            &mut stream,
            "400 Bad Request",
            "invalid or oversized request",
        );
        return;
    };

    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/") => http::send(
            &mut stream,
            "200 OK",
            "text/html; charset=utf-8",
            mobile_page::page().as_bytes(),
        ),
        ("GET", path) if path == mobile_page::css_path() || path == "/mobile.css" => http::send(
            &mut stream,
            "200 OK",
            "text/css; charset=utf-8",
            mobile_page::style().as_bytes(),
        ),
        ("GET", path) if path == mobile_page::js_path() || path == "/mobile.js" => http::send(
            &mut stream,
            "200 OK",
            "text/javascript; charset=utf-8",
            mobile_page::script().as_bytes(),
        ),
        ("GET", "/logo.png") => {
            let mark = mobile_page::logo();
            if mark.is_empty() {
                http::not_found(&mut stream);
            } else {
                http::send(&mut stream, "200 OK", "image/png", mark);
            }
        }
        ("GET", "/favicon.ico") => http::send(&mut stream, "204 No Content", "image/x-icon", b""),
        ("GET", path)
            if auth::is_pairing_path(path) && (auth::route_matches(path) || allowed(&request)) =>
        {
            http::send(
                &mut stream,
                "200 OK",
                "text/html; charset=utf-8",
                mobile_page::page().as_bytes(),
            )
        }
        ("POST", "/api/pair") => pair(&mut stream, &request),
        _ if !allowed(&request) => error(&mut stream, "401 Unauthorized", "pair first"),
        ("GET", "/ws") => {
            let offered = credential(&request).unwrap_or_default();
            mobile_socket::open(stream, &request, offered);
        }
        _ => protected(&mut stream, &request),
    }
}

fn option<'a>(rest: &'a [String], name: &str) -> Option<&'a str> {
    rest.iter()
        .position(|item| item == name)
        .and_then(|index| rest.get(index + 1))
        .map(String::as_str)
}

fn origin(rest: &[String]) -> Result<String, String> {
    origin::public(option(rest, "--origin"))?.ok_or_else(|| {
        "pass the stable public HTTPS URL with --origin https://host.example.com".to_string()
    })
}

pub fn command(rest: &[String]) -> Result<(), String> {
    if rest
        .iter()
        .any(|item| item == "--pair" || item == "--revoke")
    {
        let pairing =
            auth::pairing_for(&origin(rest)?, rest.iter().any(|item| item == "--revoke"))?;
        println!("{}", pairing.url);
        return Ok(());
    }
    let port = match option(rest, "--port") {
        Some(value) => value
            .parse()
            .map_err(|_| format!("invalid port: {value}"))?,
        None => 8787,
    };
    let host = option(rest, "--host").unwrap_or("127.0.0.1");
    let public = origin::public(option(rest, "--origin"))?;
    run(host, port, public.as_deref())
}

pub fn run(host: &str, port: u16, origin: Option<&str>) -> Result<(), String> {
    store::ensure_root()?;
    let scope = mobile_scope::adopt()?;
    auth::session()?;
    let public_url = origin
        .map(|value| auth::pairing_for(value, false).map(|pairing| pairing.url))
        .transpose()?;
    let bind = format!("{host}:{port}");
    let listener = TcpListener::bind(&bind).map_err(|e| format!("{bind}: {e}"))?;
    let _runtime = mobile_service::register(&bind)?;

    println!("hats serve on http://{bind}");
    println!("Showing {} and no other Conductor copy.", scope.label());
    match public_url {
        Some(url) => println!("Pair this browser once (expires in ten minutes):\n\n  {url}"),
        None => println!(
            "Loopback only. For remote access, configure the named HTTPS tunnel and restart with:\n\n  hats serve --origin https://host.example.com"
        ),
    }
    if host != "127.0.0.1" && host != "localhost" {
        println!("Warning: this listener is not confined to loopback. The public tunnel does not require that.");
    }

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                std::thread::spawn(move || handle(stream));
            }
            Err(problem) => eprintln!("hats serve: {problem}"),
        }
    }
    Ok(())
}
