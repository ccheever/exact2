//! A listener through the executor (LLP 1133.005 D1, D3, D6), with the
//! module's crate linked: real `unix:` sockets, events in the stream's
//! queue under D3's bounds, sends by connection, and the release.
use super::super::super::Core;
use exact_runner::{
    FailureKind, Listen, Message, Outcome, Request, RequestOut, SocketOp, SocketOrigin, SocketSend,
};
use std::{
    io::{BufRead, BufReader, Read, Write},
    os::unix::net::UnixStream,
    path::PathBuf,
    sync::mpsc::{channel, Receiver},
    time::{Duration, Instant},
};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("xe-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn setup(grants: &str) -> (Core, Receiver<()>) {
    let (wake, woke) = channel();
    let core = Core::start(
        None,
        grants,
        Box::new(move || {
            let _ = wake.send(());
        }),
    );
    (core, woke)
}

fn listener(ticket: u64, path: &std::path::Path, listen: Listen) -> RequestOut {
    let address = format!("unix:{}", path.display());
    let exact_runner::Answer::Later(request) = exact_runner::Answer::stream(Request {
        listen: Some(Box::new(listen)),
        ..Request::listen(&address)
    }) else {
        unreachable!()
    };
    RequestOut {
        ticket,
        target: "control".into(),
        request,
        forced: false,
    }
}

fn next(core: &Core, woke: &Receiver<()>) -> Outcome {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        core.begin_pump();
        if let Some(done) = core.drain().pop() {
            return done.1;
        }
        woke.recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .expect("an outcome within five seconds");
    }
}

fn event(outcome: Outcome) -> (String, String, String) {
    match outcome {
        Outcome::Message(Message {
            event, id, data, ..
        }) => (event, id, data),
        other => panic!("expected an event, got {other:?}"),
    }
}

fn expect(core: &Core, woke: &Receiver<()>, want: &str, id: &str) -> String {
    let (e, i, data) = event(next(core, woke));
    assert_eq!((e.as_str(), i.as_str()), (want, id), "{data}");
    data
}

fn batch(ops: Vec<SocketOp>) -> SocketSend {
    let ops = ops
        .into_iter()
        .map(|op| (1, "control".into(), op))
        .collect();
    SocketSend {
        origin: SocketOrigin::Mutation("reply".into()),
        ops,
    }
}

