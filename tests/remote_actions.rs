//! Durable mobile run-control and new-chat receipts.

mod common;

use common::Sandbox;

fn sqlite() -> bool {
    std::process::Command::new("sqlite3")
        .arg("-version")
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

fn database(s: &Sandbox) -> String {
    let path = s.path("conductor.db");
    let sql = "create table session_messages (id text, session_id text, role text, \
         content text, created_at text, sent_at text, cancelled_at text); \
         create table workspaces (id text, repository_id text); \
         insert into workspaces values ('w1','r1'); \
         create table sessions (id text, claude_session_id text, agent_type text, workspace_id text, model text); \
         insert into sessions values ('s1','router-s1','claude','w1','opus-5-1m');";
    let out = std::process::Command::new("sqlite3")
        .arg(&path)
        .arg(sql)
        .output()
        .expect("building the fixture database");
    assert!(out.status.success(), "sqlite3 refused the fixture");
    path.to_string_lossy().to_string()
}

fn remote(s: &Sandbox, db: &str, args: &[&str]) -> String {
    let mut command = vec!["remote"];
    command.extend_from_slice(args);
    s.hats_env(&command, &[("CONDUCTOR_DB", db)]).ok().out()
}

#[test]
fn a_new_chat_waits_for_conductors_database_acknowledgement() {
    if !sqlite() {
        eprintln!("skipped: sqlite3 is not installed");
        return;
    }
    let s = Sandbox::new();
    let db = database(&s);
    let made: serde_json::Value =
        serde_json::from_str(&remote(&s, &db, &["create-enqueue", "s1"])).unwrap();
    let taken: serde_json::Value = serde_json::from_str(&remote(&s, &db, &["take", "s1"])).unwrap();
    let claim = &taken["create"];
    assert_eq!(claim["id"], made["id"]);
    let raw = serde_json::to_string(claim).unwrap();
    let before: serde_json::Value =
        serde_json::from_str(&remote(&s, &db, &["create-check", &raw])).unwrap();
    assert_eq!(before["applied"], false);

    let out = std::process::Command::new("sqlite3")
        .arg(&db)
        .arg("insert into sessions values ('s2','router-s2','codex','w1','')")
        .output()
        .unwrap();
    assert!(out.status.success());
    let after: serde_json::Value =
        serde_json::from_str(&remote(&s, &db, &["create-check", &raw])).unwrap();
    assert_eq!(after["session"], "s2");
    remote(&s, &db, &["create-complete", &raw]);

    let file = s
        .accounts()
        .join("remote-create")
        .join(format!("{}.json", made["id"].as_str().unwrap_or_default()));
    let receipt: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
    assert_eq!(receipt["done"], true);
    assert_eq!(receipt["result"], "s2");
    remote(
        &s,
        &db,
        &["create-ack", made["id"].as_str().unwrap_or_default()],
    );
    assert!(!file.exists());
}

#[test]
fn a_refused_run_setting_becomes_a_visible_failure_receipt() {
    if !sqlite() {
        eprintln!("skipped: sqlite3 is not installed");
        return;
    }
    let s = Sandbox::new();
    let db = database(&s);
    let made: serde_json::Value = serde_json::from_str(&remote(
        &s,
        &db,
        &["control-enqueue", "s1", "model", "gpt-5.6-sol", "gpt-5.5"],
    ))
    .unwrap();
    for _ in 0..4 {
        let claim: serde_json::Value =
            serde_json::from_str(&remote(&s, &db, &["control-claim", "s1"])).unwrap();
        remote(
            &s,
            &db,
            &["control-release", &serde_json::to_string(&claim).unwrap()],
        );
    }
    assert_eq!(remote(&s, &db, &["control-claim", "s1"]).trim(), "null");
    let file = s
        .accounts()
        .join("remote-controls/s1")
        .join(format!("{}.json", made["id"].as_str().unwrap_or_default()));
    let receipt: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
    assert_eq!(receipt["done"], true);
    assert!(receipt["error"]
        .as_str()
        .unwrap_or_default()
        .contains("four attempts"));
    remote(
        &s,
        &db,
        &["control-ack", "s1", made["id"].as_str().unwrap_or_default()],
    );
    assert!(!file.exists());
}

#[test]
fn a_cross_agent_model_opens_and_reports_conductors_new_chat() {
    if !sqlite() {
        eprintln!("skipped: sqlite3 is not installed");
        return;
    }
    let s = Sandbox::new();
    let db = database(&s);
    let made: serde_json::Value = serde_json::from_str(&remote(
        &s,
        &db,
        &["control-enqueue", "s1", "model", "gpt-5.6-sol", "opus-5-1m"],
    ))
    .unwrap();
    let claim: serde_json::Value =
        serde_json::from_str(&remote(&s, &db, &["control-claim", "s1"])).unwrap();
    let raw = serde_json::to_string(&claim).unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&remote(&s, &db, &["control-check", &raw]))
            .unwrap()["applied"],
        false
    );

    let out = std::process::Command::new("sqlite3")
        .arg(&db)
        .arg("insert into sessions values ('s2','router-s2','codex','w1','gpt-5.6-sol')")
        .output()
        .unwrap();
    assert!(out.status.success());
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&remote(&s, &db, &["control-check", &raw]))
            .unwrap()["applied"],
        true
    );
    remote(&s, &db, &["control-complete", &raw]);
    let file = s
        .accounts()
        .join("remote-controls/s1")
        .join(format!("{}.json", made["id"].as_str().unwrap_or_default()));
    let receipt: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
    assert_eq!(receipt["done"], true);
    assert_eq!(receipt["result"], "s2");
    remote(
        &s,
        &db,
        &["control-ack", "s1", made["id"].as_str().unwrap_or_default()],
    );
    assert!(!file.exists());
}
