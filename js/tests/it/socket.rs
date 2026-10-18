//! A resource's two-way socket in a TypeScript module, through the runner
//! (LLP 1016.000 D7): the mapper's handle and its replies, a mutation's
//! staged frames, a resource's answer's sends, and what refuses them.

#![cfg(exact_js_engine)]

use exact_js::Module;
use exact_kernel::Kernel;
use exact_plan::Value;
use exact_runner::{
    Dispatch, Message, Outcome, RequestOut, Response, Runner, SocketOp, SocketOrigin,
};

const HBC: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/socket.hbc"));

const SRC: &str = r#"
component App
  state id = "a"
  state q = ""
  resource feed = feed(id) as shape string
  resource side = feed("side") as shape string
  resource reply = ask(id, q) as shape string
  resource deliver = deliver(side) as shape string
  mutation said as shape string
  mutation watched as shape string
  action say(what: string)
    send said = say(id, what)
  action ask(what: string)
    q = what
  action askAndSay(what: string)
    q = what
    send said = say(id, what)
  action watch
    send watched = feed("m")
  action listen
    send watched = feed("quiet")
  action switch
    id = "b"
  view
    text feed testId="feed"
"#;

fn boot() -> (Runner<Module>, u64) {
    let (r, feed, _) = boot_both();
    (r, feed)
}

/// The runner, and the tickets of `feed`'s socket and `side`'s.
fn boot_both() -> (Runner<Module>, u64, u64) {
    let mut m = Module::new(
        HBC.to_vec(),
        "test.socket",
        "net.websocket wss://echo.test\n",
    );
    m.load().expect("the fixture loads");
    let plan = contract::compile(SRC).expect("the fixture compiles");
    let mut r = Runner::boot(plan, m, Kernel::with_monospace(), Default::default(), "/").unwrap();
    let out = r.take_requests();
    assert_eq!(out.len(), 2);
    assert!(out[0].request.stream && out[0].request.socket);
    assert_eq!(out[0].request.url, "wss://echo.test/a");
    assert_eq!(out[1].request.url, "wss://echo.test/side");
    (r, out[0].ticket, out[1].ticket)
}

fn deliver(r: &mut Runner<Module>, ticket: u64, message: Message) -> String {
    r.fulfill(ticket, Outcome::Message(message)).unwrap();
    feed(r)
}

