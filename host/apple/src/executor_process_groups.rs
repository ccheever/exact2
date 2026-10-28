//! The process groups real children lead (LLP 1133.007 D9): the record a
//! crash leaves, the next launch's cleanup of it, the quit, and the
//! process's exit.
//!
//! Each live group is recorded in the app's cache directory with what tells
//! it apart later: its leader's start time, the boot, the session its
//! members share (the app's), and the recording process with its own start
//! time. A launch reaps once per process, before its first spawn, and only
//! records whose recorder is gone: another live session or instance of the
//! app keeps its children. A recorded group is ended when its leader lives
//! with the recorded start time. With the leader gone, a session tells the
//! app's members apart only when the app led it: a GUI app launchd starts
//! is in session 1 with every other such app (`getsid` answers 1), and one
//! started from a terminal shares the shell's. So members are ended then
//! only when the recorded session is the recorder's own (its id is the
//! recorder's pid) and they started no earlier than the leader did;
//! otherwise the group is left. An `exit` that no quit preceded (an agent
//! drive's) ends the live groups first, at `atexit`.
use super::Shared;
use ibex2::process as os;
use serde_json::{json, Value as Json};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

/// After a quit's signal, the longest it waits before `SIGKILL` (D2).
const QUIT_LIMIT_MS: f64 = 5000.0;

/// Edits to the record files, which every session of the app shares.
static RECORDS: Mutex<()> = Mutex::new(());
/// The record files this process has reaped: once each, before its first
/// spawn.
static REAPED: Mutex<Option<HashSet<PathBuf>>> = Mutex::new(None);
/// Every executor's live groups, with their kill signal and grace, for the
/// process's exit.
static LIVE: Mutex<Vec<(i32, String, f64)>> = Mutex::new(Vec::new());

/// Where this app records its children's groups: its cache directory.
fn records(shared: &Shared) -> Option<PathBuf> {
    Some(shared.root_paths.get()?[1].join("exact-children.json"))
}

/// An exclusive lock beside record file `path`, held while it is read and
/// written: every instance of the app shares the file, and two writers
/// would each drop the other's entries. `None` where it cannot be had.
fn lock_records(path: &Path) -> Option<std::fs::File> {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let file = (std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true))
    .open(path.with_extension("lock"))
    .ok()?;
    file.lock().ok()?;
    Some(file)
}

fn read_records(path: &Path) -> Vec<Json> {
    (std::fs::read(path).ok())
        .and_then(|bytes| serde_json::from_slice::<Vec<Json>>(&bytes).ok())
        .unwrap_or_default()
}

fn write_records(path: &Path, entries: &[Json]) {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let tmp = path.with_extension(format!("json.{}.tmp", std::process::id()));
    if std::fs::write(&tmp, Json::Array(entries.to_vec()).to_string()).is_ok() {
        let _ = std::fs::rename(&tmp, path);
    }
}

/// This process, as a record names its recorder.
fn me() -> (i64, Option<u64>) {
    let pid = std::process::id() as i32;
    (i64::from(pid), os::start_time(pid))
}

/// Whether `entry` was recorded by this process.
fn mine(entry: &Json) -> bool {
    let (pid, start) = me();
    entry["recorder"].as_i64() == Some(pid) && entry["recorderStart"].as_u64() == start
}

/// Whether the process that recorded `entry` still runs.
fn recorder_alive(entry: &Json) -> bool {
    let (Some(pid), Some(start)) = (entry["recorder"].as_i64(), entry["recorderStart"].as_u64())
    else {
        return false;
    };
    os::start_time(pid as i32) == Some(start)
}

