//! Moving a running chat to the account it was just switched to.
//!
//! A chat's agent takes its config directory when it starts and never reads
//! another, so a pin alone only decides the next process. Conductor starts that
//! next process itself, with `--resume`, the next time the chat is used after its
//! agent has gone, and the router then reads the pin. Transcripts are shared by
//! every profile, so the conversation carries over.
//!
//! So the agent is stopped, and only while Conductor records the chat as idle:
//! stopping one mid-turn would lose the turn. A chat that is working is left to
//! finish, and a detached waiter stops it once Conductor marks it idle.

use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::{paths, places};

const POLL: Duration = Duration::from_secs(2);
const PATIENCE: Duration = Duration::from_secs(2 * 60 * 60);

/// What happened to the chat's running agent.
pub enum Outcome {
    /// No agent is running for it; the next one starts on the new account.
    NotRunning,
    /// Already on that account.
    AlreadyThere,
    /// Stopped now; the next message brings it back on the new account.
    Restarted,
    /// Mid-turn; it moves once the turn ends.
    AfterTurn,
}

/// Moves `session`'s running agent to `profile` as soon as that is safe.
pub fn move_now_or_later(agent: &str, profile: &str, session: &str) -> Outcome {
    let want = paths::profile_dir(agent, profile);
    let stale = agents_off(agent, session, &want.to_string_lossy());
    if stale.is_empty() {
        if running(agent, session) {
            return Outcome::AlreadyThere;
        }
        return Outcome::NotRunning;
    }
    if idle(session) {
        stop(&stale);
        forget(agent, session);
        return Outcome::Restarted;
    }
    wait_detached(agent, profile, session);
    Outcome::AfterTurn
}

/// `hats respawn-when-idle <agent> <profile> <session>`, the detached half.
pub fn run(rest: &[String]) -> Result<(), String> {
    match (
        rest.first(),
        rest.get(1),
        rest.get(2).and_then(|s| crate::id::session(s)),
    ) {
        (Some(agent), Some(profile), Some(session)) => {
            wait_then_move(agent, profile, session);
            Ok(())
        }
        _ => Err("usage: hats respawn-when-idle <agent> <profile> <session>".into()),
    }
}

/// The detached half: waits for the turn to end, then stops the agent.
fn wait_then_move(agent: &str, profile: &str, session: &str) {
    let want = paths::profile_dir(agent, profile)
        .to_string_lossy()
        .to_string();
    let started = Instant::now();
    while started.elapsed() < PATIENCE {
        match crate::session::pinned(agent, session) {
            Some(other) if other != profile => return,
            _ => {}
        }
        let stale = agents_off(agent, session, &want);
        if stale.is_empty() {
            return;
        }
        if idle(session) {
            stop(&stale);
            forget(agent, session);
            return;
        }
        std::thread::sleep(POLL);
    }
}

fn wait_detached(agent: &str, profile: &str, session: &str) {
    let Ok(me) = std::env::current_exe() else {
        return;
    };
    let _ = Command::new(me)
        .args(["respawn-when-idle", agent, profile, session])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn();
}

/// Whether Conductor records the chat as idle. Unknown counts as busy.
fn idle(session: &str) -> bool {
    let sql = format!(
        "select status from sessions where id = '{session}' or claude_session_id = '{session}'"
    );
    let rows = places::rows(&sql);
    !rows.is_empty() && rows.iter().all(|s| s == "idle")
}

/// Every id this chat can appear under on an agent's command line.
fn ids(session: &str) -> Vec<String> {
    let sql = format!(
        "select id, coalesce(claude_session_id, '') from sessions \
         where id = '{session}' or claude_session_id = '{session}'"
    );
    let mut out = vec![session.to_string()];
    for row in places::rows(&sql) {
        for id in row.split('|').filter(|s| !s.is_empty()) {
            if !out.iter().any(|o| o == id) {
                out.push(id.to_string());
            }
        }
    }
    out
}

/// One running agent: its pid, the chat id on its command line, its config
/// directory and the workspace Conductor started it in.
struct Running {
    pid: String,
    session: String,
    config: String,
    workspace: String,
}

/// Every running agent Conductor started through the router.
///
/// The executable is everything before the first flag, because Conductor's copy
/// lives under "Application Support" and a space splits it. `-E` appends the
/// environment, which is where the config directory and workspace are.
fn scan(agent: &str) -> Vec<Running> {
    let Ok(out) = Command::new("ps")
        .args(["-axEww", "-o", "pid=,command="])
        .output()
    else {
        return Vec::new();
    };
    let var = format!("{}=", paths::env_var_for(agent));
    let mut found = Vec::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let line = line.trim_start();
        let Some((pid, rest)) = line.split_once(char::is_whitespace) else {
            continue;
        };
        let exe = rest.split(" --").next().unwrap_or("").trim_end();
        if !exe.ends_with(&format!("/{agent}")) {
            continue;
        }
        let words: Vec<&str> = rest.split_whitespace().collect();
        let value = |prefix: &str| {
            words
                .iter()
                .find_map(|w| w.strip_prefix(prefix))
                .unwrap_or("")
                .to_string()
        };
        let session = Some(value("--session-id="))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| value("--resume="));
        if session.is_empty() {
            continue;
        }
        found.push(Running {
            pid: pid.to_string(),
            session,
            config: value(&var),
            workspace: value("CONDUCTOR_WORKSPACE_PATH="),
        });
    }
    found
}

/// Agent processes for this chat, with their config directory.
fn agents(agent: &str, session: &str) -> Vec<(String, String)> {
    let ids = ids(session);
    scan(agent)
        .into_iter()
        .filter(|r| ids.contains(&r.session))
        .map(|r| (r.pid, r.config))
        .collect()
}

/// Moves every chat running in a workspace to `profile`, except chats set to
/// another account on their own. Returns how many were moving.
pub fn move_workspace(agent: &str, profile: &str, dir: &std::path::Path) -> usize {
    let want = paths::profile_dir(agent, profile)
        .to_string_lossy()
        .to_string();
    let here = dir.to_string_lossy().to_string();
    let mut moving = 0;
    for run in scan(agent) {
        if run.workspace.trim_end_matches('/') != here.trim_end_matches('/') {
            continue;
        }
        if same(&run.config, &want) {
            continue;
        }
        match crate::session::pinned(agent, &run.session) {
            Some(other) if other != profile => continue,
            _ => {}
        }
        moving += 1;
        if idle(&run.session) {
            stop(&[run.pid]);
            forget(agent, &run.session);
        } else {
            wait_detached(agent, profile, &run.session);
        }
    }
    moving
}

fn same(a: &str, b: &str) -> bool {
    a.trim_end_matches('/') == b.trim_end_matches('/')
}

fn running(agent: &str, session: &str) -> bool {
    !agents(agent, session).is_empty()
}

/// The chat's agents running on any account other than `want`.
fn agents_off(agent: &str, session: &str, want: &str) -> Vec<String> {
    agents(agent, session)
        .into_iter()
        .filter(|(_, dir)| !same(dir, want))
        .map(|(pid, _)| pid)
        .collect()
}

/// Forgets which account the stopped agent had, under every id the chat has.
fn forget(agent: &str, session: &str) {
    for id in ids(session) {
        crate::session::forget_started(agent, &id);
    }
}

fn stop(pids: &[String]) {
    for pid in pids {
        let _ = Command::new("kill").arg("-TERM").arg(pid).status();
    }
}
