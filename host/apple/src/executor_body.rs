//! A request body read from an app file (LLP 1108 D6 R2): `fetch(url,
//! {exactBodyFrom: "app:/tmp/…"})` from TypeScript, `Request::body_from`
//! from Rust. The worker that runs the request reads the file just before
//! it goes out, under the same `fs.read` grant and app-directory handles a
//! source's `readFile` uses, at most [`MAX_BODY_FROM_BYTES`]. Any refusal
//! (no app files here, a denied path, a missing file, one over the bound)
//! is the request's failure, and nothing is sent. The bytes never cross the
//! source, so a 2 MB upload costs its step nothing.
//!
//! Memory: the file is read whole into the request, as a byte body would
//! have been, at most 64 MiB, on a reader thread of its own. At most
//! [`MAX_READERS`] readers run at once in a process, each counted until its
//! thread ends, so readers stuck on a stalled disk after their requests were
//! settled refuse new file bodies rather than pile up.
//!
//! Time: the request's deadline is armed before the read. The worker
//! watches it and the abort while the reader runs: either settles the
//! request at once and nothing is sent; the reader is told, stops at its
//! next 1 MiB chunk, and its bytes are dropped.
use exact_runner::{FailureKind, Outcome, Request, MAX_BODY_FROM_BYTES};
use ibex2::stdlib::app_fs::AppDirectories;
use std::path::PathBuf;

/// The app's `app:/data`, `app:/cache` and `app:/tmp` handles, opened once,
/// at the first request whose body is an app file (or why they would not
/// open); `None` on a host or drive that has no app files.
pub(super) type Roots<'a> = Option<&'a Result<AppDirectories, String>>;

/// Open the app's directories, made as storage makes them, so their handles
/// pin the roots from now on: a root renamed or replaced later (by a
/// symlink to another app's tree, say) is not followed, as `readFile`'s
/// handles do not follow it. A root that is a symbolic link when first
/// opened is refused for the same reason.
pub(super) fn open(roots: &[PathBuf; 3]) -> Result<AppDirectories, String> {
    for root in roots {
        if std::fs::symlink_metadata(root).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(format!(
                "app storage: {} is a symbolic link",
                root.display()
            ));
        }
        std::fs::create_dir_all(root).map_err(|e| format!("app storage: {e}"))?;
    }
    AppDirectories::new(&roots[0], &roots[1], &roots[2]).map_err(|e| e.to_string())
}

/// Read `request.body_from` into `request.body` under `grants` (the app's,
/// narrowed by the request's own scope). A request without one is untouched.
/// `stop` says when the request has ended meanwhile (its deadline passed, it
/// was aborted): the read runs on a thread of its own, so the worker settles
/// the request then without waiting for a stalled read, whose late bytes are
/// dropped.
pub(super) fn resolve(
    roots: Roots<'_>,
    grants: &str,
    request: &mut Request,
    stop: &dyn Fn() -> Option<Outcome>,
) -> Result<(), Outcome> {
    let Some(path) = request.body_from.clone() else {
        return Ok(());
    };
    if let Some(why) = request.body_from_refusal() {
        return Err(refused(why.to_string()));
    }
    let (roots, grants, scope) = (roots.cloned(), grants.to_string(), request.grants.clone());
    let bytes = acquire(
        &READERS,
        move |cancel| read(roots.as_ref(), &grants, scope.as_deref(), &path, cancel),
        stop,
    )?;
    request.body = bytes;
    request.body_from = None;
    Ok(())
}

/// Reads still running in this process, each holding a thread, a descriptor
/// and up to [`MAX_BODY_FROM_BYTES`], counted until its thread ends, whether
/// or not its request is still waiting; past [`MAX_READERS`] (a stalled
/// disk) a new file body is refused rather than piled up.
pub(super) struct Readers(std::sync::atomic::AtomicUsize);
impl Readers {
    pub(super) const fn new() -> Readers {
        Readers(std::sync::atomic::AtomicUsize::new(0))
    }
    #[cfg(test)]
    pub(super) fn running(&self) -> usize {
        self.0.load(std::sync::atomic::Ordering::Acquire)
    }
}
static READERS: Readers = Readers::new();
/// The most file reads at once, across this process's executors: memory
/// at most this many times 64 MiB.
pub(super) const MAX_READERS: usize = 4;
/// What one step of a read takes before it looks whether its request ended.
const CHUNK: usize = 1 << 20;

