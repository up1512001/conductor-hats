//! Turning routing on and off, and the CLI's own reports.

mod common;

use common::Sandbox;

#[test]
fn install_preserves_existing_conductor_settings() {
    let s = Sandbox::new();
    std::fs::write(
        s.settings(),
        "\"$schema\" = \"https://conductor.build/schemas/settings.schema.json\"\n\n[git]\nbranch_prefix = \"feat/\"\n",
    )
    .unwrap();
    s.hats(&["install"]).ok();

    let body = std::fs::read_to_string(s.settings()).unwrap();
    assert!(
        body.contains("$schema"),
        "the schema line survives:\n{body}"
    );
    assert!(body.contains("branch_prefix = \"feat/\""), "{body}");
    assert!(body.contains("claude_code_executable_path"), "{body}");

    let key = body
        .lines()
        .position(|l| l.contains("claude_code_executable_path"));
    let table = body.lines().position(|l| l.starts_with("[git]"));
    assert!(key < table, "written above the first table:\n{body}");
}

/// Older versions symlinked the deployment at the checkout. `rm -f` on a
/// directory fails, and that left a stale copy behind.
#[test]
fn install_replaces_a_stale_deployment() {
    let s = Sandbox::new();
    s.profile("claude", "work");
    s.hats(&["install"]).ok();

    std::fs::remove_dir_all(s.accounts().join("bin")).unwrap();
    std::os::unix::fs::symlink(s.path("ws-a"), s.accounts().join("bin")).unwrap();
    s.hats(&["install"]).ok();

    let bin = s.accounts().join("bin");
    assert!(bin.is_dir(), "the deployment is a real directory");
    assert!(
        !bin.symlink_metadata().unwrap().file_type().is_symlink(),
        "not a symlink"
    );

    let out = std::process::Command::new(bin.join("hats"))
        .arg("version")
        .output()
        .expect("the deployed CLI answers");
    assert!(String::from_utf8_lossy(&out.stdout).starts_with("hats"));
}

/// What matters is that a deployment which drifted is replaced by a working one.
#[test]
fn install_redeploys_over_a_broken_copy() {
    let s = Sandbox::new();
    s.profile("claude", "work");
    s.hats(&["install"]).ok();

    let deployed = s.accounts().join("bin/hats");
    let before = std::process::Command::new(&deployed)
        .arg("version")
        .output()
        .unwrap();
    let before = String::from_utf8_lossy(&before.stdout).to_string();

    std::fs::remove_file(&deployed).unwrap();
    std::fs::write(&deployed, "#!/bin/sh\necho broken\n").unwrap();
    let mut perms = std::fs::metadata(&deployed).unwrap().permissions();
    {
        use std::os::unix::fs::PermissionsExt;
        perms.set_mode(0o755);
    }
    std::fs::set_permissions(&deployed, perms).unwrap();

    s.hats(&["install"]).ok();
    let after = std::process::Command::new(&deployed)
        .arg("version")
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&after.stdout),
        before,
        "a stale copy is overwritten"
    );
}

#[test]
fn uninstall_reverses_install() {
    let s = Sandbox::new();
    s.hats(&["install"]).ok();
    s.hats(&["uninstall"]).ok();

    let body = std::fs::read_to_string(s.settings()).unwrap_or_default();
    assert!(!body.contains("claude_code_executable_path"), "{body}");
}

#[test]
fn a_route_to_a_removed_profile_stops_resolving() {
    let s = Sandbox::new();
    s.profile("claude", "work");
    s.profile("claude", "personal");
    s.hats(&["use", "work", "claude", &s.workspace("ws-a")])
        .ok();
    s.hats(&["use", "personal", "claude", &s.workspace("ws-b")])
        .ok();
    s.hats(&["remove", "work", "claude"]).ok();

    let personal = s
        .accounts()
        .join("claude/personal")
        .to_string_lossy()
        .to_string();
    assert_eq!(
        s.route("claude", "ws-b", &[]),
        personal,
        "the other route is intact"
    );
    assert_eq!(
        s.route("claude", "ws-a", &[]),
        "",
        "the removed one resolves to nothing"
    );
}

