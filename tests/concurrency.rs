//! Every workspace shares one routes file, so changing account in two of them at
//! once used to be a race: each process read the file, edited its copy and wrote
//! the whole thing back, and the slower write erased the faster one.
//!
//! These are deliberately larger than any real workload. A lost update is rare
//! enough that a handful of writers would pass by luck: with the lock removed,
//! 100 writers lost 9 routes and 10 writers lost none.

mod common;

use common::Sandbox;

/// Runs one `hats` invocation per thread and waits for all of them.
fn storm(s: &Sandbox, count: usize, each: impl Fn(usize) -> Vec<String> + Sync) {
    std::thread::scope(|scope| {
        for i in 1..=count {
            let args = each(i);
            scope.spawn(move || {
                let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
                s.hats(&borrowed);
            });
        }
    });
}

#[test]
fn a_hundred_concurrent_route_writes_all_survive() {
    let s = Sandbox::new();
    s.profile("claude", "work");
    let workspaces: Vec<String> = (1..=100).map(|i| s.workspace(&format!("w{i}"))).collect();

    storm(&s, 100, |i| {
        vec![
            "use".into(),
            "work".into(),
            "claude".into(),
            workspaces[i - 1].clone(),
        ]
    });

    let routes = s.read("accounts/routes");
    let written: Vec<&str> = routes.lines().filter(|l| l.ends_with("\twork")).collect();
    assert_eq!(written.len(), 100, "every route was written:\n{routes}");

    let mut keys: Vec<&str> = written
        .iter()
        .map(|l| l.split('\t').next().unwrap())
        .collect();
    keys.sort_unstable();
    keys.dedup();
    assert_eq!(keys.len(), 100, "and none was written twice");
}

#[test]
fn concurrent_writes_leave_no_partial_lines() {
    let s = Sandbox::new();
    s.profile("claude", "work");
    let workspaces: Vec<String> = (1..=60).map(|i| s.workspace(&format!("w{i}"))).collect();

    storm(&s, 60, |i| {
        vec![
            "use".into(),
            "work".into(),
            "claude".into(),
            workspaces[i - 1].clone(),
        ]
    });

    let routes = s.read("accounts/routes");
    for line in routes.lines() {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        assert!(
            line.starts_with('/') && line.ends_with("\twork"),
            "truncated or interleaved line {line:?} in:\n{routes}"
        );
    }
}

#[test]
fn an_unrelated_route_is_not_lost_by_a_concurrent_write() {
    let s = Sandbox::new();
    s.profile("claude", "work");
    s.profile("claude", "personal");
    let keep = s.workspace("keep");
    s.hats(&["use", "personal", "claude", &keep]).ok();

    let workspaces: Vec<String> = (1..=50).map(|i| s.workspace(&format!("w{i}"))).collect();
    storm(&s, 50, |i| {
        vec![
            "use".into(),
            "work".into(),
            "claude".into(),
            workspaces[i - 1].clone(),
        ]
    });

    let routes = s.read("accounts/routes");
    let want = format!("{keep}\tpersonal");
    assert_eq!(
        routes.lines().filter(|l| *l == want).count(),
        1,
        "the route nobody touched is still there:\n{routes}"
    );
}

#[test]
fn concurrent_session_pins_do_not_collide() {
    let s = Sandbox::new();
    s.profile("claude", "work");
    s.hats(&["use", "work", "claude", &s.workspace("ws-a")])
        .ok();

    let s = &s;
    std::thread::scope(|scope| {
        for i in 1..=50 {
            scope.spawn(move || {
                s.route("claude", "ws-a", &[&format!("--session-id=s{i}")]);
            });
        }
    });

    /* `started` is a directory beside the pins, holding what each chat's agent
     * actually took. Both are per session and both have to survive the race. */
    let dir = s.accounts().join("sessions/claude");
    let pins: Vec<_> = std::fs::read_dir(&dir)
        .expect("session pins")
        .flatten()
        .filter(|e| e.path().is_file())
        .collect();
    assert_eq!(pins.len(), 50, "every session got its own pin");
    for pin in pins {
        let body = std::fs::read_to_string(pin.path()).unwrap_or_default();
        assert_eq!(body.trim(), "work", "every pin names the routed account");
    }

    let started: Vec<_> = std::fs::read_dir(dir.join("started"))
        .expect("what each chat started on")
        .flatten()
        .collect();
    assert_eq!(
        started.len(),
        50,
        "every session recorded what it started on"
    );
    for one in started {
        let body = std::fs::read_to_string(one.path()).unwrap_or_default();
        assert_eq!(body.trim(), "work", "a chat recorded the wrong account");
    }
}

#[test]
fn a_route_change_racing_a_pin_leaves_both_readable() {
    let s = Sandbox::new();
    s.profile("claude", "work");
    s.profile("claude", "personal");
    s.hats(&["use", "work", "claude", &s.workspace("ws-a")])
        .ok();
    let other = s.workspace("ws-b");

    let s = &s;
    std::thread::scope(|scope| {
        for i in 1..=25 {
            let other = other.clone();
            scope.spawn(move || {
                s.route("claude", "ws-a", &[&format!("--session-id=p{i}")]);
            });
            scope.spawn(move || {
                s.hats(&["use", "personal", "claude", &other]);
            });
        }
    });

    s.hats(&["list"]).ok().says(&format!("{other}\tpersonal"));
}

#[test]
fn removing_an_account_takes_its_routes_with_it() {
    let s = Sandbox::new();
    s.profile("claude", "work");
    s.profile("claude", "personal");
    for i in 1..=30 {
        s.hats(&["use", "work", "claude", &s.workspace(&format!("w{i}"))])
            .ok();
    }
    s.hats(&["use", "personal", "claude", &s.workspace("other")])
        .ok();

    s.hats(&["remove", "work", "claude"]).ok();

    let routes = s.read("accounts/routes");
    assert_eq!(
        routes.lines().filter(|l| l.ends_with("\twork")).count(),
        0,
        "no route points at the removed account:\n{routes}"
    );
    assert_eq!(
        routes.lines().filter(|l| l.ends_with("\tpersonal")).count(),
        1,
        "the other account keeps its route:\n{routes}"
    );
}
