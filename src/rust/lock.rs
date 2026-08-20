//! A cross-process lock, and writes that either land whole or not at all.
//!
//! Routes are one small file rewritten in full, and every workspace shares it.
//! Read-modify-write without a lock loses updates whenever two workspaces change
//! account at once, and writing in place truncates the file for anyone reading
//! it at that moment. Both are cheap to avoid at this size.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// Long enough that a slow sign-in holding the lock is not robbed of it, short
/// enough that a killed process does not wedge every later write.
const STALE_AFTER: Duration = Duration::from_secs(30);
const WAIT_FOR: Duration = Duration::from_secs(10);

pub struct Lock(PathBuf);

impl Lock {
    /// `create_new` is atomic across processes, so exactly one caller creates the
    /// file and the rest spin until it goes away.
    pub fn acquire(target: &Path) -> Result<Self, String> {
        let path = lock_path(target);
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let deadline = SystemTime::now() + WAIT_FOR;
        loop {
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(mut file) => {
                    let _ = write!(file, "{}", std::process::id());
                    return Ok(Self(path));
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    if stale(&path) {
                        let _ = std::fs::remove_file(&path);
                        continue;
                    }
                    if SystemTime::now() >= deadline {
                        return Err(format!(
                            "timed out waiting for {}: another hats process is holding it",
                            path.display()
                        ));
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(e) => return Err(format!("{}: {e}", path.display())),
            }
        }
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn lock_path(target: &Path) -> PathBuf {
    let name = target
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    target.with_file_name(format!(".{name}.lock"))
}

fn stale(path: &Path) -> bool {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .map(|t| {
            SystemTime::now()
                .duration_since(t)
                .map(|d| d > STALE_AFTER)
                .unwrap_or(false)
        })
        .unwrap_or(false)
}

/// Write to a neighbouring temporary file, flush it to disk, then rename over the
/// target. Rename is atomic within a filesystem, so a reader sees either the old
/// file or the new one, and a crash between the two leaves the old one intact.
pub fn write_atomic(target: &Path, body: &str) -> Result<(), String> {
    write_atomic_bytes(target, body.as_bytes())
}

/// The same, for a file that is not text.
pub fn write_atomic_bytes(target: &Path, body: &[u8]) -> Result<(), String> {
    let dir = target.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let name = target
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let tmp = dir.join(format!(".{name}.{}.tmp", std::process::id()));

    let result = (|| -> std::io::Result<()> {
        let mut file = std::fs::File::create(&tmp)?;
        file.write_all(body)?;
        file.sync_all()
    })();
    if let Err(e) = result {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("{}: {e}", tmp.display()));
    }

    if let Err(e) = std::fs::rename(&tmp, target) {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("{}: {e}", target.display()));
    }
    if let Ok(handle) = std::fs::File::open(dir) {
        let _ = handle.sync_all();
    }
    Ok(())
}
