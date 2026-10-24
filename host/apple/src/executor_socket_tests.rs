//! A two-way socket on a scripted transport (LLP 1016.000 D7): every frame,
//! close, overflow and abort is the test's to make, and every frame sent
//! is recorded.
use super::super::super::Core;
use exact_runner::{
    FailureKind, Message, Outcome, Request, RequestOut, SocketOp, SocketOrigin, SocketSend,
};
use ibex2::{
    boundary::HostError,
    stdlib::{
        abort::AbortSignal,
        websocket::{self, Event, Incoming, MessageSender, MessageSource, SocketTransport},
    },
};
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc::{channel, Receiver, RecvTimeoutError, Sender},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

type Feed = Sender<Result<Event, HostError>>;

/// The far side: the test's feeds into each socket opened, what was sent,
/// and the bytes the transport says it buffers.
#[derive(Default)]
struct Peer {
    feeds: Mutex<Vec<Feed>>,
    sent: Mutex<Vec<String>>,
    buffered: AtomicUsize,
    frames: AtomicUsize,
}

struct Scripted(Arc<Peer>);
impl SocketTransport for Scripted {
    fn connect(
        &self,
        _: &url::Url,
        _: usize,
        signal: &AbortSignal,
    ) -> Result<Box<dyn MessageSource>, HostError> {
        let (feed, rx) = channel();
        self.0.feeds.lock().unwrap().push(feed);
        Ok(Box::new(Socket {
            rx,
            signal: signal.clone(),
            sender: Arc::new(Sender_(self.0.clone())),
        }))
    }
}

struct Socket {
    rx: Receiver<Result<Event, HostError>>,
    signal: AbortSignal,
    sender: Arc<Sender_>,
}
impl MessageSource for Socket {
    fn next(&mut self) -> Result<Incoming, HostError> {
        unreachable!("a two-way socket reads events")
    }
    fn next_event(&mut self) -> Result<Event, HostError> {
        loop {
            self.signal.check()?;
            match self.rx.recv_timeout(Duration::from_millis(5)) {
                Ok(event) => return event,
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => {
                    return Ok(Event::Close {
                        code: 1006,
                        reason: String::new(),
                        was_clean: false,
                    })
                }
            }
        }
    }
    fn sender(&self) -> Option<Arc<dyn MessageSender>> {
        Some(self.sender.clone())
    }
}

struct Sender_(Arc<Peer>);
impl MessageSender for Sender_ {
    fn send_text(&self, text: &str) -> Result<(), HostError> {
        self.0.sent.lock().unwrap().push(format!("text {text}"));
        Ok(())
    }
    fn send_binary(&self, bytes: &[u8]) -> Result<(), HostError> {
        self.0
            .sent
            .lock()
            .unwrap()
            .push(format!("binary {bytes:?}"));
        Ok(())
    }
    fn close(&self, code: Option<u16>, reason: &str) -> Result<(), HostError> {
        let code = code.unwrap_or(1005);
        self.0
            .sent
            .lock()
            .unwrap()
            .push(format!("close {code} {reason}"));
        Ok(())
    }
    fn buffered_amount(&self) -> usize {
        self.0.buffered.load(Ordering::Relaxed)
    }
    fn buffered_messages(&self) -> usize {
        self.0.frames.load(Ordering::Relaxed)
    }
}

const GRANTS: &str = "net.websocket ws://echo.test";

fn setup() -> (Core, Arc<Peer>, Receiver<()>) {
    let peer = Arc::new(Peer::default());
    let (wake, woke) = channel();
    let scripted = peer.clone();
    let core = Core::start(
        None,
        GRANTS,
        Box::new(move || {
            let _ = wake.send(());
        }),
    )
    .streams_on(move || {
        ibex2::host::Host::new().with_socket_transport(Box::new(Scripted(scripted.clone())))
    });
    (core, peer, woke)
}

fn socket(ticket: u64) -> RequestOut {
    let exact_runner::Answer::Later(request) = exact_runner::Answer::stream(Request {
        socket: true,
        ..Request::get("ws://echo.test/feed")
    }) else {
        unreachable!()
    };
    RequestOut {
        ticket,
        target: "feed".into(),
        request,
        forced: false,
    }
}

