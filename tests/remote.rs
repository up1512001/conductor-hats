//! Durable mobile messages, leases, confirmation and private files.

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
         create table sessions (id text, claude_session_id text, agent_type text, workspace_id text); \
         insert into sessions values ('s1','router-s1','claude','w1');";
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
fn one_panel_holds_a_message_lease_at_a_time() {
    if !sqlite() {
        eprintln!("skipped: sqlite3 is not installed");
        return;
    }
    let s = Sandbox::new();
    let db = database(&s);
    remote(&s, &db, &["enqueue", "s1", "ship it"]);

    let first: serde_json::Value =
        serde_json::from_str(&remote(&s, &db, &["claim", "s1"])).expect("a claim");
    assert_eq!(first["message"], "ship it");
    assert_eq!(remote(&s, &db, &["claim", "s1"]).trim(), "null");

    remote(
        &s,
        &db,
        &[
            "release",
            "s1",
            first["id"].as_str().unwrap_or(""),
            first["lease"].as_str().unwrap_or(""),
        ],
    );
    let again: serde_json::Value =
        serde_json::from_str(&remote(&s, &db, &["claim", "s1"])).expect("a second claim");
    assert_eq!(again["id"], first["id"]);
    assert_ne!(again["lease"], first["lease"]);
}

#[test]
fn run_settings_are_durable_and_route_to_the_exact_chat() {
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
    let route: serde_json::Value = serde_json::from_str(&remote(&s, &db, &["next", "w1"])).unwrap();
    assert_eq!(route["session"], "s1");
    assert_eq!(route["workspace_id"], "w1");
    assert_eq!(route["repository_id"], "r1");

    let claim: serde_json::Value =
        serde_json::from_str(&remote(&s, &db, &["control-claim", "s1"])).unwrap();
    assert_eq!(claim["id"], made["id"]);
    assert_eq!(claim["value"], "gpt-5.6-sol");
    remote(
        &s,
        &db,
        &["control-complete", &serde_json::to_string(&claim).unwrap()],
    );
    assert_eq!(remote(&s, &db, &["control-claim", "s1"]).trim(), "null");
}

#[test]
fn queued_navigation_stays_inside_the_current_conductor_database() {
    if !sqlite() {
        eprintln!("skipped: sqlite3 is not installed");
        return;
    }
    let s = Sandbox::new();
    let support = s.path("Library/Application Support");
    let regular = support.join("com.conductor.app/conductor.db");
    let dev = support.join("com.conductor.dev/conductor.db");
    for (path, workspace, repository, session) in [
        (
            &regular,
            "regular-workspace",
            "regular-repository",
            "a-regular-session",
        ),
        (&dev, "dev-workspace", "dev-repository", "z-dev-session"),
    ] {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let sql = format!(
            "create table workspaces (id text, repository_id text); \
             create table sessions (id text, claude_session_id text, agent_type text, workspace_id text); \
             insert into workspaces values ('{workspace}','{repository}'); \
             insert into sessions values ('{session}','{session}','codex','{workspace}');"
        );
        assert!(std::process::Command::new("sqlite3")
            .arg(path)
            .arg(sql)
            .status()
            .unwrap()
            .success());
    }
    s.hats_env(
        &["remote", "enqueue", "a-regular-session", "regular message"],
        &[("CONDUCTOR_DB", &regular.to_string_lossy())],
    )
    .ok();
    s.hats_env(
        &["remote", "enqueue", "z-dev-session", "dev message"],
        &[("CONDUCTOR_DB", &dev.to_string_lossy())],
    )
    .ok();

    let route: serde_json::Value = serde_json::from_str(
        &s.hats_env(
            &["remote", "next", "dev-workspace"],
            &[("CONDUCTOR_DB", &dev.to_string_lossy())],
        )
        .ok()
        .out(),
    )
    .unwrap();
    assert_eq!(route["session"], "z-dev-session");
    assert_eq!(route["workspace_id"], "dev-workspace");
    let status = s
        .hats(&["remote", "mobile-status", "dev-workspace"])
        .ok()
        .json();
    assert_eq!(status["service"]["source"], "Conductor Dev");
    assert_eq!(status["service"]["running"], false);
    assert_eq!(
        s.hats_env(
            &["remote", "next", "missing-workspace"],
            &[("CONDUCTOR_DB", &dev.to_string_lossy())],
        )
        .ok()
        .out()
        .trim(),
        "null"
    );
}

#[test]
fn database_readback_is_the_delivery_acknowledgement() {
    if !sqlite() {
        eprintln!("skipped: sqlite3 is not installed");
        return;
    }
    let s = Sandbox::new();
    let db = database(&s);
    remote(&s, &db, &["enqueue", "s1", "it's ready"]);
    let claim: serde_json::Value =
        serde_json::from_str(&remote(&s, &db, &["claim", "s1"])).expect("a claim");

    let out = std::process::Command::new("sqlite3")
        .arg(&db)
        .arg(
            "insert into session_messages values ('m1','s1','user','it''s ready','now','now',null)",
        )
        .output()
        .expect("recording Conductor's acknowledgement");
    assert!(out.status.success(), "sqlite3 refused the acknowledgement");

    let confirmed: serde_json::Value = serde_json::from_str(&remote(
        &s,
        &db,
        &[
            "confirm",
            "s1",
            claim["id"].as_str().unwrap_or(""),
            claim["lease"].as_str().unwrap_or(""),
        ],
    ))
    .expect("a confirmation");
    assert_eq!(confirmed["delivered"], true);
    assert_eq!(remote(&s, &db, &["pending", "s1"]).trim(), "[]");
}

#[test]
fn identical_messages_each_need_their_own_acknowledgement() {
    if !sqlite() {
        eprintln!("skipped: sqlite3 is not installed");
        return;
    }
    let s = Sandbox::new();
    let db = database(&s);
    remote(&s, &db, &["enqueue", "s1", "again"]);
    remote(&s, &db, &["enqueue", "s1", "again"]);
    let first: serde_json::Value =
        serde_json::from_str(&remote(&s, &db, &["claim", "s1"])).unwrap();
    let out = std::process::Command::new("sqlite3")
        .arg(&db)
        .arg("insert into session_messages values ('m1','s1','user','again','now','now',null)")
        .output()
        .unwrap();
    assert!(out.status.success());
    remote(
        &s,
        &db,
        &[
            "confirm",
            "s1",
            first["id"].as_str().unwrap_or(""),
            first["lease"].as_str().unwrap_or(""),
        ],
    );

    let second: serde_json::Value =
        serde_json::from_str(&remote(&s, &db, &["claim", "s1"])).unwrap();
    assert_eq!(second["message"], "again");
    assert_ne!(second["id"], first["id"]);
}

#[cfg(unix)]
#[test]
fn queued_content_is_private_on_disk() {
    use std::os::unix::fs::PermissionsExt;

    if !sqlite() {
        eprintln!("skipped: sqlite3 is not installed");
        return;
    }
    let s = Sandbox::new();
    let db = database(&s);
    let item: serde_json::Value =
        serde_json::from_str(&remote(&s, &db, &["enqueue", "s1", "private"])).unwrap();
    let file = s
        .accounts()
        .join("remote/s1")
        .join(format!("{}.json", item["id"].as_str().unwrap_or("")));
    let file_mode = std::fs::metadata(file).unwrap().permissions().mode() & 0o777;
    let dir_mode = std::fs::metadata(s.accounts().join("remote/s1"))
        .unwrap()
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(file_mode, 0o600);
    assert_eq!(dir_mode, 0o700);
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
        .arg("insert into sessions values ('s2','router-s2','codex','w1')")
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
