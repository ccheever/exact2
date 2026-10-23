//! The directory a listener binds in, watched while it listens (D4): when
//! its path vanishes (the newer instance closed and unlinked it), the
//! listener binds again. `kqueue` on Apple, `inotify` elsewhere; either
//! descriptor polls readable when an entry was added, removed or renamed.
use std::{
    os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd},
    path::Path,
};

/// A watch on one directory.
pub(crate) struct Watch {
    fd: OwnedFd,
    /// The directory, held open for `kqueue`.
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    _dir: OwnedFd,
}

fn cstring(path: &Path) -> std::io::Result<std::ffi::CString> {
    use std::os::unix::ffi::OsStrExt;
    std::ffi::CString::new(path.as_os_str().as_bytes())
        .map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidInput))
}

fn owned(fd: RawFd) -> std::io::Result<OwnedFd> {
    if fd < 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(unsafe { OwnedFd::from_raw_fd(fd) })
    }
}

impl Watch {
    /// Watch `dir`.
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    pub(crate) fn new(dir: &Path) -> std::io::Result<Watch> {
        let name = cstring(dir)?;
        let dir = owned(unsafe { libc::open(name.as_ptr(), libc::O_EVTONLY | libc::O_CLOEXEC) })?;
        let fd = owned(unsafe { libc::kqueue() })?;
        let change = libc::kevent {
            ident: dir.as_raw_fd() as usize,
            filter: libc::EVFILT_VNODE,
            flags: libc::EV_ADD | libc::EV_CLEAR,
            fflags: libc::NOTE_WRITE | libc::NOTE_DELETE | libc::NOTE_RENAME,
            data: 0,
            udata: std::ptr::null_mut(),
        };
        let zero = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        let added =
            unsafe { libc::kevent(fd.as_raw_fd(), &change, 1, std::ptr::null_mut(), 0, &zero) };
        if added < 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(Watch { fd, _dir: dir })
    }

    /// Watch `dir`.
    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
    pub(crate) fn new(dir: &Path) -> std::io::Result<Watch> {
        let name = cstring(dir)?;
        let fd = owned(unsafe { libc::inotify_init1(libc::IN_NONBLOCK | libc::IN_CLOEXEC) })?;
        let mask = libc::IN_DELETE | libc::IN_MOVED_FROM | libc::IN_MOVED_TO | libc::IN_CREATE;
        if unsafe { libc::inotify_add_watch(fd.as_raw_fd(), name.as_ptr(), mask) } < 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(Watch { fd })
    }

    /// The descriptor to poll.
    pub(crate) fn fd(&self) -> RawFd {
        self.fd.as_raw_fd()
    }

    /// Take what is pending, so the descriptor polls quiet again.
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    pub(crate) fn drain(&self) {
        let zero = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        let mut events: [libc::kevent; 8] = unsafe { std::mem::zeroed() };
        while unsafe {
            libc::kevent(
                self.fd.as_raw_fd(),
                std::ptr::null(),
                0,
                events.as_mut_ptr(),
                events.len() as i32,
                &zero,
            )
        } > 0
        {}
    }

    /// Take what is pending, so the descriptor polls quiet again.
    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
    pub(crate) fn drain(&self) {
        let mut buffer = [0u8; 4096];
        while unsafe {
            libc::read(
                self.fd.as_raw_fd(),
                buffer.as_mut_ptr().cast(),
                buffer.len(),
            )
        } > 0
        {}
    }
}
