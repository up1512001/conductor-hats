//! Injecting the account panel into a Conductor copy's frontend.
//!
//! The patched value must fit where the original was, since relocating the pointer
//! is not possible. It does fit, because Conductor compresses below brotli's
//! maximum and recompressing at quality 11 wins back around 200 KB. Only
//! `value_len` changes; every offset in the file stays put.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use crate::binary_in;
use crate::macho::{Asset, MachO};

pub const MARKER: &str = "__conductorHats";

/// Distinctive enough to find the guard in a bundle, stable across releases.
pub const GUARD_MARKER: &str = "minimum-client-version check";

/// The panel, compiled in at build time. See build.rs.
pub const PANEL: &str = include_str!(concat!(env!("OUT_DIR"), "/account-ui.js"));

/// The boot guard, which has to run ahead of Conductor's own modules.
pub const GUARD: &str = include_str!(concat!(env!("OUT_DIR"), "/boot-guard.js"));

pub fn decompress(blob: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    brotli::Decompressor::new(blob, 4096)
        .read_to_end(&mut out)
        .map_err(|e| format!("brotli decompress failed: {e}"))?;
    Ok(out)
}

pub fn compress(data: &[u8]) -> Result<Vec<u8>, String> {
    let params = brotli::enc::BrotliEncoderParams {
        quality: 11,
        size_hint: data.len(),
        ..Default::default()
    };
    let mut out = Vec::new();
    {
        let mut writer = brotli::CompressorWriter::with_params(&mut out, 4096, &params);
        writer
            .write_all(data)
            .map_err(|e| format!("brotli compress failed: {e}"))?;
        /* Dropping the writer flushes, and discards any error while doing it, so
         * a stream that failed to finish would look exactly like one that did.
         * Flushing here is the only way to see that failure. */
        writer
            .flush()
            .map_err(|e| format!("brotli compress failed to finish: {e}"))?;
    }
    Ok(out)
}

/// Compresses, then decompresses the result and insists it matches.
///
/// The frontend has one chance to parse this: a stream that decodes to something
/// truncated produces an application that launches, signs clean, and paints
/// nothing. Cheap to check, and the alternative is finding out by opening it.
pub(crate) fn compress_verified(data: &[u8]) -> Result<Vec<u8>, String> {
    let packed = compress(data)?;
    let back = decompress(&packed)?;
    if back.len() != data.len() {
        return Err(format!(
            "brotli round trip lost data: {} bytes in, {} back out.\n\
             Refusing to write a bundle the frontend cannot parse.",
            data.len(),
            back.len()
        ));
    }
    if back != data {
        return Err("brotli round trip changed the bundle's contents".into());
    }
    Ok(packed)
}

/// The chunk holding the toolbar and the composer, identified by name.
///
/// It used to fall back to the largest JavaScript asset, which is a guess: the
/// largest chunk in some future build is whichever one happens to be largest,
/// and injecting the panel into it produces an application that launches with
/// no panel and a rewritten bundle. Refusing is the better failure, because it
/// says what changed.
pub fn pick_bundle(assets: Vec<Asset>) -> Result<Asset, String> {
    let js: Vec<Asset> = assets
        .into_iter()
        .filter(|a| a.key.ends_with(".js"))
        .collect();
    if js.is_empty() {
        return Err("no JavaScript assets found: is this a Conductor binary?".into());
    }
    let mut named: Vec<Asset> = js
        .into_iter()
        .filter(|a| a.key.contains("renderApp"))
        .collect();
    match named.len() {
        1 => Ok(named.pop().expect("checked length")),
        0 => Err(
            "no renderApp asset in this build, so the panel has nowhere to go.\n\
                  Conductor's bundle layout has changed; hats needs updating rather \
                  than guessing at another chunk."
                .into(),
        ),
        n => Err(format!(
            "{n} assets look like renderApp, so which one holds the toolbar is a guess.\n\
             Conductor's bundle layout has changed; hats needs updating."
        )),
    }
}

/// A rename does not carry the executable bit from the file it replaced.
pub(crate) fn copy_mode(from: &Path, to: &Path) {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(meta) = std::fs::metadata(from) {
        let mode = meta.permissions().mode();
        let _ = std::fs::set_permissions(to, std::fs::Permissions::from_mode(mode));
    }
}

