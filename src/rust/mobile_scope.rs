//! Private record of which Conductor app a phone is allowed to mirror.

use std::path::PathBuf;

use crate::{auth, paths, source};

fn path() -> PathBuf {
    paths::accounts_root().join("serve-source")
}

pub fn current() -> Option<source::Source> {
    source::from_key(&paths::first_line(&path())?)
}

pub fn bind(source: &source::Source) -> Result<bool, String> {
    let changed = current().as_ref() != Some(source);
    auth::write_private(&path(), &source.key())?;
    Ok(changed)
}

pub fn matches(source: &source::Source) -> bool {
    current().as_ref() == Some(source)
}

/// Confines this process to the Conductor copy the phone is paired with.
///
/// The queue lives under hats' own root and is shared by every Conductor copy on
/// the machine, while `places` reads all of them unless told otherwise. A queue
/// command run without that confinement can therefore resolve a chat in the
/// release app, count a delivery against its database, or hand the panel in one
/// copy an item belonging to the other. Adopting the recorded binding first
/// makes every one of those a lookup that simply finds nothing.
///
/// Not for the pairing commands: those choose which copy to bind, so they have
/// to be able to see a copy that is not the bound one.
pub fn adopt() -> Result<source::Source, String> {
    if std::env::var_os("CONDUCTOR_DB").is_some() {
        return source::active()
            .ok_or_else(|| "CONDUCTOR_DB does not identify a readable Conductor database".into());
    }
    let bound = current().ok_or_else(|| {
        String::from(
            "mobile access is not paired with a Conductor app; create a pairing code first",
        )
    })?;
    std::env::set_var("CONDUCTOR_DB", bound.database());
    Ok(bound)
}

pub fn clear() {
    let _ = std::fs::remove_file(path());
}