/// Record group `pgid`, which a child just started, with its quit signal
/// and grace. The same write drops this process's groups that have ended,
/// so a restarting child costs one write per incarnation.
pub(super) fn remember(shared: &Shared, pgid: i32, signal: &str, grace: f64) {
    reap_once(shared);
    (shared.processes.groups.lock().unwrap()).insert(pgid, (signal.to_string(), grace));
    LIVE.lock().unwrap().push((pgid, signal.to_string(), grace));
    INSTALLED.store(std::process::id(), Ordering::Relaxed);
    os::wait_at_exit(end_at_exit);
    let Some(path) = records(shared) else {
        return;
    };
    let (recorder, recorder_start) = me();
    let _guard = RECORDS.lock().unwrap();
    let _file = lock_records(&path);
    let mut entries = read_records(&path);
    entries.retain(|e| {
        !mine(e)
            || e["pgid"]
                .as_i64()
                .is_some_and(|g| os::group_alive(g as i32))
    });
    entries.push(json!({
        "pgid": pgid,
        "start": os::start_time(pgid),
        "boot": os::boot_id(),
        "session": os::session_of(pgid),
        "recorder": recorder,
        "recorderStart": recorder_start,
    }));
    write_records(&path, &entries);
}

/// Group `pgid` has ended: this process stops quitting it. Its record goes
/// at the next [`remember`] or quit, or the next launch finds it ended.
pub(super) fn forget(shared: &Shared, pgid: i32) {
    shared.processes.groups.lock().unwrap().remove(&pgid);
    LIVE.lock().unwrap().retain(|(g, ..)| *g != pgid);
}

/// The process that installed [`end_at_exit`]: a child another library
/// forked inherits the `atexit` list, and its `exit` signals nothing.
static INSTALLED: AtomicU32 = AtomicU32::new(0);

/// At the process's exit: every live group gets its kill signal, then
/// `SIGKILL` after its grace (at most 5 s). A quit already drained its own.
fn end_at_exit() {
    if INSTALLED.load(Ordering::Relaxed) != std::process::id() {
        return;
    }
    let Ok(mut live) = LIVE.try_lock() else {
        return;
    };
    let live: Vec<(i32, String, f64)> = std::mem::take(&mut *live)
        .into_iter()
        .filter(|(pid, ..)| os::group_alive(*pid))
        .map(|(pid, signal, grace)| (pid, signal, grace.min(QUIT_LIMIT_MS)))
        .collect();
    for (pid, signal, _) in &live {
        let _ = os::signal_group(*pid, os::signal_number(signal).unwrap_or(os::SIGTERM));
    }
    escalate(&live);
}

/// Reap `shared`'s record file once in this process, before its first
/// spawn: a later executor of the same app (a plan swap, another window)
/// finds it done.
pub(super) fn reap_once(shared: &Shared) {
    let Some(path) = records(shared) else {
        return;
    };
    let mut reaped = REAPED.lock().unwrap();
    if reaped.get_or_insert_with(HashSet::new).insert(path) {
        reap(shared);
    }
}

/// End what the app's crashed launches left (D9), keeping the records of
/// processes that still run.
pub(super) fn reap(shared: &Shared) {
    let Some(path) = records(shared) else {
        return;
    };
    let _guard = RECORDS.lock().unwrap();
    let _file = lock_records(&path);
    let entries = read_records(&path);
    if entries.is_empty() {
        return;
    }
    let boot = os::boot_id();
    let mut kept = Vec::new();
    for entry in entries {
        if entry["boot"].as_str() != boot.as_deref() {
            continue;
        }
        if recorder_alive(&entry) {
            kept.push(entry);
            continue;
        }
        for member in survivors(&entry) {
            let _ = os::signal_process(member, os::SIGKILL);
        }
    }
    write_records(&path, &kept);
}

/// The members of a recorded group that are still the app's children: see
/// the module's comment. None when the group id names another's group, or
/// nothing tells the app's members apart.
fn survivors(entry: &Json) -> Vec<i32> {
    let (Some(pgid), Some(start)) = (entry["pgid"].as_i64(), entry["start"].as_u64()) else {
        return Vec::new();
    };
    let pgid = pgid as i32;
    if pgid <= 1 {
        return Vec::new();
    }
    let later = |m: &i32| os::start_time(*m).is_some_and(|t| t >= start);
    match os::start_time(pgid) {
        // The leader lives: it is the recorded one only if it started then.
        Some(leader) if leader == start => {
            os::group_members(pgid).into_iter().filter(later).collect()
        }
        Some(_) => Vec::new(),
        // The leader is gone: only a session the app led tells its members.
        None => {
            let session = entry["session"].as_i64().filter(|s| *s > 1);
            match session.filter(|s| Some(*s) == entry["recorder"].as_i64()) {
                Some(session) => (os::group_members(pgid).into_iter())
                    .filter(|m| os::session_of(*m).map(i64::from) == Some(session))
                    .filter(later)
                    .collect(),
                None => Vec::new(),
            }
        }
    }
}

