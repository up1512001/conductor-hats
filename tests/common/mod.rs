#![allow(dead_code)]

//! One sandbox per test: its own accounts root, settings file and stub agents.
//!
//! Nothing here touches a real Conductor install, a real ~/.claude directory or
//! the keychain. Every command runs against the binary cargo just built, so the
//! tests judge the artifact rather than a copy of its logic.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The binary under test, wired up by cargo.
pub const HATS: &str = env!("CARGO_BIN_EXE_hats");

pub struct Sandbox {
    pub root: PathBuf,
}

pub struct Run {
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

impl Run {
    pub fn out(&self) -> String {
        format!("{}{}", self.stdout, self.stderr)
    }

    pub fn ok(&self) -> &Self {
        assert_eq!(
            self.status,
            0,
            "expected success, got {}:\n{}",
            self.status,
            self.out()
        );
        self
    }

    pub fn failed(&self) -> &Self {
        assert_ne!(self.status, 0, "expected a non-zero exit:\n{}", self.out());
        self
    }

    pub fn says(&self, needle: &str) -> &Self {
        assert!(
            self.out().contains(needle),
            "expected output containing {needle:?}, got:\n{}",
            self.out()
        );
        self
    }

    pub fn silent_about(&self, needle: &str) -> &Self {
        assert!(
            !self.out().contains(needle),
            "expected output without {needle:?}, got:\n{}",
            self.out()
        );
        self
    }
}

impl Sandbox {
    pub fn new() -> Self {
        let base = std::env::temp_dir();
        let root = (0..64)
            .find_map(|attempt| {
                let nanos = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.subsec_nanos())
                    .unwrap_or(0);
                let path = base.join(format!(
                    "hats-test-{}-{nanos}-{attempt}",
                    std::process::id()
                ));
                std::fs::create_dir(&path).ok().map(|_| path)
            })
            .expect("a private sandbox directory");

        let me = Self { root };
        me.write_stub("stub-claude", "CLAUDE_CONFIG_DIR");
        me.write_stub("stub-codex", "CODEX_HOME");
        for name in ["ws-a", "ws-b", "repo/.conductor"] {
            std::fs::create_dir_all(me.root.join(name)).expect("sandbox layout");
        }
        me.link_router("claude-router");
        me.link_router("codex-router");
        me.hats(&["init"]).ok();
        me
    }

    /// Echoes the config directory it was handed, which is the only thing the
    /// routing tests need to know.
    fn write_stub(&self, name: &str, var: &str) {
        let path = self.root.join(name);
        std::fs::write(
            &path,
            format!("#!/bin/sh\necho \"{var}=${{{var}:-}}\"\necho \"ARGV=$*\"\n"),
        )
        .expect("writing a stub agent");
        make_executable(&path);
    }

    /// The routers are the same binary under another name, so the sandbox needs
    /// those names to exist.
    fn link_router(&self, name: &str) {
        let link = self.root.join(name);
        let _ = std::fs::remove_file(&link);
        std::os::unix::fs::symlink(HATS, &link).expect("linking a router");
    }

    pub fn accounts(&self) -> PathBuf {
        self.root.join("accounts")
    }

    pub fn settings(&self) -> PathBuf {
        self.root.join("settings.toml")
    }

    pub fn path(&self, rest: &str) -> PathBuf {
        self.root.join(rest)
    }

    pub fn read(&self, rest: &str) -> String {
        std::fs::read_to_string(self.root.join(rest)).unwrap_or_default()
    }

    pub fn exists(&self, rest: &str) -> bool {
        self.root.join(rest).exists()
    }

    fn base(&self, program: &str) -> Command {
        let mut cmd = Command::new(program);
        cmd.env("CONDUCTOR_ACCOUNTS_ROOT", self.accounts())
            .env("CONDUCTOR_ACCT_SETTINGS_FILE", self.settings())
            .env("CONDUCTOR_ACCT_COMMANDS_DIR", self.root.join("commands"))
            .env(
                "CONDUCTOR_ACCOUNTS_CLAUDE_BIN",
                self.root.join("stub-claude"),
            )
            .env("CONDUCTOR_ACCOUNTS_CODEX_BIN", self.root.join("stub-codex"))
            .env_remove("CONDUCTOR_ACCOUNT")
            .env_remove("CONDUCTOR_WORKSPACE_PATH")
            .env_remove("CONDUCTOR_ROOT_PATH")
            .env_remove("CLAUDE_CONFIG_DIR")
            .env_remove("CODEX_HOME")
            .env_remove("CONDUCTOR_ACCOUNTS_ROUTING")
            .env_remove("CONDUCTOR_ACCOUNTS_DEPTH")
            .current_dir(&self.root);
        cmd
    }

