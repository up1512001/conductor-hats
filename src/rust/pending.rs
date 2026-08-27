//! The account chosen while a workspace was being created.
//!
//! The composer chip is pressed before the workspace exists, so there is nothing
//! yet to attach the choice to. Binding the repository was the old answer and it
//! is the wrong one: a binding is a single value for every workspace in the
//! repository, so creating one workspace on Work and the next on Personal left
//! both on Personal, and every other workspace in that repository with it.
//!
//! The choice applies to the workspaces created after it, and stands until
//! another is made. That covers a batch created in one go and two creations in a
//! row alike, and it matches what the chip still says on screen.
//!
//! Which workspaces those are has to be exact. Letting any agent take it failed:
//! Conductor starts one with the working directory set to `/` before the
//! workspace's own, and that swallowed it. Restricting it to real workspaces is
//! still not enough, because a dozen are open at any time and each respawns an
//! agent on a resume, a model switch or a generator restart.
//!
//! So the workspaces that already existed are written down alongside the choice,
//! and only one that was not on that list may use it. A workspace that uses it
//! writes itself an ordinary route there and then, so a later choice cannot move
//! it afterwards.

use std::path::{Path, PathBuf};

use crate::{id, paths, store};

fn path(agent: &str) -> PathBuf {
    paths::accounts_root().join(format!("next-{agent}"))
}

/// Records the choice, and the workspaces that already exist, so the ones that
/// appear afterwards can be told apart from them.
pub fn set(name: &str, agent: &str) -> Result<(), String> {
    store::ensure_root()?;
    crate::profile::require(agent, name)?;
    let mut body = format!("{name}\n");
    for existing in crate::places::workspace_paths() {
        body.push_str(&existing.to_string_lossy());
        body.push('\n');
    }
    crate::lock::write_atomic(&path(agent), &body)?;
    println!("Workspaces created from now on will use '{name}'.");
    println!();
    println!("It applies to the ones made after this, until another is chosen.");
    println!("Everything that already exists keeps the account it has.");
    Ok(())
}

pub fn peek(agent: &str) -> Option<String> {
    paths::first_line(&path(agent))
}

/// Uses the choice, writing the workspace an ordinary route so the answer holds
/// even after another choice is made.
/// The choice this workspace would use, without recording anything.
///
/// The toolbar has to say the right account the moment a workspace is created,
/// which is before anything has started an agent in it. Reporting the default
/// until the first message would show one account and run another.
pub fn would_take(agent: &str, dir: &Path) -> Option<String> {
    if !crate::places::is_workspace(dir) {
        return None;
    }
    let body = std::fs::read_to_string(path(agent)).ok()?;
    let mut lines = body.lines();
    let name = id::profile_or_none(lines.next()?)?.to_string();
    let here = std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    if lines.any(|before| Path::new(before) == here) {
        return None;
    }
    Some(name)
}

pub fn take(agent: &str, dir: &Path) -> Option<String> {
    /* Used by a workspace, and only by one that did not exist when the choice was
     * made. Conductor starts an agent at `/` before the workspace's own, and every
     * workspace already open respawns agents of its own accord.
     *
     * The choice is not cleared. Several workspaces can be created from one press
     * of the chip, and all of them should come up on what it said. The route
     * written below is what fixes each of them in place. */
    let name = would_take(agent, dir)?;
    let key = dir.to_string_lossy().to_string();
    let _ = store::write_route(&key, &name);
    Some(name)
}