fn settled(core: &Core) -> bool {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        {
            let state = core.shared.state.lock().unwrap();
            if state.counts == [0, 0] && state.bytes == [0, 0] && state.running.is_empty() {
                return true;
            }
        }
        if Instant::now() > deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
#[ignore = "async lane: real unix sockets and threads"]
fn a_listener_reads_lines_sends_by_connection_and_goes_with_its_stream() {
    let dir = scratch("echo");
    let path = dir.join("app.sock");
    let (core, woke) = setup(&format!("net.listen unix:{}", dir.display()));
    core.run_owned(listener(1, &path, Listen::default()), None)
        .unwrap();
    let bound = expect(&core, &woke, "listening", "");
    assert!(bound.contains("\"tookOver\":false"), "{bound}");
    let mut client = UnixStream::connect(&path).unwrap();
    expect(&core, &woke, "connection", "1");
    client.write_all(b"one\ntwo\n").unwrap();
    assert_eq!(expect(&core, &woke, "message", "1"), "one");
    assert_eq!(expect(&core, &woke, "message", "1"), "two");
    let reply = batch(vec![
        SocketOp::Line {
            connection: 1,
            text: "answer".into(),
        },
        SocketOp::End { connection: 1 },
    ]);
    assert_eq!(core.socket(&reply), Ok(vec![(1, 1, 0)]));
    let mut reader = BufReader::new(client);
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    assert_eq!(line, "answer\n");
    let mut rest = Vec::new();
    reader.read_to_end(&mut rest).unwrap();
    assert!(rest.is_empty());
    assert_eq!(expect(&core, &woke, "end", "1"), "app");
    let late = batch(vec![SocketOp::Line {
        connection: 1,
        text: "late".into(),
    }]);
    assert_eq!(
        core.socket(&late),
        Err("InvalidStateError: connection 1 ended before the send went out".into())
    );
    assert_eq!(
        (core.listen_reader())(1, 1),
        None,
        "an ended connection has no count"
    );
    // The runner lets the stream go: the listener closes and its file goes.
    core.forget(|_| false);
    assert!(settled(&core));
    assert!(!path.exists());
}

#[test]
#[ignore = "async lane: real unix sockets and threads"]
fn a_connection_past_its_queue_ends_overflow_after_what_was_queued() {
    let dir = scratch("queue");
    let path = dir.join("app.sock");
    let (core, woke) = setup("net.listen unix:");
    core.run_owned(listener(1, &path, Listen::default()), None)
        .unwrap();
    expect(&core, &woke, "listening", "");
    let mut client = UnixStream::connect(&path).unwrap();
    expect(&core, &woke, "connection", "1");
    // 300 lines while the runner takes none: 256 wait, the next overflows.
    let lines: String = (0..300).map(|i| format!("line {i}\n")).collect();
    client.write_all(lines.as_bytes()).unwrap();
    let mut buf = [0u8; 1];
    assert_eq!(
        client.read(&mut buf).unwrap(),
        0,
        "the connection was closed"
    );
    for i in 0..256 {
        assert_eq!(expect(&core, &woke, "message", "1"), format!("line {i}"));
    }
    assert_eq!(expect(&core, &woke, "end", "1"), "overflow");
    // The listener stays open.
    let _again = UnixStream::connect(&path).unwrap();
    expect(&core, &woke, "connection", "2");
}

#[test]
#[ignore = "async lane: real unix sockets and threads"]
fn an_address_outside_the_grant_or_in_use_ends_the_stream_named() {
    let dir = scratch("refused");
    let (core, woke) = setup(&format!("net.listen unix:{}/granted", dir.display()));
    core.run_owned(listener(1, &dir.join("x.sock"), Listen::default()), None)
        .unwrap();
    match next(&core, &woke) {
        Outcome::Failed {
            kind: FailureKind::Refused,
            message,
        } => assert!(
            message.contains("outside the net.listen grants"),
            "{message}"
        ),
        other => panic!("expected a refusal, got {other:?}"),
    }
    let busy = dir.join("granted/busy.sock");
    std::fs::create_dir_all(busy.parent().unwrap()).unwrap();
    let _other = std::os::unix::net::UnixListener::bind(&busy).unwrap();
    core.run_owned(listener(2, &busy, Listen::default()), None)
        .unwrap();
    match next(&core, &woke) {
        Outcome::Failed {
            kind: FailureKind::Network,
            message,
        } => assert!(message.starts_with("AddressInUse: "), "{message}"),
        other => panic!("expected the address in use, got {other:?}"),
    }
    assert!(settled(&core));
}

/// The executor's copy of the module's `Config` lays out as the module's.
#[test]
fn the_config_this_side_passes_is_the_module_s() {
    use super::{AbiConfig as Ours, Config as Theirs};
    use std::mem::{align_of, offset_of, size_of};
    assert_eq!(size_of::<Ours>(), size_of::<Theirs>());
    assert_eq!(align_of::<Ours>(), align_of::<Theirs>());
    let offsets = |o: [usize; 8]| o;
    assert_eq!(
        offsets([
            offset_of!(Ours, address),
            offset_of!(Ours, address_len),
            offset_of!(Ours, grants),
            offset_of!(Ours, grants_len),
            offset_of!(Ours, takeover),
            offset_of!(Ours, first_line_timeout_ms),
            offset_of!(Ours, max_line_bytes),
            offset_of!(Ours, max_connections),
        ]),
        [
            offset_of!(Theirs, address),
            offset_of!(Theirs, address_len),
            offset_of!(Theirs, grants),
            offset_of!(Theirs, grants_len),
            offset_of!(Theirs, takeover),
            offset_of!(Theirs, first_line_timeout_ms),
            offset_of!(Theirs, max_line_bytes),
            offset_of!(Theirs, max_connections),
        ]
    );
}
