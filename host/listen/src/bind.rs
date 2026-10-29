//! D4: the directory, the staging name, publication and removal.
//!
//! The parent directory is the user's own and `0700`, so nobody else can
//! create, rename or remove an entry in it. The socket is bound under a
//! staging name, `chmod`ed `0600` and published by `link` (claiming a free
//! path) or `rename` (replacing a stale socket, or a live one under
//! `takeover`); the process-wide `umask` is never touched. Publication and
//! removal run under an exclusive `flock` on `<path>.lock`, so a listener
//! never removes a newer one's socket between its inode check and its
//! `unlink`. A program that publishes on the path without the lock can still
//! race that check, a declared limit (LLP 1133.005 §10).
use std::fs;
use std::io;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::io::AsRawFd;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};

/// Why binding failed, as the `error` event names it (D1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// Outside D4's checks.
    Refused(String),
    /// A live socket holds the path and `takeover` is not set.
    AddressInUse(String),
    /// Anything else the OS reports.
    Network(String),
}

impl Failure {
    /// The event's `kind`.
    pub fn kind(&self) -> &'static str {
        match self {
            Failure::Refused(_) => "Refused",
            Failure::AddressInUse(_) => "AddressInUse",
            Failure::Network(_) => "Network",
        }
    }

    /// The event's `message`.
    pub fn message(&self) -> &str {
        match self {
            Failure::Refused(m) | Failure::AddressInUse(m) | Failure::Network(m) => m,
        }
    }
}

fn network(what: &str, path: &Path, e: io::Error) -> Failure {
    Failure::Network(format!("{what} {}: {e}", path.display()))
}

/// The bytes `sun_path` holds, its NUL included.
#[cfg(any(target_os = "linux", target_os = "android"))]
const SUN_PATH: usize = 108;
#[cfg(not(any(target_os = "linux", target_os = "android")))]
const SUN_PATH: usize = 104;

/// The effective user id.
pub fn uid() -> u32 {
    // SAFETY: geteuid has no preconditions and cannot fail.
    unsafe { libc::geteuid() }
}

/// Make or check the parent of `path`: created `0700` if missing (its own
/// parent must exist); an existing one must be a directory, not a symlink,
/// owned by the user, and is set to `0700`. Every ancestor above it must be
/// the user's or root's and not writable by others unless sticky (`/tmp`),
/// as OpenSSH's `StrictModes` checks a key's directories.
pub fn prepare(path: &Path) -> Result<(), Failure> {
    let parent = path
        .parent()
        .ok_or_else(|| Failure::Refused("no parent".into()))?;
    let shown = parent.display();
    match fs::symlink_metadata(parent) {
        Ok(_) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            let above = parent.parent().unwrap_or(Path::new("/"));
            if !fs::symlink_metadata(above).is_ok_and(|m| m.is_dir()) {
                return Err(Failure::Refused(format!(
                    "{}, the directory above the socket's, does not exist",
                    above.display()
                )));
            }
            match fs::DirBuilder::new().mode(0o700).create(parent) {
                Ok(()) => {}
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
                Err(e) => return Err(network("cannot create", parent, e)),
            }
        }
        Err(e) => return Err(network("cannot read", parent, e)),
    }
    let meta = fs::symlink_metadata(parent).map_err(|e| network("cannot read", parent, e))?;
    if meta.file_type().is_symlink() {
        return Err(Failure::Refused(format!("{shown} is a symlink")));
    }
    if !meta.is_dir() {
        return Err(Failure::Refused(format!("{shown} is not a directory")));
    }
    if meta.uid() != uid() {
        return Err(Failure::Refused(format!("{shown} belongs to another user")));
    }
    if meta.mode() & 0o777 != 0o700 {
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700))
            .map_err(|e| network("cannot chmod", parent, e))?;
    }
    for ancestor in parent.ancestors().skip(1) {
        let meta = fs::metadata(ancestor).map_err(|e| network("cannot read", ancestor, e))?;
        if meta.uid() != uid() && meta.uid() != 0 {
            return Err(Failure::Refused(format!(
                "{} belongs to another user",
                ancestor.display()
            )));
        }
        if meta.mode() & 0o002 != 0 && meta.mode() & 0o1000 == 0 {
            return Err(Failure::Refused(format!(
                "{} is writable by others and not sticky",
                ancestor.display()
            )));
        }
    }
    Ok(())
}

