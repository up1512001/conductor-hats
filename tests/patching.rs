//! Reading and rewriting a signed application.
//!
//! Every offset in a Mach-O comes out of the file being read, so a malformed one
//! has to produce an error rather than a panic or a read past the buffer. And
//! the live application may not be touched until a complete patched image
//! exists, so a refusal leaves the previous installation exactly as it was.

mod common;

use common::Sandbox;

const MH_MAGIC_64: [u8; 4] = [0xcf, 0xfa, 0xed, 0xfe];

/// A 64-bit header with `ncmds` load commands, followed by whatever is given.
fn mach_o(ncmds: u32, body: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&MH_MAGIC_64);
    out.extend_from_slice(&0x0100_000cu32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&ncmds.to_le_bytes());
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(body);
    out
}

fn app_with(s: &Sandbox, name: &str, bytes: &[u8]) -> String {
    let dir = s.workspace(&format!("{name}.app/Contents/MacOS"));
    std::fs::write(format!("{dir}/conductor"), bytes).unwrap();
    s.path(&format!("{name}.app")).to_string_lossy().to_string()
}

/// Each of these was reachable with unchecked arithmetic: a length that
/// overflows when added to an offset wraps, and the slice that follows is taken
/// from somewhere else entirely. A header with no load commands is not here
/// because it is well formed and simply holds nothing.
#[test]
fn a_malformed_binary_is_refused_rather_than_followed() {
    let s = Sandbox::new();
    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("truncated-header", MH_MAGIC_64.to_vec()),
        ("zero-length-command", mach_o(1, &0u64.to_le_bytes())),
        (
            "command-past-the-end",
            mach_o(1, &[0x19, 0, 0, 0, 0xff, 0xff, 0xff, 0x7f]),
        ),
        (
            "command-length-overflows",
            mach_o(1, &[0x19, 0, 0, 0, 0xff, 0xff, 0xff, 0xff]),
        ),
        (
            "ncmds-beyond-the-file",
            mach_o(u32::MAX, &[0x19, 0, 0, 0, 8, 0, 0, 0]),
        ),
        ("not-a-mach-o", b"#!/bin/sh\necho hello\n".to_vec()),
    ];

    for (name, bytes) in cases {
        let app = app_with(&s, name, &bytes);
        let run = s.hats(&["assets", "--app", &app]);
        assert_ne!(run.status, 0, "{name} should be refused:\n{}", run.out());
        assert!(
            !run.out().contains("panicked"),
            "{name} panicked instead of erroring:\n{}",
            run.out()
        );
    }
}

#[test]
fn patching_a_malformed_binary_changes_nothing() {
    let s = Sandbox::new();
    let bytes = mach_o(1, &[0x19, 0, 0, 0, 0xff, 0xff, 0xff, 0x7f]);
    let app = app_with(&s, "Broken", &bytes);
    let binary = format!("{app}/Contents/MacOS/conductor");

    s.hats(&["patch", "--app", &app, "--i-know"]).failed();

    assert_eq!(
        std::fs::read(&binary).unwrap(),
        bytes,
        "the application must be left exactly as it was"
    );
}

/// The bundle used to be chosen as "the largest JavaScript asset", which is a
/// guess. A build whose largest chunk is not the toolbar would have been patched
/// into silence: an application that launches, with a rewritten bundle and no
/// panel.
#[test]
fn a_binary_with_no_recognisable_bundle_is_refused() {
    let s = Sandbox::new();
    let app = app_with(&s, "Empty", &mach_o(0, &[]));

    s.hats(&["patch", "--app", &app, "--i-know"]).failed();
}

#[test]
fn revert_without_a_backup_leaves_the_binary_alone() {
    let s = Sandbox::new();
    let bytes = mach_o(0, &[]);
    let app = app_with(&s, "Untouched", &bytes);
    let binary = format!("{app}/Contents/MacOS/conductor");

    s.hats(&["revert", "--app", &app]).failed();
    assert_eq!(std::fs::read(&binary).unwrap(), bytes);
}
