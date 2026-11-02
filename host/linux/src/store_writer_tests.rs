use super::super::system::store_snapshot;
use super::*;
use ibex2::kv::KvStore as _;

const CHILD: &str = "EXACT_LINUX_STORE_WRITER_TEST";

/// How many commits the writer may start (`None`: any), how many it has, and how many it has written: a test steps
/// it one commit at a time.
static STEPS: (Mutex<(Option<usize>, usize, usize)>, Condvar) =
    (Mutex::new((None, 0, 0)), Condvar::new());

/// The writer, before each commit: waits for the test to let it start one more.
pub(super) fn step() {
    let (lock, moved) = &STEPS;
    let mut steps = lock.lock().unwrap_or_else(|e| e.into_inner());
    while steps.0.is_some_and(|allowed| steps.1 >= allowed) {
        steps = moved.wait(steps).unwrap_or_else(|e| e.into_inner());
    }
    steps.1 += 1;
}

/// The writer, after each commit's files.
pub(super) fn finished() {
    let (lock, moved) = &STEPS;
    lock.lock().unwrap_or_else(|e| e.into_inner()).2 += 1;
    moved.notify_all();
}

/// Lets the writer start up to `allowed` commits in all (`None`: any).
fn allow(allowed: Option<usize>) {
    let (lock, moved) = &STEPS;
    lock.lock().unwrap().0 = allowed;
    moved.notify_all();
}

/// Waits until the writer has written `n` commits' files.
fn written(n: usize) {
    let (lock, moved) = &STEPS;
    let steps = lock.lock().unwrap();
    let waited = moved
        .wait_timeout_while(steps, Duration::from_secs(10), |s| s.2 < n)
        .unwrap()
        .1;
    assert!(!waited.timed_out(), "the writer never wrote {n} commits");
}

/// Runs test `name` again in a child of its own, with a home of its own (the writer is the process's, and the store
/// is found from the environment); the child's output.
fn child(name: &str, home: &std::path::Path, extra: &[(&str, &str)]) -> std::process::Output {
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", &format!("host::store_writer::tests::{name}")])
        .env(CHILD, "1")
        .env("HOME", home)
        .env("XDG_DATA_HOME", home)
        .env_remove("EXACT_AGENT")
        .env_remove("EXACT_AGENT_STORAGE")
        .env_remove("EXACT_STORE");
    for (key, value) in extra {
        command.env(key, value);
    }
    command.output().unwrap()
}

