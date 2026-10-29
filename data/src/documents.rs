//! Documents the person chose (LLP 1069.010 D1): a `doc:/<n>/<name>` path
//! names a file or folder a host minted when the person picked it, opened
//! it from the system, or typed it into the app's `open-file` field. The
//! real location stays here, behind `<n>`; an app holds only the name and
//! reaches the bytes through `storage.fs` under `fs.read doc:/` or
//! `fs.write doc:/` (the `File System Access API`'s `FileSystemHandle`, the
//! web's name for the same thing).
//!
//! A `doc:` path exists only once a host has minted it, so a grant over
//! `doc:/` admits what the person chose and never a location the app
//! names. `..`, `.` and empty segments are refused, as `PathPrefix`
//! refuses them. Each minting host owns its entries and forgets them when
//! its session ends ([`forget`]), except those its app keeps ([`kept`]).

pub mod kept;

use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// The namespace every minted path starts with.
pub const PREFIX: &str = "doc:/";

struct Entry {
    real: PathBuf,
    name: String,
    owner: u64,
    /// The app that keeps this entry across launches ([`kept`]): any of its
    /// sessions resolves it, and it outlives the session that minted it.
    keeper: Option<Arc<str>>,
    directory: Arc<File>,
    leaf: Option<String>,
}

impl Entry {
    fn reaches(&self, owner: u64, apps: &BTreeMap<u64, Arc<str>>) -> bool {
        self.owner == owner
            || self
                .keeper
                .as_ref()
                .is_some_and(|keeper| apps.get(&owner) == Some(keeper))
    }
}

struct Table {
    /// Every live entry by its `<n>`.
    entries: BTreeMap<u64, Entry>,
    /// The last `<n>` given. A number is never given twice in a process, and
    /// a keeping app's never one an earlier launch gave ([`kept`]).
    last: u64,
    /// The app each session that keeps documents belongs to.
    apps: BTreeMap<u64, Arc<str>>,
}

static TABLE: Mutex<Table> = Mutex::new(Table {
    entries: BTreeMap::new(),
    last: 0,
    apps: BTreeMap::new(),
});

fn table() -> std::sync::MutexGuard<'static, Table> {
    TABLE.lock().unwrap_or_else(|e| e.into_inner())
}

/// The selected folder (or a selected file's parent), opened and retained,
/// so replacing its pathname cannot redirect later document work; `None`
/// when it is not a directory.
fn retain(root: &Path, selected_folder: bool) -> Option<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(target_vendor = "apple")]
    if !selected_folder {
        use std::os::unix::fs::OpenOptionsExt;
        // A security-scoped file grants access to that file, not permission
        // to enumerate its parent. Retain the parent only for relative lookup.
        options.custom_flags(libc::O_SEARCH);
    }
    #[cfg(not(target_vendor = "apple"))]
    let _ = selected_folder;
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x0200_0000); // FILE_FLAG_BACKUP_SEMANTICS
    }
    let directory = options.open(root).ok()?;
    directory.metadata().ok()?.is_dir().then_some(directory)
}

/// Mint (or find) the handle for `real`, a file or folder the person chose,
/// on behalf of `owner` (a session): `doc:/<n>/<name>`. `None` for a path
/// with no final name (`/`).
pub fn mint(real: &Path, owner: u64) -> Option<String> {
    mint_as(real, owner, true)
}

