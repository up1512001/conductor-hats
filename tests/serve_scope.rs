//! Which Conductor copy the phone screen is allowed to show.
//!
//! Two copies of Conductor can be installed at once, a release one and a dev
//! one, and each keeps its own database. The panel starts its listener with
//! `CONDUCTOR_DB` already pointing at the app it is injected into; a listener
//! started by hand from a terminal has no such variable and used to read every
//! copy on the machine at once, so the phone showed the release app's projects
//! while the panel that delivers replies was in the other one.

mod common;

use std::io::Write;
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use common::{Sandbox, HATS};

fn sqlite() -> bool {
    Command::new("sqlite3")
        .arg("-version")
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

/// One Conductor database where its own app would keep it.
fn conductor(s: &Sandbox, bundle: &str, workspace: &str) -> PathBuf {
    let dir = s.path("Library/Application Support").join(bundle);
    std::fs::create_dir_all(&dir).expect("an application support directory");
    let db = dir.join("conductor.db");
    let sql = format!(
        "create table workspaces (id text, workspace_path text, state text, \
           directory_name text, repository_id text, active_session_id text);\n\
         insert into workspaces values ('w-{workspace}', '/tmp/ws/{workspace}', 'ready', \
           '{workspace}', 'r-1', null);\n\
         create table repos (id text, root_path text, name text);\n\
         insert into repos values ('r-1', '/tmp/code/{workspace}', '{workspace}');\n\
         create table sessions (id text, claude_session_id text, agent_type text, \
           status text, unread_count integer, title text, context_used_percent real, \
           workspace_id text, is_hidden integer, updated_at text);\n\
         insert into sessions values ('s-{workspace}', 's-{workspace}', 'claude', \
           'idle', 0, '{workspace} chat', 0, 'w-{workspace}', 0, '2026-01-01');\n"
    );
    let out = Command::new("sqlite3")
        .arg(&db)
        .arg(&sql)
        .output()
        .expect("building the fixture database");
    assert!(out.status.success(), "sqlite3 refused the fixture");
    db
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("a free loopback port")
        .local_addr()
        .expect("its address")
        .port()
}

/// Runs the listener until it has said which app it bound to, then stops it.
///
/// `hats serve` never returns on its own, so its output goes to a file that the
/// test polls rather than to a pipe it would have to drain from another thread.
fn serve_banner(s: &Sandbox) -> String {
    let log = s.path("serve.out");
    let port = free_port().to_string();
    let mut child = Command::new(HATS)
        .args(["serve", "--host", "127.0.0.1", "--port", &port])
        .env("HOME", &s.root)
        .env("CONDUCTOR_ACCOUNTS_ROOT", s.accounts())
        .env_remove("CONDUCTOR_DB")
        .current_dir(&s.root)
        .stdin(Stdio::null())
        .stdout(Stdio::from(
            std::fs::File::create(&log).expect("the serve log"),
        ))
        .stderr(Stdio::from(
            std::fs::File::create(s.path("serve.err")).expect("the serve error log"),
        ))
        .spawn()
        .expect("starting hats serve");

    let deadline = Instant::now() + Duration::from_secs(10);
    let mut seen = String::new();
    while Instant::now() < deadline {
        seen = std::fs::read_to_string(&log).unwrap_or_default();
        if seen.contains("Conductor") {
            break;
        }
        if child.try_wait().ok().flatten().is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(40));
    }
    let _ = child.kill();
    let _ = child.wait();
    format!("{seen}{}", s.read("serve.err"))
}

#[test]
fn a_terminal_listener_shows_only_the_conductor_copy_the_phone_is_paired_with() {
    if !sqlite() {
        eprintln!("skipped: sqlite3 is not installed");
        return;
    }
    let s = Sandbox::new();
    s.hats(&["install"]).ok();
    conductor(&s, "com.conductor.app", "release");
    let dev = conductor(&s, "com.conductor.dep", "development");

    let mut bound = std::fs::File::create(s.accounts().join("serve-source")).expect("the binding");
    bound
        .write_all(dev.to_string_lossy().as_bytes())
        .expect("recording the bound app");
    drop(bound);

    let banner = serve_banner(&s);
    assert!(
        banner.contains("Showing Conductor Dev and no other Conductor copy."),
        "the listener did not bind to the paired app:\n{banner}"
    );
}

#[test]
fn an_unpaired_listener_refuses_to_expose_every_copy() {
    if !sqlite() {
        eprintln!("skipped: sqlite3 is not installed");
        return;
    }
    let s = Sandbox::new();
    s.hats(&["install"]).ok();
    conductor(&s, "com.conductor.app", "release");
    conductor(&s, "com.conductor.dep", "development");

    let banner = serve_banner(&s);
    assert!(
        banner.contains("mobile access is not paired"),
        "an unpaired listener did not fail closed:\n{banner}"
    );
}

