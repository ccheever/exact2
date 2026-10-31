//! The file operations exact2 LLP 1133.007 D6a adds, by Node's names where
//! Node has them (Exact patch 24): `access`, `chmod`, `symlink`, a file's
//! `digest` read in bounded memory, and `flock`, an exclusive advisory lock
//! (POSIX `flock(2)`, a declared deviation: Node has none). A lock taken here
//! waits in [`LOCKS`] until the runtime that asked takes it
//! (`boundary_abi.rs`), which then holds it until its code's `unlock`, the
//! end of the answer that took it, or the runtime's shutdown. Each takes its
//! one argument as text in
//! the operation's data: the mode, the link's target, the algorithm, the
//! lock's timeout in milliseconds. The path is admitted above.
use super::fs::FsResult;
use crate::boundary::HostError;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Locks taken and not yet moved to a runtime, by handle.
static LOCKS: Mutex<(u64, Option<HashMap<u64, std::fs::File>>)> = Mutex::new((0, None));

fn argument(data: Option<&[u8]>) -> String {
    String::from_utf8_lossy(data.unwrap_or(&[]))
        .trim()
        .to_string()
}

fn failed(path: &Path, e: impl std::fmt::Display) -> HostError {
    HostError::Failed(format!("{}: {e}", path.display()))
}

/// The digest of `file`, read a block at a time.
pub(crate) fn digest(mut file: std::fs::File, algorithm: &str) -> Result<String, String> {
    use sha2::Digest;
    use std::io::Read;
    fn run<D: sha2::Digest>(mut hasher: D, file: &mut std::fs::File) -> Result<Vec<u8>, String> {
        let mut block = vec![0u8; 256 << 10];
        loop {
            match file.read(&mut block) {
                Ok(0) => return Ok(hasher.finalize().to_vec()),
                Ok(n) => hasher.update(&block[..n]),
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e.to_string()),
            }
        }
    }
    let bytes = match algorithm.to_ascii_uppercase().as_str() {
        "SHA-256" | "SHA256" => run(sha2::Sha256::new(), &mut file)?,
        "SHA-384" | "SHA384" => run(sha2::Sha384::new(), &mut file)?,
        "SHA-512" | "SHA512" => run(sha2::Sha512::new(), &mut file)?,
        other => {
            return Err(format!(
                "TypeError: digest: SHA-256, SHA-384 or SHA-512, not {other}"
            ))
        }
    };
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// Take an exclusive lock on `file`, waiting up to `timeout`: its handle.
pub(crate) fn lock(file: std::fs::File, timeout: Duration) -> Result<u64, String> {
    let until = Instant::now() + timeout;
    loop {
        match try_lock(&file) {
            Ok(true) => break,
            Ok(false) if Instant::now() < until => std::thread::sleep(Duration::from_millis(20)),
            Ok(false) => return Err("EBUSY: the lock is held by another".into()),
            Err(e) => return Err(e),
        }
    }
    let mut locks = LOCKS.lock().unwrap_or_else(|e| e.into_inner());
    locks.0 += 1;
    let handle = locks.0;
    locks
        .1
        .get_or_insert_with(HashMap::new)
        .insert(handle, file);
    Ok(handle)
}

#[cfg(unix)]
fn try_lock(file: &std::fs::File) -> Result<bool, String> {
    use std::os::fd::AsRawFd;
    // SAFETY: a descriptor this call owns for its duration.
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
        return Ok(true);
    }
    let e = std::io::Error::last_os_error();
    match e.raw_os_error() {
        Some(libc::EWOULDBLOCK) | Some(libc::EINTR) => Ok(false),
        _ => Err(e.to_string()),
    }
}

#[cfg(not(unix))]
fn try_lock(_file: &std::fs::File) -> Result<bool, String> {
    Err("unsupported: flock waits for this host (LLP 1133.007 stage 3)".into())
}

/// Take a lock out of this table: the runtime that asked holds it from here.
#[allow(dead_code)] // the engine's async operations, which some builds leave out
pub(crate) fn take(handle: &str) -> Option<std::fs::File> {
    let id: u64 = handle.parse().ok()?;
    let mut locks = LOCKS.lock().unwrap_or_else(|e| e.into_inner());
    locks.1.as_mut()?.remove(&id)
}

/// Release a lock by its handle.
pub(crate) fn unlock(handle: &str) -> Result<(), String> {
    let id: u64 = handle
        .parse()
        .map_err(|_| format!("unlock: not a lock handle: {handle}"))?;
    let mut locks = LOCKS.lock().unwrap_or_else(|e| e.into_inner());
    // Closing the descriptor releases its flock.
    match locks.1.as_mut().and_then(|m| m.remove(&id)) {
        Some(_) => Ok(()),
        None => Err(format!("unlock: no lock {handle}")),
    }
}

/// Whether this user may access `path` as `mode` asks (`fs.constants`:
/// `F_OK` 0, `X_OK` 1, `W_OK` 2, `R_OK` 4), counting ownership, ACLs and a
/// `noexec` mount (`faccessat` with `AT_EACCESS`).
#[cfg(unix)]
fn access(path: &Path, mode: i32) -> Result<(), HostError> {
    use std::os::unix::ffi::OsStrExt;
    let c = std::ffi::CString::new(path.as_os_str().as_bytes()).map_err(|e| failed(path, e))?;
    // SAFETY: a NUL-terminated path.
    if unsafe { libc::faccessat(libc::AT_FDCWD, c.as_ptr(), mode, libc::AT_EACCESS) } == 0 {
        Ok(())
    } else {
        Err(failed(path, std::io::Error::last_os_error()))
    }
}

