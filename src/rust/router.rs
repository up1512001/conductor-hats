//! Standing in front of every agent Conductor starts.
//!
//! Two invariants, and neither is negotiable:
//!
//!   1. **Fail open.** Every decision runs inside `catch_unwind`, and any failure
//!      leaves the environment untouched. A broken install costs the routing,
//!      never the agent.
//!   2. **exec, never fork.** The agent's background spare host and Conductor's
//!      stdio pipes both assume a direct child.

use std::os::unix::process::CommandExt;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;
use std::process::Command;

use crate::{paths, resolve};

const DEPTH: &str = "CONDUCTOR_ACCOUNTS_DEPTH";
const ROUTING: &str = "CONDUCTOR_ACCOUNTS_ROUTING";

/// Counts generations rather than testing a flag. The variable is inherited by
/// everything the agent starts, so launching Conductor from inside a routed
/// session made a flag-based guard refuse every agent. One stale generation is
/// tolerated; a real loop trips on the second.
fn guard(agent: &str) -> Result<u32, i32> {
    let depth: u32 = std::env::var(DEPTH)
        .ok()
        .and_then(|d| d.parse().ok())
        .unwrap_or(0);
    if depth >= 2 {
        eprintln!("{agent}-router: refusing to route into itself (depth {depth})");
        return Err(70);
    }
    Ok(depth + 1)
}

pub fn run(agent: &str, args: Vec<String>) -> ! {
    let depth = match guard(agent) {
        Ok(d) => d,
        Err(code) => std::process::exit(code),
    };

    let var = paths::env_var_for(agent);
    let env_bound = std::env::var_os(var)
        .map(|v| !v.is_empty())
        .unwrap_or(false);

    let decision = catch_unwind(AssertUnwindSafe(|| {
        let dir = paths::workspace_dir();
        let session = paths::session_id(&args);
        let profile = resolve::decide(agent, &dir, session.as_deref(), env_bound);
        let binary = resolve::agent_binary(agent);
        (profile, binary)
    }))
    .unwrap_or((None, None));

    let (profile, binary) = decision;

    if let Some(profile) = profile {
        let dir = paths::profile_dir(agent, &profile);
        if dir.is_dir() {
            std::env::set_var(var, &dir);
        } else {
            eprintln!(
                "{agent}-router: profile '{profile}' has no config dir at {} \
                 (using the account already configured)",
                dir.display()
            );
        }
    }

    let binary = binary.unwrap_or_else(|| fallback_binary(agent));
    let err = Command::new(&binary)
        .args(&args)
        .env(DEPTH, depth.to_string())
        .env(ROUTING, agent)
        .exec();

    eprintln!("{agent}-router: could not exec {}: {err}", binary.display());
    std::process::exit(127);
}

/// Last resort, so a failure to locate the agent still tries the obvious place
/// rather than exiting with nothing attempted.
fn fallback_binary(agent: &str) -> PathBuf {
    paths::home()
        .join("Library/Application Support/com.conductor.app/bin")
        .join(agent)
}