    pub fn hats(&self, args: &[&str]) -> Run {
        self.run(self.base(HATS), args)
    }

    /// The same, with extra environment, for the overrides the router honours.
    pub fn hats_env(&self, args: &[&str], env: &[(&str, &str)]) -> Run {
        let mut cmd = self.base(HATS);
        for (k, v) in env {
            cmd.env(k, v);
        }
        self.run(cmd, args)
    }

    fn run(&self, mut cmd: Command, args: &[&str]) -> Run {
        let out = cmd.args(args).output().expect("running hats");
        Run {
            status: out.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&out.stdout).to_string(),
            stderr: String::from_utf8_lossy(&out.stderr).to_string(),
        }
    }

    /// A router invocation with the sandbox's environment and nothing of the
    /// caller's. The tests run inside a routed Conductor session themselves, so
    /// an inherited CLAUDE_CONFIG_DIR otherwise reads as a routing decision the
    /// router never made.
    pub fn router(&self, agent: &str, workspace: &str) -> Command {
        let router = self.root.join(format!("{agent}-router"));
        let workspace = self.root.join(workspace);
        let mut cmd = self.base(&router.to_string_lossy());
        cmd.env("CONDUCTOR_WORKSPACE_PATH", &workspace)
            .current_dir(&workspace);
        cmd
    }

    /// What the agent was handed, from an already-built router command.
    pub fn config_dir_from(&self, agent: &str, mut cmd: Command, args: &[&str]) -> String {
        let out = cmd.args(args).output().expect("running the router");
        let var = if agent == "codex" {
            "CODEX_HOME="
        } else {
            "CLAUDE_CONFIG_DIR="
        };
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .find_map(|l| l.strip_prefix(var))
            .unwrap_or_default()
            .to_string()
    }

    /// Runs the router the way Conductor would, and reports the config directory
    /// the agent actually received.
    pub fn route(&self, agent: &str, workspace: &str, args: &[&str]) -> String {
        let router = self.root.join(format!("{agent}-router"));
        let workspace = self.root.join(workspace);
        let mut cmd = self.base(&router.to_string_lossy());
        cmd.env("CONDUCTOR_WORKSPACE_PATH", &workspace)
            .current_dir(&workspace);
        let run = self.run(cmd, args);
        let var = if agent == "codex" {
            "CODEX_HOME="
        } else {
            "CLAUDE_CONFIG_DIR="
        };
        run.stdout
            .lines()
            .find_map(|l| l.strip_prefix(var))
            .unwrap_or_default()
            .to_string()
    }

    /// A profile that exists and has an address cached, without signing in.
    pub fn profile(&self, agent: &str, name: &str) {
        self.profile_with(agent, name, &format!("{name}@example.test"));
    }

    pub fn profile_with(&self, agent: &str, name: &str, email: &str) {
        self.bare(agent, name);
        let dir = self.accounts().join(agent).join(name);
        std::fs::write(dir.join(".label"), format!("{email}\n")).expect("a label");
    }

    /// A directory and nothing else: no credentials, no cached address.
    pub fn bare(&self, agent: &str, name: &str) {
        std::fs::create_dir_all(self.accounts().join(agent).join(name)).expect("a fake profile");
    }

    /// Credentials but no cached address, which is what a fresh sign-in looks
    /// like before the agent has written its state file.
    pub fn credentialed(&self, agent: &str, name: &str) {
        self.bare(agent, name);
        let file = if agent == "codex" {
            "auth.json"
        } else {
            ".credentials.json"
        };
        std::fs::write(
            self.accounts().join(agent).join(name).join(file),
            "{\"claudeAiOauth\":{}}\n",
        )
        .expect("credentials");
    }

    pub fn signed_in(&self, agent: &str, name: &str) {
        self.profile(agent, name);
        let file = if agent == "codex" {
            "auth.json"
        } else {
            ".credentials.json"
        };
        std::fs::write(
            self.accounts().join(agent).join(name).join(file),
            "{\"x\":1}",
        )
        .expect("credentials");
    }

    /// Replaces the stub with one that refuses whatever it is asked to do.
    pub fn failing_agent(&self) {
        let path = self.root.join("stub-claude");
        std::fs::write(&path, "#!/bin/sh\necho refusing >&2\nexit 3\n").expect("a failing stub");
        make_executable(&path);
    }

    pub fn workspace(&self, name: &str) -> String {
        let dir = self.root.join(name);
        std::fs::create_dir_all(&dir).expect("a workspace");
        dir.to_string_lossy().to_string()
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(path).expect("stat").permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(path, perms).expect("chmod");
}

/// The repository root, for the tests that read the source rather than run it.
pub fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}