/// Escalations a quit left running, which the process's exit waits for.
static ESCALATING: (Mutex<usize>, Condvar) = (Mutex::new(0), Condvar::new());

/// The app quits, or this executor retires (D2, D9): every group this
/// executor started gets its kill signal now; each is waited for, off the
/// caller's thread, up to its grace or 5 s, whichever is shorter, and then
/// gets `SIGKILL`. The process's exit waits for that (at most 5 s), so a
/// quit's children end before it does.
pub(super) fn quit(shared: &Shared, grace_ms: f64) {
    let live: Vec<(i32, String, f64)> = (shared.processes.groups.lock().unwrap().drain())
        .map(|(pid, (signal, grace))| (pid, signal, grace.min(grace_ms).min(QUIT_LIMIT_MS)))
        .filter(|(pid, ..)| os::group_alive(*pid))
        .collect();
    (LIVE.lock().unwrap()).retain(|(g, ..)| !live.iter().any(|(l, ..)| l == g));
    if live.is_empty() {
        prune(records(shared).as_deref());
        return;
    }
    for (pid, signal, _) in &live {
        let _ = os::signal_group(*pid, os::signal_number(signal).unwrap_or(os::SIGTERM));
    }
    *ESCALATING.0.lock().unwrap() += 1;
    os::wait_at_exit(wait_for_escalations);
    let path = records(shared);
    let spawned = std::thread::Builder::new()
        .name("exact-child-quit".into())
        .spawn(move || {
            escalate(&live);
            prune(path.as_deref());
            let mut n = ESCALATING.0.lock().unwrap();
            *n -= 1;
            ESCALATING.1.notify_all();
        });
    if spawned.is_err() {
        let mut n = ESCALATING.0.lock().unwrap();
        *n -= 1;
    }
}

/// Drop this process's entries whose groups have ended.
fn prune(path: Option<&Path>) {
    let Some(path) = path else {
        return;
    };
    let _guard = RECORDS.lock().unwrap();
    let _file = lock_records(path);
    let entries = read_records(path);
    let ended = |e: &Json| {
        mine(e)
            && !e["pgid"]
                .as_i64()
                .is_some_and(|g| os::group_alive(g as i32))
    };
    if entries.iter().any(ended) {
        let kept: Vec<Json> = entries.into_iter().filter(|e| !ended(e)).collect();
        write_records(path, &kept);
    }
}

/// Wait out each group's grace, then `SIGKILL` what is left of it.
fn escalate(live: &[(i32, String, f64)]) {
    let started = Instant::now();
    loop {
        let elapsed = started.elapsed().as_secs_f64() * 1000.0;
        let mut waiting = false;
        for (pid, _, grace) in live {
            if os::group_alive(*pid) {
                waiting = true;
                if elapsed >= *grace {
                    let _ = os::signal_group(*pid, os::SIGKILL);
                }
            }
        }
        if !waiting || elapsed >= QUIT_LIMIT_MS + 100.0 {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// At the process's exit: the quits' escalations, for at most 5 s.
fn wait_for_escalations() {
    let deadline = Instant::now() + Duration::from_millis(QUIT_LIMIT_MS as u64 + 200);
    let mut n = ESCALATING.0.lock().unwrap_or_else(|e| e.into_inner());
    while *n > 0 {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return;
        }
        n = ESCALATING
            .1
            .wait_timeout(n, left)
            .unwrap_or_else(|e| e.into_inner())
            .0;
    }
}

/// End group `pgid` from this thread: `signal`, then `SIGKILL` after
/// `grace` (at most 5 s) if any member is left. A one-shot's group, which
/// may outlive its leader, ends before its answer.
pub(super) fn end(pgid: i32, signal: &str, grace: f64) {
    if !os::group_alive(pgid) {
        return;
    }
    let _ = os::signal_group(pgid, os::signal_number(signal).unwrap_or(os::SIGTERM));
    escalate(&[(pgid, signal.to_string(), grace.min(QUIT_LIMIT_MS))]);
}
