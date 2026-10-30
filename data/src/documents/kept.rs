//! Documents kept across launches (@ref LLP 1069.010.000 D3–D6).
//!
//! An app granted `doc.keep` keeps the exact file or folder a person chose
//! (D3): its `doc:/<n>/<name>` resolves again after a relaunch, in any of the
//! app's sessions (D4). Each app's kept set is a file the host names,
//! outside `app:/` and anything a module can write (D6), holding what each
//! entry needs: `<n>`, the name at the choice, the object's identity (its
//! device and inode), where it was last found, the mode the choice gave,
//! and the host's own handle on it (a security-scoped bookmark on a Mac).
//! The platform's recents are the source of truth: the host says what they
//! hold ([`held`]), and an entry they no longer hold is dropped.
//!
//! A kept item is the object chosen, never whatever is at its path: an entry
//! whose object the host cannot find at launch is forgotten ([`restore`]),
//! and the app's own atomic save carries the entry to the file it wrote
//! ([`carry`]).

use super::PREFIX;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

/// The topic `storage.fs.watch('doc:/')` records: announced to an app's
/// sessions when what it keeps changes (D5).
pub const LIST_TOPIC: &str = "doc-watch:doc:/";

/// The file or folder an entry names: its device and inode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Identity {
    /// `st_dev`.
    pub device: u64,
    /// `st_ino`.
    pub inode: u64,
}

impl Identity {
    /// The object at `path` (a final link followed, as a choice follows
    /// it) and whether it is a folder; `None` where nothing is, or on a
    /// platform with no inode.
    pub fn of(path: &Path) -> Option<(Identity, bool)> {
        Self::of_metadata(std::fs::metadata(path).ok()?)
    }

    /// The object at `relative` beneath the open folder `directory` (no link
    /// followed at the last name), and whether it is a folder: read through
    /// the descriptor, so the folder's pathname does not matter.
    pub fn at(directory: &std::fs::File, relative: &str) -> Option<(Identity, bool)> {
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            let name = std::ffi::CString::new(relative).ok()?;
            let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
            // SAFETY: an open descriptor, a NUL-terminated name, and room
            // for the stat, which fstatat filled when it returned 0.
            let stat = unsafe {
                if libc::fstatat(
                    directory.as_raw_fd(),
                    name.as_ptr(),
                    stat.as_mut_ptr(),
                    libc::AT_SYMLINK_NOFOLLOW,
                ) != 0
                {
                    return None;
                }
                stat.assume_init()
            };
            // As `MetadataExt::dev` widens it, so the two compare.
            #[allow(clippy::unnecessary_cast)]
            let device = stat.st_dev as u64;
            Some((
                Identity {
                    device,
                    inode: stat.st_ino as u64,
                },
                stat.st_mode & libc::S_IFMT == libc::S_IFDIR,
            ))
        }
        #[cfg(not(unix))]
        {
            let _ = (directory, relative);
            None
        }
    }

    /// The folder at `relative` beneath the open folder `directory`, opened
    /// without following a link at its last name.
    fn folder(directory: &std::fs::File, relative: &str) -> Option<std::fs::File> {
        #[cfg(unix)]
        {
            use std::os::fd::{AsRawFd, FromRawFd};
            let name = std::ffi::CString::new(relative).ok()?;
            let flags = libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC;
            // SAFETY: an open descriptor and a NUL-terminated name; a
            // descriptor openat returns is owned by the File made from it.
            let fd = unsafe { libc::openat(directory.as_raw_fd(), name.as_ptr(), flags) };
            (fd >= 0).then(|| unsafe { std::fs::File::from_raw_fd(fd) })
        }
        #[cfg(not(unix))]
        {
            let _ = (directory, relative);
            None
        }
    }

    /// The object an open `file` is, and whether it is a folder.
    pub fn of_file(file: &std::fs::File) -> Option<(Identity, bool)> {
        Self::of_metadata(file.metadata().ok()?)
    }

    fn of_metadata(m: std::fs::Metadata) -> Option<(Identity, bool)> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            Some((
                Identity {
                    device: m.dev(),
                    inode: m.ino(),
                },
                m.is_dir(),
            ))
        }
        #[cfg(not(unix))]
        {
            let _ = m;
            None
        }
    }
}

