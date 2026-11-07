//! Pointing the copy's updater at nothing.
//!
//! Conductor's updater endpoint is compiled into the Tauri config in the
//! binary. Left alone, the copy downloads the vendor's build and installs it
//! over itself, which drops the panel and puts back `com.conductor.app`, so the
//! "copy" opens the real app's database and keychain. The boot guard answers
//! the frontend's checks; this covers the native "Check for Updates" path too.
//!
//! The host is swapped for a closed local port of the same length. The URL stays
//! `https`, so the config still parses, and every check fails at once.

use std::path::Path;

pub const LIVE: &[u8] = b"https://cdn.crabnebula.app/update/";
const DEAD: &[u8] = b"https://127.0.0.1:9/hats/./update/";

/// Rewrites the endpoint in place; it must appear exactly once.
pub fn disable(binary: &Path) -> Result<(), String> {
    let mut data = std::fs::read(binary).map_err(|e| format!("{}: {e}", binary.display()))?;
    let hits: Vec<usize> = data
        .windows(LIVE.len())
        .enumerate()
        .filter(|(_, w)| *w == LIVE)
        .map(|(i, _)| i)
        .collect();
    if hits.len() != 1 {
        return Err(format!(
            "expected exactly 1 updater endpoint, found {}",
            hits.len()
        ));
    }
    data[hits[0]..hits[0] + DEAD.len()].copy_from_slice(DEAD);
    std::fs::write(binary, &data).map_err(|e| format!("{}: {e}", binary.display()))?;
    println!("    updater endpoint: disabled");
    Ok(())
}

/// Whether the binary can still reach Conductor's update server.
pub fn live(binary: &Path) -> bool {
    std::fs::read(binary)
        .map(|d| d.windows(LIVE.len()).any(|w| w == LIVE))
        .unwrap_or(false)
}
