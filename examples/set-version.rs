//! Sets the version everywhere it appears, in one place.
//!
//!   cargo run --example set-version -- 0.4.0     set it
//!   cargo run --example set-version -- --check   report each file, fail on skew
//!
//! The version lives in four files. They ship together, so a skew between them
//! is a bug, and tests/hygiene.rs asserts they match. This is what keeps that
//! true without anyone having to remember all four.
//!
//! An example rather than a binary, so a release build never carries it.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

struct Field {
    file: &'static str,
    prefix: &'static str,
    suffix: &'static str,
}

const FIELDS: [Field; 3] = [
    Field {
        file: "Cargo.toml",
        prefix: "version = \"",
        suffix: "\"",
    },
    Field {
        file: "package.json",
        prefix: "\"version\": \"",
        suffix: "\",",
    },
    Field {
        file: "src/panel/index.ts",
        prefix: "const VERSION = \"",
        suffix: "\";",
    },
];

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(field: &Field) -> Option<String> {
    let body = std::fs::read_to_string(root().join(field.file)).ok()?;
    body.lines().find_map(|line| {
        let trimmed = line.trim();
        Some(
            trimmed
                .strip_prefix(field.prefix)?
                .strip_suffix(field.suffix)?
                .to_string(),
        )
    })
}

/// The first heading that names a release, skipping an Unreleased one.
fn changelog() -> Option<String> {
    let body = std::fs::read_to_string(root().join("CHANGELOG.md")).ok()?;
    body.lines()
        .filter_map(|line| line.strip_prefix("## "))
        .map(|line| {
            line.split_whitespace()
                .next()
                .unwrap_or_default()
                .to_string()
        })
        .find(|entry| entry.starts_with(|c: char| c.is_ascii_digit()))
}

fn report() -> Vec<(String, String)> {
    let mut found: Vec<(String, String)> = FIELDS
        .iter()
        .map(|f| (f.file.to_string(), read(f).unwrap_or_else(|| "?".into())))
        .collect();
    found.push((
        "CHANGELOG.md".into(),
        changelog().unwrap_or_else(|| "?".into()),
    ));
    found
}

fn check() -> Result<String, String> {
    let found = report();
    for (file, version) in &found {
        println!("{file:<20} {version}");
    }
    let mut versions: Vec<&String> = found.iter().map(|(_, v)| v).collect();
    versions.sort();
    versions.dedup();
    match versions.as_slice() {
        [one] => Ok((*one).clone()),
        _ => Err("versions disagree".into()),
    }
}

/// Rewrites the one line that carries the version, leaving the rest untouched.
fn rewrite(path: &Path, matches: impl Fn(&str) -> bool, line: &str) -> Result<(), String> {
    let body = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut done = false;
    let out: Vec<String> = body
        .lines()
        .map(|existing| {
            if done || !matches(existing.trim()) {
                return existing.to_string();
            }
            done = true;
            let indent: String = existing.chars().take_while(|c| c.is_whitespace()).collect();
            format!("{indent}{line}")
        })
        .collect();
    if !done {
        return Err(format!("no version line in {}", path.display()));
    }
    let mut text = out.join("\n");
    text.push('\n');
    std::fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))
}

fn set(new: &str) -> Result<(), String> {
    let parts: Vec<&str> = new.split('.').collect();
    if parts.len() != 3
        || !parts
            .iter()
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
    {
        return Err(format!(
            "expected a semantic version like 0.3.0, got '{new}'"
        ));
    }

    for field in &FIELDS {
        let path = root().join(field.file);
        let prefix = field.prefix;
        let suffix = field.suffix;
        rewrite(
            &path,
            |line| line.starts_with(prefix) && line.ends_with(suffix),
            &format!("{prefix}{new}{suffix}"),
        )?;
    }

    let changelog_path = root().join("CHANGELOG.md");
    match rewrite(
        &changelog_path,
        |line| line == "## Unreleased",
        &format!("## {new}"),
    ) {
        Ok(()) => println!("CHANGELOG: '## Unreleased' is now '## {new}'"),
        Err(_) => eprintln!("CHANGELOG: no '## Unreleased' heading, add one for the next release"),
    }

    let _ = std::process::Command::new("cargo")
        .args(["update", "--workspace", "--quiet"])
        .current_dir(root())
        .status();
    Ok(())
}

fn main() -> ExitCode {
    let arg = std::env::args().nth(1).unwrap_or_default();
    let result = match arg.as_str() {
        "--check" => check().map(|v| println!("all agree on {v}")),
        "" => Err("usage: cargo run --example set-version -- <version> | --check".into()),
        new => set(new).and_then(|_| check().map(|v| println!("all agree on {v}"))),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("set-version: {e}");
            ExitCode::FAILURE
        }
    }
}
