//! D4's directory watch: a descriptor `poll` finds readable when an entry
//! of the socket's directory is created, removed or renamed (`kqueue` on
//! Apple, `inotify` on Linux), so a listener learns at once that its path
//! vanished or now names another listener's socket.
use std::io;
use std::os::unix::io::RawFd;
use std::path::Path;

/// A watch on one directory, closed when dropped.
pub struct Watch {
    fd: RawFd,
    #[cfg(target_vendor = "apple")]
    dir: RawFd,
}

fn cstring(path: &Path) -> io::Result<std::ffi::CString> {
    use std::os::unix::ffi::OsStrExt;
    std::ffi::CString::new(path.as_os_str().as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "a path with a NUL"))
}

impl Watch {
    /// Watch `dir`.
    #[cfg(target_vendor = "apple")]
    pub fn new(dir: &Path) -> io::Result<Watch> {
        let name = cstring(dir)?;
        // SAFETY: plain system calls on descriptors this function owns; each
        // failure closes what it opened.
        unsafe {
            let dir = libc::open(name.as_ptr(), libc::O_EVTONLY | libc::O_CLOEXEC);
            if dir < 0 {
                return Err(io::Error::last_os_error());
            }
            let fd = libc::kqueue();
            if fd < 0 {
                let e = io::Error::last_os_error();
                libc::close(dir);
                return Err(e);
            }
            libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC);
            let change = libc::kevent {
                ident: dir as usize,
                filter: libc::EVFILT_VNODE,
                flags: libc::EV_ADD | libc::EV_CLEAR,
                fflags: libc::NOTE_WRITE | libc::NOTE_DELETE | libc::NOTE_RENAME,
                data: 0,
                udata: std::ptr::null_mut(),
            };
            if libc::kevent(fd, &change, 1, std::ptr::null_mut(), 0, std::ptr::null()) < 0 {
                let e = io::Error::last_os_error();
                libc::close(fd);
                libc::close(dir);
                return Err(e);
            }
            Ok(Watch { fd, dir })
        }
    }

    /// Watch `dir`.
    #[cfg(not(target_vendor = "apple"))]
    pub fn new(dir: &Path) -> io::Result<Watch> {
        let name = cstring(dir)?;
        // SAFETY: plain system calls on a descriptor this function owns.
        unsafe {
            let fd = libc::inotify_init1(libc::IN_NONBLOCK | libc::IN_CLOEXEC);
            if fd < 0 {
                return Err(io::Error::last_os_error());
            }
            let mask = libc::IN_CREATE
                | libc::IN_DELETE
                | libc::IN_MOVED_FROM
                | libc::IN_MOVED_TO
                | libc::IN_DELETE_SELF;
            if libc::inotify_add_watch(fd, name.as_ptr(), mask) < 0 {
                let e = io::Error::last_os_error();
                libc::close(fd);
                return Err(e);
            }
            Ok(Watch { fd })
        }
    }

    /// The descriptor `poll` waits on.
    pub fn fd(&self) -> RawFd {
        self.fd
    }

    /// Take every pending notice: the caller looks at the path itself.
    pub fn drain(&self) {
        #[cfg(target_vendor = "apple")]
        {
            let zero = libc::timespec {
                tv_sec: 0,
                tv_nsec: 0,
            };
            // SAFETY: an empty kevent array of 8 on the stack.
            unsafe {
                let mut events: [libc::kevent; 8] = std::mem::zeroed();
                while libc::kevent(self.fd, std::ptr::null(), 0, events.as_mut_ptr(), 8, &zero) > 0
                {
                }
            }
        }
        #[cfg(not(target_vendor = "apple"))]
        {
            let mut buf = [0u8; 4096];
            // SAFETY: reading into a buffer on the stack.
            while unsafe { libc::read(self.fd, buf.as_mut_ptr().cast(), buf.len()) } > 0 {}
        }
    }
}

impl Drop for Watch {
    fn drop(&mut self) {
        // SAFETY: the descriptors are this watch's own.
        unsafe {
            libc::close(self.fd);
            #[cfg(target_vendor = "apple")]
            libc::close(self.dir);
        }
    }
}