/// [`mint`], reusing an entry for the same path when `reuse` says so; a
/// kept entry another object now stands at gives way to a new number.
pub(crate) fn mint_as(real: &Path, owner: u64, reuse: bool) -> Option<String> {
    // One file is one handle however it was spelt (`/tmp` is `/private/tmp`
    // on a Mac); a file that does not exist yet (a save) keeps its spelling.
    let canonical = std::fs::canonicalize(real).ok();
    let real = canonical.as_deref().unwrap_or(real);
    // The relative leaf must name the selected entry exactly. Lossy conversion
    // could instead read or overwrite a different file containing U+FFFD.
    let name = real.file_name()?.to_str()?.to_owned();
    if name.is_empty() || name == "." || name == ".." {
        return None;
    }
    // Open before deduplication: a pathname may now name a replacement folder.
    let folder = std::fs::metadata(real).is_ok_and(|m| m.is_dir());
    let directory = retain(if folder { real } else { real.parent()? }, folder)?;
    let selected = same_file::Handle::from_file(directory.try_clone().ok()?).ok()?;
    let leaf = (!folder).then(|| name.clone());
    let mut t = table();
    let found = t.entries.iter().find_map(|(n, e)| {
        (reuse
            && e.reaches(owner, &t.apps)
            && e.real == real
            && e.leaf == leaf
            && e.directory
                .try_clone()
                .ok()
                .and_then(|file| same_file::Handle::from_file(file).ok())
                .is_some_and(|previous| previous == selected))
        .then_some((*n, e.name.clone()))
    });
    let (n, name) = match found {
        // A kept entry renamed since it was chosen keeps the name it was chosen by.
        Some(found) => found,
        None => {
            t.last += 1;
            let n = t.last;
            t.entries.insert(
                n,
                Entry {
                    real: real.to_path_buf(),
                    name: name.clone(),
                    owner,
                    keeper: None,
                    directory: Arc::new(directory),
                    leaf,
                },
            );
            // A keeping app's counter outlives the launch (D4): a number
            // it could not write down is not given.
            if let Some(app) = t.apps.get(&owner).cloned() {
                drop(t);
                if !kept::given(&app, n) {
                    table().entries.remove(&n);
                    return None;
                }
            }
            (n, name)
        }
    };
    Some(format!("{PREFIX}{n}/{name}"))
}

/// What a host's own routes deliver for a real path (Finder, Open With,
/// the command line, ⌘O, Open Recent, a path typed into the `open-file`
/// field): a folder is its own handle; a file is delivered beneath a handle
/// to its folder, so a reader can list what is beside it. `None` for a path
/// that is not absolute or does not exist.
pub fn open_route(path: &str, owner: u64) -> Option<String> {
    let real = Path::new(path);
    if !real.is_absolute() {
        return None;
    }
    // Every explicit opening chooses the target, as the picker does. Resolve
    // a selected file's link before presenting it beneath its parent handle.
    let real = std::fs::canonicalize(real).ok()?;
    if std::fs::metadata(&real).ok()?.is_dir() {
        return mint(&real, owner);
    }
    let name = real.file_name()?.to_str()?.to_owned();
    mint(real.parent()?, owner).map(|folder| format!("{folder}/{name}"))
}

/// Forget every handle `owner` minted that its app does not keep: its
/// session ended.
pub fn forget(owner: u64) {
    let mut t = table();
    t.entries
        .retain(|_, e| e.owner != owner || e.keeper.is_some());
    t.apps.remove(&owner);
}

/// `owner` is a session of `app`, which keeps documents ([`kept`]): it
/// resolves the app's kept entries, and every `<n>` minted from now on is
/// above `last`, the highest an earlier launch of the app gave.
pub(crate) fn join(owner: u64, app: &Arc<str>, last: u64) {
    let mut t = table();
    t.apps.insert(owner, app.clone());
    t.last = t.last.max(last);
}