/// [`child`], with a home made for it and taken away after, passing when the child does: whether this is the child.
fn in_child(name: &str) -> bool {
    if std::env::var_os(CHILD).is_some() {
        return true;
    }
    let home = std::env::temp_dir().join(format!(
        "exact-linux-store-writer-{}-{name}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&home);
    std::fs::create_dir_all(&home).unwrap();
    let output = child(name, &home, &[]);
    let _ = std::fs::remove_dir_all(&home);
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    false
}

fn write(name: &str, value: Option<&str>) -> exact_runner::StoreWrite {
    exact_runner::StoreWrite {
        name: name.into(),
        value: value.map(|v| v.as_bytes().to_vec()),
    }
}

/// What app `app`'s kept answer `key` is on disk now, read straight from the files (no drain).
fn on_disk(app: &str, key: &str) -> Option<String> {
    on_disk_at(&crate::picker::store_root(app).unwrap().unwrap(), key)
}

/// What the kept answer `key` is in store root `root` now.
fn on_disk_at(root: &std::path::Path, key: &str) -> Option<String> {
    let kv = ibex2::kv::FileStore::new(root.join("kv"));
    kv.get("exact.kept", key)
        .unwrap()
        .map(|v| String::from_utf8(v).unwrap())
}

/// Makes app `app`'s kept answers unwritable (their scope's directory a file), or writable again.
fn block_kept(app: &str, blocked: bool) {
    let kv = crate::picker::store_root(app).unwrap().unwrap().join("kv");
    let scope = kv.join("exact.kept");
    if blocked {
        std::fs::create_dir_all(&kv).unwrap();
        let _ = std::fs::remove_dir_all(&scope);
        std::fs::write(&scope, "in the way").unwrap();
    } else {
        std::fs::remove_file(&scope).unwrap();
    }
}

/// The commits are written in turn, each as one `flush`, never merged: with the writer let through one commit at a
/// time, the first commit's keys are on disk together before the second's value of a shared key, and nothing is on
/// disk while the writer is held (each commit returned at once, and a drain times out).
#[test]
fn commits_are_written_one_at_a_time_in_order() {
    if !in_child("commits_are_written_one_at_a_time_in_order") {
        return;
    }
    let app = "test.exact.store-writer-order";
    allow(Some(0));
    assert!(submit(
        app,
        vec![
            write("exact.kept.a", Some("1")),
            write("exact.kept.b", Some("1"))
        ]
    )
    .is_empty());
    assert!(submit(app, vec![write("exact.kept.a", Some("2"))]).is_empty());
    assert!(!drain(Duration::from_millis(50)), "the writer is held");
    assert_eq!((on_disk(app, "a"), on_disk(app, "b")), (None, None));
    allow(Some(1));
    written(1);
    assert_eq!(
        (on_disk(app, "a").as_deref(), on_disk(app, "b").as_deref()),
        (Some("1"), Some("1")),
        "the first commit alone"
    );
    allow(None);
    assert!(drain(Duration::from_secs(10)));
    assert_eq!(
        (on_disk(app, "a").as_deref(), on_disk(app, "b").as_deref()),
        (Some("2"), Some("1"))
    );
}

/// A boot in this process (its snapshot) reads every value committed before it, the snapshot waiting for the writer
/// itself; the values are in the runner's store all along, so nothing else reads the files.
#[test]
fn a_boot_reads_back_every_write_committed_before_it() {
    if !in_child("a_boot_reads_back_every_write_committed_before_it") {
        return;
    }
    let app = "test.exact.store-writer-boot";
    for value in ["1", "2", "3"] {
        assert!(submit(app, vec![write("exact.state.count", Some(value))]).is_empty());
    }
    let snapshot = store_snapshot(app, false);
    assert!(
        snapshot.contains(&("exact.state.count".into(), b"3".to_vec())),
        "{snapshot:?}"
    );
    assert!(submit(app, vec![write("exact.state.count", None)]).is_empty());
    assert!(!store_snapshot(app, false)
        .iter()
        .any(|(name, _)| name == "exact.state.count"));
}

/// A commit with nothing to write, made while a commit that will fail is still being written, still has it tried
/// again (as a commit on the committing thread did), and a drain tries a failed write once more before it says
/// the writes are done.
#[test]
fn a_failed_write_is_tried_again_by_the_next_commit_and_by_a_drain() {
    if !in_child("a_failed_write_is_tried_again_by_the_next_commit_and_by_a_drain") {
        return;
    }
    let app = "test.exact.store-writer-retry";
    block_kept(app, true);
    allow(Some(0));
    assert!(submit(app, vec![write("exact.kept.k", Some("v"))]).is_empty());
    // The empty commit comes while the first is still to be written: its retry is queued all the same.
    assert!(submit(app, Vec::new()).is_empty());
    allow(Some(1));
    written(1);
    block_kept(app, false);
    allow(None);
    // (No drain: the empty commit's own retry, the writer's second commit.)
    written(2);
    assert_eq!(
        on_disk(app, "k").as_deref(),
        Some("v"),
        "the empty commit's retry wrote it"
    );
    // A failure with no commit after it: the drain tries it once more.
    block_kept(app, true);
    let warned = submit(app, vec![write("exact.kept.k", Some("w"))]);
    assert_eq!(
        warned.len(),
        1,
        "the first failure's warning, logged by this commit: {warned:?}"
    );
    written(3);
    block_kept(app, false);
    assert!(drain(Duration::from_secs(10)));
    assert_eq!(
        on_disk(app, "k").as_deref(),
        Some("w"),
        "the drain's retry wrote it"
    );
}

/// A failed write's warning goes to its own app's next commit, never another app's.
#[test]
fn a_warning_is_logged_by_its_own_app() {
    if !in_child("a_warning_is_logged_by_its_own_app") {
        return;
    }
    let (failing, other) = ("test.exact.store-writer-a", "test.exact.store-writer-b");
    block_kept(failing, true);
    assert!(submit(failing, vec![write("exact.kept.k", Some("v"))]).is_empty());
    assert!(drain(Duration::from_secs(10)));
    assert!(
        submit(other, vec![write("exact.kept.k", Some("v"))]).is_empty(),
        "not the other app's"
    );
    let warned = submit(failing, Vec::new());
    assert_eq!(warned.len(), 1, "{warned:?}");
    assert!(
        warned[0].starts_with("store exact.kept.k failed:"),
        "{warned:?}"
    );
    block_kept(failing, false);
    assert!(drain(Duration::from_secs(10)));
    assert_eq!(on_disk(failing, "k").as_deref(), Some("v"));
}

/// What a commit wrote before the activity paused survives a kill right after: the pause drains the writer
/// (`Native.storeDrain`), and the child then ends at once, with no exit handler of its own.
#[cfg(unix)]
#[test]
fn a_kill_after_the_drain_keeps_every_write_committed_before_it() {
    let name = "a_kill_after_the_drain_keeps_every_write_committed_before_it";
    let app = "test.exact.store-writer-kill";
    if std::env::var_os(CHILD).is_none() {
        let home = std::env::temp_dir().join(format!(
            "exact-linux-store-writer-{}-kill",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).unwrap();
        let killed = child(name, &home, &[]);
        use std::os::unix::process::ExitStatusExt as _;
        assert_eq!(
            killed.status.signal(),
            Some(6),
            "the first child ends by abort: {}",
            String::from_utf8_lossy(&killed.stderr)
        );
        let next = child(name, &home, &[("EXACT_LINUX_STORE_WRITER_READ", "1")]);
        let _ = std::fs::remove_dir_all(&home);
        assert!(
            next.status.success(),
            "{}{}",
            String::from_utf8_lossy(&next.stdout),
            String::from_utf8_lossy(&next.stderr)
        );
        return;
    }
    if std::env::var_os("EXACT_LINUX_STORE_WRITER_READ").is_some() {
        // The next process: every value is there.
        for i in 0..50 {
            assert_eq!(on_disk(app, &format!("k{i}")), Some(format!("v{i}")));
        }
        return;
    }
    for i in 0..50 {
        assert!(submit(
            app,
            vec![write(&format!("exact.kept.k{i}"), Some(&format!("v{i}")))]
        )
        .is_empty());
    }
    assert!(drain(Duration::from_secs(10)));
    // As Android's kill after onStop: no unwinding, no destructors, no flush of anything else.
    std::process::abort();
}

/// An empty commit made where the environment says something else than a retry already queued (the store found
/// again after it could not be) is queued too, not folded into that retry; and a drain's retry goes to where the
/// app's last commit went, not where the environment says by then.
#[cfg(unix)]
#[test]
fn a_retry_goes_where_its_commit_said() {
    if !in_child("a_retry_goes_where_its_commit_said") {
        return;
    }
    let app = "test.exact.store-writer-roots";
    let home = std::env::var_os("HOME").unwrap();
    let root = crate::picker::store_root(app).unwrap().unwrap();
    block_kept(app, true);
    allow(Some(0));
    assert!(submit(app, vec![write("exact.kept.k", Some("v"))]).is_empty());
    // A commit while no store can be found (its retry fails), then one with the store found again.
    std::env::remove_var("HOME");
    std::env::remove_var("XDG_DATA_HOME");
    assert!(submit(app, Vec::new()).is_empty());
    std::env::set_var("HOME", &home);
    std::env::set_var("XDG_DATA_HOME", &home);
    assert!(submit(app, Vec::new()).is_empty());
    allow(Some(2));
    written(2);
    block_kept(app, false);
    allow(None);
    // (No drain: the third commit, the store found again, is the one that writes it.)
    written(3);
    assert_eq!(on_disk(app, "k").as_deref(), Some("v"));
    // A failure, then the environment names another store before the drain: its retry still goes to the first.
    block_kept(app, true);
    assert!(drain(Duration::from_secs(10)));
    // (The first part's failures are logged by this commit.)
    submit(app, vec![write("exact.kept.k", Some("w"))]);
    written(4);
    block_kept(app, false);
    let elsewhere = std::path::Path::new(&home).join("elsewhere");
    std::fs::create_dir_all(&elsewhere).unwrap();
    std::env::set_var("HOME", &elsewhere);
    std::env::set_var("XDG_DATA_HOME", &elsewhere);
    assert!(drain(Duration::from_secs(10)));
    assert_eq!(
        on_disk_at(&root, "k").as_deref(),
        Some("w"),
        "written where its commit said"
    );
    assert_eq!(
        on_disk(app, "k"),
        None,
        "not where the environment says now"
    );
}

/// A boot's wait tries a failed write once in all, however many of its 5 s rounds pass: a retry already taken is
/// not taken again.
#[test]
fn a_drain_that_already_retried_does_not_retry_again() {
    if !in_child("a_drain_that_already_retried_does_not_retry_again") {
        return;
    }
    let app = "test.exact.store-writer-once";
    block_kept(app, true);
    assert!(submit(app, vec![write("exact.kept.k", Some("v"))]).is_empty());
    written(1);
    block_kept(app, false);
    assert!(drain_retrying(Duration::from_secs(10), &mut true));
    assert_eq!(on_disk(app, "k"), None, "no second retry");
    assert!(drain_retrying(Duration::from_secs(10), &mut false));
    assert_eq!(on_disk(app, "k").as_deref(), Some("v"), "the one retry");
}
