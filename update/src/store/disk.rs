//! The store's files: writes that survive a crash, whole-or-absent
//! temporaries, reads checked against their signed cards, and content kept
//! once by digest.
//!
//! @ref LLP 1026 D11 (crash recovery; assets by digest)

use crate::envelope::{sha256_hex, FileCard};
use std::collections::BTreeSet;
use std::io::Write as _;
use std::path::{Path, PathBuf};

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

/// Write a file whole or not at all: a temporary beside it, then a rename
/// over the old one. `durable` syncs the temporary before the rename and the
/// directory after it, so the rename survives a crash too; without the syncs
/// a power loss can leave the new name holding no bytes (LLP 1026 D11: the
/// record is the rollback floor).
pub(super) fn write_atomic(path: &Path, bytes: &[u8], durable: bool) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("{} has no directory", path.display()))?;
    let tmp = parent.join(temporary_name());
    let written = if durable {
        write_file(&tmp, bytes)
    } else {
        std::fs::write(&tmp, bytes).map_err(|e| format!("cannot write {}: {e}", tmp.display()))
    };
    written.inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("cannot put {} in place: {e}", path.display())
    })?;
    if durable {
        sync_dir(parent)?;
    }
    Ok(())
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

/// Content stored once by SHA-256 at `blobs/<digest>`. An entry's plan and
/// assets are hard links to it (copies where a filesystem refuses links), so
/// an asset unchanged across releases takes its bytes once however many
/// entries name it, and finding what the store holds is a path, not a scan.
pub(super) struct Blobs(PathBuf);

impl Blobs {
    pub(super) fn at(store: &Path) -> Blobs {
        Blobs(store.join("blobs"))
    }

    pub(super) fn dir(&self) -> &Path {
        &self.0
    }

    /// Whether the store holds `card`'s bytes whole: read and checked against
    /// the card, so a damaged blob is fetched again, never linked forward.
    pub(super) fn holds(&self, card: &FileCard) -> bool {
        read_card(&self.0.join(&card.sha256), card).is_ok()
    }

    /// Keep `bytes`, already checked against `digest`, whole and durable.
    pub(super) fn put(&self, digest: &str, bytes: &[u8]) -> Result<(), String> {
        std::fs::create_dir_all(&self.0)
            .map_err(|e| format!("cannot make {}: {e}", self.0.display()))?;
        write_atomic(&self.0.join(digest), bytes, true)
    }

    /// Give an entry blob `digest` at `path`: a hard link, else a synced copy.
    pub(super) fn place(&self, digest: &str, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("cannot make {}: {e}", parent.display()))?;
        }
        let blob = self.0.join(digest);
        if std::fs::hard_link(&blob, path).is_ok() {
            return Ok(());
        }
        std::fs::copy(&blob, path)
            .and_then(|_| std::fs::File::open(path)?.sync_all())
            .map_err(|e| format!("cannot place {}: {e}", path.display()))
    }

    /// Remove every blob no digest in `keep` names; temporaries of a write in
    /// flight are left alone. Best effort: a failure waits for the next pass.
    pub(super) fn collect(&self, keep: &BTreeSet<String>) {
        let Ok(read) = std::fs::read_dir(&self.0) else {
            return;
        };
        for item in read.flatten() {
            let name = item.file_name().to_string_lossy().into_owned();
            if !name.starts_with(".tmp-") && !keep.contains(&name) {
                let _ = std::fs::remove_file(item.path());
            }
        }
    }
}