/// Restore a kept entry at its own `<n>` (D4), or move it to where it is now,
/// for `owner`, a session of `app`: `real` is where the object is, `name`
/// the name it had when chosen, `identity` the object recorded. Whether
/// `<n>` resolves to it: the identity is read through the folder retained,
/// so one swapped in at the path in between is refused.
pub(crate) fn adopt(
    n: u64,
    real: &Path,
    name: &str,
    identity: kept::Identity,
    folder: bool,
    owner: u64,
    app: &Arc<str>,
) -> bool {
    let leaf = if folder {
        None
    } else {
        match real.file_name().and_then(|n| n.to_str()) {
            Some(leaf) => Some(leaf.to_owned()),
            None => return false,
        }
    };
    let Some(root) = (if folder { Some(real) } else { real.parent() }) else {
        return false;
    };
    let Some(directory) = retain(root, folder) else {
        return false;
    };
    let found = match &leaf {
        None => kept::Identity::of_file(&directory) == Some((identity, true)),
        Some(leaf) => kept::Identity::at(&directory, leaf) == Some((identity, false)),
    };
    if !found {
        return false;
    }
    let mut t = table();
    // Chosen again where it is now: the entry follows it.
    if t.entries
        .get(&n)
        .is_some_and(|e| e.keeper.as_ref() != Some(app))
    {
        return false;
    }
    t.last = t.last.max(n);
    t.entries.insert(
        n,
        Entry {
            real: real.to_path_buf(),
            name: name.to_owned(),
            owner,
            keeper: Some(app.clone()),
            directory: Arc::new(directory),
            leaf,
        },
    );
    true
}

/// The retained folder a `doc:` path's operation goes through, and the
/// name beneath it, for `owner`; `None` for `doc:/<n>` or the chosen folder
/// itself.
pub(crate) fn target(path: &str, owner: u64) -> Option<(Arc<File>, String)> {
    match resolve(path, owner).ok()? {
        Resolved::Real {
            directory, path, ..
        } if !path.is_empty() => Some((directory, path)),
        _ => None,
    }
}

/// The app `owner` keeps documents for, if it keeps any.
pub(crate) fn app_of(owner: u64) -> Option<Arc<str>> {
    table().apps.get(&owner).cloned()
}

/// What a relaunch leaves of `app`: nothing in this process.
#[cfg(all(test, unix))]
pub(crate) fn relaunch(app: &str) {
    kept::drop_store(app);
    let mut t = table();
    let Table { entries, apps, .. } = &mut *t;
    entries.retain(|_, e| {
        e.keeper.as_deref() != Some(app) && apps.get(&e.owner).map(|a| &**a) != Some(app)
    });
    apps.retain(|_, a| &**a != app);
}

/// Keep entry `n` for `app`: it outlives the session that minted it.
pub(crate) fn keep(n: u64, app: &Arc<str>) {
    if let Some(e) = table().entries.get_mut(&n) {
        e.keeper = Some(app.clone());
    }
}

/// Entry `n`'s real location, its name, and whether it is a folder.
pub(crate) fn entry(n: u64) -> Option<(PathBuf, String, bool)> {
    let t = table();
    let e = t.entries.get(&n)?;
    Some((e.real.clone(), e.name.clone(), e.leaf.is_none()))
}

/// The `<n>` of a `doc:/<n>/…` path.
pub(crate) fn number(path: &str) -> Option<u64> {
    path.strip_prefix(PREFIX)?.split('/').next()?.parse().ok()
}

/// Whether `path` is in the `doc:` namespace.
pub fn is_document(path: &str) -> bool {
    path.starts_with(PREFIX)
}

/// Where a `doc:` path leads.
#[derive(Debug, Clone)]
pub enum Resolved {
    /// `doc:/<n>`: the handle's own directory, holding only its entry.
    Root(String),
    /// An owned directory and a relative name beneath it (empty for the
    /// selected folder itself). A selected file exposes only its own leaf.
    Real {
        directory: Arc<File>,
        path: String,
        /// A selected file is one leaf in its retained parent, not a subtree:
        /// the name it was chosen by, which a kept file keeps when it is
        /// renamed (`path` is its leaf now).
        selected_file: Option<String>,
    },
}