/// An exclusive `flock` on `<path>.lock`, held until dropped.
pub struct Lock(fs::File);

impl Lock {
    /// Take the lock beside `path`, waiting for it.
    pub fn take(path: &Path) -> Result<Lock, Failure> {
        let mut name = path.as_os_str().to_owned();
        name.push(".lock");
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&name)
            .map_err(|e| network("cannot open", Path::new(&name), e))?;
        loop {
            // SAFETY: the descriptor is open for the call.
            if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) } == 0 {
                return Ok(Lock(file));
            }
            let e = io::Error::last_os_error();
            if e.kind() != io::ErrorKind::Interrupted {
                return Err(network("cannot lock", Path::new(&name), e));
            }
        }
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        // SAFETY: the descriptor is open until the file drops after this.
        unsafe { libc::flock(self.0.as_raw_fd(), libc::LOCK_UN) };
    }
}

/// The file a published socket is, by device and inode.
pub type Identity = (u64, u64);

/// The identity of what is at `path` now, if anything.
pub fn identity(path: &Path) -> Option<Identity> {
    fs::symlink_metadata(path).ok().map(|m| (m.dev(), m.ino()))
}

/// What is at the path, as publication sees it.
enum Found {
    Free,
    /// A socket nobody accepts on: its owner died without removing it.
    Stale,
    Live,
}

fn found(path: &Path) -> Result<Found, Failure> {
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Found::Free),
        Err(e) => return Err(network("cannot read", path, e)),
    };
    use std::os::unix::fs::FileTypeExt;
    if !meta.file_type().is_socket() {
        return Err(Failure::Refused(format!(
            "{} exists and is not a socket",
            path.display()
        )));
    }
    match UnixStream::connect(path) {
        Ok(_) => Ok(Found::Live),
        Err(e) if e.kind() == io::ErrorKind::ConnectionRefused => Ok(Found::Stale),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Found::Free),
        Err(e) => Err(network("cannot reach", path, e)),
    }
}

static STAGED: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// A private staging name in `path`'s directory: `.l` and 8 hex digits.
fn staging(path: &Path) -> Result<PathBuf, Failure> {
    let n = STAGED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let name = format!(".l{:04x}{:04x}", std::process::id() & 0xffff, n & 0xffff);
    let staged = path.with_file_name(name);
    if staged.as_os_str().len() >= SUN_PATH {
        return Err(Failure::Refused(format!(
            "{} leaves no room for a staging name beside the socket",
            path.display()
        )));
    }
    Ok(staged)
}

/// A bound listener and the file it is published as.
pub struct Bound {
    /// The accepting socket.
    pub listener: UnixListener,
    /// The published file, by identity.
    pub identity: Identity,
    /// It replaced a live socket (`takeover`).
    pub took_over: bool,
}

/// Bind and publish at `path` (D4), under the lock. `link` claims a free
/// path; a stale socket, and a live one under `takeover`, is replaced by
/// `rename`. Without `takeover` a live socket is `AddressInUse`. `over`: a
/// listener of this process holds the path, so it is replaced unasked.
pub fn publish(path: &Path, takeover: bool, over: bool) -> Result<Bound, Failure> {
    prepare(path)?;
    let _lock = Lock::take(path)?;
    // Asking a live socket costs its owner a connection that ends at once.
    let (replace, took_over) = match if over { Found::Live } else { found(path)? } {
        Found::Live if over => (identity(path).is_some(), true),
        Found::Free => (false, false),
        Found::Stale => (true, false),
        Found::Live if takeover => (true, true),
        Found::Live => {
            return Err(Failure::AddressInUse(format!(
                "{} is held by a listener that accepts connections",
                path.display()
            )))
        }
    };
    let staged = staging(path)?;
    // A staging name left by a crash is this user's, in this user's directory.
    let _ = fs::remove_file(&staged);
    let listener = UnixListener::bind(&staged).map_err(|e| network("cannot bind", path, e))?;
    let published = fs::set_permissions(&staged, fs::Permissions::from_mode(0o600)).and_then(
        |()| match replace {
            true => fs::rename(&staged, path),
            false => fs::hard_link(&staged, path).and_then(|()| fs::remove_file(&staged)),
        },
    );
    if let Err(e) = published {
        let _ = fs::remove_file(&staged);
        return Err(if e.kind() == io::ErrorKind::AlreadyExists {
            Failure::AddressInUse(format!("{} was claimed meanwhile", path.display()))
        } else {
            network("cannot publish", path, e)
        });
    }
    listener
        .set_nonblocking(true)
        .map_err(|e| network("cannot listen on", path, e))?;
    let identity = identity(path).ok_or_else(|| {
        Failure::Network(format!("{} vanished as it was published", path.display()))
    })?;
    remember(path, identity);
    Ok(Bound {
        listener,
        identity,
        took_over,
    })
}

