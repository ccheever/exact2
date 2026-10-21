//! The crash breadcrumb (@ref LLP 1075.003.000.001 §4.4). A Rust panic
//! aborts in every shipped profile and cannot be caught, so the host leaves
//! a record of the hatch calls that were open, which the next launch reads:
//! "the last run ended while inside hatch element avatar (built, call
//! 4127)". It says "ended while inside", not "crashed in": another thread
//! may have been the cause. On in production too.
//!
//! The file is the Apple host's, byte for byte (`HatchBreadcrumb.swift`):
//! one a process, `hatch-breadcrumb-<pid>`, 4,096 bytes, mapped shared, so
//! a store reaches the page cache with no syscall and the kernel keeps the
//! page if the process dies. The process holds a lock on its file for as
//! long as it lives; a later launch reads and deletes each file it can lock.
//!
//! ```text
//!   8 slots of 512 bytes, one a session:
//!      0  u32 used        4  u32 depth      8  u32 overflow    12  u32 calls
//!     16  16 bytes: the session's label, UTF-8
//!     32  8 records of 60 bytes, a stack:
//!           0  u32 checksum (of the 56 bytes after it)
//!           4  u32 incarnation    8  u32 call    12  u8 moment   13  u8 length
//!          14  46 bytes: the hatch, as the journal names it, UTF-8
//! ```
//!
//! A push writes its record, then publishes the new depth; a pop lowers the
//! depth. A reader trusts a record only below the published depth and with a
//! valid checksum, so a death inside a push leaves the last whole record. A
//! call nested past 8 only counts (`overflow`).
//!
//! Where: `$XDG_DATA_HOME/exact/<app id>` (else `~/.local/share/…`) on
//! Linux, `%LOCALAPPDATA%\exact\<app id>` on Windows; under the agent a
//! directory in the temporary directory keyed by the app id, which its next
//! launch finds again.
#![allow(unsafe_code)]

use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::atomic::{compiler_fence, Ordering};

pub(crate) const SLOTS: usize = 8;
const SLOT_BYTES: usize = 512;
pub(crate) const DEPTH_MAX: u32 = 8;
const HEADER_BYTES: usize = 32;
const RECORD_BYTES: usize = 60;
const NAME_BYTES: usize = 46;
const LABEL_BYTES: usize = 16;
const MOMENTS: [&str; 4] = ["built", "changed", "ended", "called"];
const PREFIX: &str = "hatch-breadcrumb-";

/// This process's breadcrumb file. Dropped, it is unmapped, unlocked and
/// deleted, as a clean exit leaves it; a process that dies drops nothing.
pub(crate) struct Breadcrumb {
    path: PathBuf,
    file: Option<File>,
    map: Option<memmap2::MmapMut>,
    taken: [bool; SLOTS],
    keep: bool,
}

impl Drop for Breadcrumb {
    fn drop(&mut self) {
        // Unmapped and closed before it is deleted, as Windows requires.
        self.map.take();
        self.file.take();
        if !self.keep {
            let _ = std::fs::remove_file(&self.path);
            // A test's directory is its own (`directory`), and goes with it.
            if cfg!(test) {
                let _ = self.path.parent().map(std::fs::remove_dir);
            }
        }
    }
}

