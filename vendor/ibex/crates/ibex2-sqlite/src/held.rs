//! Who has each database file open in this process (Exact local patch 25).
//!
//! An embedder may link a second SQLite library beside this one. Two in one
//! process are safe only while no file is open in both: each keeps its own
//! POSIX advisory locks, and closing any descriptor of a file drops every
//! lock the process holds on it (<https://www.sqlite.org/howtocorrupt.html>,
//! §2.2.1). So a file is open here or reserved for the other library, never
//! both. [`crate::SqliteProvider::open`] refuses a reserved file and counts
//! the connections it opens; [`reserve`] refuses a file a connection has open.
//!
//! A file is one entry however it is named: by its name with the directory
//! canonicalized (`/tmp` and `/private/tmp`), and once it exists by its
//! device and inode too, so a case alias on a case-insensitive volume or a
//! hard link is the same file. An entry made before its file existed learns
//! the file's identity at the next check. Nothing here opens the file.
use std::{
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard},
};

/// Who holds a file another party asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Holder {
    /// It is reserved for the embedder's other SQLite library.
    Reserved,
    /// A connection of this provider has it open.
    Open,
}

enum Hold {
    Reserved,
    Open(usize),
}

struct Entry {
    serial: u64,
    name: PathBuf,
    identity: Option<(u64, u64)>,
    hold: Hold,
}

struct Files {
    entries: Vec<Entry>,
    next: u64,
}

static FILES: Mutex<Files> = Mutex::new(Files {
    entries: Vec::new(),
    next: 0,
});

fn files() -> MutexGuard<'static, Files> {
    FILES.lock().unwrap_or_else(|e| e.into_inner())
}

fn name(path: &Path) -> PathBuf {
    match (path.parent().map(std::fs::canonicalize), path.file_name()) {
        (Some(Ok(parent)), Some(leaf)) => parent.join(leaf),
        _ => path.to_path_buf(),
    }
}

/// The file's device and inode, read without opening it; none while it does
/// not exist, and none off Unix, where a file is known by its name alone.
fn identity(path: &Path) -> Option<(u64, u64)> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let meta = std::fs::symlink_metadata(path).ok()?;
        Some((meta.dev(), meta.ino()))
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        None
    }
}

impl Files {
    /// The entry for the file at `path`, if one holds it.
    fn find(&mut self, path: &Path) -> (PathBuf, Option<usize>) {
        let name = name(path);
        let identity = identity(&name);
        for entry in &mut self.entries {
            if entry.identity.is_none() {
                entry.identity = self::identity(&entry.name);
            }
        }
        let found = self.entries.iter().position(|entry| {
            entry.name == name || (identity.is_some() && entry.identity == identity)
        });
        (name, found)
    }

    fn insert(&mut self, name: PathBuf, hold: Hold) -> u64 {
        self.next += 1;
        let identity = identity(&name);
        self.entries.push(Entry {
            serial: self.next,
            name,
            identity,
            hold,
        });
        self.next
    }

    fn remove(&mut self, serial: u64) {
        self.entries.retain(|entry| entry.serial != serial);
    }
}

/// Reserve the file at `path` for the embedder's other SQLite library;
/// refused, naming who holds it, while a connection of this provider has it
/// open or it is already reserved. The reservation lasts until its
/// [`Reservation`] drops, which that library's holder does only after
/// closing the file.
pub fn reserve(path: &Path) -> Result<Reservation, Holder> {
    let mut files = files();
    match files.find(path) {
        (_, Some(at)) => Err(match files.entries[at].hold {
            Hold::Reserved => Holder::Reserved,
            Hold::Open(_) => Holder::Open,
        }),
        (name, None) => Ok(Reservation(files.insert(name, Hold::Reserved))),
    }
}

/// A file reserved by [`reserve`]; dropping it ends the reservation.
#[derive(Debug)]
pub struct Reservation(u64);

impl Drop for Reservation {
    fn drop(&mut self) {
        files().remove(self.0);
    }
}

/// One connection's count of its file, taken before the file is opened and
/// released once: at a successful close, or when the connection drops.
pub(crate) struct Count(Mutex<Option<u64>>);

/// Count a connection to the file at `path`, refused while it is reserved.
pub(crate) fn open(path: &Path) -> Result<Count, Holder> {
    let mut files = files();
    let serial = match files.find(path) {
        (_, Some(at)) => {
            let entry = &mut files.entries[at];
            match &mut entry.hold {
                Hold::Reserved => return Err(Holder::Reserved),
                Hold::Open(count) => *count += 1,
            }
            entry.serial
        }
        (name, None) => files.insert(name, Hold::Open(1)),
    };
    Ok(Count(Mutex::new(Some(serial))))
}

impl Count {
    pub(crate) fn release(&self) {
        let Some(serial) = self.0.lock().unwrap_or_else(|e| e.into_inner()).take() else {
            return;
        };
        let mut files = files();
        let Some(at) = files.entries.iter().position(|e| e.serial == serial) else {
            return;
        };
        if let Hold::Open(count) = &mut files.entries[at].hold {
            *count -= 1;
            if *count == 0 {
                files.remove(serial);
            }
        }
    }
}

impl Drop for Count {
    fn drop(&mut self) {
        self.release();
    }
}