/// Remove `path` while it is still the file `ours`, under the lock.
pub fn unpublish(path: &Path, ours: Identity) {
    PUBLISHED
        .lock()
        .unwrap()
        .retain(|(p, i)| !(p == path && *i == ours));
    let Ok(_lock) = Lock::take(path) else { return };
    if identity(path) == Some(ours) {
        let _ = fs::remove_file(path);
    }
}

/// The files this process has published and not removed: removed at its
/// exit too, so a host that exits without closing its sessions (a drive's
/// end, `exit()`) leaves no socket behind (D4). A crash still leaves a stale
/// one, which the next listener replaces.
static PUBLISHED: std::sync::Mutex<Vec<(PathBuf, Identity)>> = std::sync::Mutex::new(Vec::new());

extern "C" fn at_exit() {
    let published = std::mem::take(&mut *PUBLISHED.lock().unwrap_or_else(|e| e.into_inner()));
    for (path, ours) in published {
        unpublish(&path, ours);
    }
}

fn remember(path: &Path, ours: Identity) {
    static REGISTERED: std::sync::Once = std::sync::Once::new();
    // SAFETY: `at_exit` is a plain function that lives as long as the process.
    REGISTERED.call_once(|| unsafe {
        libc::atexit(at_exit);
    });
    PUBLISHED.lock().unwrap().push((path.to_path_buf(), ours));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> PathBuf {
        let base = std::env::temp_dir().canonicalize().unwrap();
        let dir = base.join(format!("exact-listen-bind-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    #[ignore = "async lane: binds real unix sockets"]
    fn a_missing_directory_is_made_private() {
        let d = dir("make");
        let path = d.join("a.sock");
        let bound = publish(&path, false, false).unwrap();
        let mode = fs::metadata(&d).unwrap().mode() & 0o777;
        assert_eq!(mode, 0o700);
        let mode = fs::symlink_metadata(&path).unwrap().mode() & 0o777;
        assert_eq!(mode, 0o600);
        assert!(UnixStream::connect(&path).is_ok());
        unpublish(&path, bound.identity);
        assert!(!path.exists());
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    #[ignore = "async lane: binds real unix sockets"]
    fn a_symlinked_or_plain_file_is_refused() {
        let d = dir("refuse");
        fs::create_dir(&d).unwrap();
        let real = d.join("real");
        fs::create_dir(&real).unwrap();
        std::os::unix::fs::symlink(&real, d.join("link")).unwrap();
        let link = publish(&d.join("link/a.sock"), false, false).err().unwrap();
        assert!(link.message().contains("symlink"), "{link:?}");
        fs::write(d.join("file"), "x").unwrap();
        let file = publish(&d.join("file/a.sock"), false, false).err().unwrap();
        assert!(file.message().contains("not a directory"), "{file:?}");
        fs::write(real.join("plain"), "x").unwrap();
        let plain = publish(&real.join("plain"), true, false).err().unwrap();
        assert_eq!(plain.kind(), "Refused");
        assert_eq!(fs::read_to_string(real.join("plain")).unwrap(), "x");
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    #[ignore = "async lane: binds real unix sockets"]
    fn live_stale_and_takeover() {
        let d = dir("live");
        let path = d.join("a.sock");
        let first = publish(&path, false, false).unwrap();
        let busy = publish(&path, false, false).err().unwrap();
        assert_eq!(busy.kind(), "AddressInUse");
        let second = publish(&path, true, false).unwrap();
        assert!(second.took_over);
        assert_ne!(second.identity, first.identity);
        // The older one removes nothing now that the path is the newer's.
        unpublish(&path, first.identity);
        assert_eq!(identity(&path), Some(second.identity));
        drop(second.listener);
        // A socket nobody accepts on is stale, and replaced.
        let third = publish(&path, false, false).unwrap();
        assert!(!third.took_over);
        unpublish(&path, third.identity);
        fs::remove_dir_all(&d).unwrap();
    }
}