/// Where this app's breadcrumbs live.
pub(crate) fn directory(app_id: &str, agent: bool) -> PathBuf {
    let id: String = match app_id.is_empty() {
        true => "app".into(),
        false => app_id
            .chars()
            .map(|c| match c.is_ascii_alphanumeric() || "._-".contains(c) {
                true => c,
                false => '_',
            })
            .collect(),
    };
    // A test's presenters share a process: each test thread has its own.
    if cfg!(test) {
        let thread = std::thread::current().id();
        let name = format!("exact-test-breadcrumbs-{}-{thread:?}", std::process::id());
        return std::env::temp_dir().join(name);
    }
    // The painting host's own, by name: the Apple host keeps the same file
    // for the same app in the same temporary directory, and neither reads
    // the other's runs.
    if agent {
        let host = if cfg!(windows) { "windows" } else { "linux" };
        return std::env::temp_dir().join(format!("exact-agent-breadcrumbs-{id}-{host}"));
    }
    let set = |key: &str| std::env::var_os(key).filter(|v| !v.is_empty());
    let base = if cfg!(windows) {
        set("LOCALAPPDATA").map(PathBuf::from)
    } else {
        set("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| set("HOME").map(|home| PathBuf::from(home).join(".local/share")))
    };
    base.unwrap_or_else(std::env::temp_dir)
        .join("exact")
        .join(id)
}

/// Take the file's lock without waiting, for as long as it stays open: true
/// when this process now holds it. `flock` on Unix, which Android has and the
/// standard library's lock does not cover there; elsewhere the library's.
fn lock(file: &std::fs::File) -> bool {
    #[cfg(unix)]
    {
        use std::os::fd::AsRawFd;
        // SAFETY: the descriptor is this open file's, valid for the call.
        unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) == 0 }
    }
    #[cfg(not(unix))]
    {
        file.try_lock().is_ok()
    }
}

fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

