//! The stable public HTTPS address in front of the loopback listener.

use std::path::PathBuf;

use crate::{auth, lock, paths};

fn path() -> PathBuf {
    paths::accounts_root().join("serve-origin")
}

pub fn normalized(origin: &str) -> Result<String, String> {
    let origin = origin.trim().trim_end_matches('/');
    let authority = origin
        .strip_prefix("https://")
        .ok_or("the public origin must start with https://")?;
    if authority.is_empty()
        || authority.contains(['/', '?', '#', '@'])
        || authority.chars().any(char::is_whitespace)
    {
        return Err("the public origin must be one HTTPS hostname with no path".into());
    }
    let hostname = authority
        .strip_prefix('[')
        .and_then(|rest| rest.split_once(']').map(|(host, _)| host))
        .unwrap_or_else(|| authority.split(':').next().unwrap_or(authority))
        .to_ascii_lowercase();
    let loopback = hostname == "localhost"
        || hostname.ends_with(".localhost")
        || hostname.starts_with("127.")
        || hostname == "::1";
    if loopback {
        return Err("the pairing address must be reachable from the phone, not loopback".into());
    }
    Ok(origin.to_string())
}

pub fn configured() -> Option<String> {
    let path = path();
    let found = paths::first_line(&path)?;
    let origin = normalized(&found).ok()?;
    let _ = auth::set_private_mode(&path);
    Some(origin)
}

pub fn save(origin: &str) -> Result<String, String> {
    let origin = normalized(origin)?;
    let path = path();
    let _guard = lock::Lock::acquire(&path)?;
    auth::write_private(&path, &origin)?;
    Ok(origin)
}

pub fn public(offered: Option<&str>) -> Result<Option<String>, String> {
    if let Some(origin) = offered {
        return save(origin).map(Some);
    }
    if let Ok(origin) = std::env::var("HATS_SERVE_ORIGIN") {
        return save(&origin).map(Some);
    }
    Ok(configured())
}
