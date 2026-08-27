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

pub struct Line {
    pub role: String,
    pub at: String,
    pub text: String,
}

/// Pulls the readable text out of one SDK envelope.
///
/// The blocks are walked rather than indexed: a reply that thought before it
/// spoke has the thinking at 0 and the words later, and taking index 0 returns
/// an empty string that looks like missing data.
fn text_of(blocks: Option<&serde_json::Value>) -> String {
    let Some(list) = blocks.and_then(|b| b.as_array()) else {
        return String::new();
    };
    let mut out = String::new();
    for block in list {
        if block.get("type").and_then(|t| t.as_str()) != Some("text") {
            continue;
        }
        if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(text.trim());
        }
    }
    out
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
    for row in list {
        let field = |name: &str| row.get(name).and_then(|v| v.as_str()).unwrap_or_default();
        /* `system` is tool traffic and `user` inside an envelope is a tool
         * result. Neither is anything a person said. */
        let text = match field("kind") {
            "text" => field("body").trim().to_string(),
            "assistant" => text_of(row.get("body")),
            _ => continue,
        };
        if text.is_empty() {
            continue;
        }
        out.push(Line {
            role: field("role").to_string(),
            at: field("at").to_string(),
            text,
        });
    }
    out.reverse();
    out
}

#[derive(serde::Serialize)]
struct Wire<'a> {
    role: &'a str,
    at: &'a str,
    text: &'a str,
}

pub fn as_json(session: &str, limit: usize) -> String {
    let lines = lines(session, limit);
    let wire: Vec<Wire> = lines
        .iter()
        .map(|l| Wire {
            role: &l.role,
            at: &l.at,
            text: &l.text,
        })
        .collect();
    serde_json::to_string(&wire).unwrap_or_else(|_| "[]".into())
}
