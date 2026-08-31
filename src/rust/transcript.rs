//! Reading the visible shape of a Conductor conversation.
//!
//! Plain user rows and SDK JSON envelopes share one database column. Assistant
//! text, thinking, tool calls and tool results all have different shapes, while
//! system envelopes describe visible events such as compaction and retries. The
//! parser keeps those distinctions so the phone can draw what Conductor draws
//! instead of flattening the work into a misleading wall of prose.

use crate::{id, places};

/// Newest envelopes first for a bounded read, reversed before rendering.
const MESSAGES: &str = "select json_group_array(json_object('id',id,'role',role,'kind', \
     case when json_valid(content) then coalesce(json_extract(content,'$.type'),'') else 'text' end, \
     'body',case when not json_valid(content) then content \
       when json_extract(content,'$.type') in ('assistant','user') \
         then json_extract(content,'$.message.content') else json(content) end, \
     'at',coalesce(sent_at,created_at),'model',coalesce(model,''))) \
   from (select * from session_messages where session_id = ";

pub struct Line {
    pub id: String,
    pub kind: String,
    pub role: String,
    pub at: String,
    pub name: String,
    pub text: String,
    pub detail: String,
    pub failed: bool,
    pub model: String,
}

fn line(kind: &str, role: &str, at: &str, name: &str, text: String) -> Line {
    Line {
        id: String::new(),
        kind: kind.into(),
        role: role.into(),
        at: at.into(),
        name: name.into(),
        text,
        detail: String::new(),
        failed: false,
        model: String::new(),
    }
}

fn string(value: Option<&serde_json::Value>) -> String {
    let Some(value) = value else {
        return String::new();
    };
    if let Some(text) = value.as_str() {
        return text.trim().to_string();
    }
    if let Some(list) = value.as_array() {
        return list
            .iter()
            .map(|part| string(Some(part)))
            .filter(|part| !part.is_empty())
            .collect::<Vec<String>>()
            .join("\n");
    }
    if let Some(map) = value.as_object() {
        for key in ["text", "content", "output", "stdout", "stderr", "message"] {
            let found = string(map.get(key));
            if !found.is_empty() {
                return found;
            }
        }
    }
    serde_json::to_string(value).unwrap_or_default()
}

/// The compact detail Conductor places beside a tool verb.
fn tool_detail(input: Option<&serde_json::Value>) -> String {
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
        "description",
    ] {
        if let Some(found) = input.get(key).and_then(|v| v.as_str()) {
            return found.trim().to_string();
        }
    }
    string(Some(input))
}

fn blocks_of(blocks: Option<&serde_json::Value>, role: &str, at: &str, out: &mut Vec<Line>) {
    let Some(list) = blocks.and_then(|b| b.as_array()) else {
        return;
    };
    for block in list {
        let kind = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
        let found = match kind {
            "text" => line("say", role, at, "", string(block.get("text"))),
            "thinking" => line(
                "thinking",
                "assistant",
                at,
                "Thinking",
                string(block.get("thinking")),
            ),
            "tool_use" => {
                let mut row = line(
                    "tool",
                    "assistant",
                    at,
                    block.get("name").and_then(|v| v.as_str()).unwrap_or("Tool"),
                    tool_detail(block.get("input")),
                );
                row.id = string(block.get("id"));
                row.detail = string(block.get("input"));
                row
            }
            "tool_result" => {
                let mut row = line(
                    "tool_result",
                    "assistant",
                    at,
                    "Result",
                    string(block.get("content")),
                );
                row.id = string(block.get("tool_use_id"));
                row.failed = block
                    .get("is_error")
                    .and_then(|value| value.as_bool())
                    .unwrap_or(false);
                row
            }
            _ => continue,
        };
        if !found.text.is_empty() || found.kind == "tool" {
            out.push(found);
        }
    }
}

