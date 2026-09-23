//! The store's files: writes that survive a crash, whole-or-absent
//! temporaries, and reads checked against their signed cards.
//!
//! @ref LLP 1026 D11 (crash recovery; assets by digest)

use crate::envelope::{sha256_hex, FileCard};
use std::io::Write as _;
use std::path::Path;

/// Remove every `.tmp-…` left in `dir` by a write that did not finish.
pub(super) fn sweep(dir: &Path) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    for item in read.flatten() {
        if !item.file_name().to_string_lossy().starts_with(".tmp-") {
            continue;
        }
        let path = item.path();
        if path.is_dir() {
            let _ = std::fs::remove_dir_all(path);
        } else {
            let _ = std::fs::remove_file(path);
        }
    }
}

pub(super) fn regular_file(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_file())
}

pub(super) fn read_card(path: &Path, card: &FileCard) -> Result<Vec<u8>, String> {
    if !regular_file(path) {
        return Err(format!(
            "{} is not a regular file for {}",
            path.display(),
            card.name
        ));
    }
    let bytes = std::fs::read(path)
        .map_err(|e| format!("cannot read {} for {}: {e}", path.display(), card.name))?;
    if bytes.len() as u64 != card.bytes {
        return Err(format!(
            "{} is {} bytes; the signed card declared {}",
            card.name,
            bytes.len(),
            card.bytes
        ));
    }
    let digest = sha256_hex(&bytes);
    if digest != card.sha256 {
        return Err(format!(
            "{} hashes to {digest}; the signed card declared {}",
            card.name, card.sha256
        ));
    }
    Ok(bytes)
}

/// Write a new file, making its directories, and sync it to disk before
/// returning: a rename that publishes it must never outlive its bytes.
pub(super) fn write_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("cannot make {}: {e}", parent.display()))?;
    }
    let mut file =
        std::fs::File::create(path).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|e| format!("cannot write {}: {e}", path.display()))
}

/// Sync a directory's entries — a name added, renamed or removed in it —
/// to disk.
pub(super) fn sync_dir(dir: &Path) -> Result<(), String> {
    std::fs::File::open(dir)
        .and_then(|d| d.sync_all())
        .map_err(|e| format!("cannot sync {}: {e}", dir.display()))
}

/// Write a file whole or not at all, and durably: a synced temporary beside
/// it, a rename over the old one, then the directory synced so the rename
/// survives a crash too. Without the syncs a power loss can leave the new
/// name holding no bytes (LLP 1026 D11: the record is the rollback floor).
pub(super) fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("{} has no directory", path.display()))?;
    let tmp = parent.join(temporary_name());
    write_file(&tmp, bytes).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("cannot put {} in place: {e}", path.display())
    })?;
    sync_dir(parent)
}

/// A name nothing else in this directory holds: the process, the clock, and a
/// counter, so two threads of one process cannot collide either.
pub(super) fn temporary_name() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!(
        ".tmp-{}-{nanos}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    )
}
