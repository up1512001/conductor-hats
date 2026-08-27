//! The account chosen while a workspace was being created.
//!
//! The composer chip is pressed before the workspace exists, so there is nothing
//! yet to attach the choice to. Binding the repository was the old answer and it
//! is the wrong one: a binding is a single value for every workspace in the
//! repository, so creating one workspace on Work and the next on Personal left
//! both on Personal, and every other workspace in that repository with it.
//!
//! A one-shot instead. The next agent to start in a workspace with no account of
//! its own takes it, writes itself an ordinary route, and clears it. Two
//! creations in a row therefore keep their own accounts.

use std::path::{Path, PathBuf};

use crate::{id, paths, store};

fn path(agent: &str) -> PathBuf {
    paths::accounts_root().join(format!("next-{agent}"))
}

pub fn set(name: &str, agent: &str) -> Result<(), String> {
    store::ensure_root()?;
    crate::profile::require(agent, name)?;
    crate::lock::write_atomic(&path(agent), &format!("{name}\n"))?;
    println!("The next workspace created will use '{name}'.");
    println!();
    println!("It applies once, to that workspace alone. Everything else keeps the");
    println!("account it already has.");
    Ok(())
}

pub fn peek(agent: &str) -> Option<String> {
    paths::first_line(&path(agent))
}

pub fn clear(agent: &str) {
    let _ = std::fs::remove_file(path(agent));
}

/// Reads the choice and spends it, writing the workspace an ordinary route so
/// the answer survives being asked again.
pub fn take(agent: &str, dir: &Path) -> Option<String> {
    /* Spent by a workspace, and only by a workspace. Conductor starts an agent
     * with the working directory set to `/` before it starts the workspace's own,
     * and more at the repository root; measured, each of those took the choice and
     * left the new workspace with none. */
    if !crate::places::is_workspace(dir) {
        return None;
    }
    let found = paths::first_line(&path(agent))?;
    let name = id::profile_or_none(&found)?.to_string();
    clear(agent);
    let key = dir.to_string_lossy().to_string();
    let _ = store::write_route(&key, &name);
    Some(name)
}
