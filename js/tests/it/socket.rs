//! A resource's two-way socket in a TypeScript module, through the runner
//! (LLP 1016.000 D7): the mapper's handle, a mutation's staged frames, and
//! what refuses them.

#![cfg(exact_js_engine)]

use exact_js::Module;
use exact_kernel::Kernel;
use exact_plan::Value;
use exact_runner::{Message, Outcome, Runner, SocketOp};

const HBC: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/socket.hbc"));

const SRC: &str = r#"
component App
  state id = "a"
  resource feed = feed(id) as shape string
  mutation said as shape string
  action say(what: string)
    send said = say(id, what)
  action switch
    id = "b"
  view
    text feed testId="feed"
"#;

fn boot() -> (Runner<Module>, u64) {
    let mut m = Module::new(
        HBC.to_vec(),
        "test.socket",
        "net.websocket wss://echo.test\n",
    );
    m.load().expect("the fixture loads");
    let plan = contract::compile(SRC).expect("the fixture compiles");
    let mut r = Runner::boot(plan, m, Kernel::with_monospace(), Default::default(), "/").unwrap();
    let out = r.take_requests();
    assert_eq!(out.len(), 1);
    assert!(out[0].request.stream && out[0].request.socket);
    assert_eq!(out[0].request.url, "wss://echo.test/a");
    (r, out[0].ticket)
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

fn open(r: &mut Runner<Module>, ticket: u64) -> String {
    let open = Message {
        event: "open".into(),
        ..Message::default()
    };
    deliver(r, ticket, open)
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

#[test]
fn the_mapper_gets_the_handle_and_a_mutation_sends_text_and_bytes_in_order() {
    let (mut r, ticket) = boot();
    assert_eq!(
        open(&mut r, ticket),
        "open  1 (socket.send is only available in a mutation's synchronous source)"
    );
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
    assert_eq!(feed(&r), "close 4001 finished true");
    assert_eq!(
        say(&mut r, "late"),
        ("InvalidStateError: the socket is not open".into(), vec![])
    );
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
    for what in ["throws", "async", "later"] {
        assert_eq!(say(&mut r, what).1, vec![], "{what}");
    }
    let journal = journal(&r);
    assert!(journal.contains("the source threw"), "{journal}");
    assert!(
        journal.contains("a socket mutation must answer synchronously"),
        "{journal}"
    );
    assert!(
        journal.contains("socket.send is only available in a mutation's synchronous source"),
        "{journal}"
    );
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
