//! `fork` and `execve` (exact2 LLP 1133.007 D9), with the steps between
//! them that `posix_spawn` cannot take on every platform: a process group
//! (or session) of its own; on Linux `PR_SET_PDEATHSIG(SIGKILL)` and a check
//! that its parent is still the spawner; every signal's default disposition
//! and an empty mask, every signal blocked from the `fork` until then so no
//! inherited handler runs in the child; descriptors past 2 closed at `exec`
//! (`close_range`,
//! else each one); its working directory. Only async-signal-safe calls run
//! between `fork` and `execve`: everything they read is made before.
//!
//! The death signal follows the thread that forked: a caller forks from a
//! thread that outlives the child (exact2's supervisor waits for it).
use super::Given;
use std::ffi::{CStr, CString};
use std::io;
use std::os::fd::AsRawFd;

fn pointers(strings: &[CString]) -> Vec<*const libc::c_char> {
    (strings.iter())
        .map(|s| s.as_ptr())
        .chain(std::iter::once(std::ptr::null()))
        .collect()
}

pub(super) fn spawn(
    program: &CStr,
    argv: &[CString],
    envp: &[CString],
    cwd: &CStr,
    given: &[Given],
    session: bool,
) -> io::Result<i32> {
    let (argv, envp) = (pointers(argv), pointers(envp));
    let null = c"/dev/null";
    let sources: Vec<libc::c_int> = given
        .iter()
        .map(|g| match g {
            Given::Fd(fd) => fd.as_raw_fd(),
            Given::Null => -1,
        })
        .collect();
    // The child writes its errno here if `execve` fails; close-on-exec, so
    // a successful `execve` closes it and the parent reads nothing.
    let (report_read, report_write) = super::pipe()?;
    let mut limit: libc::rlimit = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    // SAFETY: a valid out pointer.
    unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut limit) };
    let max_fd = limit.rlim_cur.min(65_536) as libc::c_int;
    // Where `close_range` is missing, the descriptors open now, walked
    // before the fork (the walk allocates, which the child may not), mark
    // those past the loop's bound too.
    let open: Vec<libc::c_int> = std::fs::read_dir("/proc/self/fd")
        .map(|dir| {
            (dir.flatten())
                .filter_map(|e| e.file_name().to_str()?.parse().ok())
                .filter(|fd| *fd > 2)
                .collect()
        })
        .unwrap_or_default();
    // SAFETY: plain syscall.
    let parent = unsafe { libc::getpid() };
    // SAFETY: valid sigset pointers; the parent's mask is restored below on
    // every path, and the child's once its handlers are defaults.
    let old = unsafe {
        let mut all: libc::sigset_t = std::mem::zeroed();
        libc::sigfillset(&mut all);
        let mut old: libc::sigset_t = std::mem::zeroed();
        libc::pthread_sigmask(libc::SIG_SETMASK, &all, &mut old);
        old
    };
    // SAFETY: the child runs only async-signal-safe calls on memory made
    // above, then execs or exits.
    let pid = unsafe { libc::fork() };
    if pid != 0 {
        // SAFETY: restores the mask read above.
        unsafe { libc::pthread_sigmask(libc::SIG_SETMASK, &old, std::ptr::null_mut()) };
    }
    if pid < 0 {
        return Err(io::Error::last_os_error());
    }
    if pid == 0 {
        // SAFETY: the child's side, as above.
        unsafe {
            let report = report_write.as_raw_fd();
            if session {
                libc::setsid();
            } else {
                libc::setpgid(0, 0);
            }
            #[cfg(target_os = "linux")]
            {
                libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL);
                if libc::getppid() != parent {
                    libc::_exit(1);
                }
            }
            #[cfg(not(target_os = "linux"))]
            let _ = parent;
            for signal in 1..65 {
                if signal != libc::SIGKILL && signal != libc::SIGSTOP {
                    libc::signal(signal, libc::SIG_DFL);
                }
            }
            let mut none: libc::sigset_t = std::mem::zeroed();
            libc::sigemptyset(&mut none);
            libc::sigprocmask(libc::SIG_SETMASK, &none, std::ptr::null_mut());
            for (fd, source) in sources.iter().enumerate() {
                let fd = fd as libc::c_int;
                let source = if *source < 0 {
                    libc::open(null.as_ptr(), libc::O_RDWR)
                } else {
                    *source
                };
                if source < 0 || libc::dup2(source, fd) < 0 {
                    fail(report);
                }
                // Already in place: `dup2` changed nothing, its
                // close-on-exec flag included, which must not survive.
                if source == fd {
                    libc::fcntl(fd, libc::F_SETFD, 0);
                }
            }
            // Everything past 2 closes at exec; the report pipe was already
            // close-on-exec.
            #[cfg(target_os = "linux")]
            let marked = libc::syscall(
                libc::SYS_close_range,
                3u32,
                u32::MAX,
                4u32, /* CLOSE_RANGE_CLOEXEC */
            ) == 0;
            #[cfg(not(target_os = "linux"))]
            let marked = false;
            if !marked {
                for fd in (3..max_fd).chain(open.iter().copied()) {
                    libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC);
                }
            }
            if libc::chdir(cwd.as_ptr()) != 0 {
                fail(report);
            }
            libc::execve(program.as_ptr(), argv.as_ptr(), envp.as_ptr());
            fail(report);
        }
    }
    // The parent sets the group too, so a signal to it right away cannot
    // race the child's own `setpgid`; after its `exec` this fails, harmlessly.
    if !session {
        // SAFETY: plain syscall.
        unsafe { libc::setpgid(pid, pid) };
    }
    drop(report_write);
    let mut errno = [0u8; 4];
    let mut read = 0;
    let file = std::fs::File::from(report_read);
    loop {
        use std::io::Read;
        match (&file).read(&mut errno[read..]) {
            Ok(0) => break,
            Ok(n) => {
                read += n;
                if read == 4 {
                    break;
                }
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
    if read == 0 {
        return Ok(pid);
    }
    // It never ran: reap it, and say why.
    let _ = super::wait(pid);
    Err(io::Error::from_raw_os_error(i32::from_ne_bytes(errno)))
}

/// In the child: write `errno` to the report pipe and exit.
unsafe fn fail(fd: libc::c_int) -> ! {
    let bytes = (*errno_location()).to_ne_bytes();
    libc::write(fd, bytes.as_ptr().cast(), bytes.len());
    libc::_exit(127)
}

// `errno`'s address, by each libc's name for it: this file builds for every
// unix but Apple's.
#[cfg(any(
    target_os = "linux",
    target_os = "emscripten",
    target_os = "redox",
    target_os = "hurd"
))]
unsafe fn errno_location() -> *mut libc::c_int {
    libc::__errno_location()
}
#[cfg(any(target_os = "android", target_os = "netbsd", target_os = "openbsd"))]
unsafe fn errno_location() -> *mut libc::c_int {
    libc::__errno()
}
#[cfg(any(target_os = "freebsd", target_os = "dragonfly"))]
unsafe fn errno_location() -> *mut libc::c_int {
    libc::__error()
}
#[cfg(any(target_os = "solaris", target_os = "illumos"))]
unsafe fn errno_location() -> *mut libc::c_int {
    libc::___errno()
}

/// `/proc/<pid>/stat`'s fields after the command's closing parenthesis.
fn stat_fields(pid: i32) -> Option<Vec<String>> {
    let text = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let after = &text[text.rfind(')')? + 1..];
    Some(after.split_whitespace().map(str::to_string).collect())
}

pub(super) fn start_time(pid: i32) -> Option<u64> {
    // Field 22, `starttime`, is the 20th after the command.
    stat_fields(pid)?.get(19)?.parse().ok()
}

pub(super) fn boot_id() -> Option<String> {
    std::fs::read_to_string("/proc/sys/kernel/random/boot_id")
        .ok()
        .map(|s| s.trim().to_string())
}

pub(super) fn group_members(pgid: i32) -> Vec<i32> {
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|e| e.file_name().to_str()?.parse::<i32>().ok())
        // Field 5, `pgrp`, is the 3rd after the command.
        .filter(|pid| stat_fields(*pid).and_then(|f| f.get(2)?.parse::<i32>().ok()) == Some(pgid))
        .collect()
}
