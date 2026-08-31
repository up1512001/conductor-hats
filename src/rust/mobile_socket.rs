//! Authenticated WebSocket snapshots and commands for the phone client.

use std::net::TcpStream;
use std::time::{Duration, Instant};

use tungstenite::protocol::{Message, Role, WebSocket, WebSocketConfig};

use crate::{
    auth, http, mobile_service, mobile_stamp, mobile_state, origin, remote, remote_control,
    remote_create,
};

fn header<'a>(request: &'a http::Request, name: &str) -> &'a str {
    request.headers.get(name).map(String::as_str).unwrap_or("")
}

/// Below this a frame is not worth compressing, and small control replies stay
/// readable text on the wire.
const PACK_OVER: usize = 4 * 1024;

/// Snapshots are JSON and compress about five times over, measured on a real
/// transcript: 263 KB of chat list and transcript became 54 KB. That matters on
/// a phone at the far end of a tunnel far more than it costs to gzip here.
///
/// One direction only. Nothing the phone sends is ever decompressed, so no
/// client can hand the Mac a small frame that expands into an enormous one.
fn packed(value: &str) -> Option<Vec<u8>> {
    use std::io::Write;
    if value.len() < PACK_OVER {
        return None;
    }
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    encoder.write_all(value.as_bytes()).ok()?;
    encoder.finish().ok()
}

fn send(socket: &mut WebSocket<TcpStream>, value: &str) -> bool {
    let message = match packed(value) {
        Some(packed) => Message::Binary(packed.into()),
        None => Message::Text(value.to_string().into()),
    };
    socket.send(message).is_ok()
}

fn reply(
    socket: &mut WebSocket<TcpStream>,
    kind: &str,
    value: serde_json::Value,
    request: &str,
) -> bool {
    send(
        socket,
        &serde_json::json!({ "type": kind, "value": value, "request": request }).to_string(),
    )
}

fn command(
    socket: &mut WebSocket<TcpStream>,
    selected: &mut Option<String>,
    text: &str,
) -> Result<bool, String> {
    let input: serde_json::Value =
        serde_json::from_str(text).map_err(|_| "invalid WebSocket command")?;
    let kind = input
        .get("type")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    let request = input
        .get("request")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    let chat = input
        .get("session")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    match kind {
        "subscribe" => {
            *selected = crate::id::session(chat).map(str::to_string);
            Ok(true)
        }
        "send" => {
            let message = input
                .get("message")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            let item = remote::enqueue(chat, message)?;
            reply(
                socket,
                "accepted",
                serde_json::json!({ "id": item.id }),
                request,
            );
            Ok(true)
        }
        "account" => {
            let value = input
                .get("value")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            mobile_state::choose_account(chat, value)?;
            reply(
                socket,
                "applied",
                serde_json::json!({ "setting": "account" }),
                request,
            );
            Ok(true)
        }
        "control" => {
            let setting = input
                .get("setting")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            let value = input
                .get("value")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            let before = input
                .get("before")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            let item = remote_control::enqueue(chat, setting, value, before)?;
            reply(
                socket,
                "accepted-setting",
                serde_json::json!({ "id": item.id, "setting": item.setting }),
                request,
            );
            Ok(true)
        }
        "control-ack" => {
            let id = input
                .get("id")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            remote_control::ack(chat, id)?;
            Ok(true)
        }
        "new-chat" => {
            let item = remote_create::enqueue(chat)?;
            reply(
                socket,
                "accepted-new-chat",
                serde_json::json!({ "id": item.id }),
                request,
            );
            Ok(true)
        }
        "create-ack" => {
            let id = input
                .get("id")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            remote_create::ack(id)?;
            Ok(true)
        }
        "refresh" => Ok(true),
        _ => Err("unknown WebSocket command".into()),
    }
}