fn feed(r: &Runner<Module>) -> String {
    r.resource("feed")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

/// The socket opens; the subscribe its mapper sends then is set aside.
fn open(r: &mut Runner<Module>, ticket: u64) -> String {
    let open = Message {
        event: "open".into(),
        ..Message::default()
    };
    let shown = deliver(r, ticket, open);
    r.take_socket_sends();
    shown
}

fn frame(r: &mut Runner<Module>, ticket: u64, data: &str) -> String {
    let m = Message {
        data: data.into(),
        ..Message::default()
    };
    r.fulfill(ticket, Outcome::Message(m)).unwrap();
    feed(r)
}

/// The batches the host would send now: who staged each, and its operations.
fn sends(r: &mut Runner<Module>) -> Vec<(SocketOrigin, Vec<SocketOp>)> {
    (r.take_socket_sends().into_iter())
        .map(|s| (s.origin, s.ops.into_iter().map(|(_, _, op)| op).collect()))
        .collect()
}

fn mapper(ops: Vec<SocketOp>) -> (SocketOrigin, Vec<SocketOp>) {
    (SocketOrigin::Mapper("feed".into()), ops)
}

/// The mutation's answer, and the operations it staged.
fn say(r: &mut Runner<Module>, what: &str) -> (String, Vec<SocketOp>) {
    r.act("say", vec![Value::str(what)]).unwrap();
    let said = match r.slot("said") {
        Some(Value::Option(Some(v))) => v.as_str().unwrap_or("").to_string(),
        _ => String::new(),
    };
    let ops = r
        .take_socket_sends()
        .into_iter()
        .flat_map(|s| s.ops.into_iter().map(|(_, _, op)| op))
        .collect();
    (said, ops)
}

fn journal(r: &Runner<Module>) -> String {
    r.journal().collect::<Vec<_>>().join("\n")
}

/// Where a handle refuses an operation outside a turn that may send.
const ONLY: &str = "socket.send is only available in a mutation's synchronous source, its socket's own mapper or an answer in flight";

/// A batch an answer's turn sent: the answer it is named for, and its text frames.
fn answer(name: &str, frames: &[&str]) -> (SocketOrigin, Vec<SocketOp>) {
    let ops = frames
        .iter()
        .map(|t| SocketOp::Text(t.to_string()))
        .collect();
    (SocketOrigin::Answer(name.into()), ops)
}

/// Ask `reply` with `q`: the requests that commit handed out.
fn ask(r: &mut Runner<Module>, q: &str) -> Vec<RequestOut> {
    r.act("ask", vec![Value::str(q)]).unwrap();
    r.take_requests()
}

fn reply(r: &Runner<Module>) -> String {
    r.resource("reply")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

/// `reply`'s answer waits on the module; the reply frame on `side` wakes
/// it, and it is asked again.
fn settle_waiting(r: &mut Runner<Module>, waiting: &RequestOut, side: u64, frame_data: &str) {
    let token = waiting.request.continuation.expect("a waiting answer");
    assert!(matches!(r.dispatch_work(token), Dispatch::Held));
    frame(r, side, frame_data);
    let (woken, Dispatch::Again) = r.release_work().into_iter().next().expect("woken") else {
        panic!("a waiting answer is asked again");
    };
    assert_eq!(woken, token);
    r.fulfill(waiting.ticket, Dispatch::again_outcome())
        .unwrap();
}

fn ok() -> Outcome {
    Outcome::Response(Response {
        status: 200,
        headers: Vec::new(),
        body: b"ok".to_vec(),
    })
}

#[test]
fn the_mapper_gets_the_handle_and_a_mutation_sends_text_and_bytes_in_order() {
    let (mut r, ticket) = boot();
    assert_eq!(open(&mut r, ticket), "open  1");
    let text = |t: &str| SocketOp::Text(t.into());
    assert_eq!(
        say(&mut r, "hello"),
        ("sent".into(), vec![text("hello"), text("hello again")])
    );
    assert_eq!(
        say(&mut r, "binary"),
        ("sent".into(), vec![SocketOp::Binary(vec![1, 2, 3])])
    );
    for data in ["echo hello", "echo hello again"] {
        let m = Message {
            data: data.into(),
            ..Message::default()
        };
        assert!(deliver(&mut r, ticket, m).starts_with(&format!("message {data} 1")));
    }
    let bytes = Message {
        binary: Some(vec![4, 5]),
        ..Message::default()
    };
    assert!(deliver(&mut r, ticket, bytes).starts_with("message bytes 4,5 1"));
    let close = SocketOp::Close {
        code: 4001,
        reason: "finished".into(),
    };
    assert_eq!(say(&mut r, "close"), ("closed".into(), vec![close]));
    let closed = Outcome::Closed {
        code: 4001,
        reason: "finished".into(),
        was_clean: true,
    };
    r.fulfill(ticket, closed).unwrap();
    assert_eq!(feed(&r), format!("close 4001 finished true ({ONLY})"));
    assert_eq!(
        say(&mut r, "late"),
        ("InvalidStateError: the socket is not open".into(), vec![])
    );
}

/// The mapper replies on its own socket while it answers `open` and each
/// `message`: subscribe, an Ack per frame, a close. Each frame's replies
/// go out with its commit, in arrival order, ahead of a later mutation's.
#[test]
fn the_mapper_replies_on_its_own_socket_during_open_and_each_message() {
    let (mut r, ticket) = boot();
    let open = Message {
        event: "open".into(),
        ..Message::default()
    };
    assert_eq!(deliver(&mut r, ticket, open), "open  1");
    let text = |t: &str| SocketOp::Text(t.into());
    assert_eq!(sends(&mut r), [mapper(vec![text("subscribe")])]);
    for n in 1..=3 {
        assert_eq!(
            frame(&mut r, ticket, &format!("frame {n}")),
            format!("message frame {n} 1")
        );
    }
    r.act("say", vec![Value::str("after")]).unwrap();
    let said = (
        SocketOrigin::Mutation("said".into()),
        vec![text("after"), text("after again")],
    );
    assert_eq!(
        sends(&mut r),
        [
            mapper(vec![text("Ack 1")]),
            mapper(vec![text("Ack 2")]),
            mapper(vec![text("Ack 3")]),
            said,
        ]
    );
    // Its `bufferedAmount` counts what this call has staged.
    assert_eq!(
        frame(&mut r, ticket, "mapper probe"),
        "message mapper probe 1 0:3"
    );
    assert_eq!(sends(&mut r), [mapper(vec![text("abc")])]);
    frame(&mut r, ticket, "mapper close");
    let close = SocketOp::Close {
        code: 4001,
        reason: "done".into(),
    };
    assert_eq!(sends(&mut r), [mapper(vec![close])]);
}

/// What a mapper call may not do sends nothing: a throw, a snapshot
/// outside its shape, overload (which fails the snapshot even when the
/// mapper catches it); another socket's handle and a fetch are refused,
/// and a continuation is not the call's. Each frame that fails ends the
/// stream, so each case boots afresh.
#[test]
fn what_a_mapper_may_not_do_sends_nothing() {
    // Whether the call had staged frames before it failed: the journal
    // says they were refused.
    for (data, why, staged) in [
        ("mapper throws", "the mapper threw", true),
        ("mapper misshapen", "outside its shape", true),
        (
            "mapper too many",
            "socket mapper call exceeds 256 operations",
            true,
        ),
        ("mapper too big", "socket mapper call exceeds 1 MiB", false),
    ] {
        let (mut r, ticket) = boot();
        open(&mut r, ticket);
        frame(&mut r, ticket, "frame 1");
        frame(&mut r, ticket, data);
        assert_eq!(
            sends(&mut r),
            [mapper(vec![SocketOp::Text("Ack 1".into())])],
            "{data}"
        );
        assert!(
            !r.holds(ticket),
            "{data}: a frame that fails ends the stream"
        );
        let journal = journal(&r);
        assert!(journal.contains(why), "{data}: {journal}");
        let refused = journal.lines().find(|l| l.contains("mapper feed failed: "));
        assert_eq!(
            refused.is_some_and(|l| l.contains(why) && l.ends_with("; socket frames were refused")),
            staged,
            "{data}: {journal}"
        );
    }
    let (mut r, ticket, side) = boot_both();
    open(&mut r, ticket);
    open(&mut r, side);
    assert_eq!(
        frame(&mut r, ticket, "mapper other"),
        format!("message mapper other 1 ({ONLY})")
    );
    frame(&mut r, ticket, "mapper later");
    frame(&mut r, ticket, "mapper fetch");
    assert!(sends(&mut r).is_empty());
    assert!(
        r.take_requests().is_empty(),
        "the mapper's fetch never goes out"
    );
    // The continuation runs in the next turn, `feed`'s ask for "b", and
    // sends as that answer on the old socket, which this commit releases:
    // a host refuses the batch at publication.
    r.act("switch", vec![]).unwrap();
    let newer = r.take_requests()[0].ticket;
    let batches = r.take_socket_sends();
    assert_eq!(batches.len(), 1);
    assert_eq!(batches[0].origin, SocketOrigin::Answer("feed".into()));
    let never = (ticket, "feed".to_string(), SocketOp::Text("never".into()));
    assert_eq!(batches[0].ops, [never]);
    assert!(!r.holds(ticket));
    open(&mut r, newer);
    let report = frame(&mut r, newer, "report");
    let fetch = "fetch() is unavailable in a socket's mapper: it answers each frame now";
    assert_eq!(report, format!("message report 1 {fetch}"));
    assert!(sends(&mut r).is_empty());
}

#[test]
fn what_the_handle_refuses_sends_nothing_it_refused() {
    let (mut r, ticket) = boot();
    open(&mut r, ticket);
    assert_eq!(
        say(&mut r, "bad code"),
        (
            "InvalidAccessError: invalid socket close code".into(),
            vec![]
        )
    );
    let (said, ops) = say(&mut r, "after close");
    assert_eq!(said, "InvalidStateError: the socket is not open");
    assert!(matches!(ops[..], [SocketOp::Close { code: 1000, .. }]));
    let (said, ops) = say(&mut r, "too many");
    assert_eq!(
        said,
        "QuotaExceededError: socket mutation exceeds 256 operations"
    );
    assert_eq!(ops.len(), 256, "the frames before the refusal stand");
    let (said, ops) = say(&mut r, "too big");
    assert_eq!(said, "QuotaExceededError: socket mutation exceeds 1 MiB");
    assert!(ops.is_empty());
    for what in ["throws", "async"] {
        assert_eq!(say(&mut r, what).1, vec![], "{what}");
    }
    let journal = journal(&r);
    assert!(journal.contains("the source threw"), "{journal}");
    assert!(
        journal.contains("a socket mutation must answer synchronously"),
        "{journal}"
    );
}

/// `bufferedAmount` is the transport's count as the turn began plus what
/// the turn stages; a turn that sends nothing still reads a drained buffer.
#[test]
fn buffered_amount_is_the_turn_s_snapshot_plus_what_it_stages() {
    let (mut r, ticket) = boot();
    open(&mut r, ticket);
    use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
    let transport = std::sync::Arc::new(AtomicU64::new(0));
    let reads = transport.clone();
    r.set_socket_reader(std::sync::Arc::new(move |t| {
        (t == ticket).then(|| reads.load(Relaxed))
    }));
    let (said, ops) = say(&mut r, "probe");
    assert_eq!(said, "0:3");
    assert_eq!(ops, vec![SocketOp::Text("abc".into())]);
    // The transport is backed up: this turn reads it, and its frame would
    // pass 1 MiB with it.
    transport.store(900_000, Relaxed);
    assert_eq!(say(&mut r, "read").0, "900000");
    let (said, ops) = say(&mut r, "big");
    assert_eq!(said, "QuotaExceededError: socket buffer exceeds 1 MiB");
    assert!(ops.is_empty());
    // It drains while the module sends nothing; the next turn sees it.
    transport.store(0, Relaxed);
    assert_eq!(say(&mut r, "read").0, "0");
    assert_eq!(
        say(&mut r, "big"),
        ("sent".into(), vec![SocketOp::Text("x".repeat(200_000))])
    );
    let state = exact_runner::agent::state(&r);
    assert!(
        state.contains(r#""readyState":1,"bufferedAmount":0"#),
        "{state}"
    );
}

/// A mutation that sends and fetches is refused before either effect,
/// whichever it does first: no frame is staged and no request handed out.
#[test]
fn a_mutation_cannot_both_send_and_fetch() {
    let (mut r, ticket) = boot();
    open(&mut r, ticket);
    for what in [
        "send then fetch",
        "fetch then send",
        "send then fetch later",
    ] {
        let (said, ops) = say(&mut r, what);
        assert_eq!((said.as_str(), ops), ("", vec![]), "{what}");
        assert!(r.take_requests().is_empty(), "{what}: no fetch goes out");
    }
    // A refused mutation's continuation fetches nothing either.
    assert_eq!(say(&mut r, "send, await, fetch"), (String::new(), vec![]));
    assert!(
        r.take_requests().is_empty(),
        "the continuation's fetch never goes out"
    );
    let journal = journal(&r);
    assert_eq!(
        journal
            .matches("send said failed: a mutation cannot both send on a socket and fetch")
            .count(),
        3,
        "{journal}"
    );
    assert_eq!(say(&mut r, "x").1.len(), 2, "the module goes on");
}

#[test]
fn the_caller_s_abort_makes_the_handle_stale() {
    let (mut r, ticket) = boot();
    open(&mut r, ticket);
    assert_eq!(
        say(&mut r, "abort"),
        ("InvalidStateError: the socket is not open".into(), vec![])
    );
    assert_eq!(say(&mut r, "x").1, vec![]);
}

#[test]
fn new_arguments_make_the_old_handle_stale() {
    let (mut r, ticket) = boot();
    open(&mut r, ticket);
    r.act("switch", vec![]).unwrap();
    assert!(!r.holds(ticket));
    let newer = r.take_requests()[0].ticket;
    // The module kept the handle under "a"; the source now asks "b".
    assert_eq!(say(&mut r, "x").0, "");
    open(&mut r, newer);
    assert_eq!(say(&mut r, "x").1.len(), 2);
}

/// A socket no resource owns (a mutation's stream) takes none of its
/// mapper's replies: the frame fails, and the journal says so.
#[test]
fn a_mutation_s_stream_takes_no_mapper_replies() {
    let (mut r, _) = boot();
    r.act("watch", vec![]).unwrap();
    let out = r.take_requests();
    assert_eq!(out[0].request.url, "wss://echo.test/m");
    let open = Message {
        event: "open".into(),
        ..Message::default()
    };
    r.fulfill(out[0].ticket, Outcome::Message(open)).unwrap();
    assert!(sends(&mut r).is_empty());
    let journal = journal(&r);
    assert!(
        journal.contains(
            "mapper `feed` failed: InvalidStateError: the socket is not open; socket frames were refused"
        ),
        "{journal}"
    );
}

/// A resource's answer sends its request in its source's call and awaits
/// the reply; the code after an await runs in the turn that settled it and
/// sends as that turn's answer: `deliver`'s, which ran the mapper's queue,
/// or its own fetch's delivery. What it sent stands though it then throws.
#[test]
fn an_answer_sends_its_request_and_awaits_the_reply() {
    let (mut r, ticket, side) = boot_both();
    open(&mut r, ticket);
    open(&mut r, side);
    let waiting = ask(&mut r, "sync 1");
    assert_eq!(waiting.len(), 1, "it waits on the module's open streams");
    assert_eq!(sends(&mut r), [answer("reply", &["request 1"])]);
    settle_waiting(&mut r, &waiting[0], side, "reply 1 hi");
    assert_eq!(reply(&r), "got hi");
    assert!(sends(&mut r).is_empty());

    let waiting = ask(&mut r, "chain 2");
    assert_eq!(sends(&mut r), [answer("reply", &["request 2"])]);
    settle_waiting(&mut r, &waiting[0], side, "reply 2 there");
    assert_eq!(reply(&r), "chained there");
    assert_eq!(sends(&mut r), [answer("deliver", &["thanks 2"])]);

    let asked = ask(&mut r, "fetch 3");
    assert_eq!(asked.len(), 1);
    assert_eq!(asked[0].request.url, "https://echo.test/x");
    assert_eq!(sends(&mut r), [answer("reply", &["request 3"])]);
    r.fulfill(asked[0].ticket, ok()).unwrap();
    assert_eq!(reply(&r), "fetched");
    assert_eq!(sends(&mut r), [answer("reply", &["after fetch 3"])]);

    ask(&mut r, "throws");
    assert_eq!(sends(&mut r), [answer("reply", &["request throws"])]);
    assert!(journal(&r).contains("the answer threw"));
}

/// An answer the runner lets go has no more turns: what it sent stays sent,
/// and the continuation its rejected fetch runs as it is let go runs in no
/// answer's turn, so its send refuses.
#[test]
fn an_answer_let_go_sends_nothing_more() {
    let (mut r, ticket) = boot();
    open(&mut r, ticket);
    let asked = ask(&mut r, "abandon 1");
    assert_eq!(asked[0].request.url, "https://echo.test/slow");
    assert_eq!(sends(&mut r), [answer("reply", &["request 1"])]);
    assert!(ask(&mut r, "").is_empty());
    assert!(!r.holds(asked[0].ticket), "new arguments let it go");
    assert!(sends(&mut r).is_empty(), "its cancel never goes out");
    assert_eq!(
        frame(&mut r, ticket, "report"),
        format!("message report 1 {ONLY}")
    );
}

/// D7's bounds hold per answer, over all its turns; past them, after its own
/// close, or on a socket no resource owns, `send` throws, what went before
/// stands, and the answer is not failed.
#[test]
fn what_an_answer_may_not_send() {
    let (mut r, ticket) = boot();
    open(&mut r, ticket);
    ask(&mut r, "many 1");
    assert_eq!(
        reply(&r),
        "QuotaExceededError: socket answer exceeds 256 operations"
    );
    let batches = sends(&mut r);
    assert_eq!((batches.len(), batches[0].1.len()), (1, 256));
    let asked = ask(&mut r, "split 1");
    assert_eq!(sends(&mut r)[0].1.len(), 200);
    r.fulfill(asked[0].ticket, ok()).unwrap();
    assert_eq!(
        reply(&r),
        "QuotaExceededError: socket answer exceeds 256 operations"
    );
    assert_eq!(
        sends(&mut r)[0].1.len(),
        56,
        "its second turn sends what is left of 256"
    );
    ask(&mut r, "huge 1");
    assert_eq!(reply(&r), "QuotaExceededError: socket answer exceeds 1 MiB");
    assert!(sends(&mut r).is_empty());
    ask(&mut r, "close 1");
    assert_eq!(reply(&r), "InvalidStateError: the socket is not open");
    let close = SocketOp::Close {
        code: 4001,
        reason: "done".into(),
    };
    assert_eq!(
        sends(&mut r),
        [(SocketOrigin::Answer("reply".into()), vec![close])]
    );
    // A mutation's stream.
    r.act("listen", vec![]).unwrap();
    let quiet = r.take_requests()[0].ticket;
    open(&mut r, quiet);
    ask(&mut r, "quiet 1");
    assert_eq!(reply(&r), "InvalidStateError: the socket is not open");
    assert!(sends(&mut r).is_empty());
}

/// An answer's `bufferedAmount` is the transport's bytes as its turn began,
/// the batches of earlier turns the host has not taken (a mutation's in
/// the same commit), and what the turn has sent.
#[test]
fn an_answer_s_buffered_amount_counts_what_is_not_yet_sent() {
    let (mut r, ticket) = boot();
    open(&mut r, ticket);
    ask(&mut r, "probe 1");
    assert_eq!(reply(&r), "0:3");
    sends(&mut r);
    // `say(id, "probe 2")` stages "probe 2" and "probe 2 again", 20 bytes.
    r.act("askAndSay", vec![Value::str("probe 2")]).unwrap();
    assert_eq!(reply(&r), "20:23");
    sends(&mut r);
    use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
    let transport = std::sync::Arc::new(AtomicU64::new(900_000));
    let reads = transport.clone();
    r.set_socket_reader(std::sync::Arc::new(move |t| {
        (t == ticket).then(|| reads.load(Relaxed))
    }));
    ask(&mut r, "big 3");
    assert_eq!(reply(&r), "QuotaExceededError: socket buffer exceeds 1 MiB");
    assert!(sends(&mut r).is_empty());
    transport.store(0, Relaxed);
    ask(&mut r, "big 4");
    assert_eq!(reply(&r), "sent");
    assert_eq!(sends(&mut r)[0].1, [SocketOp::Text("x".repeat(200_000))]);
}

/// A mutation whose source answered with a promise is an answer in flight:
/// what its continuation sends goes out as that answer's, named by the
/// mutation. One whose source answered now has given its answer, so a
/// continuation it left behind refuses.
#[test]
fn a_waiting_mutation_sends_as_an_answer() {
    let (mut r, ticket) = boot();
    open(&mut r, ticket);
    r.act("say", vec![Value::str("later")]).unwrap();
    assert_eq!(sends(&mut r), [answer("said", &["after"])]);
    assert_eq!(r.slot("said"), Some(&Value::some(Value::str("sent"))));
    assert_eq!(say(&mut r, "then send"), ("now".into(), vec![]));
    assert_eq!(
        frame(&mut r, ticket, "report"),
        format!("message report 1 {ONLY}")
    );
}

/// A promise the mapper settles resumes the code that awaited it in the
/// next turn the module runs. Here no answer is asked before the waiting
/// answer's re-ask, which runs with no answer current, so the send after the
/// await refuses. A let-go answer whose reply another answer's turn settles
/// resumes in that turn and sends as that answer.
#[test]
fn where_an_answer_s_continuation_runs() {
    let (mut r, ticket, side) = boot_both();
    open(&mut r, ticket);
    open(&mut r, side);
    let waiting = ask(&mut r, "chain 5");
    assert_eq!(sends(&mut r), [answer("reply", &["request 5"])]);
    settle_waiting(&mut r, &waiting[0], ticket, "direct 5 hi");
    assert_eq!(reply(&r), format!("Error: {ONLY}"));
    assert!(sends(&mut r).is_empty());

    ask(&mut r, "chain 6");
    assert_eq!(sends(&mut r), [answer("reply", &["request 6"])]);
    assert!(ask(&mut r, "").is_empty(), "new arguments let it go");
    frame(&mut r, side, "reply 6 late");
    assert_eq!(sends(&mut r), [answer("deliver", &["thanks 6"])]);
}
