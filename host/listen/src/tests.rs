//! The listener over real `unix:` sockets: lines in and out, the ends a
//! connection can have, the hold within the process (D2) and publication
//! under the lock (D4). Real sockets and wall-clock waits: the async lane.
use super::*;
use std::io::{BufRead, BufReader};
use std::sync::mpsc;

/// One event as the tests read it.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Seen {
    Listening,
    Connection(u64),
    Line(u64, String),
    End(u64, &'static str),
    Standing(&'static str),
    Error(&'static str),
}

fn dir(name: &str) -> PathBuf {
    let base = std::env::temp_dir().canonicalize().unwrap();
    let dir = base.join(format!("exact-listen-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn config(path: &std::path::Path, takeover: bool) -> Config {
    Config {
        address: format!("unix:{}", path.display()),
        path: path.to_path_buf(),
        takeover,
        first_line: None,
        max_line: 64,
        max_connections: 4,
    }
}

fn open(config: Config) -> (Listener, mpsc::Receiver<Seen>) {
    let (tx, rx) = mpsc::channel();
    let sink = Box::new(move |event: Event| {
        let seen = match event {
            Event::Listening => Seen::Listening,
            Event::Connection(id) => Seen::Connection(id),
            Event::Line(id, line) => Seen::Line(id, String::from_utf8_lossy(line).into()),
            Event::End(id, reason) => Seen::End(id, reason),
            Event::Standing(word) => Seen::Standing(word),
            Event::Error(failure) => Seen::Error(failure.kind()),
        };
        let _ = tx.send(seen);
        Taken::Queued
    });
    (Listener::open(config, sink).unwrap(), rx)
}

fn next(rx: &mpsc::Receiver<Seen>) -> Seen {
    rx.recv_timeout(Duration::from_secs(10)).expect("an event")
}

/// The next standing, past the probe connections publication makes.
fn standing(rx: &mpsc::Receiver<Seen>) -> Seen {
    loop {
        match next(rx) {
            Seen::Connection(_) | Seen::End(_, "peer") => {}
            seen => return seen,
        }
    }
}

#[test]
#[ignore = "async lane: binds real unix sockets"]
fn lines_in_and_a_reply_out() {
    let d = dir("echo");
    let path = d.join("a.sock");
    let (listener, rx) = open(config(&path, false));
    assert_eq!(next(&rx), Seen::Listening);
    let mut client = UnixStream::connect(&path).unwrap();
    assert_eq!(next(&rx), Seen::Connection(1));
    client.write_all(b"hello\r\nwor").unwrap();
    assert_eq!(next(&rx), Seen::Line(1, "hello".into()));
    client.write_all(b"ld\n").unwrap();
    assert_eq!(next(&rx), Seen::Line(1, "world".into()));
    assert_eq!(listener.view(), vec![(1, 2, 0)]);
    listener.send(1, Some(b"ok"), true).unwrap();
    let mut reply = String::new();
    BufReader::new(&client).read_line(&mut reply).unwrap();
    assert_eq!(reply, "ok\n");
    assert_eq!(next(&rx), Seen::End(1, "app"));
    // Ended: closed both ways, and a later send is dropped.
    let mut rest = Vec::new();
    client.read_to_end(&mut rest).unwrap();
    assert!(rest.is_empty());
    assert_eq!(listener.send(1, Some(b"late"), false), Err(Refused::Gone));
    listener.close();
    assert!(!path.exists(), "the socket file goes with the listener");
    std::fs::remove_dir_all(&d).unwrap();
}

#[test]
#[ignore = "async lane: binds real unix sockets"]
fn peer_timeout_overflow_and_the_connection_bound() {
    let d = dir("ends");
    let path = d.join("a.sock");
    let mut c = config(&path, false);
    c.first_line = Some(Duration::from_millis(200));
    c.max_connections = 2;
    let (listener, rx) = open(c);
    assert_eq!(next(&rx), Seen::Listening);
    let quiet = UnixStream::connect(&path).unwrap();
    assert_eq!(next(&rx), Seen::Connection(1));
    assert_eq!(next(&rx), Seen::End(1, "timeout"));
    drop(quiet);
    let mut long = UnixStream::connect(&path).unwrap();
    assert_eq!(next(&rx), Seen::Connection(2));
    long.write_all(b"short\n").unwrap();
    assert_eq!(next(&rx), Seen::Line(2, "short".into()));
    long.write_all(&[b'x'; 100]).unwrap();
    assert_eq!(next(&rx), Seen::End(2, "overflow"));
    let a = UnixStream::connect(&path).unwrap();
    assert_eq!(next(&rx), Seen::Connection(3));
    let _b = UnixStream::connect(&path).unwrap();
    assert_eq!(next(&rx), Seen::Connection(4));
    // A third at once is closed with no event.
    let mut over = UnixStream::connect(&path).unwrap();
    let mut rest = Vec::new();
    over.read_to_end(&mut rest).unwrap();
    drop(a);
    assert_eq!(next(&rx), Seen::End(3, "peer"));
    // A line past the send bound is refused, not dropped.
    let big = vec![b'y'; SEND_BYTES];
    assert_eq!(listener.send(4, Some(&big), false), Err(Refused::Quota));
    listener.close();
    std::fs::remove_dir_all(&d).unwrap();
}

#[test]
#[ignore = "async lane: binds real unix sockets"]
fn within_the_process_a_newcomer_waits_or_takes_over() {
    let d = dir("hold");
    let path = d.join("a.sock");
    let (first, rx1) = open(config(&path, false));
    assert_eq!(next(&rx1), Seen::Listening);
    // Without takeover it waits, and binds when the holder lets go.
    let (second, rx2) = open(config(&path, false));
    assert_eq!(next(&rx2), Seen::Standing("waiting"));
    first.close();
    assert_eq!(next(&rx2), Seen::Listening);
    // With it, the newest takes the address; the older keeps its connection.
    let kept = UnixStream::connect(&path).unwrap();
    assert_eq!(next(&rx2), Seen::Connection(1));
    let (third, rx3) = open(config(&path, true));
    assert_eq!(next(&rx3), Seen::Listening);
    assert_eq!(next(&rx3), Seen::Standing("took over"));
    assert_eq!(next(&rx2), Seen::Standing("displaced"));
    let _new = UnixStream::connect(&path).unwrap();
    assert_eq!(next(&rx3), Seen::Connection(1));
    second.send(1, Some(b"still mine"), false).unwrap();
    let mut line = String::new();
    BufReader::new(&kept).read_line(&mut line).unwrap();
    assert_eq!(line, "still mine\n");
    // The newer lets go: the older takes the address back.
    third.close();
    assert_eq!(next(&rx2), Seen::Standing("holding"));
    let _back = UnixStream::connect(&path).unwrap();
    assert_eq!(next(&rx2), Seen::Connection(2));
    second.close();
    assert!(!path.exists());
    std::fs::remove_dir_all(&d).unwrap();
}

#[test]
#[ignore = "async lane: binds real unix sockets"]
fn another_process_taking_over_displaces_and_its_close_gives_it_back() {
    let d = dir("watch");
    let path = d.join("a.sock");
    let (listener, rx) = open(config(&path, false));
    assert_eq!(next(&rx), Seen::Listening);
    // What another process's takeover does to the path, under the lock.
    let other = bind::publish(&path, true, false).unwrap();
    // Its probe of the live socket is a connection that ends at once.
    assert_eq!(standing(&rx), Seen::Standing("displaced"));
    bind::unpublish(&path, other.identity);
    drop(other);
    assert_eq!(standing(&rx), Seen::Standing("holding"));
    // Connection 1 was the probe, when it was accepted before the takeover.
    let _c = UnixStream::connect(&path).unwrap();
    assert!(matches!(next(&rx), Seen::Connection(1 | 2)));
    listener.close();
    std::fs::remove_dir_all(&d).unwrap();
}

#[test]
#[ignore = "async lane: binds real unix sockets"]
fn a_takeover_while_the_older_closes_keeps_the_newer_socket() {
    let d = dir("race");
    let path = d.join("a.sock");
    for _ in 0..50 {
        let older = bind::publish(&path, false, false).unwrap();
        let (p1, p2) = (path.clone(), path.clone());
        let ours = older.identity;
        let closing = std::thread::spawn(move || bind::unpublish(&p1, ours));
        let newer = std::thread::spawn(move || bind::publish(&p2, true, false).unwrap());
        closing.join().unwrap();
        let newer = newer.join().unwrap();
        drop(older);
        assert_eq!(bind::identity(&path), Some(newer.identity));
        assert!(UnixStream::connect(&path).is_ok());
        bind::unpublish(&path, newer.identity);
    }
    std::fs::remove_dir_all(&d).unwrap();
}

#[test]
#[ignore = "async lane: binds real unix sockets"]
fn a_live_socket_without_takeover_is_an_error() {
    let d = dir("busy");
    let path = d.join("a.sock");
    let held = bind::publish(&path, false, false).unwrap();
    let (_listener, rx) = open(config(&path, false));
    assert_eq!(next(&rx), Seen::Error("AddressInUse"));
    bind::unpublish(&path, held.identity);
    std::fs::remove_dir_all(&d).unwrap();
}