fn read_command(
    socket: &mut WebSocket<TcpStream>,
    selected: &mut Option<String>,
) -> Result<Option<bool>, tungstenite::Error> {
    match socket.read() {
        Ok(Message::Text(text)) => match command(socket, selected, &text) {
            Ok(changed) => Ok(Some(changed)),
            Err(problem) => {
                let request = serde_json::from_str::<serde_json::Value>(&text)
                    .ok()
                    .and_then(|input| input.get("request")?.as_str().map(str::to_string))
                    .unwrap_or_default();
                reply(socket, "error", serde_json::json!(problem), &request);
                Ok(Some(false))
            }
        },
        Ok(Message::Close(_)) => Ok(None),
        Ok(Message::Ping(body)) => {
            socket.send(Message::Pong(body))?;
            Ok(Some(false))
        }
        Ok(_) => Ok(Some(false)),
        Err(tungstenite::Error::Io(error))
            if matches!(
                error.kind(),
                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
            ) =>
        {
            Ok(Some(false))
        }
        Err(error) => Err(error),
    }
}

pub fn open(mut stream: TcpStream, request: &http::Request, credential: String) {
    let same_origin = origin::configured()
        .map(|expected| header(request, "origin").eq_ignore_ascii_case(&expected))
        .unwrap_or(false);
    if !same_origin {
        http::send(
            &mut stream,
            "403 Forbidden",
            "application/json",
            b"{\"error\":\"the WebSocket origin is not allowed\"}",
        );
        return;
    }
    if !header(request, "connection")
        .split(',')
        .any(|value| value.trim().eq_ignore_ascii_case("upgrade"))
        || !header(request, "upgrade").eq_ignore_ascii_case("websocket")
        || header(request, "sec-websocket-version") != "13"
    {
        http::send(
            &mut stream,
            "400 Bad Request",
            "application/json",
            b"{\"error\":\"a WebSocket upgrade is required\"}",
        );
        return;
    }
    let key = header(request, "sec-websocket-key");
    if key.is_empty() {
        return;
    }
    let accept = tungstenite::handshake::derive_accept_key(key.as_bytes());
    if !http::websocket(&mut stream, &accept) {
        return;
    }
    let _client = mobile_service::connected();
    let _ = stream.set_read_timeout(Some(Duration::from_millis(350)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(15)));
    let config = WebSocketConfig::default()
        .max_message_size(Some(128 * 1024))
        .max_frame_size(Some(128 * 1024));
    let mut socket = WebSocket::from_raw_socket(stream, Role::Server, Some(config));
    let mut selected = None;
    let mut sections = mobile_stamp::Sections::default();
    let mut force = true;
    let mut heartbeat = Instant::now();
    loop {
        let current = auth::session();
        if !current
            .map(|expected| auth::same(&credential, &expected))
            .unwrap_or(false)
        {
            let _ = socket.close(None);
            return;
        }
        let next = mobile_stamp::read(selected.as_deref(), &sections);
        let want = mobile_state::Want {
            chats: force || next.chats != sections.chats,
            active: force || next.active != sections.active,
            accounts: force || next.accounts != sections.accounts,
        };
        if want.any() {
            let stamp = format!("{}|{}|{}", next.chats, next.active, next.accounts);
            let snapshot = mobile_state::snapshot(selected.as_deref(), want, stamp).unwrap_or_else(
                |problem| serde_json::json!({ "type": "error", "value": problem }).to_string(),
            );
            if !send(&mut socket, &snapshot) {
                return;
            }
            /* Only after it is on the wire. A section recorded as delivered and
             * then dropped is a section the phone never sees again. */
            sections = next;
            force = false;
        }
        match read_command(&mut socket, &mut selected) {
            Ok(Some(changed)) => force |= changed,
            Ok(None) => return,
            Err(tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed) => return,
            Err(_) => return,
        }
        if heartbeat.elapsed() >= Duration::from_secs(25) {
            if socket.send(Message::Ping(Vec::new().into())).is_err() {
                return;
            }
            heartbeat = Instant::now();
        }
    }
}

#[cfg(test)]
#[path = "mobile_socket_tests.rs"]
mod tests;
