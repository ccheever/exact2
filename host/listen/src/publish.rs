//! D4's binding: a socket bound under a private staging name, set `0600`,
//! then published onto the path by `link` (only while the path is free) or
//! `rename` (replacing a stale socket, or a live one under `takeover`).
//! Publication and removal run under an exclusive `flock` on `<path>.lock`
//! beside the socket, so an older listener's inode check and unlink cannot
//! race a newer one's rename. The process `umask` is never touched.
use super::address::{Failure, Resolved};
use std::{
    fs::File,
    io::ErrorKind,
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd},
        unix::{
            fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
            net::UnixListener,
        },
    },
    path::{Path, PathBuf},
    sync::atomic::{AtomicU32, Ordering},
};

/// A socket published at the path.
pub(crate) struct Bound {
    pub listener: UnixListener,
    /// The socket file's inode: removal leaves a path that is no longer it.
    pub ino: u64,
    /// It replaced a live socket (`takeover`).
    pub took_over: bool,
}

/// What is at the path now.
enum Found {
    Free,
    /// A socket that accepts a connection.
    Live,
    /// A socket that refuses one: its owner died without removing it.
    Stale,
    /// Anything that is not a socket: never removed.
    Other,
}

/// The exclusive lock on `<path>.lock`, held while it lives.
pub(crate) struct Lock(File);

impl Drop for Lock {
    fn drop(&mut self) {
        unsafe { libc::flock(self.0.as_raw_fd(), libc::LOCK_UN) };
    }
}

/// Take `<path>.lock`, made `0600` in the `0700` parent when missing.
pub(crate) fn lock(r: &Resolved) -> Result<Lock, Failure> {
    let mut name = r.path.clone().into_os_string();
    name.push(".lock");
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(PathBuf::from(name))
        .map_err(|e| Failure::os("opening the listener's lock file", e))?;
    loop {
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) } == 0 {
            return Ok(Lock(file));
        }
        let e = std::io::Error::last_os_error();
        if e.kind() != ErrorKind::Interrupted {
            return Err(Failure::os("locking the listener's lock file", e));
        }
    }
}

/// What is at `path`, by `lstat` and, for a socket when `probe`, a
/// connection attempt; without one a socket counts as live.
fn found(path: &Path, probe: bool) -> Result<Found, Failure> {
    match std::fs::symlink_metadata(path) {
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(Found::Free),
        Err(e) => Err(Failure::os(&format!("reading {}", path.display()), e)),
        Ok(meta) if !std::os::unix::fs::FileTypeExt::is_socket(&meta.file_type()) => {
            Ok(Found::Other)
        }
        Ok(_) if !probe => Ok(Found::Live),
        Ok(_) => match connects(path) {
            Ok(()) => Ok(Found::Live),
            Err(e) if e.kind() == ErrorKind::ConnectionRefused => Ok(Found::Stale),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(Found::Free),
            Err(e) => Err(Failure::os(&format!("probing {}", path.display()), e)),
        },
    }
}

/// A connection attempt that never blocks, made while the path's lock is
/// held: a listener behind a full backlog answers `EAGAIN` on Linux, which
/// counts as live, where a blocking connect would wait for room. On Apple
/// a full backlog refuses (`ECONNREFUSED`), which reads as stale: a declared
/// limit of the probe.
fn connects(path: &Path) -> std::io::Result<()> {
    use std::os::unix::ffi::OsStrExt;
    let bytes = path.as_os_str().as_bytes();
    let mut addr: libc::sockaddr_un = unsafe { std::mem::zeroed() };
    if bytes.len() >= addr.sun_path.len() {
        return Err(ErrorKind::InvalidInput.into());
    }
    addr.sun_family = libc::AF_UNIX as libc::sa_family_t;
    for (to, from) in addr.sun_path.iter_mut().zip(bytes) {
        *to = *from as libc::c_char;
    }
    let fd = unsafe { libc::socket(libc::AF_UNIX, libc::SOCK_STREAM, 0) };
    if fd < 0 {
        return Err(std::io::Error::last_os_error());
    }
    let fd = unsafe { OwnedFd::from_raw_fd(fd) };
    unsafe {
        let flags = libc::fcntl(fd.as_raw_fd(), libc::F_GETFL);
        libc::fcntl(fd.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK);
    }
    let len = std::mem::size_of::<libc::sockaddr_un>() as libc::socklen_t;
    let ptr = (&addr as *const libc::sockaddr_un).cast::<libc::sockaddr>();
    if unsafe { libc::connect(fd.as_raw_fd(), ptr, len) } == 0 {
        return Ok(());
    }
    let e = std::io::Error::last_os_error();
    match e.raw_os_error() {
        Some(libc::EINPROGRESS | libc::EAGAIN) => Ok(()),
        _ => Err(e),
    }
}