fn feed(peer: &Peer, at: usize) -> Feed {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(feed) = peer.feeds.lock().unwrap().get(at) {
            return feed.clone();
        }
        assert!(Instant::now() < deadline, "socket {at} never opened");
        std::thread::sleep(Duration::from_millis(2));
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

/// Wait for the reader to end, so every frame it queued is in place.
fn ended(core: &Core) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !core.shared.state.lock().unwrap().running.is_empty() {
        assert!(Instant::now() < deadline, "the reader never ended");
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn settled(core: &Core) -> bool {
    let state = core.shared.state.lock().unwrap();
    state.counts == [0, 0] && state.bytes == [0, 0] && state.running.is_empty()
}

fn text(m: &Outcome) -> &str {
    match m {
        Outcome::Message(m) => &m.data,
        other => panic!("expected a message, got {other:?}"),
    }
}

fn batch(ops: Vec<SocketOp>) -> SocketSend {
    from(SocketOrigin::Mutation("say".into()), ops)
}

fn from(origin: SocketOrigin, ops: Vec<SocketOp>) -> SocketSend {
    let ops = ops.into_iter().map(|op| (1, "feed".into(), op)).collect();
    SocketSend { origin, ops }
}

fn frame(data: &str) -> Result<Event, HostError> {
    Ok(Event::Message(websocket::Message::Text(data.into())))
}

#[test]
fn a_two_way_socket_opens_sends_in_order_and_ends_with_its_close() {
    let (core, peer, woke) = setup();
    core.run_owned(socket(1), None).unwrap();
    assert!(
        matches!(next(&core, &woke), Outcome::Message(Message { ref event, .. }) if event == "open")
    );
    let ops = vec![
        SocketOp::Text("hello".into()),
        SocketOp::Binary(vec![1, 2]),
        SocketOp::Text("bye".into()),
    ];
    peer.buffered.store(7, Ordering::Relaxed);
    let read = core.socket_reader();
    assert_eq!(read(1), Some(7), "the transport's bytes, live");
    assert_eq!(core.socket(&batch(ops)), Ok(vec![(1, 3, 7)]));
    assert_eq!(
        *peer.sent.lock().unwrap(),
        ["text hello", "binary [1, 2]", "text bye"]
    );
    // Frames that arrive before the runner takes any are each delivered.
    let wire = feed(&peer, 0);
    wire.send(frame("one")).unwrap();
    wire.send(Ok(Event::Message(websocket::Message::Binary(vec![3]))))
        .unwrap();
    wire.send(frame("two")).unwrap();
    wire.send(Ok(Event::Close {
        code: 4001,
        reason: "finished".into(),
        was_clean: true,
    }))
    .unwrap();
    ended(&core);
    assert_eq!(text(&next(&core, &woke)), "one");
    assert!(
        matches!(next(&core, &woke), Outcome::Message(Message { binary: Some(ref b), .. }) if b == &[3])
    );
    assert_eq!(text(&next(&core, &woke)), "two");
    assert_eq!(
        next(&core, &woke),
        Outcome::Closed {
            code: 4001,
            reason: "finished".into(),
            was_clean: true
        }
    );
    assert!(settled(&core));
    assert_eq!(read(1), None, "an ended socket has no count");
    let late = batch(vec![SocketOp::Text("late".into())]);
    assert_eq!(
        core.socket(&late),
        Err("InvalidStateError: the socket is not open".into())
    );
}

/// A far side that sends its next frame only once the last is
/// acknowledged: the mapper's replies, published as each frame's commit
/// stands, go out on the socket while its reader is open, in order and
/// ahead of a later mutation's frames.
#[test]
fn a_mapper_s_replies_go_out_while_its_socket_reads() {
    let (core, peer, woke) = setup();
    core.run_owned(socket(1), None).unwrap();
    let mapper = |text: String| {
        from(
            SocketOrigin::Mapper("feed".into()),
            vec![SocketOp::Text(text)],
        )
    };
    assert!(
        matches!(next(&core, &woke), Outcome::Message(Message { ref event, .. }) if event == "open")
    );
    assert_eq!(
        core.socket(&mapper("subscribe".into())),
        Ok(vec![(1, 1, 0)])
    );
    let wire = feed(&peer, 0);
    for n in 1..=3 {
        // The far side's condition for its next frame.
        let owed = match n {
            1 => "text subscribe".to_string(),
            n => format!("text Ack {}", n - 1),
        };
        assert_eq!(peer.sent.lock().unwrap().last(), Some(&owed));
        wire.send(frame(&format!("frame {n}"))).unwrap();
        assert_eq!(text(&next(&core, &woke)), format!("frame {n}"));
        assert_eq!(
            core.socket(&mapper(format!("Ack {n}"))),
            Ok(vec![(1, 1, 0)])
        );
    }
    let said = batch(vec![SocketOp::Text("after".into())]);
    assert_eq!(core.socket(&said), Ok(vec![(1, 1, 0)]));
    assert_eq!(
        *peer.sent.lock().unwrap(),
        [
            "text subscribe",
            "text Ack 1",
            "text Ack 2",
            "text Ack 3",
            "text after"
        ]
    );
}

#[test]
fn a_batch_is_refused_whole_past_the_buffer_or_after_a_close() {
    let (core, peer, woke) = setup();
    core.run_owned(socket(1), None).unwrap();
    next(&core, &woke);
    peer.buffered.store((1 << 20) - 4, Ordering::Relaxed);
    let two = batch(vec![
        SocketOp::Text("ab".into()),
        SocketOp::Text("cde".into()),
    ]);
    assert_eq!(
        core.socket(&two),
        Err("QuotaExceededError: socket buffer exceeds 1 MiB".into())
    );
    assert!(
        peer.sent.lock().unwrap().is_empty(),
        "nothing of it was sent"
    );
    peer.buffered.store(0, Ordering::Relaxed);
    // The transport would close rather than queue a 257th frame.
    peer.frames.store(255, Ordering::Relaxed);
    assert_eq!(
        core.socket(&two),
        Err("QuotaExceededError: socket buffer exceeds 256 frames".into())
    );
    assert!(peer.sent.lock().unwrap().is_empty());
    peer.frames.store(0, Ordering::Relaxed);
    let close = SocketOp::Close {
        code: 1000,
        reason: String::new(),
    };
    assert_eq!(core.socket(&batch(vec![close])), Ok(vec![(1, 0, 0)]));
    assert!(core.socket(&two).is_err(), "nothing after the close");
    assert_eq!(*peer.sent.lock().unwrap(), ["close 1000 "]);
}

#[test]
fn an_overflowing_queue_delivers_what_it_held_then_ends_refused() {
    let (core, peer, woke) = setup();
    core.run_owned(socket(1), None).unwrap();
    next(&core, &woke);
    let wire = feed(&peer, 0);
    for n in 0..=256 {
        wire.send(frame(&n.to_string())).unwrap();
    }
    ended(&core);
    for n in 0..256 {
        assert_eq!(text(&next(&core, &woke)), n.to_string());
    }
    assert_eq!(
        next(&core, &woke),
        Outcome::Failed {
            kind: FailureKind::Refused,
            message: "socket receive queue exceeds 256 frames or 1 MiB".into()
        }
    );
    assert!(settled(&core));
}

#[test]
fn a_receive_queue_allows_one_mib_beyond_the_response_ceiling() {
    for sizes in [vec![3 << 20, 3 << 20], vec![3 << 20, 2 << 20, 1]] {
        let (core, peer, woke) = setup();
        let mut request = socket(1);
        request.request = request.request.independent_http(4 << 20);
        core.run_owned(request, None).unwrap();
        next(&core, &woke);
        let wire = feed(&peer, 0);
        for size in &sizes {
            assert!(wire.send(frame(&"x".repeat(*size))).is_ok());
        }
        ended(&core);
        for size in &sizes[..sizes.len() - 1] {
            assert_eq!(text(&next(&core, &woke)).len(), *size);
        }
        assert_eq!(
            next(&core, &woke),
            Outcome::Failed {
                kind: FailureKind::Refused,
                message: "socket receive queue exceeds 256 frames or 1 MiB".into()
            }
        );
        assert!(settled(&core));
    }
}

#[test]
fn a_frame_over_the_ceiling_ends_it_refused_after_the_frames_before() {
    let (core, peer, woke) = setup();
    core.run_owned(socket(1), None).unwrap();
    next(&core, &woke);
    let wire = feed(&peer, 0);
    wire.send(frame("kept")).unwrap();
    let over = HostError::Failed("the socket message exceeded its configured limit".into());
    wire.send(Err(over)).unwrap();
    ended(&core);
    assert_eq!(text(&next(&core, &woke)), "kept");
    assert!(matches!(
        next(&core, &woke),
        Outcome::Failed { kind: FailureKind::Refused, ref message } if message.contains("ceiling")
    ));
}

#[test]
fn a_socket_let_go_refuses_its_batch_and_closes() {
    let (core, peer, woke) = setup();
    core.run_owned(socket(1), None).unwrap();
    next(&core, &woke);
    core.forget(|_| false);
    // A frame during the close's grace is dropped; the socket stays until
    // the abort, so the close is not cut off.
    feed(&peer, 0).send(frame("late")).unwrap();
    assert!(
        !core.shared.sockets.lock().unwrap().is_empty(),
        "kept for the close"
    );
    let hi = batch(vec![SocketOp::Text("hi".into())]);
    assert!(core.socket(&hi).is_err());
    assert_eq!(
        *peer.sent.lock().unwrap(),
        ["close 1000 "],
        "a closing handshake, then nothing"
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    while !settled(&core) {
        assert!(Instant::now() < deadline, "the reservation was kept");
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(core.shared.sockets.lock().unwrap().is_empty());
}

#[test]
fn open_takes_no_place_among_the_256_frames() {
    let (core, peer, woke) = setup();
    core.run_owned(socket(1), None).unwrap();
    let wire = feed(&peer, 0);
    for n in 0..256 {
        wire.send(frame(&n.to_string())).unwrap();
    }
    let close = Event::Close {
        code: 1000,
        reason: String::new(),
        was_clean: true,
    };
    wire.send(Ok(close)).unwrap();
    ended(&core);
    assert!(
        matches!(next(&core, &woke), Outcome::Message(Message { ref event, .. }) if event == "open")
    );
    for n in 0..256 {
        assert_eq!(text(&next(&core, &woke)), n.to_string());
    }
    assert!(matches!(
        next(&core, &woke),
        Outcome::Closed { code: 1000, .. }
    ));
}
