//! The routes file: one workspace path per line, tab or space, then a profile.
//!
//! `default<TAB><profile>` is the fallback when nothing else matches.

use std::path::Path;

use crate::{id, paths};

pub struct Match {
    pub profile: String,
    /// True when the route named this directory rather than a parent or the
    /// default. The most specific thing anyone can express, so it outranks a
    /// repository binding.
    pub exact: bool,
}

/// Longest matching prefix wins. A route covers a directory and everything under
/// it, never a sibling sharing a name prefix: `/a/b` must not match `/a/bc`.
///
/// Each side is compared in both its written and its fully resolved form. On
/// macOS `/var` is a symlink to `/private/var`, and a route recorded from a shell
/// keeps the logical path while a resolved one does not, so comparing only one
/// form silently matches nothing.
pub fn resolve(dir: &Path) -> Option<Match> {
    let text = std::fs::read_to_string(paths::routes_file()).ok()?;
    let forms = both_forms(dir);

    let mut best: Option<(usize, Match)> = None;
    let mut fallback: Option<String> = None;

    for line in text.lines() {
        let line = line.trim_end();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((path, profile)) = split(line) else {
            continue;
        };
        let Some(profile) = id::profile_or_none(profile) else {
            continue;
        };
        if path == "default" {
            fallback = Some(profile.to_string());
            continue;
        }
        let candidates = both_forms(Path::new(path));
        let Some(exact) = compare(&forms, &candidates) else {
            continue;
        };
        if best
            .as_ref()
            .map(|(len, _)| path.len() > *len)
            .unwrap_or(true)
        {
            best = Some((
                path.len(),
                Match {
                    profile: profile.to_string(),
                    exact,
                },
            ));
        }
    }

    best.map(|(_, m)| m).or_else(|| {
        fallback.map(|profile| Match {
            profile,
            exact: false,
        })
    })
}

fn split(line: &str) -> Option<(&str, &str)> {
    let at = line.find(['\t', ' '])?;
    let (path, rest) = line.split_at(at);
    let profile = rest.trim();
    if path.is_empty() || profile.is_empty() {
        None
    } else {
        Some((path, profile))
    }
}

/// A path as written, and as the filesystem resolves it.
fn both_forms(path: &Path) -> Vec<String> {
    let mut out = vec![path.to_string_lossy().to_string()];
    if let Ok(real) = path.canonicalize() {
        let real = real.to_string_lossy().to_string();
        if !out.contains(&real) {
            out.push(real);
        }
    }
    out
}

/// `Some(true)` when a route names the directory itself, `Some(false)` when it
/// covers a parent of it, `None` when it does not apply.
fn compare(dir: &[String], route: &[String]) -> Option<bool> {
    for d in dir {
        for r in route {
            if d == r {
                return Some(true);
            }
        }
    }
    for d in dir {
        for r in route {
            if d.starts_with(&format!("{r}/")) {
                return Some(false);
            }
        }
    }
    None
}
