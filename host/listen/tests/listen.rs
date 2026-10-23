//! The listener through its C ABI over real `unix:` sockets (LLP 1133.005
//! D1–D4): lines both ways, each way a connection ends, D4's path rules,
//! and D2's waiting and takeover within one process. Each test binds under
//! its own directory in the temporary directory.
use exact_listen::{
    event, exact_listen_buffered, exact_listen_close, exact_listen_end, exact_listen_open,
    exact_listen_send, reply, Config,
};
use std::{
    ffi::c_void,
    io::{BufRead, BufReader, Read, Write},
    os::unix::{fs::MetadataExt, net::UnixStream},
    path::{Path, PathBuf},
    sync::{Arc, Condvar, Mutex},
    time::{Duration, Instant},
};

/// What a listener told its sink, and what the sink answers.
#[derive(Default)]
struct Events {
    seen: Mutex<Vec<(u32, u64, String)>>,
    arrived: Condvar,
    /// The answer to every `CONNECTION`.
    connection_reply: Mutex<u32>,
}

extern "C" fn sink(
    ctx: *mut c_void,
    kind: u32,
    connection: u64,
    data: *const u8,
    len: usize,
) -> u32 {
    let events = unsafe { &*(ctx as *const Events) };
    let text =
        String::from_utf8_lossy(unsafe { std::slice::from_raw_parts(data, len) }).into_owned();
    events.seen.lock().unwrap().push((kind, connection, text));
    events.arrived.notify_all();
    if kind == event::CONNECTION {
        *events.connection_reply.lock().unwrap()
    } else {
        reply::TAKEN
    }
}

struct Opened {
    handle: u64,
    events: Arc<Events>,
}

impl Opened {
    fn wait(&self, kind: u32) -> (u64, String) {
        self.wait_for(|k, _, _| k == kind)
    }

    fn wait_for(&self, want: impl Fn(u32, u64, &str) -> bool) -> (u64, String) {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut seen = self.events.seen.lock().unwrap();
        loop {
            if let Some(at) = seen.iter().position(|(k, c, t)| want(*k, *c, t)) {
                let (_, c, t) = seen.remove(at);
                return (c, t);
            }
            let left = deadline.saturating_duration_since(Instant::now());
            assert!(!left.is_zero(), "no such event; seen {:?}", *seen);
            seen = self.events.arrived.wait_timeout(seen, left).unwrap().0;
        }
    }

    fn has(&self, kind: u32) -> bool {
        self.events
            .seen
            .lock()
            .unwrap()
            .iter()
            .any(|(k, ..)| *k == kind)
    }

    fn send(&self, connection: u64, text: &str) -> i32 {
        unsafe { exact_listen_send(self.handle, connection, text.as_ptr(), text.len()) }
    }
}

impl Drop for Opened {
    fn drop(&mut self) {
        exact_listen_close(self.handle);
    }
}

#[derive(Clone, Copy)]
struct Options {
    takeover: bool,
    first_line_timeout_ms: u32,
    max_line_bytes: u32,
    max_connections: u32,
}

const DEFAULTS: Options = Options {
    takeover: false,
    first_line_timeout_ms: 0,
    max_line_bytes: 65_536,
    max_connections: 16,
};