#[test]
fn the_codex_router_swaps_codex_home() {
    let s = Sandbox::new();
    s.profile("codex", "work");
    s.hats(&["use", "work", "codex", &s.workspace("ws-a")]).ok();

    let want = s
        .accounts()
        .join("codex/work")
        .to_string_lossy()
        .to_string();
    assert_eq!(s.route("codex", "ws-a", &[]), want);
}

#[test]
fn which_reports_the_effective_account() {
    let s = Sandbox::new();
    s.profile("claude", "work");
    s.hats(&["install"]).ok();
    s.hats(&["use", "work", "claude", &s.workspace("ws-a")])
        .ok();

    s.hats(&["which", &s.workspace("ws-a")])
        .says("route:      work   (this workspace)")
        .says("effective:  work");
}

#[test]
fn status_is_one_line_per_agent_plus_the_location() {
    let s = Sandbox::new();
    s.profile("claude", "work");
    s.hats(&["install"]).ok();
    s.hats(&["use", "work", "claude", &s.workspace("ws-a")])
        .ok();

    let out = s.hats(&["status", &s.workspace("ws-a")]);
    assert_eq!(out.stdout.lines().count(), 2, "{}", out.stdout);
}

#[test]
fn doctor_flags_a_dangling_route() {
    let s = Sandbox::new();
    s.profile("claude", "work");
    s.hats(&["use", "work", "claude", &s.workspace("ws-a")])
        .ok();
    std::fs::remove_dir_all(s.accounts().join("claude/work")).unwrap();

    s.hats(&["doctor"]).says("points at missing profile 'work'");
}

#[test]
fn profile_names_are_validated() {
    let s = Sandbox::new();
    s.hats(&["add", "../escape"])
        .failed()
        .says("invalid profile name");
}

/// install.sh puts a symlink on $PATH, and the binary has to work through it.
#[test]
fn the_cli_works_through_a_symlink() {
    let s = Sandbox::new();
    s.profile("claude", "work");
    let onpath = s.workspace("onpath");
    let link = format!("{onpath}/hats");
    std::os::unix::fs::symlink(common::HATS, &link).unwrap();

    let mut cmd = std::process::Command::new(&link);
    cmd.env("CONDUCTOR_ACCOUNTS_ROOT", s.accounts())
        .env("CONDUCTOR_ACCT_SETTINGS_FILE", s.settings());
    let out = cmd.arg("version").output().expect("a symlinked CLI runs");
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("hats"));

    let mut cmd = std::process::Command::new(&link);
    cmd.env("CONDUCTOR_ACCOUNTS_ROOT", s.accounts())
        .env("CONDUCTOR_ACCT_SETTINGS_FILE", s.settings());
    let out = cmd.arg("list").output().expect("and reaches its state");
    assert!(String::from_utf8_lossy(&out.stdout).contains("work"));
}

#[test]
fn the_router_works_through_a_symlink() {
    let s = Sandbox::new();
    s.profile("claude", "work");
    let ws = s.workspace("ws-a");
    s.hats(&["use", "work", "claude", &ws]).ok();

    let onpath = s.workspace("onpath");
    let link = format!("{onpath}/claude-router");
    std::os::unix::fs::symlink(common::HATS, &link).unwrap();

    let out = std::process::Command::new(&link)
        .env("CONDUCTOR_ACCOUNTS_ROOT", s.accounts())
        .env("CONDUCTOR_ACCOUNTS_CLAUDE_BIN", s.path("stub-claude"))
        .env("CONDUCTOR_WORKSPACE_PATH", &ws)
        .env_remove("CLAUDE_CONFIG_DIR")
        .env_remove("CONDUCTOR_ACCOUNT")
        .current_dir(&ws)
        .output()
        .expect("a symlinked router runs");
    let got = String::from_utf8_lossy(&out.stdout)
        .lines()
        .find_map(|l| l.strip_prefix("CLAUDE_CONFIG_DIR="))
        .unwrap_or_default()
        .to_string();
    assert_eq!(got, s.accounts().join("claude/work").to_string_lossy());
}
