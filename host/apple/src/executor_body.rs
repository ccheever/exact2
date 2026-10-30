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
    // All three are looked at before any is made, so a refusal makes nothing.
    let link = |root: &&PathBuf| std::fs::symlink_metadata(root).is_ok_and(|m| m.is_symlink());
    if let Some(root) = roots.iter().find(link) {
        return Err(format!(
            "app storage: {} is a symbolic link",
            root.display()
        ));
    }
    for root in roots {
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
    size(roots, grants, scope, path)?;
    read_at_most(
        roots,
        grants,
        scope,
        path,
        MAX_BODY_FROM_BYTES,
        false,
        cancel,
    )
    .map_err(|e| e.unwrap_or_else(|| too_large(path, "it grew to more bytes than".into())))
}

/// The admitted `grants` and the opened directories for a read of `path`.
fn open_for<'a>(
    roots: Roots<'a>,
    grants: &str,
    scope: Option<&str>,
    path: &str,
) -> Result<(ibex2::grant::GrantSet, &'a AppDirectories), Outcome> {
    open_as("exactBodyFrom", roots, grants, scope, path)
}

/// [`open_for`], its refusals named by the option `what` (`exactBodyTo`'s too).
pub(super) fn open_as<'a>(
    what: &str,
    roots: Roots<'a>,
    grants: &str,
    scope: Option<&str>,
    path: &str,
) -> Result<(ibex2::grant::GrantSet, &'a AppDirectories), Outcome> {
    let Some(directories) = roots else {
        return Err(Outcome::Failed {
            kind: FailureKind::Unsupported,
            message: format!("{what} {path}: this host has no app files (no storage here)"),
        });
    };
    let admitted = exact_data::storage::scope(grants, scope)
        .map_err(|e| refused(format!("{what} {path}: {e}")))?;
    let grants = ibex2::grant::GrantSet::parse(&exact_runner::io_grants(admitted))
        .map_err(|e| refused(format!("the app's grants did not parse: {e}")))?;
    let directories = directories.as_ref().map_err(|e| {
        refused(format!(
            "{what} {path}: the app's files would not open: {e}"
        ))
    })?;
    Ok((grants, directories))
}

fn named(path: &str, e: ibex2::boundary::HostError) -> Outcome {
    named_as("exactBodyFrom", path, e)
}

/// A filesystem refusal of `path`, named by the option `what`.
pub(super) fn named_as(what: &str, path: &str, e: ibex2::boundary::HostError) -> Outcome {
    match e {
        // As `readFile`'s denial reads (js/src/prelude.js `deniedPath`).
        ibex2::boundary::HostError::Denied { capability } => refused(format!(
            "{what} {path}: denied: {capability} {path}: no grant covers it; grant `{capability} {path}`"
        )),
        e => refused(format!("{what} {path}: {e}")),
    }
}

/// The file's size: `stat` admits the read and names a missing file or a
/// folder before anything is read, and one over the bound.
pub(super) fn size(
    roots: Roots<'_>,
    grants: &str,
    scope: Option<&str>,
    path: &str,
) -> Result<u64, Outcome> {
    use ibex2::stdlib::fs::{FsOp, FsResult};
    let (grants, directories) = open_for(roots, grants, scope, path)?;
    match ibex2::stdlib::fs::run(&grants, Some(directories), FsOp::Stat, path, None, None)
        .map_err(|e| named(path, e))?
    {
        FsResult::Stat(stat) if !stat.is_file => {
            Err(refused(format!("exactBodyFrom {path}: not a file")))
        }
        FsResult::Stat(stat) if stat.size > MAX_BODY_FROM_BYTES => {
            Err(too_large(path, format!("{} bytes is", stat.size)))
        }
        FsResult::Stat(stat) => Ok(stat.size),
        _ => Err(refused(format!("exactBodyFrom {path}: not a file"))),
    }
}

/// At most `cap` bytes of the file, in chunks through the opened
/// descriptor, stopping between chunks once the request has ended. `Err(None)`
/// when the file holds more than `cap` (it grew, or was replaced). `exact`:
/// `cap` is the stat's size, which the async core charged (LLP 1120 D5), and
/// the buffer is allocated once at that size and one byte, never more.
pub(super) fn read_at_most(
    roots: Roots<'_>,
    grants: &str,
    scope: Option<&str>,
    path: &str,
    cap: u64,
    exact: bool,
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<Vec<u8>, Option<Outcome>> {
    let (grants, directories) = open_for(roots, grants, scope, path).map_err(Some)?;
    let bytes = {
        use std::io::Read;
        let mut file = directories
            .open_file(&grants, path)
            .map_err(|e| Some(named(path, e)))?
            .take(cap + 1);
        let mut bytes = if exact {
            Vec::with_capacity(cap as usize + 1)
        } else {
            Vec::new()
        };
        loop {
            if cancel.load(std::sync::atomic::Ordering::Acquire) {
                return Err(Some(refused(format!(
                    "exactBodyFrom {path}: the request ended"
                ))));
            }
            let at = bytes.len();
            let end = if exact {
                (at + CHUNK).min(cap as usize + 1)
            } else {
                at + CHUNK
            };
            bytes.resize(end, 0);
            let n = loop {
                match file.read(&mut bytes[at..]) {
                    Ok(n) => break n,
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                    Err(e) => {
                        return Err(Some(refused(format!(
                            "exactBodyFrom {path}: filesystem: {e}"
                        ))))
                    }
                }
            };
            bytes.truncate(at + n);
            if n == 0 {
                break bytes;
            }
        }
    };
    if bytes.len() as u64 > cap {
        return Err(None);
    }
    Ok(bytes)
}

/// Run `job` on a reader thread of its own, holding one of the process's
/// [`MAX_READERS`] places until it ends; refused, as today's fifth reader
/// is, when none is free or no thread can be made (LLP 1120 D4).
pub(super) fn reader(job: impl FnOnce() + Send + 'static) -> Result<(), Outcome> {
    use std::sync::atomic::Ordering;
    if READERS
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
    let place = Reader(&READERS);
    std::thread::Builder::new()
        .name("exact-body-from".into())
        .spawn(move || {
            let _place = place;
            job();
        })
        .map(|_| ())
        .map_err(|e| refused(format!("exactBodyFrom: the read could not start: {e}")))
}

fn too_large(path: &str, size: String) -> Outcome {
    refused(format!(
        "exactBodyFrom {path}: too-large: {size} over {MAX_BODY_FROM_BYTES}"
    ))
}