/// One kept document.
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    /// Its `<n>`, for good.
    pub n: u64,
    /// Its name when chosen: the `<name>` of its path.
    pub name: String,
    /// A folder, not a file.
    pub folder: bool,
    /// `read` or `readwrite`, as the choice granted it.
    pub mode: String,
    /// When it was last chosen or opened, in milliseconds since 1970.
    pub opened: u64,
    /// The object chosen.
    pub identity: Identity,
    /// Where it was last found.
    pub path: PathBuf,
    /// The host's own handle on it, opaque here (base64 of a security-scoped
    /// bookmark on a Mac); empty where the path is the handle.
    pub bookmark: String,
}

type Announce = Arc<dyn Fn() + Send + Sync>;

struct Store {
    file: PathBuf,
    /// Held while the process runs: one process of the app writes the set.
    _lock: std::fs::File,
    /// Above every `<n>` the app has given, in any launch (D4).
    next: u64,
    /// Most recently opened first.
    records: Vec<Record>,
    /// Whether this launch has found its records' objects ([`restore`]).
    restored: bool,
    /// Each session's announcement of [`LIST_TOPIC`].
    listeners: BTreeMap<u64, Announce>,
}

static STORES: Mutex<BTreeMap<Arc<str>, Store>> = Mutex::new(BTreeMap::new());

type Forgotten = Arc<dyn Fn(Identity, &Path) + Send + Sync>;
static FORGOTTEN: Mutex<Option<Forgotten>> = Mutex::new(None);

fn stores() -> MutexGuard<'static, BTreeMap<Arc<str>, Store>> {
    STORES.lock().unwrap_or_else(|e| e.into_inner())
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// `owner` is a session of `app`, whose kept set is `file`: read it if this
/// process has not, and make every `<n>` the session mints from now on one
/// the app never gave. The host calls this before the session mints
/// anything.
/// A set that exists but does not read keeps nothing this launch and is
/// left as it is: starting it again would give its numbers again. So does
/// a second process of the app, while the first holds the set.
pub fn open(owner: u64, app: &str, file: &Path) {
    let mut all = stores();
    if !all.contains_key(app) {
        let Some(store) = load(file) else {
            return;
        };
        all.insert(Arc::from(app), store);
    }
    let (key, store) = all.get_key_value(app).expect("inserted");
    super::join(owner, key, store.next - 1);
}

fn load(file: &Path) -> Option<Store> {
    std::fs::create_dir_all(file.parent()?).ok()?;
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(file.with_extension("lock"))
        .ok()?;
    lock.try_lock().ok()?;
    let read: Value = match std::fs::read(file) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .ok()
            .filter(|v: &Value| v["version"] == 1)?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Value::Null,
        Err(_) => return None,
    };
    let records: Vec<Record> = read["documents"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(record)
        .collect();
    let next = records
        .iter()
        .map(|r| r.n + 1)
        .chain(read["next"].as_u64())
        .max()
        .unwrap_or(1)
        .max(1);
    Some(Store {
        file: file.to_path_buf(),
        _lock: lock,
        next,
        records,
        restored: false,
        listeners: BTreeMap::new(),
    })
}

fn record(v: &Value) -> Option<Record> {
    Some(Record {
        n: v["n"].as_u64().filter(|n| *n > 0)?,
        name: v["name"]
            .as_str()
            .filter(|n| !n.is_empty() && !n.contains('/'))?
            .into(),
        folder: v["kind"].as_str()? == "directory",
        mode: v["mode"].as_str()?.into(),
        opened: v["opened"].as_u64().unwrap_or(0),
        identity: Identity {
            device: v["device"].as_u64()?,
            inode: v["inode"].as_u64()?,
        },
        path: PathBuf::from(v["path"].as_str()?),
        bookmark: v["bookmark"].as_str().unwrap_or_default().into(),
    })
}