/// A fresh socket at a private staging name in the parent, `0600`, and its
/// inode.
fn stage(r: &Resolved) -> Result<(UnixListener, PathBuf, u64), Failure> {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let seed = std::process::id()
        .wrapping_mul(2_654_435_761)
        .wrapping_add(NEXT.fetch_add(1, Ordering::Relaxed).wrapping_mul(40_503))
        ^ std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.subsec_nanos());
    let staging = r.parent.join(format!(".s{seed:08x}"));
    let _ = std::fs::remove_file(&staging);
    let listener =
        UnixListener::bind(&staging).map_err(|e| Failure::os("binding the socket", e))?;
    let staged = (|| {
        std::fs::set_permissions(&staging, std::fs::Permissions::from_mode(0o600))?;
        listener.set_nonblocking(true)?;
        std::fs::symlink_metadata(&staging).map(|m| m.ino())
    })();
    match staged {
        Ok(ino) => Ok((listener, staging, ino)),
        Err(e) => {
            let _ = std::fs::remove_file(&staging);
            Err(Failure::os("staging the socket", e))
        }
    }
}

/// Bind and publish at `r.path`: `link` onto a free path, `rename` over a
/// stale socket, and over a live one only with `takeover`. With `takeover`
/// any socket there is replaced with no probe, so no connection of ours
/// reaches the listener it displaces; `took_over` then says a socket was
/// there, live or stale. `holder()` is asked under the lock: false (a newer
/// listener of this process took the path over meanwhile) publishes
/// nothing, and answers `None`.
pub(crate) fn bind(
    r: &Resolved,
    takeover: bool,
    holder: impl FnOnce() -> bool,
) -> Result<Option<Bound>, Failure> {
    let _lock = lock(r)?;
    if !holder() {
        return Ok(None);
    }
    let (listener, staging, ino) = stage(r)?;
    let published = (|| match found(&r.path, !takeover)? {
        Found::Free => match std::fs::hard_link(&staging, &r.path) {
            Ok(()) => Ok(false),
            Err(e) if e.kind() == ErrorKind::AlreadyExists => Err(in_use(r)),
            Err(e) => Err(Failure::os("publishing the socket", e)),
        },
        Found::Stale => rename(&staging, &r.path).map(|()| false),
        Found::Live if takeover => rename(&staging, &r.path).map(|()| true),
        Found::Live => Err(in_use(r)),
        Found::Other => Err(Failure::refused(format!(
            "{} exists and is not a socket",
            r.path.display()
        ))),
    })();
    let _ = std::fs::remove_file(&staging);
    published.map(|took_over| {
        Some(Bound {
            listener,
            ino,
            took_over,
        })
    })
}

/// Bind again where the path is free (the listener holding it let go):
/// `link`, which claims it only while it is free. `None` when the path is
/// there: it is this listener's, or another's that claimed it first.
pub(crate) fn relink(r: &Resolved) -> Result<Option<Bound>, Failure> {
    let _lock = lock(r)?;
    if !matches!(found(&r.path, false)?, Found::Free) {
        return Ok(None);
    }
    let (listener, staging, ino) = stage(r)?;
    let linked = std::fs::hard_link(&staging, &r.path);
    let _ = std::fs::remove_file(&staging);
    match linked {
        Ok(()) => Ok(Some(Bound {
            listener,
            ino,
            took_over: false,
        })),
        Err(e) if e.kind() == ErrorKind::AlreadyExists => Ok(None),
        Err(e) => Err(Failure::os("publishing the socket", e)),
    }
}

/// Remove the path while it is still the socket file `ino`.
pub(crate) fn remove(r: &Resolved, ino: u64) {
    let Ok(_lock) = lock(r) else { return };
    if std::fs::symlink_metadata(&r.path).is_ok_and(|m| m.ino() == ino) {
        let _ = std::fs::remove_file(&r.path);
    }
}

/// Whether the path is the socket file `ino`.
pub(crate) fn is(r: &Resolved, ino: u64) -> bool {
    std::fs::symlink_metadata(&r.path).is_ok_and(|m| m.ino() == ino)
}

/// Whether nothing is at the path.
pub(crate) fn vanished(r: &Resolved) -> bool {
    matches!(std::fs::symlink_metadata(&r.path), Err(e) if e.kind() == ErrorKind::NotFound)
}

fn rename(from: &Path, to: &Path) -> Result<(), Failure> {
    std::fs::rename(from, to).map_err(|e| Failure::os("publishing the socket", e))
}

fn in_use(r: &Resolved) -> Failure {
    Failure {
        kind: "AddressInUse",
        message: format!("unix:{} is in use", r.path.display()),
    }
}
