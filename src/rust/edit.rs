//! Rewriting frontend assets in place, one or more at a time.
//!
//! The panel is one instance of this: append a script to the chunk that draws
//! the toolbar. Diagnosis needs the general form, because finding out why a
//! patched copy paints nothing means injecting something other than the panel
//! (a no-op, an error reporter) and sometimes into a different asset.
//!
//! Every edit stays inside the slot the asset already occupies, so all offsets
//! in the file hold and several edits can be applied to one image.

use std::path::Path;

use crate::macho::MachO;
use crate::patch::{compress_verified, decompress, pick_bundle, Report, MARKER};

/// Enough of the frontend to be sure the chunk is the one the panel expects,
/// checked after decompressing rather than on the compressed bytes.
const ANCHORS: [&str; 2] = ["createElement", "useState"];

pub struct Edit {
    /// Exact asset key, or None for the chunk that draws the toolbar.
    pub key: Option<String>,
    /// Put the script before the asset rather than after it.
    pub prepend: bool,
    /// Refuse unless the asset looks like the frontend and carries no panel.
    pub checked: bool,
    pub script: String,
}

impl Edit {
    pub fn panel(script: &str) -> Self {
        Self {
            key: None,
            prepend: false,
            checked: true,
            script: script.to_string(),
        }
    }
}

/// Build a patched image from `pristine`, apply every edit, write it in one step.
///
/// The live binary is untouched until the whole image exists, so a failure
/// anywhere leaves the previous installation exactly as it was. Starting from
/// `pristine` is also what keeps patching twice from stacking.
pub fn apply(binary: &Path, pristine: &Path, edits: &[Edit]) -> Result<Vec<Report>, String> {
    let macho = MachO::open(pristine)?;
    let mut data = macho.data.clone();
    let mut reports = Vec::new();

    for edit in edits {
        let assets = macho.assets();
        let asset = match &edit.key {
            Some(key) => assets
                .into_iter()
                .find(|a| &a.key == key)
                .ok_or_else(|| format!("no asset named {key}"))?,
            None => pick_bundle(assets)?,
        };
        let original = data
            .get(asset.offset..asset.offset + asset.length)
            .ok_or("the asset map points outside the file")?;
        let plain = decompress(original)?;
        let text = String::from_utf8_lossy(&plain);
        if edit.checked {
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
        }

        let mut merged = Vec::with_capacity(plain.len() + edit.script.len() + 2);
        if edit.prepend {
            merged.extend_from_slice(edit.script.as_bytes());
            merged.extend_from_slice(b"\n;");
            merged.extend_from_slice(&plain);
        } else {
            merged.extend_from_slice(&plain);
            merged.extend_from_slice(b"\n;");
            merged.extend_from_slice(edit.script.as_bytes());
        }
        let packed = compress_verified(&merged)?;
        if packed.len() > asset.length {
            return Err(format!(
                "the patched bundle does not fit: {} > {} available.\n\
                 Relocating the asset is not implemented; trim the script.",
                packed.len(),
                asset.length
            ));
        }

        data[asset.offset..asset.offset + packed.len()].copy_from_slice(&packed);
        for byte in &mut data[asset.offset + packed.len()..asset.offset + asset.length] {
            *byte = 0;
        }
        data[asset.entry + 24..asset.entry + 32]
            .copy_from_slice(&(packed.len() as u64).to_le_bytes());

        reports.push(Report {
            key: asset.key,
            was: asset.length,
            plain: plain.len(),
            now: packed.len(),
            headroom: asset.length - packed.len(),
        });
    }

    crate::lock::write_atomic_bytes(binary, &data)?;
    crate::patch::copy_mode(pristine, binary);
    Ok(reports)
}