fn metric(value: &serde_json::Value, at: &str) -> Option<Line> {
    let failed = value
        .get("is_error")
        .and_then(|item| item.as_bool())
        .unwrap_or(false);
    let turns = value.get("num_turns").and_then(|item| item.as_u64());
    let cost = value.get("total_cost_usd").and_then(|item| item.as_f64());
    let mut parts = Vec::new();
    if let Some(turns) = turns {
        parts.push(format!("{turns} turn{}", if turns == 1 { "" } else { "s" }));
    }
    if let Some(cost) = cost {
        parts.push(format!("${cost:.4}"));
    }
    if parts.is_empty() && !failed {
        return None;
    }
    let mut row = line(
        if failed { "error" } else { "result" },
        "system",
        at,
        if failed { "Failed" } else { "Completed" },
        parts.join(" · "),
    );
    row.failed = failed;
    Some(row)
}

fn event(value: &serde_json::Value, at: &str) -> Option<Line> {
    let kind = value
        .get("type")
        .and_then(|item| item.as_str())
        .unwrap_or("");
    if kind == "error" {
        let mut row = line("error", "system", at, "Error", string(value.get("content")));
        row.detail = string(value.get("errorInfo"));
        row.failed = true;
        return Some(row);
    }
    if kind == "result" {
        return metric(value, at);
    }
    if kind == "codex_goal_updated" {
        let goal = value.get("goal");
        let name = goal
            .and_then(|item| item.get("status"))
            .and_then(|item| item.as_str())
            .unwrap_or("updated");
        return Some(line(
            "event",
            "system",
            at,
            "Goal",
            format!("{name}: {}", string(goal)),
        ));
    }
    let subtype = value
        .get("subtype")
        .and_then(|item| item.as_str())
        .unwrap_or("");
    let name = match subtype {
        "compact_boundary" => "Context compacted",
        "api_retry" => "Retrying",
        "task_started" => "Task started",
        "task_updated" => "Task updated",
        "task_notification" => "Task finished",
        "code_change_published" => "Changes published",
        "model_refusal_fallback" => "Model fallback",
        _ => return None,
    };
    let text = ["description", "content", "error", "task_type"]
        .iter()
        .map(|key| string(value.get(*key)))
        .find(|text| !text.is_empty())
        .unwrap_or_default();
    Some(line("event", "system", at, name, text))
}

pub fn lines(session: &str, limit: usize) -> Vec<Line> {
    let Some(session) = id::session(session) else {
        return Vec::new();
    };
    let sql = format!(
        "{MESSAGES}'{session}' and cancelled_at is null \
         order by coalesce(sent_at,created_at) desc limit {})",
        limit.clamp(1, 300)
    );
    let parsed = places::rows(&sql)
        .iter()
        .filter_map(|row| serde_json::from_str::<serde_json::Value>(row).ok())
        .find(|value| {
            value
                .as_array()
                .map(|list| !list.is_empty())
                .unwrap_or(false)
        });
    let Some(list) = parsed.as_ref().and_then(|value| value.as_array()) else {
        return Vec::new();
    };

    let mut out = Vec::new();
    for row in list.iter().rev() {
        let field = |name: &str| row.get(name).and_then(|value| value.as_str()).unwrap_or("");
        let at = field("at");
        match field("kind") {
            "text" => out.push(line("say", field("role"), at, "", string(row.get("body")))),
            "assistant" | "user" => blocks_of(row.get("body"), field("kind"), at, &mut out),
            _ => {
                if let Some(mut found) = row.get("body").and_then(|body| event(body, at)) {
                    found.id = field("id").to_string();
                    found.model = field("model").to_string();
                    out.push(found);
                }
            }
        }
    }
    out.retain(|row| !row.text.is_empty() || matches!(row.kind.as_str(), "tool" | "event"));
    out
}

#[derive(serde::Serialize)]
struct Wire<'a> {
    id: &'a str,
    kind: &'a str,
    role: &'a str,
    at: &'a str,
    name: &'a str,
    text: &'a str,
    detail: &'a str,
    failed: bool,
    model: &'a str,
}

pub fn as_json(session: &str, limit: usize) -> String {
    let lines = lines(session, limit);
    let wire: Vec<Wire> = lines
        .iter()
        .map(|row| Wire {
            id: &row.id,
            kind: &row.kind,
            role: &row.role,
            at: &row.at,
            name: &row.name,
            text: &row.text,
            detail: &row.detail,
            failed: row.failed,
            model: &row.model,
        })
        .collect();
    serde_json::to_string(&wire).unwrap_or_else(|_| "[]".into())
}