/// Releases a reader's place when its thread ends.
struct Reader(&'static Readers);
impl Drop for Reader {
    fn drop(&mut self) {
        self.0 .0.fetch_sub(1, std::sync::atomic::Ordering::AcqRel);
    }
}

/// Run `read` off this thread and wait for it, or for `stop` to end the
/// request first. A read that loses is told (`cancel`, which it checks
/// between chunks), and its result dropped; it keeps its place in `readers`
/// until its thread ends.
pub(super) fn acquire(
    readers: &'static Readers,
    read: impl FnOnce(&std::sync::atomic::AtomicBool) -> Result<Vec<u8>, Outcome> + Send + 'static,
    stop: &dyn Fn() -> Option<Outcome>,
) -> Result<Vec<u8>, Outcome> {
    use std::sync::atomic::{AtomicBool, Ordering};
    if let Some(ended) = stop() {
        return Err(ended);
    }
    if readers
        .0
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
            (n < MAX_READERS).then_some(n + 1)
        })
        .is_err()
    {
        return Err(refused(format!(
            "exactBodyFrom: {MAX_READERS} file reads are still running (a stalled disk?); nothing was sent"
        )));
    }
    let place = Reader(readers);
    let cancel = std::sync::Arc::new(AtomicBool::new(false));
    let told = cancel.clone();
    let (done, result) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("exact-body-from".into())
        .spawn(move || {
            let _place = place;
            let _ = done.send(read(&told));
        })
        .map_err(|e| refused(format!("exactBodyFrom: the read could not start: {e}")))?;
    let ended = |outcome| {
        cancel.store(true, Ordering::Release);
        Err(outcome)
    };
    loop {
        match result.recv_timeout(std::time::Duration::from_millis(5)) {
            Ok(read) => return stop().map_or(read, ended),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                if let Some(outcome) = stop() {
                    return ended(outcome);
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                return Err(refused("exactBodyFrom: the read failed".into()));
            }
        }
    }
}

fn refused(message: String) -> Outcome {
    Outcome::Failed {
        kind: FailureKind::Refused,
        message,
    }
}

/// The file's bytes, or the refusal that names why not.
pub(super) fn read(
    roots: Roots<'_>,
    grants: &str,
    scope: Option<&str>,
    path: &str,
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<Vec<u8>, Outcome> {
    use ibex2::{
        boundary::HostError,
        stdlib::fs::{FsOp, FsResult},
    };
    let named = |e: HostError| {
        match e {
        // As `readFile`'s denial reads (js/src/prelude.js `deniedPath`).
        HostError::Denied { capability } => refused(format!(
            "exactBodyFrom {path}: denied: {capability} {path}: no grant covers it; grant `{capability} {path}`"
        )),
        e => refused(format!("exactBodyFrom {path}: {e}")),
    }
    };
    let Some(directories) = roots else {
        return Err(Outcome::Failed {
            kind: FailureKind::Unsupported,
            message: format!("exactBodyFrom {path}: this host has no app files (no storage here)"),
        });
    };
    let admitted = exact_data::storage::scope(grants, scope)
        .map_err(|e| refused(format!("exactBodyFrom {path}: {e}")))?;
    let grants = ibex2::grant::GrantSet::parse(&exact_runner::io_grants(admitted))
        .map_err(|e| refused(format!("the app's grants did not parse: {e}")))?;
    let directories = directories.as_ref().map_err(|e| {
        refused(format!(
            "exactBodyFrom {path}: the app's files would not open: {e}"
        ))
    })?;
    // `stat` admits the read and names a missing file or a folder before
    // anything is read; the read itself is capped, so a file that grew since
    // is refused without being read whole.
    match ibex2::stdlib::fs::run(&grants, Some(directories), FsOp::Stat, path, None, None)
        .map_err(named)?
    {
        FsResult::Stat(stat) if !stat.is_file => {
            return Err(refused(format!("exactBodyFrom {path}: not a file")));
        }
        FsResult::Stat(stat) if stat.size > MAX_BODY_FROM_BYTES => {
            return Err(too_large(path, format!("{} bytes is", stat.size)));
        }
        _ => {}
    }
    // In chunks through the opened descriptor, at most the bound and one
    // byte, stopping between chunks once the request has ended.
    #[cfg(unix)]
    let bytes = {
        use std::io::Read;
        let mut file = directories
            .open_file(&grants, path)
            .map_err(named)?
            .take(MAX_BODY_FROM_BYTES + 1);
        let mut bytes = Vec::new();
        loop {
            if cancel.load(std::sync::atomic::Ordering::Acquire) {
                return Err(refused(format!("exactBodyFrom {path}: the request ended")));
            }
            let at = bytes.len();
            bytes.resize(at + CHUNK, 0);
            let n = loop {
                match file.read(&mut bytes[at..]) {
                    Ok(n) => break n,
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                    Err(e) => {
                        return Err(refused(format!("exactBodyFrom {path}: filesystem: {e}")))
                    }
                }
            };
            bytes.truncate(at + n);
            if n == 0 {
                break bytes;
            }
        }
    };
    // Elsewhere (Windows) there is no read capped on the opened handle yet,
    // and an uncapped read could pass the bound for a file that grew since
    // `stat`: refused, naming why, rather than read whole.
    #[cfg(not(unix))]
    let bytes: Vec<u8> = {
        let _ = (&directories, cancel);
        return Err(Outcome::Failed {
            kind: FailureKind::Unsupported,
            message: format!(
                "exactBodyFrom {path}: not on this host yet: it has no capped file read"
            ),
        });
    };
    if bytes.len() as u64 > MAX_BODY_FROM_BYTES {
        return Err(too_large(path, "it grew to more bytes than".into()));
    }
    Ok(bytes)
}

fn too_large(path: &str, size: String) -> Outcome {
    refused(format!(
        "exactBodyFrom {path}: too-large: {size} over {MAX_BODY_FROM_BYTES}"
    ))
}
