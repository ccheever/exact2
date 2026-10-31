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
//! canonicalized (`/tmp` and `/private/tmp`), and by its device and inode,
//! so a case alias on a case-insensitive volume or a hard link is the same
//! file. An absent file is created, empty, when its entry is made, under the
//! registry's lock, so every entry has an identity from the start (SQLite
//! takes an empty file as a new database); each check reads it again from
//! the entry's name, so a name unlinked and made again is the new file.
//! Nothing here opens an existing file: closing that descriptor would drop
//! the other library's locks.
use std::{
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard},
};

/// Why a file was not given to the party that asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    /// It is reserved for the embedder's other SQLite library.
    Reserved,
    /// A connection of this provider has it open.
    Open,
    /// It did not exist, and could not be created.
    Create(String),
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
    /// The entry for the file at `path`, if one holds it. Each entry's
    /// identity is read again from its name: one whose name was unlinked, or
    /// now names another file, follows what the name names now.
    fn find(&mut self, path: &Path) -> (PathBuf, Option<usize>) {
        let name = name(path);
        let identity = identity(&name);
        for entry in &mut self.entries {
            entry.identity = self::identity(&entry.name);
        }
        let found = self.entries.iter().position(|entry| {
            entry.name == name || (identity.is_some() && entry.identity == identity)
        });
        (name, found)
    }

    /// An entry for the file `name` names, which no entry holds, creating it
    /// empty when it is absent; and whether it existed.
    fn insert(&mut self, name: PathBuf, hold: Hold) -> Result<(u64, bool), Refused> {
        let existed = std::fs::symlink_metadata(&name).is_ok();
        if !existed {
            // `create_new` (O_EXCL) follows no link and fails on a file made meanwhile.
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&name)
            {
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(e) => return Err(Refused::Create(format!("{}: {e}", name.display()))),
            }
        }
        self.next += 1;
        let identity = identity(&name);
        self.entries.push(Entry {
            serial: self.next,
            name,
            identity,
            hold,
        });
        Ok((self.next, existed))
    }

    fn remove(&mut self, serial: u64) {
        self.entries.retain(|entry| entry.serial != serial);
    }
}

/// Reserve the file at `path` for the embedder's other SQLite library,
/// creating it empty when it is absent; refused, saying why, while a
/// connection of this provider has it open, it is already reserved, or it
/// cannot be created. The reservation lasts until its [`Reservation`]
/// drops, which that library's holder does only after closing the file.
pub fn reserve(path: &Path) -> Result<Reservation, Refused> {
    let mut files = files();
    match files.find(path) {
        (_, Some(at)) => Err(match files.entries[at].hold {
            Hold::Reserved => Refused::Reserved,
            Hold::Open(_) => Refused::Open,
        }),
        (name, None) => {
            let (serial, existed) = files.insert(name, Hold::Reserved)?;
            Ok(Reservation { serial, existed })
        }
    }
}

/// A file reserved by [`reserve`]; dropping it ends the reservation.
#[derive(Debug)]
pub struct Reservation {
    serial: u64,
    existed: bool,
}

impl Reservation {
    /// Whether the file existed before [`reserve`]; else it made it, empty.
    pub fn existed(&self) -> bool {
        self.existed
    }
}

impl Drop for Reservation {
    fn drop(&mut self) {
        files().remove(self.serial);
    }
}

/// One connection's count of its file, taken before the file is opened and
/// released once: at a successful close, or when the connection drops.
pub(crate) struct Count(Mutex<Option<u64>>);

/// Count a connection to the file at `path`, refused while it is reserved.
/// An absent file is created here, empty, so its identity is known before
/// SQLite opens it and no alias can be reserved meanwhile.
pub(crate) fn open(path: &Path) -> Result<Count, Refused> {
    let mut files = files();
    let serial = match files.find(path) {
        (_, Some(at)) => {
            let entry = &mut files.entries[at];
            match &mut entry.hold {
                Hold::Reserved => return Err(Refused::Reserved),
                Hold::Open(count) => *count += 1,
            }
            entry.serial
        }
        (name, None) => files.insert(name, Hold::Open(1))?.0,
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