pub fn backup_path(app: &Path) -> PathBuf {
    let name = app
        .file_name()
        .map(|n| n.to_string_lossy().replace(' ', "-"))
        .unwrap_or_else(|| "app".into());
    dirs_accounts_root()
        .join("ui-patch-backups")
        .join(format!("{name}.conductor.orig"))
}

pub fn dirs_accounts_root() -> PathBuf {
    std::env::var_os("CONDUCTOR_ACCOUNTS_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let home = std::env::var_os("HOME").unwrap_or_default();
            PathBuf::from(home).join(".conductor-accounts")
        })
}

pub struct Report {
    pub key: String,
    pub was: usize,
    pub plain: usize,
    pub now: usize,
    pub headroom: usize,
}

/// One asset, decompressed, by exact key.
pub fn asset_text(macho: &MachO, key: &str) -> Result<String, String> {
    let asset = macho
        .assets()
        .into_iter()
        .find(|a| a.key == key)
        .ok_or_else(|| format!("no {key} in this binary: is it a Conductor build?"))?;
    let blob = macho
        .data
        .get(asset.offset..asset.offset + asset.length)
        .ok_or("the asset map points outside the file")?;
    Ok(String::from_utf8_lossy(&decompress(blob)?).into_owned())
}

/// The module Conductor's document loads first.
///
/// Its name carries a content hash, so it is read out of index.html rather than
/// guessed. The guard belongs in this chunk and nowhere else: injected into a
/// chunk that loads later it installs after Conductor has taken its own
/// reference to `fetch`, and never sees the request it exists to watch.
pub fn pick_entry(html: &str) -> Result<String, String> {
    for tag in html.split("<script").skip(1) {
        let head = &tag[..tag.find('>').unwrap_or(tag.len())];
        if !head.contains("type=\"module\"") {
            continue;
        }
        if let Some(src) = head
            .split("src=\"")
            .nth(1)
            .and_then(|r| r.split('"').next())
        {
            return Ok(src.to_string());
        }
    }
    Err(
        "index.html loads no module script, so the guard has nowhere to go.\n\
         Conductor's document has changed; hats needs updating."
            .into(),
    )
}

/// Splice both scripts into a copy: the guard ahead of the entry module, the
/// panel onto the chunk that draws the toolbar. See `edit::apply`.
pub fn inject(binary: &Path, pristine: &Path) -> Result<Vec<Report>, String> {
    let entry = {
        let macho = MachO::open(pristine)?;
        pick_entry(&asset_text(&macho, "/index.html")?)?
    };
    crate::edit::apply(
        binary,
        pristine,
        &[
            crate::edit::Edit {
                key: Some(entry),
                prepend: true,
                checked: false,
                script: GUARD.to_string(),
            },
            crate::edit::Edit::panel(PANEL),
        ],
    )
}

/// One frontend asset out of a Conductor copy, decompressed.
///
/// The phone screen shows Conductor's own mark, and this reads it from the
/// installed application at runtime rather than carrying a copy. The repository
/// is published, and the mark is not ours to redistribute.
pub fn asset_bytes(app: &Path, pattern: &str) -> Result<Vec<u8>, String> {
    let macho = MachO::open(&binary_in(app))?;
    let asset = macho
        .assets()
        .into_iter()
        .find(|asset| asset.key.contains(pattern))
        .ok_or_else(|| format!("no asset matching {pattern}"))?;
    let blob = macho
        .data
        .get(asset.offset..asset.offset + asset.length)
        .ok_or("the asset map points outside the file")?;
    decompress(blob)
}

pub fn list(app: &Path, pattern: Option<&str>, dump: bool) -> Result<(), String> {
    let macho = MachO::open(&binary_in(app))?;
    let mut shown = 0;
    for asset in macho.assets() {
        if let Some(p) = pattern {
            if !asset.key.contains(p) {
                continue;
            }
        }
        if dump {
            let blob = macho
                .data
                .get(asset.offset..asset.offset + asset.length)
                .ok_or("the asset map points outside the file")?;
            let plain = decompress(blob)?;
            use std::io::Write;
            std::io::stdout()
                .write_all(&plain)
                .map_err(|e| format!("writing the asset: {e}"))?;
            return Ok(());
        }
        println!("{:>10}  {}", asset.length, asset.key);
        shown += 1;
    }
    println!("{shown} assets");
    Ok(())
}
