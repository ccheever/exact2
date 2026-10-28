//! The app's store writes, off the thread that commits. A kv or secret write
//! syncs its file and its directory, which took up to 154 ms a commit on an
//! Android emulator while Excord's gateway traffic came in, and a drag got no
//! frames meanwhile. Each commit hands its writes here and goes on; one writer
//! thread writes them as [`StoreWrites::flush`] did on the commit's thread.
//!
//! What a crash can leave is what it could leave before: the writer takes the
//! commits in order, one at a time, each to where the environment said when
//! it was committed and as `flush` wrote it (key by key in name order, each
//! key's file replaced whole), so a commit is never mixed with a later one. A
//! write that fails stays pending and is tried again with the app's next
//! commit, with writes or without, as before; its warning is logged by the
//! app's first commit after it was tried. What is newly at risk is a commit
//! handed over and not yet written when the process dies with no drain (a
//! crash, or a kill while in front): before, its writes were on disk when the
//! commit returned.
//!
//! Reads never wait: the runner's store already holds every value it wrote.
//! The only reads of the files, at a boot (the snapshot and the locked
//! secrets' backend), [`settle`] the writer first, and the activity going to
//! the background, and an orderly exit, [`drain`] it.

use super::system::{store_roots, StoreRoots, StoreWrites};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::{Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

/// One commit's writes for an app, and where they go.
struct Commit {
    app: String,
    roots: StoreRoots,
    writes: Vec<exact_runner::StoreWrite>,
}

#[derive(Default)]
struct Queue {
    /// The commits handed over, in order.
    commits: VecDeque<Commit>,
    /// The app whose commit is being written now.
    busy: Option<String>,
    /// The apps with a write that failed and waits to be tried again.
    pending: BTreeSet<String>,
    /// Each app's warnings not yet logged by one of its commits.
    warnings: BTreeMap<String, Vec<String>>,
    /// Where each app's last commit went: a drain's retry goes there, not
    /// wherever the environment says by then.
    roots: BTreeMap<String, StoreRoots>,
}

impl Queue {
    fn idle(&self) -> bool {
        self.busy.is_none() && self.commits.is_empty()
    }
}

struct Writer {
    queue: Mutex<Queue>,
    /// Work came in.
    work: Condvar,
    /// A commit was written.
    written: Condvar,
}

fn writer() -> &'static Writer {
    static WRITER: OnceLock<&'static Writer> = OnceLock::new();
    WRITER.get_or_init(|| {
        let writer: &'static Writer = Box::leak(Box::new(Writer {
            queue: Mutex::new(Queue::default()),
            work: Condvar::new(),
            written: Condvar::new(),
        }));
        std::thread::Builder::new()
            .name("exact store".into())
            .spawn(move || write_all(writer))
            .expect("the store writer thread starts");
        writer
    })
}

fn write_all(writer: &'static Writer) {
    // Each app's pending writes, as each host kept them.
    let mut stores: BTreeMap<String, StoreWrites> = BTreeMap::new();
    let mut queue = writer.queue.lock().unwrap_or_else(|e| e.into_inner());
    loop {
        let Some(commit) = queue.commits.pop_front() else {
            queue = writer.work.wait(queue).unwrap_or_else(|e| e.into_inner());
            continue;
        };
        queue.busy = Some(commit.app.clone());
        drop(queue);
        #[cfg(test)]
        tests::step();
        let store = stores.entry(commit.app.clone()).or_default();
        let warnings = store.flush_to(&commit.roots, &commit.writes);
        let pending = store.pending();
        #[cfg(test)]
        tests::finished();
        queue = writer.queue.lock().unwrap_or_else(|e| e.into_inner());
        if !warnings.is_empty() {
            queue
                .warnings
                .entry(commit.app.clone())
                .or_default()
                .extend(warnings);
        }
        if pending {
            queue.pending.insert(commit.app);
        } else {
            queue.pending.remove(&commit.app);
        }
        queue.busy = None;
        writer.written.notify_all();
    }
}

