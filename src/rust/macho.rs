//! Read-only access to Conductor's embedded frontend.
//!
//! Tauri compiles the frontend into the executable. The asset map lives in
//! __DATA_CONST as 32-byte entries of `(key_ptr, key_len, value_ptr, value_len)`,
//! keys plaintext, values brotli.

use std::path::Path;

const LC_SEGMENT_64: u32 = 0x19;
const MH_MAGIC_64: u32 = 0xfeed_facf;

pub struct Segment {
    pub name: String,
    pub vmaddr: u64,
    pub vmsize: u64,
    pub fileoff: u64,
}

pub struct MachO {
    pub data: Vec<u8>,
    pub segments: Vec<Segment>,
}

/// One asset map entry, resolved to file offsets.
pub struct Asset {
    pub key: String,
    /// Where the compressed value starts in the file.
    pub offset: usize,
    /// Length the map currently records for it.
    pub length: usize,
    /// Where the entry itself starts, so `value_len` can be rewritten.
    pub entry: usize,
}

fn u32_at(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(data.get(at..at + 4)?.try_into().ok()?))
}

fn u64_at(data: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_le_bytes(data.get(at..at + 8)?.try_into().ok()?))
}

impl MachO {
    pub fn open(path: &Path) -> Result<Self, String> {
        let data = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let magic = u32_at(&data, 0).ok_or("file is too short to be a Mach-O")?;
        if magic != MH_MAGIC_64 {
            return Err(format!(
                "{} is not a 64-bit little-endian Mach-O (magic {magic:#x})",
                path.display()
            ));
        }
        let ncmds = u32_at(&data, 16).ok_or("truncated header")? as usize;
        let mut segments = Vec::new();
        let mut at = 32;
        for _ in 0..ncmds {
            let cmd = u32_at(&data, at).ok_or("truncated load command")?;
            let size = u32_at(&data, at + 4).ok_or("truncated load command")? as usize;
            if size == 0 {
                return Err("load command of zero length".into());
            }
            if cmd == LC_SEGMENT_64 {
                let raw = data.get(at + 8..at + 24).ok_or("truncated segment name")?;
                let name = String::from_utf8_lossy(raw)
                    .trim_end_matches('\0')
                    .to_string();
                segments.push(Segment {
                    name,
                    vmaddr: u64_at(&data, at + 24).ok_or("truncated vmaddr")?,
                    vmsize: u64_at(&data, at + 32).ok_or("truncated vmsize")?,
                    fileoff: u64_at(&data, at + 40).ok_or("truncated fileoff")?,
                });
            }
            at += size;
        }
        Ok(Self { data, segments })
    }

    fn segment(&self, name: &str) -> Option<&Segment> {
        self.segments.iter().find(|s| s.name == name)
    }

    /// Virtual address to file offset.
    pub fn file_offset(&self, vmaddr: u64) -> Option<usize> {
        for s in &self.segments {
            if vmaddr >= s.vmaddr && vmaddr < s.vmaddr + s.vmsize {
                return usize::try_from(s.fileoff + (vmaddr - s.vmaddr)).ok();
            }
        }
        None
    }

    /// Every asset the map describes.
    ///
    /// The map carries no label, so entries are recognised by shape and anything
    /// that does not fit is skipped rather than guessed at.
    pub fn assets(&self) -> Vec<Asset> {
        let Some(seg) = self.segment("__DATA_CONST") else {
            return Vec::new();
        };
        let start = seg.fileoff as usize;
        let end = (start + seg.vmsize as usize).min(self.data.len());
        let mut out = Vec::new();

        let mut at = start;
        while at + 32 <= end {
            if let Some(asset) = self.entry_at(at) {
                out.push(asset);
            }
            at += 8;
        }
        out
    }

    fn entry_at(&self, at: usize) -> Option<Asset> {
        let key_ptr = u64_at(&self.data, at)?;
        let key_len = u64_at(&self.data, at + 8)? as usize;
        let val_ptr = u64_at(&self.data, at + 16)?;
        let val_len = u64_at(&self.data, at + 24)? as usize;

        if !(2..=512).contains(&key_len) || !(4..=64 << 20).contains(&val_len) {
            return None;
        }
        let key_at = self.file_offset(key_ptr)?;
        let val_at = self.file_offset(val_ptr)?;
        let raw = self.data.get(key_at..key_at + key_len)?;
        let key = std::str::from_utf8(raw).ok()?;
        if !key.starts_with('/') || !key.is_ascii() {
            return None;
        }
        self.data.get(val_at..val_at + val_len)?;
        Some(Asset {
            key: key.to_string(),
            offset: val_at,
            length: val_len,
            entry: at,
        })
    }
}