/// Write the set: a new file, flushed to disk, renamed over the old, so a
/// crash leaves one or the other.
fn save(store: &Store) -> std::io::Result<()> {
    let documents: Vec<Value> = store
        .records
        .iter()
        .map(|r| {
            json!({
                "n": r.n, "name": r.name, "kind": kind(r.folder), "mode": r.mode,
                "opened": r.opened, "device": r.identity.device, "inode": r.identity.inode,
                "path": r.path.to_string_lossy(), "bookmark": r.bookmark,
            })
        })
        .collect();
    let text = json!({ "version": 1, "next": store.next, "documents": documents }).to_string();
    let parent = store
        .file
        .parent()
        .ok_or(std::io::ErrorKind::InvalidInput)?;
    let staged = parent.join(format!(".kept-{}", std::process::id()));
    std::fs::create_dir_all(parent)?;
    let mut file = std::fs::File::create(&staged)?;
    std::io::Write::write_all(&mut file, text.as_bytes())?;
    file.sync_all()?;
    std::fs::rename(&staged, &store.file)?;
    // The rename is the folder's change: flushed too, or a crash could
    // bring back the set before it, and its counter.
    std::fs::File::open(parent)?.sync_all()
}

fn kind(folder: bool) -> &'static str {
    if folder {
        "directory"
    } else {
        "file"
    }
}

/// The app `owner` belongs to and its store, or `None` when it keeps nothing.
fn store_of(all: &mut BTreeMap<Arc<str>, Store>, owner: u64) -> Option<(Arc<str>, &mut Store)> {
    let app = super::app_of(owner)?;
    let store = all.get_mut(&app)?;
    Some((app, store))
}

/// Tell `app`'s sessions that its list changed, once the lock is let go.
fn announce(store: &Store) -> Vec<Announce> {
    store.listeners.values().cloned().collect()
}

fn told(listeners: Vec<Announce>) {
    for announce in listeners {
        announce();
    }
}

/// `owner`'s announcement of [`LIST_TOPIC`]; `None` stops it.
pub fn listen(owner: u64, announce: Option<Announce>) {
    let mut all = stores();
    if let Some((_, store)) = store_of(&mut all, owner) {
        match announce {
            Some(a) => store.listeners.insert(owner, a),
            None => store.listeners.remove(&owner),
        };
    }
}

/// `owner`'s session ended.
pub fn end(owner: u64) {
    listen(owner, None);
}

/// What the host is to find for `owner`'s app this launch: every kept
/// record, the first time it is asked; nothing after [`restore`].
pub fn to_restore(owner: u64) -> Vec<Record> {
    let mut all = stores();
    match store_of(&mut all, owner) {
        Some((_, store)) if !store.restored => store.records.clone(),
        _ => Vec::new(),
    }
}

/// Where the host found each kept object this launch: `<n>`, the path, and
/// a fresh handle when the old one was stale. A record found nowhere, or
/// where something else now is (its identity differs), is forgotten (D4);
/// the others resolve again, as `owner`'s app's.
pub fn restore(owner: u64, found: &[(u64, PathBuf, Option<String>)]) {
    let mut all = stores();
    let Some((app, store)) = store_of(&mut all, owner) else {
        return;
    };
    if store.restored {
        return;
    }
    store.restored = true;
    store.records.retain_mut(|r| {
        let Some((_, path, bookmark)) = found.iter().find(|(n, ..)| *n == r.n) else {
            return false;
        };
        if !super::adopt(r.n, path, &r.name, r.identity, r.folder, owner, &app) {
            return false;
        }
        r.path = path.clone();
        if let Some(b) = bookmark {
            r.bookmark = b.clone();
        }
        true
    });
    let _ = save(store);
    let listeners = announce(store);
    drop(all);
    told(listeners);
}

