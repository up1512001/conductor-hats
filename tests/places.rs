//! The two lists the panel matches the screen against.
//!
//! Without them the panel resolves to no target and disables every account row,
//! which is exactly what happened when the port to Rust dropped these commands.

mod common;

use common::Sandbox;

fn sqlite() -> bool {
    std::process::Command::new("sqlite3")
        .arg("-version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn database(s: &Sandbox) -> String {
    let db = s.path("conductor.db");
    let sql = "create table workspaces (workspace_path text, state text);\n\
               insert into workspaces values ('/tmp/ws/alpha', 'ready');\n\
               insert into workspaces values ('/tmp/ws/beta', 'ready');\n\
               insert into workspaces values ('/tmp/ws/gone', 'archived');\n\
               insert into workspaces values (null, 'ready');\n\
               create table repos (root_path text);\n\
               insert into repos values ('/tmp/code/my repo');\n";
    let out = std::process::Command::new("sqlite3")
        .arg(&db)
        .arg(sql)
        .output()
        .expect("building the fixture database");
    assert!(out.status.success(), "sqlite3 refused the fixture");
    db.to_string_lossy().to_string()
}

#[test]
fn workspaces_are_listed_as_name_then_path() {
    if !sqlite() {
        eprintln!("skipped: sqlite3 is not installed");
        return;
    }
    let s = Sandbox::new();
    let db = database(&s);
    let run = s.hats_env(&["workspaces"], &[("CONDUCTOR_DB", &db)]);
    let out = run.ok();

    assert_eq!(
        out.stdout.trim(),
        "alpha\t/tmp/ws/alpha\nbeta\t/tmp/ws/beta",
        "unexpected list:\n{}",
        out.stdout
    );
}

#[test]
fn an_archived_workspace_is_not_offered() {
    if !sqlite() {
        eprintln!("skipped: sqlite3 is not installed");
        return;
    }
    let s = Sandbox::new();
    let db = database(&s);
    let run = s.hats_env(&["workspaces"], &[("CONDUCTOR_DB", &db)]);
    let out = run.ok();
    assert!(
        !out.stdout.contains("gone"),
        "an archived workspace was listed:\n{}",
        out.stdout
    );
}

#[test]
fn a_repository_path_with_a_space_survives_whole() {
    if !sqlite() {
        eprintln!("skipped: sqlite3 is not installed");
        return;
    }
    let s = Sandbox::new();
    let db = database(&s);
    let run = s.hats_env(&["repos"], &[("CONDUCTOR_DB", &db)]);
    let out = run.ok();
    assert_eq!(out.stdout.trim(), "my repo\t/tmp/code/my repo");
}

#[test]
fn no_database_is_an_empty_list_rather_than_a_failure() {
    let s = Sandbox::new();
    let missing = s.path("nowhere.db");
    let path = missing.to_string_lossy().to_string();
    let run = s.hats_env(&["workspaces"], &[("CONDUCTOR_DB", &path)]);
    let out = run.ok();
    assert_eq!(out.stdout.trim(), "");
}
