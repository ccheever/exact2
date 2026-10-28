//! A resource's listener through the executor (LLP 1133.005), with the
//! linked module installed: its grant, its events as messages, sends by
//! connection, `state.streams`' view, and the file gone once it is let go.
//! A real `unix:` socket and wall-clock waits: the async lane.
use super::super::super::Core;
use exact_runner::listen::Listen;
use exact_runner::{FailureKind, Outcome, Request, RequestOut, SocketOp, SocketOrigin, SocketSend};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::sync::mpsc::{channel, Receiver};
use std::time::{Duration, Instant};

fn installed() {
    use exact_listen::abi::*;
    super::install(super::Module {
        open: exact_listen_open,
        send: exact_listen_send,
        buffered: exact_listen_buffered,
        view: exact_listen_view,
        close: exact_listen_close,
    });
}

fn setup(grants: &str) -> (Core, Receiver<()>) {
    installed();
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

fn listen(ticket: u64, path: &std::path::Path) -> RequestOut {
    let address = format!("unix:{}", path.display());
    let exact_runner::Answer::Later(request) =
        exact_runner::Answer::stream(Request::listen(&address, Listen::default()))
    else {
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
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        core.begin_pump();
        if let Some(done) = core.drain().pop() {
            return done.1;
        }
        let _ = woke.recv_timeout(Duration::from_millis(50));
        assert!(Instant::now() < deadline, "an outcome within ten seconds");
    }
}

fn event(outcome: Outcome) -> (String, String, String) {
    match outcome {
        Outcome::Message(m) => (m.event, m.id, m.data),
        other => panic!("expected an event, got {other:?}"),
    }
}

fn dir(name: &str) -> std::path::PathBuf {
    let base = std::env::temp_dir().canonicalize().unwrap();
    let dir = base.join(format!("exact-exec-listen-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
#[ignore = "async lane: binds a real unix socket"]
fn a_listener_queues_its_events_and_sends_by_connection() {
    let d = dir("echo");
    let path = d.join("a.sock");
    let (core, woke) = setup(&format!("net.listen unix:{}", d.display()));
    core.run_owned(listen(1, &path), None).unwrap();
    let (kind, _, address) = event(next(&core, &woke));
    assert_eq!(
        (kind.as_str(), address),
        ("listening", format!("unix:{}", path.display()))
    );
    let mut client = UnixStream::connect(&path).unwrap();
    assert_eq!(
        event(next(&core, &woke)),
        ("connection".into(), "1".into(), String::new())
    );
    client.write_all(b"{\"cwd\":\"/x\"}\n").unwrap();
    assert_eq!(
        event(next(&core, &woke)),
        ("message".into(), "1".into(), "{\"cwd\":\"/x\"}".into())
    );
    let view = core.listen_reader()(1).expect("a listener");
    assert_eq!(view.connections.len(), 1);
    assert_eq!(view.connections[0].received, 1);
    let ops = vec![
        SocketOp::Line {
            connection: 1,
            text: "{\"ok\":true}".into(),
        },
        SocketOp::End { connection: 1 },
    ];
    let ops = ops
        .into_iter()
        .map(|op| (1, "control".into(), op))
        .collect();
    let batch = SocketSend {
        origin: SocketOrigin::Mapper("control".into()),
        ops,
    };
    let sent = core.socket(&batch).unwrap();
    assert_eq!(sent[0].1, 1, "one line taken");
    let mut reply = String::new();
    BufReader::new(&client).read_line(&mut reply).unwrap();
    assert_eq!(reply, "{\"ok\":true}\n");
    assert_eq!(
        event(next(&core, &woke)),
        ("end".into(), "1".into(), "app".into())
    );
    // Let go: the socket file goes with it.
    core.forget(|_| false);
    let deadline = Instant::now() + Duration::from_secs(10);
    while path.exists() {
        assert!(
            Instant::now() < deadline,
            "the socket file outlived its listener"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    std::fs::remove_dir_all(&d).unwrap();
}

#[test]
fn a_path_outside_the_grants_is_refused_before_anything_is_made() {
    let d = dir("refused");
    let (core, woke) = setup("net.listen unix:/nowhere/else");
    core.run_owned(listen(1, &d.join("a.sock")), None).unwrap();
    match next(&core, &woke) {
        Outcome::Failed { kind, message } => {
            assert_eq!(kind, FailureKind::Refused);
            assert!(message.contains("net.listen"), "{message}");
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
    assert!(!d.exists(), "nothing is created for a refused address");
}