/// Keep what the person chose (D3), at `real`, for `owner`'s app, with the
/// host's `bookmark` of it: the path that names it, kept. Chosen again, it
/// is the entry it already was, now the most recent. `None` when the app
/// keeps nothing, or nothing is at `real`.
pub fn keep(owner: u64, real: &Path, bookmark: &str, mode: &str) -> Option<String> {
    super::app_of(owner)?;
    let (identity, folder) = Identity::of(real)?;
    // The picker's own handle when it chose exactly this, else this item
    // alone: a route delivers a file beneath its folder's handle, and only
    // the file is kept.
    let mut minted = super::mint(real, owner)?;
    // The same name may be a kept number another object was chosen by (a
    // replacement at its path): this choice is not that one.
    let other = |n: u64| {
        stores().values().any(|s| {
            (s.records.iter()).any(|r| r.n == n && (r.identity, r.folder) != (identity, folder))
        })
    };
    if other(super::number(&minted)?) {
        minted = super::mint_as(real, owner, false)?;
    }
    let n = super::number(&minted)?;
    let (real, name, _) = super::entry(n)?;
    let mut all = stores();
    let (app, store) = store_of(&mut all, owner)?;
    let at = store
        .records
        .iter()
        .position(|r| r.identity == identity && r.folder == folder);
    let mut r = match at {
        Some(i) => store.records.remove(i),
        None => {
            super::keep(n, &app);
            Record {
                n,
                name,
                folder,
                mode: String::new(),
                opened: 0,
                identity,
                path: real.clone(),
                bookmark: String::new(),
            }
        }
    };
    // Kept before this launch restored it: it resolves from now.
    if r.n != n && !super::adopt(r.n, &real, &r.name, identity, folder, owner, &app) {
        return None;
    }
    r.mode = mode.into();
    r.opened = now();
    r.path = real;
    r.bookmark = bookmark.into();
    let path = format!("{PREFIX}{}/{}", r.n, r.name);
    store.next = store.next.max(r.n + 1);
    store.records.insert(0, r);
    if save(store).is_err() {
        // Not written is not kept: the next launch would not know it.
        store.records.remove(0);
        return None;
    }
    let listeners = announce(store);
    drop(all);
    told(listeners);
    Some(path)
}

/// The platform's list holds the objects at `recent` (D6): an entry it does
/// not hold is dropped, as when the person cleared it or it fell off the end.
pub fn held(owner: u64, recent: &[PathBuf]) {
    let held: Vec<(Identity, bool)> = recent.iter().filter_map(|p| Identity::of(p)).collect();
    let mut all = stores();
    let Some((_, store)) = store_of(&mut all, owner) else {
        return;
    };
    let before = store.records.len();
    store
        .records
        .retain(|r| held.contains(&(r.identity, r.folder)));
    if store.records.len() == before {
        return;
    }
    let _ = save(store);
    let listeners = announce(store);
    drop(all);
    told(listeners);
}

/// `storage.fs.recentDocuments()` (D5): `owner`'s app's kept documents, most
/// recent first, without reading them. `None` when it keeps nothing.
pub fn list(owner: u64) -> Option<Value> {
    let mut all = stores();
    let (_, store) = store_of(&mut all, owner)?;
    Some(Value::Array(
        store
            .records
            .iter()
            .map(|r| {
                json!({
                    "path": format!("{PREFIX}{}/{}", r.n, r.name),
                    "name": r.name,
                    "kind": kind(r.folder),
                    "mode": r.mode,
                    "opened": r.opened,
                })
            })
            .collect(),
    ))
}