/// Resolve a `doc:` path to its retained directory and relative name, refusing a handle
/// not owned by `owner` (or forgotten), a name that is not the handle's, and any
/// `.`, `..` or empty segment.
pub fn resolve(path: &str, owner: u64) -> std::io::Result<Resolved> {
    use std::io::{Error, ErrorKind};
    let rest = path
        .strip_prefix(PREFIX)
        .ok_or_else(|| Error::new(ErrorKind::InvalidInput, "not a document path"))?;
    let rest = rest.strip_suffix('/').unwrap_or(rest);
    let mut parts = rest.split('/');
    let n: u64 = parts
        .next()
        .and_then(|n| n.parse().ok())
        .ok_or_else(|| Error::new(ErrorKind::NotFound, "no such document"))?;
    let segments: Vec<&str> = parts.collect();
    if segments
        .iter()
        .any(|s| s.is_empty() || *s == "." || *s == ".." || s.contains(['\\', '\0']))
    {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "a document path has no `.`, `..`, backslash, NUL or empty segment",
        ));
    }
    let t = table();
    let entry = t
        .entries
        .get(&n)
        .filter(|entry| entry.reaches(owner, &t.apps))
        .ok_or_else(|| {
            Error::new(
                ErrorKind::NotFound,
                "no such document (it was never opened, or its window closed)",
            )
        })?;
    let Some((first, beneath)) = segments.split_first() else {
        return Ok(Resolved::Root(entry.name.clone()));
    };
    if *first != entry.name {
        return Err(Error::new(ErrorKind::NotFound, "no such document"));
    }
    if entry.leaf.is_some() && !beneath.is_empty() {
        return Err(Error::new(
            ErrorKind::NotADirectory,
            "a document file is not a directory",
        ));
    }
    Ok(Resolved::Real {
        directory: entry.directory.clone(),
        path: entry.leaf.clone().unwrap_or_else(|| beneath.join("/")),
        selected_file: entry.leaf.is_some().then(|| entry.name.clone()),
    })
}

/// The topic an answer watches for a document (LLP 1069.010.000 D1): the
/// runner asks it again when the host announces this topic.
pub fn watch_topic(path: &str, recursive: bool) -> String {
    format!("{}{path}", if recursive { WATCH_RECURSIVE } else { WATCH })
}

const WATCH: &str = "doc-watch:";
const WATCH_RECURSIVE: &str = "doc-watch-recursive:";

/// A document watch, as a host listens for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Watched {
    /// The topic to announce.
    pub topic: String,
    /// The watched `doc:` path, as the answer named it.
    pub path: String,
    /// The watched file or folder on disk, as minted.
    pub real: PathBuf,
    /// The folder a host's watcher listens to: the folder itself, or a
    /// file's folder.
    pub folder: PathBuf,
    /// The whole subtree, not only a folder's own entries.
    pub recursive: bool,
}

/// The real location of a watch topic `owner` may read, or `None` for
/// another topic, a forgotten handle or a `doc:/<n>` root.
pub fn watched(topic: &str, owner: u64) -> Option<Watched> {
    let (path, recursive) = match topic.strip_prefix(WATCH_RECURSIVE) {
        Some(path) => (path, true),
        None => (topic.strip_prefix(WATCH)?, false),
    };
    let Resolved::Real {
        path: relative,
        selected_file,
        ..
    } = resolve(path, owner).ok()?
    else {
        return None;
    };
    let n: u64 = path[PREFIX.len()..].split('/').next()?.parse().ok()?;
    let base = table().entries.get(&n)?.real.clone();
    let real = if selected_file.is_some() || relative.is_empty() {
        base
    } else {
        base.join(&relative)
    };
    let folder = if real.is_dir() {
        real.clone()
    } else {
        real.parent()?.to_path_buf()
    };
    Some(Watched {
        topic: topic.to_owned(),
        path: path.to_owned(),
        real,
        folder,
        recursive,
    })
}

