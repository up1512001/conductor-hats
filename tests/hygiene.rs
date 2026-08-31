//! Rules about the repository itself: no personal data, no oversized files, no
//! version skew, and comments that are docblocks rather than commentary.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

/// Every file cargo would not have generated, as git sees them.
fn tracked() -> Vec<PathBuf> {
    let root = common::repo();
    let out = Command::new("git")
        .args(["ls-files"])
        .current_dir(&root)
        .output()
        .expect("git ls-files");
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| root.join(l))
        .filter(|p| p.is_file())
        .collect()
}

fn relative(path: &Path) -> String {
    path.strip_prefix(common::repo())
        .unwrap_or(path)
        .to_string_lossy()
        .to_string()
}

/// The version lives in four files that ship together, so a skew between any two
/// is a bug.
#[test]
fn every_file_agrees_on_the_version() {
    let root = common::repo();
    let read = |file: &str, prefix: &str, suffix: &str| -> String {
        let body = std::fs::read_to_string(root.join(file)).unwrap_or_default();
        body.lines()
            .find_map(|l| l.trim().strip_prefix(prefix)?.strip_suffix(suffix))
            .unwrap_or_else(|| panic!("no version line in {file}"))
            .to_string()
    };

    let cargo = read("Cargo.toml", "version = \"", "\"");
    let package = read("package.json", "\"version\": \"", "\",");
    let panel = read("src/panel/index.ts", "const VERSION = \"", "\";");
    let changelog = std::fs::read_to_string(root.join("CHANGELOG.md"))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| l.strip_prefix("## "))
        .map(|l| l.split_whitespace().next().unwrap_or_default().to_string())
        .find(|l| l.starts_with(|c: char| c.is_ascii_digit()))
        .expect("a released changelog entry");

    assert_eq!(cargo, package, "Cargo.toml and package.json disagree");
    assert_eq!(cargo, panel, "Cargo.toml and the panel disagree");
    assert_eq!(cargo, changelog, "Cargo.toml and the changelog disagree");
}

/// This is published, so an address or a home directory left in a file is a leak
/// rather than an untidiness. Both rules are stated positively so the test
/// carries no personal data: every example address must sit on a domain RFC 2606
/// reserves for documentation, and no path may name a real account.
///
/// One address is exempt, and only one: the maintainer contact the README
/// publishes on purpose so the Conductor team has somewhere to write.
const MAINTAINER_CONTACT: &str = "utsav@up1512001.com";

/// The same shape a mail client would accept. A masked demonstration address
/// like `fir**ast@ex**e.com` is deliberately not one, and documenting the
/// masking is the point of it appearing at all.
fn looks_like_an_address(word: &str) -> bool {
    let Some((user, host)) = word.split_once('@') else {
        return false;
    };
    let user_ok = !user.is_empty()
        && user
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "._%+-".contains(c));
    let host_ok = host
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || ".-".contains(c));
    let tld = host.rsplit('.').next().unwrap_or_default();
    user_ok
        && host_ok
        && host.contains('.')
        && tld.len() >= 2
        && tld.chars().all(|c| c.is_ascii_alphabetic())
}

/// RFC 2606 reserves these and everything under them, so a subdomain such as
/// mail.example.com is as safe to document as the bare name.
fn reserved(word: &str) -> bool {
    let host = word.split_once('@').map(|(_, h)| h).unwrap_or_default();
    let host = host.trim_end_matches(|c: char| !c.is_ascii_alphanumeric());
    for base in ["example.com", "example.org", "example.net"] {
        if host == base || host.ends_with(&format!(".{base}")) {
            return true;
        }
    }
    ["test", "example", "invalid", "localhost"]
        .iter()
        .any(|tld| host == *tld || host.ends_with(&format!(".{tld}")))
}

#[test]
fn no_personal_information_is_committed() {
    let mut leaks: Vec<String> = Vec::new();
    for path in tracked() {
        let Ok(body) = std::fs::read_to_string(&path) else {
            continue;
        };
        for (n, line) in body.lines().enumerate() {
            for word in line.split(|c: char| c.is_whitespace() || "\"'`(),;<>[]{}".contains(c)) {
                let word = word.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '@');
                if word == MAINTAINER_CONTACT || !looks_like_an_address(word) || reserved(word) {
                    continue;
                }
                leaks.push(format!("{}:{}: {word}", relative(&path), n + 1));
            }
            if let Some(at) = line.find("/Users/") {
                let rest = &line[at + "/Users/".len()..];
                let name: String = rest
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '-' || *c == '.')
                    .collect();
                if !name.is_empty() && !matches!(name.as_str(), "you" | "USER" | "username") {
                    leaks.push(format!("{}:{}: /Users/{name}", relative(&path), n + 1));
                }
            }
        }
    }
    assert!(
        leaks.is_empty(),
        "personal data is committed:\n{}",
        leaks.join("\n")
    );
}

/// AGENTS.md says no file over 300 lines, so the rule is enforced rather than
/// asserted. Nothing is exempt except build output, the lockfile, the licence
/// and documentation.
const LINE_LIMIT: usize = 300;

#[test]
fn no_file_exceeds_the_line_limit() {
    let mut over: Vec<String> = Vec::new();
    for path in tracked() {
        let name = relative(&path);
        if name.starts_with("dist/")
            || name.ends_with(".md")
            || name == "pnpm-lock.yaml"
            || name == "LICENSE"
            || name == "Cargo.lock"
        {
            continue;
        }
        let Ok(body) = std::fs::read_to_string(&path) else {
            continue;
        };
        let lines = body.lines().count();
        if lines > LINE_LIMIT {
            over.push(format!("{name} is {lines} lines, limit is {LINE_LIMIT}"));
        }
    }
    assert!(
        over.is_empty(),
        "files over the limit:\n{}",
        over.join("\n")
    );
}

/// Comments say what a file or a function is, at its top. Anything else is
/// commentary, and commentary rots. Enforced rather than asked for, because it
/// was asked for twice and drifted back both times.
#[test]
fn comments_are_docblocks_only() {
    let mut bad: Vec<String> = Vec::new();
    for path in tracked() {
        let name = relative(&path);
        let is_code = [".ts", ".mjs", ".scss", ".rs"]
            .iter()
            .any(|ext| name.ends_with(ext));
        if !is_code {
            continue;
        }
        let Ok(body) = std::fs::read_to_string(&path) else {
            continue;
        };
        for (n, line) in body.lines().enumerate() {
            let trimmed = line.trim_start();
            if !trimmed.starts_with("//") {
                continue;
            }
            if trimmed.starts_with("///") || trimmed.starts_with("//!") {
                continue;
            }
            bad.push(format!("{name}:{}: {}", n + 1, trimmed));
        }
    }
    assert!(
        bad.is_empty(),
        "docblocks only, no // comments:\n{}",
        bad.join("\n")
    );
}

#[test]
fn browser_source_is_typescript_only() {
    let javascript: Vec<String> = tracked()
        .iter()
        .map(|path| relative(path))
        .filter(|name| {
            name.starts_with("src/") && (name.ends_with(".js") || name.ends_with(".jsx"))
        })
        .collect();
    assert!(
        javascript.is_empty(),
        "hand-written browser JavaScript found:\n{}",
        javascript.join("\n")
    );
}