/// `storage.fs.forgetDocument(path)` (D5): the app's own removal of a kept
/// document. The host hears it ([`on_forgotten`]) and takes it off the
/// platform's list; a session reading it keeps it until the app quits.
pub fn forget(owner: u64, path: &str) -> std::io::Result<()> {
    let gone = || {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("{path} is not a document this app keeps"),
        )
    };
    let n = super::number(path).ok_or_else(gone)?;
    let mut all = stores();
    let (_, store) = store_of(&mut all, owner).ok_or_else(gone)?;
    let at = store
        .records
        .iter()
        .position(|r| {
            format!("{PREFIX}{}/{}", r.n, r.name) == path.trim_end_matches('/') && r.n == n
        })
        .ok_or_else(gone)?;
    let r = store.records.remove(at);
    if let Err(e) = save(store) {
        store.records.insert(at, r);
        return Err(e);
    }
    let listeners = announce(store);
    drop(all);
    told(listeners);
    let hook = FORGOTTEN.lock().unwrap_or_else(|e| e.into_inner()).clone();
    if let Some(hook) = hook {
        hook(r.identity, &r.path);
    }
    Ok(())
}

/// Where the host takes a document the app forgot off the platform's list:
/// told the object it was and where it was last found.
pub fn on_forgotten(hook: Forgotten) {
    *FORGOTTEN.lock().unwrap_or_else(|e| e.into_inner()) = Some(hook);
}

/// What an atomic write of `path` replaces: the kept files it is (by their
/// identity now, read through the folder the write goes through, whichever
/// handle names it: the kept file's own or a route's folder), for [`carry`]
/// after. `None` when it replaces no kept file.
pub fn before_write(owner: u64, path: &str) -> Option<Replaced> {
    let app = super::app_of(owner)?;
    let (directory, relative) = super::target(path, owner)?;
    // The folder the file is in, held now: a write beneath a chosen folder
    // goes through it, wherever it is moved during the write.
    let (directory, relative) = match relative.rsplit_once('/') {
        Some((folder, leaf)) => (
            Arc::new(Identity::folder(&directory, folder)?),
            leaf.to_owned(),
        ),
        None => (directory, relative),
    };
    let (identity, false) = Identity::at(&directory, &relative)? else {
        return None;
    };
    let all = stores();
    let kept: Vec<u64> = (all.get(&app)?.records.iter())
        .filter(|r| r.identity == identity && !r.folder)
        .map(|r| r.n)
        .collect();
    (!kept.is_empty()).then_some(Replaced {
        kept,
        directory,
        relative,
    })
}

/// The kept files an atomic write is replacing, and where it writes.
#[derive(Clone)]
pub struct Replaced {
    kept: Vec<u64>,
    directory: Arc<std::fs::File>,
    relative: String,
}

/// The app's own atomic write replaced kept files (D4): each is the file it
/// wrote now, read through the same folder.
pub fn carry(owner: u64, replaced: Replaced) {
    let Some((after, false)) = Identity::at(&replaced.directory, &replaced.relative) else {
        return;
    };
    let mut all = stores();
    let Some((_, store)) = store_of(&mut all, owner) else {
        return;
    };
    for r in store
        .records
        .iter_mut()
        .filter(|r| replaced.kept.contains(&r.n))
    {
        r.identity = after;
    }
    let _ = save(store);
}

/// `<n>` was minted for `app`'s session: its counter passes it, for good.
/// False when that could not be written, and the number may not be given.
pub(super) fn given(app: &Arc<str>, n: u64) -> bool {
    let mut all = stores();
    let Some(store) = all.get_mut(app) else {
        return true;
    };
    if n < store.next {
        return true;
    }
    let was = store.next;
    store.next = n + 1;
    if save(store).is_err() {
        store.next = was;
        return false;
    }
    true
}

/// A relaunch: the process forgets `app`'s set.
#[cfg(all(test, unix))]
pub(super) fn drop_store(app: &str) {
    stores().remove(app);
}

#[cfg(test)]
mod tests;