/// Whether a change at `changed` on disk is in a watch's scope: the file
/// itself; a folder's own entries; or, recursive, anything beneath it.
pub fn covers(watch: &Watched, changed: &Path) -> bool {
    changed == watch.real
        || if watch.recursive {
            changed.starts_with(&watch.real)
        } else {
            watch.real.is_dir() && changed.parent() == Some(watch.real.as_path())
        }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_vendor = "apple")]
    fn a_selected_file_does_not_require_listing_its_parent() {
        use std::os::unix::fs::PermissionsExt;
        use std::os::unix::io::AsRawFd;
        let root =
            std::env::temp_dir().join(format!("exact-docs-file-scope-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("selected.md"), "selected").unwrap();
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o111)).unwrap();
        let doc = mint(&root.join("selected.md"), 7203);
        let listed = std::fs::read_dir(&root);
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert!(
            listed.is_err(),
            "the parent must not be readable in this test"
        );
        let doc = doc.expect("the selected file needs only parent lookup access");
        let Resolved::Real {
            directory, path, ..
        } = resolve(&doc, 7203).unwrap()
        else {
            panic!()
        };
        assert_eq!(path, "selected.md");
        let fd = unsafe {
            libc::openat(
                directory.as_raw_fd(),
                c"selected.md".as_ptr(),
                libc::O_RDONLY,
            )
        };
        assert!(
            fd >= 0,
            "the retained directory must permit opening the selected file"
        );
        unsafe { libc::close(fd) };
        assert!(resolve(&format!("{doc}/other.md"), 7203).is_err());
        forget(7203);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_watch_covers_its_file_its_folders_entries_or_its_subtree() {
        let root = std::env::temp_dir().join(format!("exact-docs-watch-{}", std::process::id()));
        std::fs::create_dir_all(root.join("notes/deep")).unwrap();
        std::fs::write(root.join("notes/a.md"), "a").unwrap();
        let folder = mint(&root.join("notes"), 7201).unwrap();
        let file = open_route(root.join("notes/a.md").to_str().unwrap(), 7201).unwrap();
        let real = std::fs::canonicalize(root.join("notes")).unwrap();
        let w = watched(&watch_topic(&folder, false), 7201).unwrap();
        assert_eq!(
            (w.real.clone(), w.folder.clone()),
            (real.clone(), real.clone())
        );
        assert!(covers(&w, &real.join("b.md")));
        assert!(
            !covers(&w, &real.join("deep/c.md")),
            "a folder's watch is its own entries"
        );
        let r = watched(&watch_topic(&folder, true), 7201).unwrap();
        assert!(covers(&r, &real.join("deep/c.md")));
        let f = watched(&watch_topic(&file, false), 7201).unwrap();
        assert_eq!(f.folder, real);
        assert!(covers(&f, &real.join("a.md")) && !covers(&f, &real.join("b.md")));
        assert!(
            watched(&watch_topic(&folder, false), 7202).is_none(),
            "another session's"
        );
        assert!(watched("meter", 7201).is_none());
        forget(7201);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_document_handle_belongs_to_its_minting_session() {
        let folder = std::env::temp_dir().join(format!("exact-docs-owner-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let doc = mint(&folder, 7101).unwrap();
        assert!(matches!(resolve(&doc, 7101), Ok(Resolved::Real { .. })));
        assert!(resolve(&doc, 7102)
            .unwrap_err()
            .to_string()
            .contains("no such document"));
        let root = format!("doc:/{}", doc[PREFIX.len()..].split('/').next().unwrap());
        assert!(matches!(resolve(&root, 7101), Ok(Resolved::Root(_))));
        assert!(resolve(&root, 7102).is_err());
        forget(7102);
        assert!(resolve(&doc, 7101).is_ok());
        forget(7101);
        std::fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn a_minted_path_resolves_beneath_its_handle_and_nowhere_else() {
        let folder =
            std::env::temp_dir().join(format!("exact-docs-test-{}/notes", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let doc = mint(&folder, 7001).unwrap();
        assert!(doc.starts_with("doc:/") && doc.ends_with("/notes"), "{doc}");
        assert_eq!(mint(&folder, 7001).unwrap(), doc, "minting again reuses it");
        let Resolved::Real {
            directory, path, ..
        } = resolve(&format!("{doc}/a/b.md"), 7001).unwrap()
        else {
            panic!()
        };
        assert!(directory.metadata().unwrap().is_dir());
        assert_eq!(path, "a/b.md");
        let n = doc.trim_start_matches(PREFIX).split('/').next().unwrap();
        assert!(
            matches!(resolve(&format!("doc:/{n}"), 7001).unwrap(), Resolved::Root(name) if name == "notes")
        );
        assert!(resolve(&format!("{doc}/../x"), 7001).is_err());
        assert!(resolve(&format!("doc:/{n}/other/x"), 7001).is_err());
        assert!(resolve("doc:/0/x", 7001).is_err());
        assert!(resolve("app:/data/x", 7001).is_err());
        forget(7001);
        assert!(
            resolve(&doc, 7001).is_err(),
            "a forgotten handle reads nothing"
        );
        assert!(mint(Path::new("/"), 7001).is_none());
        std::fs::remove_dir_all(folder.parent().unwrap()).unwrap();
    }

    #[test]
    #[cfg(unix)]
    fn an_open_route_selects_a_file_symlinks_target() {
        use std::os::unix::fs::symlink;
        let root = std::env::temp_dir().join(format!("exact-docs-route-{}", std::process::id()));
        std::fs::create_dir_all(root.join("chosen")).unwrap();
        std::fs::create_dir_all(root.join("outside")).unwrap();
        let target = root.join("outside/note.md");
        std::fs::write(&target, "selected").unwrap();
        let link = root.join("chosen/link.md");
        symlink(&target, &link).unwrap();
        let doc = open_route(link.to_str().unwrap(), 7002).unwrap();
        assert!(doc.ends_with("/outside/note.md"), "{doc}");
        let Resolved::Real {
            directory, path, ..
        } = resolve(&doc, 7002).unwrap()
        else {
            panic!()
        };
        assert_eq!(path, "note.md");
        assert!(directory.metadata().unwrap().is_dir());
        assert!(open_route(root.join("missing.md").to_str().unwrap(), 7002).is_none());
        forget(7002);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn selecting_a_replacement_folder_mints_its_own_handle() {
        let root = std::env::temp_dir().join(format!("exact-docs-repick-{}", std::process::id()));
        let chosen = root.join("chosen");
        let moved = root.join("moved");
        std::fs::create_dir_all(&chosen).unwrap();
        let old = mint(&chosen, 7003).unwrap();
        std::fs::rename(&chosen, &moved).unwrap();
        std::fs::create_dir(&chosen).unwrap();
        let new = mint(&chosen, 7003).unwrap();
        assert_ne!(old, new);
        assert_eq!(mint(&chosen, 7003).unwrap(), new);
        let Resolved::Real {
            directory: old_dir, ..
        } = resolve(&old, 7003).unwrap()
        else {
            panic!()
        };
        let Resolved::Real {
            directory: new_dir, ..
        } = resolve(&new, 7003).unwrap()
        else {
            panic!()
        };
        assert_ne!(
            same_file::Handle::from_file(old_dir.try_clone().unwrap()).unwrap(),
            same_file::Handle::from_file(new_dir.try_clone().unwrap()).unwrap()
        );
        forget(7003);
        drop((old_dir, new_dir));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[cfg(unix)]
    fn a_selected_name_that_is_not_utf8_never_addresses_its_lossy_spelling() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;
        let root = std::env::temp_dir().join(format!("exact-docs-name-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let invalid = root.join(OsString::from_vec(b"note-\xff.md".to_vec()));
        let other = root.join("note-\u{fffd}.md");
        // Apple's filesystem rejects these names itself. Still test a save
        // selection there; Unix filesystems that admit the name test opening it.
        #[cfg(not(target_vendor = "apple"))]
        std::fs::write(&invalid, "selected").unwrap();
        std::fs::write(&other, "different").unwrap();
        assert!(mint(&invalid, 7004).is_none());
        assert!(open_route(invalid.to_string_lossy().as_ref(), 7004).is_some());
        // The valid spelling explicitly chooses the different file; the raw
        // selected filename above must not silently alias it.
        assert_eq!(std::fs::read_to_string(&other).unwrap(), "different");
        forget(7004);
        std::fs::remove_dir_all(root).unwrap();
    }
}
