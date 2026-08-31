//! The launch line printed when a rebuild does not relaunch.

use super::{scrubbed_launch, LEAKED};
use std::path::Path;

#[test]
fn the_hint_unsets_every_leaked_variable() {
    let line = scrubbed_launch(Path::new("/Applications/Example Dev.app"));
    for key in LEAKED {
        assert!(
            line.contains(&format!("-u {key}")),
            "{key} is scrubbed on relaunch but missing from the hint:\n{line}"
        );
    }
}

/// A copy-pasteable command, not a description of one. The bundle name has a
/// space in it, so the path has to stay quoted.
#[test]
fn the_hint_is_a_runnable_command() {
    let line = scrubbed_launch(Path::new("/Applications/Example Dev.app"));
    assert!(line.starts_with("env -u "), "{line}");
    assert!(
        line.ends_with("open -a '/Applications/Example Dev.app'"),
        "{line}"
    );
}
