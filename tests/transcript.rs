//! Reading a conversation back out of Conductor's database.
//!
//! Two encodings share one column and the envelope is mostly tool traffic, so
//! the failures here are quiet ones: a query that dies on the wrong row, or a
//! transcript that renders a tool call as though it were prose.

mod common;

use common::Sandbox;

fn sqlite() -> bool {
    std::process::Command::new("sqlite3")
        .arg("-version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// A conversation with every shape that has caused a bug: plain user text, a
/// reply that thought before it spoke, pure tool traffic, and a pipe in the
/// message body.
fn database(s: &Sandbox) -> String {
    let db = s.path("conductor.db");
    let ws = s.path("ws-a").to_string_lossy().to_string();
    let sql = format!(
        "create table workspaces (id text, directory_name text, workspace_path text, state text, active_session_id text);\n\
         insert into workspaces values ('w1','ws-a','{ws}','ready','s1');\n\
         create table sessions (id text, claude_session_id text, agent_type text, status text, \
             unread_count integer, title text, context_used_percent real, is_hidden integer, \
             workspace_id text, updated_at text);\n\
         insert into sessions values ('s1','s1','claude','idle',0,'Talk',0,0,'w1','2026-08-27T10:00:00Z');\n\
         create table session_messages (id text, session_id text, role text, content text, \
             created_at text, sent_at text, cancelled_at text, model text);\n\
         insert into session_messages values ('m1','s1','user','count | sort | uniq -c','2026-08-27T10:00:01Z','2026-08-27T10:00:01Z',null,'');\n\
         insert into session_messages values ('m2','s1','assistant','{{\"type\":\"assistant\",\"message\":{{\"content\":[{{\"type\":\"thinking\",\"thinking\":\"hmm\"}},{{\"type\":\"text\",\"text\":\"piped: a | b\"}}]}}}}','2026-08-27T10:00:02Z','2026-08-27T10:00:02Z',null,'opus');\n\
         insert into session_messages values ('m3','s1','assistant','{{\"type\":\"system\",\"subtype\":\"tool\"}}','2026-08-27T10:00:03Z','2026-08-27T10:00:03Z',null,'');\n\
         insert into session_messages values ('m4','s1','assistant','{{\"type\":\"assistant\",\"message\":{{\"content\":[{{\"type\":\"tool_use\",\"name\":\"Bash\"}}]}}}}','2026-08-27T10:00:04Z','2026-08-27T10:00:04Z',null,'opus');\n\
         insert into session_messages values ('m6','s1','assistant','{{\"type\":\"user\",\"message\":{{\"content\":[{{\"type\":\"tool_result\",\"tool_use_id\":\"tool-1\",\"content\":\"done\"}}]}}}}','2026-08-27T10:00:05Z','2026-08-27T10:00:05Z',null,'opus');\n\
         insert into session_messages values ('m7','s1','assistant','{{\"type\":\"system\",\"subtype\":\"compact_boundary\"}}','2026-08-27T10:00:06Z','2026-08-27T10:00:06Z',null,'');\n\
         insert into session_messages values ('m5','s1','user','cancelled','2026-08-27T10:00:07Z',null,'2026-08-27T10:00:08Z','');\n"
    );
    let out = std::process::Command::new("sqlite3")
        .arg(&db)
        .arg(&sql)
        .output()
        .expect("building the fixture database");
    assert!(out.status.success(), "sqlite3 refused the fixture");
    db.to_string_lossy().to_string()
}

fn read(s: &Sandbox, db: &str) -> serde_json::Value {
    let out = s
        .hats_env(
            &["transcript", "s1", "--limit", "50"],
            &[("CONDUCTOR_DB", db)],
        )
        .out();
    serde_json::from_str(&out).unwrap_or_else(|e| panic!("not JSON: {e}\n{out}"))
}

#[test]
fn it_draws_tool_calls_as_rows_beside_what_was_said() {
    if !sqlite() {
        eprintln!("skipped: sqlite3 is not installed");
        return;
    }
    let s = Sandbox::new();
    s.hats(&["install"]).ok();
    s.profile("claude", "work");
    s.workspace("ws-a");
    let db = database(&s);

    let lines = read(&s, &db);
    let list = lines.as_array().expect("an array");
    let kinds: Vec<&str> = list
        .iter()
        .map(|l| l["kind"].as_str().unwrap_or(""))
        .collect();

    /* Conductor shows the shape of the work, not only the prose: a tool call is
     * a row and so is thinking. Keeping only what was said loses most of it. */
    assert_eq!(
        kinds,
        vec!["say", "thinking", "say", "tool", "tool_result", "event"],
        "wrong shape:\n{lines:#}"
    );

    let tool = list
        .iter()
        .find(|l| l["kind"] == "tool")
        .expect("a tool row");
    assert_eq!(tool["name"], "Bash", "the tool is not named");
    assert!(
        list.iter()
            .any(|line| line["kind"] == "tool_result" && line["text"] == "done"),
        "the tool result is missing:\n{lines:#}"
    );

    /* Conductor's own bookkeeping is not part of the conversation. */
    assert!(
        !lines.to_string().contains("subtype"),
        "a system envelope was drawn:\n{lines:#}"
    );
}

/// The column separator is a pipe and message bodies are full of them: shell
/// pipelines, tables, code. Splitting on it returned a tool-use envelope as
/// though it were prose.
#[test]
fn a_pipe_in_a_message_survives() {
    if !sqlite() {
        eprintln!("skipped: sqlite3 is not installed");
        return;
    }
    let s = Sandbox::new();
    s.hats(&["install"]).ok();
    s.profile("claude", "work");
    s.workspace("ws-a");
    let db = database(&s);

    /* Selected by role rather than index: rows shift as the shape of an
     * envelope changes, and an index makes the test brittle for no reason. */
    let lines = read(&s, &db);
    let list = lines.as_array().expect("an array");
    let said = |role: &str| -> String {
        list.iter()
            .find(|l| l["kind"] == "say" && l["role"] == role)
            .and_then(|l| l["text"].as_str())
            .unwrap_or_default()
            .to_string()
    };
    assert_eq!(said("user"), "count | sort | uniq -c");
    assert_eq!(said("assistant"), "piped: a | b");
}

/// Thinking and tool-use blocks share the content array, so the text is not
/// reliably at index 0. Taking index 0 returned empty strings that looked like
/// missing data.
#[test]
fn text_is_found_past_a_thinking_block() {
    if !sqlite() {
        eprintln!("skipped: sqlite3 is not installed");
        return;
    }
    let s = Sandbox::new();
    s.hats(&["install"]).ok();
    s.profile("claude", "work");
    s.workspace("ws-a");
    let db = database(&s);

    let lines = read(&s, &db);
    let spoken = lines
        .as_array()
        .expect("an array")
        .iter()
        .find(|l| l["kind"] == "say" && l["role"] == "assistant")
        .and_then(|l| l["text"].as_str())
        .unwrap_or_default();
    assert!(
        spoken.contains("piped"),
        "the reply's text was not found past the thinking block:\n{lines:#}"
    );
}

/// A cancelled message was never delivered, so it is not part of the record.
#[test]
fn a_cancelled_message_is_not_shown() {
    if !sqlite() {
        eprintln!("skipped: sqlite3 is not installed");
        return;
    }
    let s = Sandbox::new();
    s.hats(&["install"]).ok();
    s.profile("claude", "work");
    s.workspace("ws-a");
    let db = database(&s);

    let text = read(&s, &db).to_string();
    assert!(!text.contains("cancelled"), "a cancelled message was shown");
}