#[cfg(not(unix))]
fn access(path: &Path, mode: i32) -> Result<(), HostError> {
    let meta = std::fs::metadata(path).map_err(|e| failed(path, e))?;
    if mode & 2 != 0 && meta.permissions().readonly() {
        return Err(failed(path, "EACCES"));
    }
    Ok(())
}

/// One of the operations on a native path, admitted above.
pub(crate) fn perform(
    op: super::fs::FsOp,
    path: &Path,
    data: Option<&[u8]>,
) -> Result<FsResult, HostError> {
    use super::fs::FsOp;
    match op {
        FsOp::Access => {
            let mode = argument(data)
                .parse::<i32>()
                .map_err(|_| HostError::InvalidArgument("access needs a mode".into()))?;
            access(path, mode).map(|()| FsResult::Done)
        }
        FsOp::Chmod => {
            let mode = argument(data)
                .parse::<u32>()
                .map_err(|_| HostError::InvalidArgument("chmod needs a mode".into()))?;
            #[cfg(unix)]
            let permissions = std::os::unix::fs::PermissionsExt::from_mode(mode & 0o7777);
            #[cfg(not(unix))]
            let permissions = {
                // Windows keeps the read-only bit alone.
                let mut p = std::fs::metadata(path)
                    .map_err(|e| failed(path, e))?
                    .permissions();
                p.set_readonly(mode & 0o222 == 0);
                p
            };
            std::fs::set_permissions(path, permissions)
                .map(|()| FsResult::Done)
                .map_err(|e| failed(path, e))
        }
        FsOp::Symlink => {
            let target = argument(data);
            #[cfg(unix)]
            let made = std::os::unix::fs::symlink(&target, path);
            #[cfg(not(unix))]
            let made = std::os::windows::fs::symlink_file(&target, path);
            made.map(|()| FsResult::Done).map_err(|e| failed(path, e))
        }
        FsOp::Digest => {
            let file = std::fs::File::open(path).map_err(|e| failed(path, e))?;
            digest(file, &argument(data))
                .map(FsResult::Text)
                .map_err(|e| failed(path, e))
        }
        FsOp::Flock => {
            let timeout = argument(data).parse::<u64>().unwrap_or(0);
            let file = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(path)
                .map_err(|e| failed(path, e))?;
            lock(file, Duration::from_millis(timeout.min(600_000)))
                .map(|handle| FsResult::Text(handle.to_string()))
                .map_err(|e| failed(path, e))
        }
        FsOp::Unlock => unlock(&path.to_string_lossy())
            .map(|()| FsResult::Done)
            .map_err(HostError::Failed),
        _ => Err(HostError::Failed("not a D6a operation".into())),
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::super::fs::{run, FsOp, FsResult};
    use crate::grant::GrantSet;

    #[test]
    fn the_operations_an_install_needs_work_on_native_paths() {
        let dir = std::env::temp_dir().join(format!("ibex2-fs-more-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let grants = GrantSet::parse(&format!(
            "fs.write {}\nfs.read {}",
            dir.display(),
            dir.display()
        ))
        .unwrap();
        let path = |name: &str| format!("{}/{name}", dir.display());
        let call = |op, p: &str, data: Option<&[u8]>| run(&grants, None, op, p, None, data);
        call(
            FsOp::WriteFile,
            &path("tool"),
            Some(b"#!/bin/sh\necho hi\n"),
        )
        .unwrap();
        call(FsOp::Chmod, &path("tool"), Some(b"493")).unwrap(); // 0o755
        let FsResult::Stat(stat) = call(FsOp::Stat, &path("tool"), None).unwrap() else {
            panic!()
        };
        assert_eq!(stat.mode, 0o755);
        call(FsOp::Access, &path("tool"), Some(b"1")).unwrap();
        let _ = std::fs::remove_file(path("link"));
        call(FsOp::Symlink, &path("link"), Some(path("tool").as_bytes())).unwrap();
        let FsResult::Stat(link) = call(FsOp::Lstat, &path("link"), None).unwrap() else {
            panic!()
        };
        assert!(link.is_symlink);
        assert_eq!(
            call(FsOp::Readlink, &path("link"), None).unwrap(),
            FsResult::Text(path("tool"))
        );
        let FsResult::Text(hex) = call(FsOp::Digest, &path("tool"), Some(b"SHA-256")).unwrap()
        else {
            panic!()
        };
        assert_eq!(hex.len(), 64);
        let FsResult::Text(handle) = call(FsOp::Flock, &path(".install.lock"), Some(b"0")).unwrap()
        else {
            panic!()
        };
        // A second taker waits, and gives up at its timeout.
        assert!(format!(
            "{:?}",
            call(FsOp::Flock, &path(".install.lock"), Some(b"50"))
        )
        .contains("EBUSY"));
        call(FsOp::Unlock, &handle, None).unwrap();
        let FsResult::Text(again) = call(FsOp::Flock, &path(".install.lock"), Some(b"0")).unwrap()
        else {
            panic!()
        };
        call(FsOp::Unlock, &again, None).unwrap();
        assert!(
            std::path::Path::new(&path(".install.lock")).exists(),
            "the lock file stays"
        );
    }
}
