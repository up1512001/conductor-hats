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
fn compress_verified(data: &[u8]) -> Result<Vec<u8>, String> {
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

/// Enough of the frontend to be sure the chunk is the one the panel expects,
/// checked after decompressing rather than on the compressed bytes.
const ANCHORS: [&str; 2] = ["createElement", "useState"];

/// A rename does not carry the executable bit from the file it replaced.
fn copy_mode(from: &Path, to: &Path) {
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

/// Build the patched binary from `pristine`, then put it in place in one step.
///
/// The live binary is not touched until the complete patched image exists, so a
/// failure anywhere above leaves the previous installation exactly as it was.
/// Starting from `pristine` is also what keeps patching twice from stacking.
pub fn inject(binary: &Path, pristine: &Path, script: &str) -> Result<Report, String> {
    let macho = MachO::open(pristine)?;
    let asset = pick_bundle(macho.assets())?;
    let original = macho
        .data
        .get(asset.offset..asset.offset + asset.length)
        .ok_or("the asset map points outside the file")?;
    let plain = decompress(original)?;
    let text = String::from_utf8_lossy(&plain);
    if text.contains(MARKER) {
        return Err("this binary already contains the account panel".into());
    }
    if let Some(missing) = ANCHORS.iter().find(|a| !text.contains(*a)) {
        return Err(format!(
            "{} does not look like Conductor's frontend: no {missing}.\n\
             Refusing to patch rather than rewrite a chunk this does not understand.",
            asset.key
        ));
    }

    let mut merged = plain.clone();
    merged.extend_from_slice(b"\n;");
    merged.extend_from_slice(script.as_bytes());
    let packed = compress_verified(&merged)?;
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
    crate::lock::write_atomic_bytes(binary, &data)?;
    copy_mode(pristine, binary);

    Ok(Report {
        key: asset.key,
        was: asset.length,
        plain: plain.len(),
        now: packed.len(),
        headroom: asset.length - packed.len(),
    })
}

/// Lists the frontend assets, or prints one decompressed for diagnosis.
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