fn open(path: &Path, grants: &str, o: Options) -> Opened {
    let events = Arc::new(Events::default());
    let address = format!("unix:{}", path.display());
    let config = Config {
        address: address.as_ptr(),
        address_len: address.len(),
        grants: grants.as_ptr(),
        grants_len: grants.len(),
        takeover: o.takeover.into(),
        first_line_timeout_ms: o.first_line_timeout_ms,
        max_line_bytes: o.max_line_bytes,
        max_connections: o.max_connections,
    };
    let handle = unsafe { exact_listen_open(&config, sink, Arc::as_ptr(&events) as *mut c_void) };
    Opened { handle, events }
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("xl-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::set_permissions(&dir, std::os::unix::fs::PermissionsExt::from_mode(0o700)).unwrap();
    dir
}

fn line(reader: &mut BufReader<UnixStream>) -> String {
    let mut text = String::new();
    reader.read_line(&mut text).unwrap();
    text
}

fn ino(path: &Path) -> u64 {
    std::fs::symlink_metadata(path).unwrap().ino()
}

#[test]
#[ignore = "async lane: real unix sockets and threads"]
fn lines_go_both_ways_and_an_app_end_writes_first() {
    let dir = scratch("echo");
    let path = dir.join("app.sock");
    let l = open(&path, "unix:", DEFAULTS);
    let (_, listening) = l.wait(event::LISTENING);
    assert!(listening.contains("\"tookOver\":false"), "{listening}");
    let mode = std::fs::symlink_metadata(&path).unwrap().mode() & 0o777;
    assert_eq!(mode, 0o600, "the socket is the user's alone");
    let mut client = UnixStream::connect(&path).unwrap();
    let (id, _) = l.wait(event::CONNECTION);
    client.write_all(b"hello\r\nw\xffrld\n").unwrap();
    assert_eq!(l.wait(event::MESSAGE), (id, "hello".into()));
    assert_eq!(l.wait(event::MESSAGE), (id, "w\u{fffd}rld".into()));
    assert_eq!(l.send(id, "HELLO"), 0);
    assert_eq!(exact_listen_end(l.handle, id), 0);
    let mut reader = BufReader::new(client);
    assert_eq!(line(&mut reader), "HELLO\n");
    let mut rest = Vec::new();
    reader.read_to_end(&mut rest).unwrap();
    assert!(rest.is_empty(), "the end closes it after what was sent");
    assert_eq!(l.wait(event::END), (id, "app".into()));
    assert_eq!(l.send(id, "late"), 1, "an ended connection takes nothing");
    assert_eq!(exact_listen_buffered(l.handle, id), u64::MAX);
    drop(l);
    assert!(!path.exists(), "closing removes the socket file");
}

#[test]
#[ignore = "async lane: real unix sockets and threads"]
fn a_peer_that_leaves_a_silent_one_and_a_long_line_each_end_their_connection() {
    let dir = scratch("ends");
    let path = dir.join("app.sock");
    let o = Options {
        first_line_timeout_ms: 200,
        max_line_bytes: 8,
        ..DEFAULTS
    };
    let l = open(&path, "unix:", o);
    l.wait(event::LISTENING);
    let leaver = UnixStream::connect(&path).unwrap();
    let (first, _) = l.wait(event::CONNECTION);
    drop(leaver);
    assert_eq!(l.wait(event::END), (first, "peer".into()));
    let _silent = UnixStream::connect(&path).unwrap();
    let (second, _) = l.wait(event::CONNECTION);
    assert_eq!(l.wait(event::END), (second, "timeout".into()));
    let mut long = UnixStream::connect(&path).unwrap();
    let (third, _) = l.wait(event::CONNECTION);
    long.write_all(b"ok\n123456789\nnever\n").unwrap();
    assert_eq!(l.wait(event::MESSAGE), (third, "ok".into()));
    assert_eq!(l.wait(event::END), (third, "overflow".into()));
    assert!(
        !l.has(event::MESSAGE),
        "nothing after the line past the bound"
    );
    // A reply after the first line runs on no timer.
    let mut late = UnixStream::connect(&path).unwrap();
    let (fourth, _) = l.wait(event::CONNECTION);
    late.write_all(b"ask\n").unwrap();
    l.wait(event::MESSAGE);
    std::thread::sleep(Duration::from_millis(400));
    assert_eq!(l.send(fourth, "answer"), 0);
    assert_eq!(line(&mut BufReader::new(late)), "answer\n");
}

#[test]
#[ignore = "async lane: real unix sockets and threads"]
fn connections_past_the_bound_or_the_sinks_room_close_with_no_event() {
    let dir = scratch("full");
    let path = dir.join("app.sock");
    let l = open(
        &path,
        "unix:",
        Options {
            max_connections: 1,
            ..DEFAULTS
        },
    );
    l.wait(event::LISTENING);
    let _kept = UnixStream::connect(&path).unwrap();
    l.wait(event::CONNECTION);
    let mut refused = UnixStream::connect(&path).unwrap();
    let mut buf = [0u8; 1];
    assert_eq!(refused.read(&mut buf).unwrap(), 0, "closed at once");
    drop(_kept);
    l.wait(event::END);
    *l.events.connection_reply.lock().unwrap() = reply::FULL;
    let mut full = UnixStream::connect(&path).unwrap();
    assert_eq!(full.read(&mut buf).unwrap(), 0, "closed at once");
    std::thread::sleep(Duration::from_millis(100));
    assert!(!l.has(event::END), "a refused connection has no end");
}

#[test]
#[ignore = "async lane: real unix sockets and threads"]
fn a_stale_socket_is_replaced_a_live_one_is_in_use_and_a_file_is_never_removed() {
    let dir = scratch("exists");
    let path = dir.join("app.sock");
    drop(std::os::unix::net::UnixListener::bind(&path).unwrap());
    let stale = open(&path, "unix:", DEFAULTS);
    stale.wait(event::LISTENING);
    drop(stale);
    let other = std::os::unix::net::UnixListener::bind(&path).unwrap();
    let live = open(&path, "unix:", DEFAULTS);
    let (_, why) = live.wait(event::ERROR);
    assert!(why.starts_with("AddressInUse: "), "{why}");
    drop(live);
    assert!(path.exists(), "the other's socket stays");
    drop(other);
    std::fs::remove_file(&path).unwrap();
    std::fs::write(&path, b"").unwrap();
    let file = open(&path, "unix:", DEFAULTS);
    let (_, why) = file.wait(event::ERROR);
    assert!(
        why.starts_with("Refused: ") && why.contains("not a socket"),
        "{why}"
    );
    drop(file);
    assert!(path.is_file());
    let outside = open(
        &dir.join("x.sock"),
        &format!("unix:{}/elsewhere", dir.display()),
        DEFAULTS,
    );
    let (_, why) = outside.wait(event::ERROR);
    assert!(
        why.starts_with("Refused: ") && why.contains("outside"),
        "{why}"
    );
}

#[test]
#[ignore = "async lane: real unix sockets and threads"]
fn within_a_process_a_second_listener_waits_or_takes_over_and_gives_back() {
    let dir = scratch("d2");
    let path = dir.join("app.sock");
    let first = open(&path, "unix:", DEFAULTS);
    first.wait(event::LISTENING);
    let waiting = open(&path, "unix:", DEFAULTS);
    waiting.wait(event::WAITING);
    let newer = open(
        &path,
        "unix:",
        Options {
            takeover: true,
            ..DEFAULTS
        },
    );
    let (_, listening) = newer.wait(event::LISTENING);
    assert!(listening.contains("\"tookOver\":true"), "{listening}");
    first.wait(event::DISPLACED);
    // A new connection reaches the newest.
    let _a = UnixStream::connect(&path).unwrap();
    newer.wait(event::CONNECTION);
    assert!(!first.has(event::CONNECTION));
    // The newer lets go: the one it displaced takes the address back.
    drop(newer);
    first.wait(event::RESUMED);
    let _b = UnixStream::connect(&path).unwrap();
    first.wait(event::CONNECTION);
    // The holder lets go: the waiting one binds.
    drop(first);
    waiting.wait(event::LISTENING);
    let _c = UnixStream::connect(&path).unwrap();
    waiting.wait(event::CONNECTION);
}

#[test]
#[ignore = "async lane: real unix sockets and threads"]
fn a_close_removes_only_its_own_socket_and_a_vanished_path_is_bound_again() {
    let dir = scratch("inode");
    let path = dir.join("app.sock");
    let l = open(&path, "unix:", DEFAULTS);
    l.wait(event::LISTENING);
    // Another process takes the path over (bind a staging name, rename).
    let staged = dir.join("other");
    let other = std::os::unix::net::UnixListener::bind(&staged).unwrap();
    std::fs::rename(&staged, &path).unwrap();
    let theirs = ino(&path);
    // It closes and unlinks: this listener binds again.
    drop(other);
    std::fs::remove_file(&path).unwrap();
    l.wait(event::RESUMED);
    let _again = UnixStream::connect(&path).unwrap();
    l.wait(event::CONNECTION);
    // Taken over again, then closed: the other's socket stays.
    let other = std::os::unix::net::UnixListener::bind(&staged).unwrap();
    std::fs::rename(&staged, &path).unwrap();
    assert_ne!(ino(&path), theirs);
    let current = ino(&path);
    drop(l);
    assert_eq!(
        ino(&path),
        current,
        "the newer instance's socket is left in place"
    );
    drop(other);
}

#[test]
#[ignore = "async lane: real unix sockets and threads"]
fn a_takeover_while_the_older_closes_leaves_the_newer_socket_in_place() {
    let dir = scratch("race");
    let path = dir.join("app.sock");
    for _ in 0..40 {
        let older = open(
            &path,
            "unix:",
            Options {
                takeover: true,
                ..DEFAULTS
            },
        );
        older.wait(event::LISTENING);
        let newer_thread = {
            let path = path.clone();
            std::thread::spawn(move || {
                let newer = open(
                    &path,
                    "unix:",
                    Options {
                        takeover: true,
                        ..DEFAULTS
                    },
                );
                newer.wait(event::LISTENING);
                newer
            })
        };
        drop(older);
        let newer = newer_thread.join().unwrap();
        let mut probe = UnixStream::connect(&path).expect("the newer one's socket is in place");
        probe.write_all(b"hi\n").unwrap();
        newer.wait(event::MESSAGE);
        drop(newer);
        assert!(!path.exists());
    }
}

#[test]
#[ignore = "async lane: real unix sockets and threads"]
fn two_listeners_started_at_once_leave_the_newest_holding_the_path() {
    let dir = scratch("together");
    let path = dir.join("app.sock");
    let takeover = Options {
        takeover: true,
        ..DEFAULTS
    };
    for _ in 0..30 {
        // The newer joins while the older may still be binding.
        let older = open(&path, "unix:", takeover);
        let newer = open(&path, "unix:", takeover);
        newer.wait(event::LISTENING);
        older.wait_for(|k, _, _| k == event::LISTENING || k == event::WAITING);
        let _a = UnixStream::connect(&path).unwrap();
        newer.wait(event::CONNECTION);
        assert!(!older.has(event::CONNECTION), "the newest holds the path");
        drop(newer);
        older.wait_for(|k, _, _| k == event::LISTENING || k == event::RESUMED);
        let _b = UnixStream::connect(&path).unwrap();
        older.wait(event::CONNECTION);
        drop(older);
        assert!(!path.exists());
    }
}