fn put_u32(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

/// FNV-1a over a record's 56 bytes after its checksum.
fn checksum(record: &[u8]) -> u32 {
    record[4..RECORD_BYTES]
        .iter()
        .fold(2_166_136_261u32, |h, b| {
            (h ^ u32::from(*b)).wrapping_mul(16_777_619)
        })
}

impl Breadcrumb {
    /// Make process `pid`'s file in `directory`, locked and mapped; `None`
    /// when it cannot be: that run has no breadcrumb.
    pub(crate) fn open(directory: &Path, pid: u32) -> Option<Breadcrumb> {
        std::fs::create_dir_all(directory).ok()?;
        let path = directory.join(format!("{PREFIX}{pid}"));
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
        let file = options.open(&path).ok()?;
        file.set_len((SLOTS * SLOT_BYTES) as u64).ok()?;
        if !lock(&file) {
            return None;
        }
        // Shared, so what is stored outlives the process in the page cache.
        let map = unsafe { memmap2::MmapMut::map_mut(&file) }.ok()?;
        Some(Breadcrumb {
            path,
            file: Some(file),
            map: Some(map),
            taken: [false; SLOTS],
            keep: false,
        })
    }

    /// The process is done with its file: unmap, unlock, and delete it, or
    /// leave it as a death would (a test's stand-in for one).
    pub(crate) fn close(mut self, delete: bool) {
        self.keep = !delete;
    }

    fn slot(&mut self, index: usize) -> &mut [u8] {
        let map = self.map.as_mut().expect("mapped until it is dropped");
        &mut map[index * SLOT_BYTES..(index + 1) * SLOT_BYTES]
    }

    /// A free slot for a session, cleared, or `None` when all 8 are taken:
    /// that session runs without a breadcrumb.
    pub(crate) fn take(&mut self, label: &str) -> Option<usize> {
        let index = self.taken.iter().position(|taken| !taken)?;
        self.taken[index] = true;
        let slot = self.slot(index);
        slot.fill(0);
        let label = &label.as_bytes()[..label.len().min(LABEL_BYTES)];
        slot[16..16 + label.len()].copy_from_slice(label);
        put_u32(slot, 0, 1);
        Some(index)
    }

    /// A session's end clears and frees its own slot, and no other's.
    pub(crate) fn free(&mut self, index: usize) {
        if self.taken.get(index) == Some(&true) {
            self.slot(index).fill(0);
            self.taken[index] = false;
        }
    }

    /// A hatch call begins.
    pub(crate) fn push(&mut self, index: usize, name: &str, moment: &str, incarnation: u32) {
        let slot = self.slot(index);
        let call = u32_at(slot, 12).wrapping_add(1);
        put_u32(slot, 12, call);
        let depth = u32_at(slot, 4);
        if depth >= DEPTH_MAX {
            let overflow = u32_at(slot, 8).wrapping_add(1);
            put_u32(slot, 8, overflow);
            return;
        }
        let at = HEADER_BYTES + depth as usize * RECORD_BYTES;
        let record = &mut slot[at..at + RECORD_BYTES];
        put_u32(record, 4, incarnation);
        put_u32(record, 8, call);
        record[12] = MOMENTS.iter().position(|m| *m == moment).unwrap_or(3) as u8;
        // The name cut at a character.
        let mut length = name.len().min(NAME_BYTES);
        while !name.is_char_boundary(length) {
            length -= 1;
        }
        record[13] = length as u8;
        record[14..14 + NAME_BYTES].fill(0);
        record[14..14 + length].copy_from_slice(&name.as_bytes()[..length]);
        let sum = checksum(record);
        put_u32(record, 0, sum);
        // The depth is stored after the record it admits is whole.
        compiler_fence(Ordering::SeqCst);
        put_u32(slot, 4, depth + 1);
    }

    /// The call returned.
    pub(crate) fn pop(&mut self, index: usize) {
        let slot = self.slot(index);
        let overflow = u32_at(slot, 8);
        if overflow > 0 {
            return put_u32(slot, 8, overflow - 1);
        }
        let depth = u32_at(slot, 4);
        if depth > 0 {
            put_u32(slot, 4, depth - 1);
        }
    }

    /// Every breadcrumb in `directory` whose process is gone (its file can
    /// be locked), read and deleted: one line for each session that ended
    /// with a hatch call open, naming the innermost one whose record is whole.
    pub(crate) fn read(directory: &Path) -> Vec<String> {
        let mut files: Vec<PathBuf> = std::fs::read_dir(directory)
            .into_iter()
            .flatten()
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with(PREFIX))
            })
            .collect();
        files.sort();
        let mut lines = Vec::new();
        for path in files {
            let Ok(file) = OpenOptions::new().read(true).write(true).open(&path) else {
                continue;
            };
            // Its process still holds it: that run has not ended.
            if !lock(&file) {
                continue;
            }
            let mut bytes = Vec::new();
            let read = std::io::Read::read_to_end(&mut &file, &mut bytes);
            drop(file);
            let _ = std::fs::remove_file(&path);
            if read.is_err() || bytes.len() != SLOTS * SLOT_BYTES {
                continue;
            }
            for slot in bytes.chunks_exact(SLOT_BYTES) {
                let depth = u32_at(slot, 4);
                if u32_at(slot, 0) != 1 || depth == 0 || depth > DEPTH_MAX {
                    continue;
                }
                // The innermost record that is whole.
                let whole = (0..depth as usize)
                    .rev()
                    .map(|d| &slot[HEADER_BYTES + d * RECORD_BYTES..][..RECORD_BYTES])
                    .find(|r| u32_at(r, 0) == checksum(r) && usize::from(r[13]) <= NAME_BYTES);
                let Some(record) = whole else { continue };
                let name = String::from_utf8_lossy(&record[14..14 + usize::from(record[13])]);
                let moment = MOMENTS[usize::from(record[12]).min(3)];
                let overflow = u32_at(slot, 8);
                let label = &slot[16..16 + LABEL_BYTES];
                let label = String::from_utf8_lossy(
                    &label[..label.iter().position(|b| *b == 0).unwrap_or(LABEL_BYTES)],
                );
                let mut line = format!(
                    "the last run ended while inside hatch {name} ({moment}, call {})",
                    u32_at(record, 8)
                );
                if overflow > 0 {
                    line.push_str(&format!(" (+{overflow} nested)"));
                }
                if !label.is_empty() {
                    line.push_str(&format!(", session {label}"));
                }
                line.push_str("; if it repeats, launch with EXACT_HATCHES=off");
                lines.push(line);
            }
        }
        lines
    }

    /// The file, for a test that tears a record.
    #[cfg(test)]
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}
