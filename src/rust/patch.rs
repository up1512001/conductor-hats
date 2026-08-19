//! Injecting the account panel into a Conductor copy's frontend.
//!
//! The patched value must fit where the original was, since relocating the pointer
//! is not possible. It does fit, because Conductor compresses below brotli's
//! maximum and recompressing at quality 11 wins back around 200 KB. Only
//! `value_len` changes; every offset in the file stays put.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use crate::macho::{Asset, MachO};

pub const MARKER: &str = "__conductorHats";

/// The panel, compiled in at build time. See build.rs.
pub const PANEL: &str = include_str!(concat!(env!("OUT_DIR"), "/account-ui.js"));

pub fn decompress(blob: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    brotli::Decompressor::new(blob, 4096)
        .read_to_end(&mut out)
        .map_err(|e| format!("brotli decompress failed: {e}"))?;
    Ok(out)
}

pub fn compress(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut params = brotli::enc::BrotliEncoderParams::default();
    params.quality = 11;
    params.size_hint = data.len();
    let mut out = Vec::new();
    {
        let mut writer = brotli::CompressorWriter::with_params(&mut out, 4096, &params);
        writer
            .write_all(data)
            .map_err(|e| format!("brotli compress failed: {e}"))?;
    }
    Ok(out)
}

/// The chunk holding the toolbar and the composer.
pub fn pick_bundle(assets: Vec<Asset>) -> Result<Asset, String> {
    let mut js: Vec<Asset> = assets
        .into_iter()
        .filter(|a| a.key.ends_with(".js"))
        .collect();
    if js.is_empty() {
        return Err("no JavaScript assets found: is this a Conductor binary?".into());
    }
    if let Some(i) = js.iter().position(|a| a.key.contains("renderApp")) {
        return Ok(js.swap_remove(i));
    }
    js.sort_by_key(|a| a.length);
    Ok(js.pop().expect("checked non-empty"))
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

/// Patch `binary` in place, starting from `pristine` so patching twice is not a
/// stack.
pub fn inject(binary: &Path, pristine: &Path, script: &str) -> Result<Report, String> {
    std::fs::copy(pristine, binary).map_err(|e| format!("restoring the pristine copy: {e}"))?;

    let macho = MachO::open(binary)?;
    let asset = pick_bundle(macho.assets())?;
    let original = macho
        .data
        .get(asset.offset..asset.offset + asset.length)
        .ok_or("the asset map points outside the file")?;
    let plain = decompress(original)?;
    if String::from_utf8_lossy(&plain).contains(MARKER) {
        return Err("this binary already contains the account panel".into());
    }

    let mut merged = plain.clone();
    merged.extend_from_slice(b"\n;");
    merged.extend_from_slice(script.as_bytes());
    let packed = compress(&merged)?;
    if packed.len() > asset.length {
        return Err(format!(
            "the patched bundle does not fit: {} > {} available.\n\
             Relocating the asset is not implemented; trim the panel.",
            packed.len(),
            asset.length
        ));
    }

    let mut data = macho.data;
    data[asset.offset..asset.offset + packed.len()].copy_from_slice(&packed);
    for byte in &mut data[asset.offset + packed.len()..asset.offset + asset.length] {
        *byte = 0;
    }
    data[asset.entry + 24..asset.entry + 32].copy_from_slice(&(packed.len() as u64).to_le_bytes());
    std::fs::write(binary, &data).map_err(|e| format!("writing {}: {e}", binary.display()))?;

    Ok(Report {
        key: asset.key,
        was: asset.length,
        plain: plain.len(),
        now: packed.len(),
        headroom: asset.length - packed.len(),
    })
}