/// Queues `app`'s writes (none: a retry of what failed) to `roots`. A retry
/// is queued only when one is owed: the app has a failed write, or a commit
/// of its own still being written (which may fail), and its last queued
/// commit is not already a retry to the same place.
fn queue_commit(
    queue: &mut Queue,
    app: &str,
    roots: StoreRoots,
    writes: Vec<exact_runner::StoreWrite>,
) -> bool {
    if writes.is_empty() {
        let owed = queue.pending.contains(app)
            || queue.busy.as_deref() == Some(app)
            || queue.commits.iter().any(|c| c.app == app);
        let waiting = queue
            .commits
            .iter()
            .rev()
            .find(|c| c.app == app)
            .is_some_and(|c| c.writes.is_empty() && c.roots == roots);
        if !owed || waiting {
            return false;
        }
    }
    queue.roots.insert(app.to_string(), roots.clone());
    queue.commits.push_back(Commit {
        app: app.to_string(),
        roots,
        writes,
    });
    true
}

/// A commit's store writes for app `app_id`, written by the writer thread in
/// turn; the warnings of this app's writes tried since its last commit. A
/// commit with no writes still has a failed write tried again, as before.
pub(super) fn submit(app_id: &str, writes: Vec<exact_runner::StoreWrite>) -> Vec<String> {
    let writer = writer();
    let mut queue = writer.queue.lock().unwrap_or_else(|e| e.into_inner());
    let warnings = queue.warnings.remove(app_id).unwrap_or_default();
    if queue_commit(&mut queue, app_id, store_roots(app_id), writes) {
        writer.work.notify_one();
    }
    warnings
}

/// The warnings of `app_id`'s writes that no commit of its has logged (an
/// orderly exit's, after its drain).
pub fn take_warnings(app_id: &str) -> Vec<String> {
    let writer = writer();
    let mut queue = writer.queue.lock().unwrap_or_else(|e| e.into_inner());
    queue.warnings.remove(app_id).unwrap_or_default()
}

/// Waits until every commit handed over is written, at most `timeout`;
/// whether they all were. A write that failed is tried once more first, as a
/// commit would try it, so a failure just before the activity goes to the
/// background is not left for a commit that may never come. Commits handed
/// over while it waits are waited for too; a write that fails again stays
/// pending and does not count. The activity going to the background drains
/// it (`Native.storeDrain`), so a process killed after `onStop` keeps what it
/// committed before `onPause`.
pub fn drain(timeout: Duration) -> bool {
    drain_retrying(timeout, &mut false)
}

/// [`drain`], its one retry of what failed taken already when `retried`.
fn drain_retrying(timeout: Duration, retried: &mut bool) -> bool {
    let writer = writer();
    let deadline = Instant::now() + timeout;
    let mut queue = writer.queue.lock().unwrap_or_else(|e| e.into_inner());
    loop {
        if queue.idle() {
            if *retried || queue.pending.is_empty() {
                return true;
            }
            *retried = true;
            // Each to where its app's last commit went.
            let apps: Vec<String> = queue.pending.iter().cloned().collect();
            for app in apps {
                if let Some(roots) = queue.roots.get(&app).cloned() {
                    queue_commit(&mut queue, &app, roots, Vec::new());
                }
            }
            writer.work.notify_one();
            continue;
        }
        let Some(left) = deadline.checked_duration_since(Instant::now()) else {
            return false;
        };
        queue = writer
            .written
            .wait_timeout(queue, left)
            .unwrap_or_else(|e| e.into_inner())
            .0;
    }
}

/// Waits for every commit handed over to be written, however long, before a
/// boot reads the files: a partly written store is never read. A failed write
/// is tried once more, once in all; said every 5 s it waits.
pub(super) fn settle() {
    let mut retried = false;
    while !drain_retrying(Duration::from_secs(5), &mut retried) {
        eprintln!("exact: waiting for the store's writes before reading it back");
    }
}

#[cfg(test)]
#[path = "store_writer_tests.rs"]
mod tests;
