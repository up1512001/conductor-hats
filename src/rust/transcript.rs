//! Reading a chat back out of Conductor's database.
//!
//! Two encodings live in one column, which is the first thing that bites. A
//! `user` row's `content` is plain text. An `assistant` row's is Claude Code's
//! SDK envelope as JSON, and running `json_extract` across both makes SQLite
//! abort the whole query with "malformed JSON", so every extract is guarded.
//!
//! Of the envelopes, only some are worth showing. Measured on this machine:
//! `system` 377k, `assistant` 136k, `user` 76k, `result` 4.4k. The bulk is tool
//! traffic. The text of a reply is in `message.content[]`, and not reliably at
//! index 0, because thinking and tool-use blocks share that array.

use crate::{id, places};

/// Newest first, so a phone can show the end of a conversation without reading
/// all of it. `sent_at` orders what was actually delivered; queued rows have
/// none and sort last, which is where they belong.
/// Emitted as JSON by SQLite rather than pipe-separated columns.
///
/// The obvious `select a, b, c` splits on `|`, and message text is full of
/// pipes: tables, shell pipelines, code. The first version of this returned a
/// tool-use envelope as though it were prose because the split landed inside
/// the JSON. Let SQLite do the quoting.
///
/// Newest first so a phone can show the end of a conversation without reading
/// all of it, then reversed for display.
const MESSAGES: &str = "select json_group_array(json_object('role', role, 'kind',      case when json_valid(content) then coalesce(json_extract(content,'$.type'),'') else 'text' end,      'body', case when json_valid(content) then json_extract(content,'$.message.content') else content end,      'at', coalesce(sent_at, created_at))) from (select * from session_messages where session_id = ";

/// One entry in a conversation as it is drawn.
///
/// Conductor shows more than what was said: a tool call is a row of its own,
/// collapsed to a verb and a detail, and thinking is a row too. Dropping those
/// and keeping only prose loses the shape of the work, which is most of what a
/// transcript is for.
pub struct Line {
    /// `say` for prose, `tool` for a call, `thinking` for reasoning.
    pub kind: String,
    pub role: String,
    pub at: String,
    /// The verb for a tool row: Bash, Read, Edit. Empty for prose.
    pub name: String,
    pub text: String,
}

fn say(role: &str, at: &str, text: String) -> Line {
    Line {
        kind: "say".into(),
        role: role.into(),
        at: at.into(),
        name: String::new(),
        text,
    }
}

/// The one detail worth putting beside a tool's name.
///
/// Each tool carries a different shape of input and only one field of it is
/// worth a row: the command for Bash, the path for a file tool. Anything else
/// falls back to the first string, which is usually the interesting one.
fn detail_of(input: Option<&serde_json::Value>) -> String {
    let Some(input) = input else {
        return String::new();
    };
    for key in [
        "command",
        "file_path",
        "path",
        "pattern",
        "query",
        "url",
        "prompt",
    ] {
        if let Some(found) = input.get(key).and_then(|v| v.as_str()) {
            return found.trim().to_string();
        }
    }
    input
        .as_object()
        .and_then(|map| map.values().find_map(|v| v.as_str()))
        .unwrap_or_default()
        .trim()
        .to_string()
}

/// Turns one assistant envelope into the rows it draws as.
fn blocks_of(blocks: Option<&serde_json::Value>, at: &str, out: &mut Vec<Line>) {
    let Some(list) = blocks.and_then(|b| b.as_array()) else {
        return;
    };
    for block in list {
        let kind = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
        match kind {
            "text" => {
                let text = block
                    .get("text")
                    .and_then(|t| t.as_str())
                    .unwrap_or_default()
                    .trim();
                if !text.is_empty() {
                    out.push(say("assistant", at, text.to_string()));
                }
            }
            "thinking" => {
                let text = block
                    .get("thinking")
                    .and_then(|t| t.as_str())
                    .unwrap_or_default()
                    .trim();
                if !text.is_empty() {
                    out.push(Line {
                        kind: "thinking".into(),
                        role: "assistant".into(),
                        at: at.into(),
                        name: "Thinking".into(),
                        text: text.to_string(),
                    });
                }
            }
            "tool_use" => out.push(Line {
                kind: "tool".into(),
                role: "assistant".into(),
                at: at.into(),
                name: block
                    .get("name")
                    .and_then(|n| n.as_str())
                    .unwrap_or("Tool")
                    .to_string(),
                text: detail_of(block.get("input")),
            }),
            _ => {}
        }
    }
}

/// The readable part of one chat: what was said, not how it was carried out.
pub fn lines(session: &str, limit: usize) -> Vec<Line> {
    let Some(session) = id::session(session) else {
        return Vec::new();
    };
    let sql = format!(
        "{MESSAGES}'{session}' and cancelled_at is null \
         order by coalesce(sent_at, created_at) desc limit {})",
        limit.clamp(1, 200)
    );

    /* One answer per database, and they must not be concatenated: two JSON
     * arrays glued together are not JSON. A chat lives in exactly one of them,
     * so the first array with anything in it is the one. */
    let answers = places::rows(&sql);
    let parsed = answers
        .iter()
        .filter_map(|row| serde_json::from_str::<serde_json::Value>(row).ok())
        .find(|value| value.as_array().map(|a| !a.is_empty()).unwrap_or(false));
    let Some(parsed) = parsed else {
        return Vec::new();
    };
    let Some(list) = parsed.as_array() else {
        return Vec::new();
    };

    let mut out = Vec::new();
    /* Oldest first. The query returns newest-envelope-first so a phone can read
     * the end of a long conversation without fetching all of it, but reversing
     * the flat list afterwards also reverses the rows inside each envelope, and
     * a tool call would land before the text that introduced it. */
    for row in list.iter().rev() {
        let field = |name: &str| row.get(name).and_then(|v| v.as_str()).unwrap_or_default();
        let at = field("at");
        /* `system` is Conductor's own bookkeeping and `user` inside an envelope
         * is a tool result: neither is part of the conversation. */
        match field("kind") {
            "text" => {
                let text = field("body").trim();
                if !text.is_empty() {
                    out.push(say(field("role"), at, text.to_string()));
                }
            }
            "assistant" => blocks_of(row.get("body"), at, &mut out),
            _ => continue,
        }
    }
    out
}

#[derive(serde::Serialize)]
struct Wire<'a> {
    kind: &'a str,
    role: &'a str,
    at: &'a str,
    name: &'a str,
    text: &'a str,
}

pub fn as_json(session: &str, limit: usize) -> String {
    let lines = lines(session, limit);
    let wire: Vec<Wire> = lines
        .iter()
        .map(|l| Wire {
            kind: &l.kind,
            role: &l.role,
            at: &l.at,
            name: &l.name,
            text: &l.text,
        })
        .collect();
    serde_json::to_string(&wire).unwrap_or_else(|_| "[]".into())
}
