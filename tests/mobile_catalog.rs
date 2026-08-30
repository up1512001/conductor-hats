//! The phone's model choices come from the paired Conductor app, not hats.

mod common;

use std::os::unix::fs::PermissionsExt;

use common::Sandbox;

fn sqlite() -> bool {
    std::process::Command::new("sqlite3")
        .arg("-version")
        .output()
        .map(|result| result.status.success())
        .unwrap_or(false)
}

fn database(s: &Sandbox, name: &str) -> std::path::PathBuf {
    let path = s.path(name);
    std::fs::File::create(&path).expect("a selected Conductor database");
    path
}

fn selected(db: &std::path::Path) {
    let sql = "create table workspaces (id text); insert into workspaces values ('workspace-a');";
    let result = std::process::Command::new("sqlite3")
        .arg(db)
        .arg(sql)
        .output()
        .expect("sqlite3 fixture");
    assert!(result.status.success(), "workspace fixture was not created");
}

#[test]
fn conductor_catalog_is_private_ordered_and_idempotent() {
    if !sqlite() {
        eprintln!("skipped: sqlite3 is not installed");
        return;
    }
    let s = Sandbox::new();
    let db = database(&s, "conductor.db");
    selected(&db);
    let catalog = r#"{"models":{"claude":["opus-5-1m","fable-5","opus-5-1m"],"codex":["gpt-5.6-sol","gpt-5.6-terra"]},"titles":{"12345678-abcd":"Conductor build overview"}}"#;
    let first = s
        .hats_env(
            &["remote", "catalog", "workspace-a", catalog],
            &[("CONDUCTOR_DB", &db.to_string_lossy())],
        )
        .ok()
        .out();
    assert!(first.contains("\"changed\":true"), "first catalog: {first}");

    let path = s.accounts().join("serve-catalog");
    let body: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(body["source"], db.to_string_lossy().as_ref());
    assert_eq!(
        body["catalog"]["models"]["claude"],
        serde_json::json!(["opus-5-1m", "fable-5"])
    );
    assert_eq!(
        body["catalog"]["titles"]["12345678-abcd"],
        "Conductor build overview"
    );
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );

    let second = s
        .hats_env(
            &["remote", "catalog", "workspace-a", catalog],
            &[("CONDUCTOR_DB", &db.to_string_lossy())],
        )
        .ok()
        .out();
    assert!(
        second.contains("\"changed\":false"),
        "same catalog: {second}"
    );
}

#[test]
fn invalid_or_unscoped_catalogs_fail_closed() {
    if !sqlite() {
        eprintln!("skipped: sqlite3 is not installed");
        return;
    }
    let s = Sandbox::new();
    let db = database(&s, "conductor.db");
    selected(&db);
    let unknown = r#"{"models":{"other":["anything"]}}"#;
    let invalid = s.hats_env(
        &["remote", "catalog", "workspace-a", unknown],
        &[("CONDUCTOR_DB", &db.to_string_lossy())],
    );
    assert_ne!(invalid.status, 0, "an unknown provider was stored");
    let incomplete = s.hats_env(
        &[
            "remote",
            "catalog",
            "workspace-a",
            r#"{"models":{"claude":["opus-5-1m"]}}"#,
        ],
        &[("CONDUCTOR_DB", &db.to_string_lossy())],
    );
    assert_ne!(
        incomplete.status, 0,
        "a partial provider catalog was stored"
    );

    let unscoped = s.hats(&[
        "remote",
        "catalog",
        "workspace-a",
        r#"{"models":{"claude":["opus-5-1m"]}}"#,
    ]);
    assert_ne!(unscoped.status, 0, "an unpaired app published models");
    assert!(unscoped.out().contains("mobile access is not paired"));
}