/// The chat list itself has to be scoped, not just the banner.
#[test]
fn chats_come_from_one_conductor_copy_when_one_is_selected() {
    if !sqlite() {
        eprintln!("skipped: sqlite3 is not installed");
        return;
    }
    let s = Sandbox::new();
    s.hats(&["install"]).ok();
    conductor(&s, "com.conductor.app", "release");
    let dev = conductor(&s, "com.conductor.dep", "development");

    let both = s.hats(&["chats", "--json"]).ok().out();
    assert!(both.contains("release chat"), "fixture missing:\n{both}");
    assert!(
        both.contains("development chat"),
        "fixture missing:\n{both}"
    );

    let scoped = s
        .hats_env(
            &["chats", "--json"],
            &[("CONDUCTOR_DB", &dev.to_string_lossy())],
        )
        .ok()
        .out();
    assert!(
        scoped.contains("development chat") && !scoped.contains("release chat"),
        "a scoped listing leaked the other Conductor copy:\n{scoped}"
    );
}

/// Asset addresses change with their contents, in the path and not a query.
///
/// A query string is advisory: a proxy may drop it from its cache key, which
/// answers `/mobile.css?v=new` from whatever it stored for `/mobile.css`. A
/// fingerprinted path cannot collide with the previous build's.
#[test]
fn the_client_is_published_at_an_address_that_changes_with_its_bytes() {
    let root = common::repo();
    let page = std::fs::read_to_string(root.join("src/rust/mobile_page.rs")).unwrap();
    let html = std::fs::read_to_string(root.join("src/mobile/index.html")).unwrap();
    let serve = std::fs::read_to_string(root.join("src/rust/serve.rs")).unwrap();

    assert!(
        page.contains("format!(\"/mobile.{}.css\", fingerprint(STYLE))"),
        "the stylesheet address does not carry its fingerprint"
    );
    assert!(
        page.contains("format!(\"/mobile.{}.js\", fingerprint(SCRIPT))"),
        "the script address does not carry its fingerprint"
    );
    assert!(
        !page.contains("?v={}"),
        "a query string is not a cache key a proxy has to honour"
    );
    assert!(
        html.contains("/mobile.css") && html.contains("/mobile.js"),
        "the source page keeps the plain names the build rewrites"
    );
    assert!(
        serve.contains("path == mobile_page::css_path()")
            && serve.contains("path == mobile_page::js_path()"),
        "the versioned addresses are not routed"
    );
}

/// A queue command must never reach the Conductor copy the phone is not on.
///
/// The queue lives under hats' root and is shared by every copy on the machine,
/// so without confinement the panel injected in one app can resolve a chat in
/// the other, count a delivery against its database, or purge an item belonging
/// to it. Pairing is excluded on purpose: it chooses which copy to bind.
#[test]
fn queue_commands_are_confined_to_the_paired_conductor_copy() {
    let cli = std::fs::read_to_string(common::repo().join("src/rust/remote_cli.rs")).unwrap();
    let scope = std::fs::read_to_string(common::repo().join("src/rust/mobile_scope.rs")).unwrap();

    assert!(
        scope.contains("pub fn adopt() -> Result<source::Source, String>")
            && scope.contains("set_var(\"CONDUCTOR_DB\""),
        "there is no way to confine a process to the bound copy"
    );
    assert!(
        std::fs::read_to_string(common::repo().join("src/rust/serve.rs"))
            .unwrap()
            .contains("mobile_scope::adopt()?"),
        "the public listener can start without a single selected Conductor copy"
    );
    assert!(
        cli.contains("mobile_scope::adopt()?"),
        "queue commands never adopt the binding"
    );
    for command in ["enqueue", "take", "purge", "claim", "confirm", "next"] {
        assert!(
            cli.contains(&format!("\"{command}\",")),
            "{command} is not confined to the bound copy"
        );
    }
    let confined = cli.find("const CONFINED").expect("the confined list");
    let list = &cli[confined..cli[confined..].find(']').unwrap() + confined];
    for pairing in [
        "mobile-pair",
        "mobile-revoke",
        "mobile-status",
        "mobile-stop",
    ] {
        assert!(
            !list.contains(pairing),
            "{pairing} must stay able to see an unbound copy"
        );
    }
}

#[test]
fn an_unscoped_queue_command_fails_closed() {
    let s = Sandbox::new();
    let result = s.hats(&["remote", "purge"]);
    assert_ne!(result.status, 0, "an unpaired queue command was allowed");
    assert!(
        result.out().contains("mobile access is not paired"),
        "the failure did not explain how to establish scope:\n{}",
        result.out()
    );
}

#[test]
fn an_invalid_explicit_database_fails_closed() {
    let s = Sandbox::new();
    let missing = s.path("missing.db");
    let result = s.hats_env(
        &["remote", "purge"],
        &[("CONDUCTOR_DB", &missing.to_string_lossy())],
    );
    assert_ne!(result.status, 0, "an invalid explicit scope was allowed");
    assert!(
        result
            .out()
            .contains("does not identify a readable Conductor database"),
        "the invalid scope failure was unclear:\n{}",
        result.out()
    );
}
