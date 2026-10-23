//! A long poll through the executor (LLP 1016.000 D8): a scripted
//! transport answers each request from the test's queue and records the
//! URL it was asked with, so every cursor, heartbeat, retry and end is the
//! test's to make.
use super::super::super::{Core, WORKERS};
use exact_runner::{FailureKind, Outcome, Request, RequestOut};
use ibex2::{
    boundary::HostError,
    stdlib::{
        abort::AbortSignal,
        fetch::{Headers, Response, StreamingResponse, Transport},
    },
};
use std::{
    collections::VecDeque,
    sync::{
        mpsc::{channel, Receiver},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

/// One answer: a reply, a network failure, or a reply held until aborted.
enum Reply {
    Is(u16, Vec<(&'static str, &'static str)>, &'static str),
    Down,
    Hold,
}

#[derive(Default)]
struct Script {
    replies: Mutex<VecDeque<Reply>>,
    asked: Mutex<Vec<String>>,
    aborted: Mutex<usize>,
}
struct Scripted(Arc<Script>);
impl Transport for Scripted {
    fn open(
        &self,
        request: &ibex2::stdlib::fetch::Request,
        signal: &AbortSignal,
    ) -> Result<StreamingResponse, HostError> {
        assert_eq!(request.method, "GET");
        assert_eq!(request.headers.get("accept"), Some("application/json"));
        self.0.asked.lock().unwrap().push(request.url.clone());
        let reply = self
            .0
            .replies
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(Reply::Hold);
        let (status, set, body) = match reply {
            Reply::Is(status, headers, body) => (status, headers, body),
            Reply::Down => return Err(HostError::Failed("connection refused".into())),
            Reply::Hold => {
                let end = Instant::now() + Duration::from_secs(10);
                while !signal.aborted() && Instant::now() < end {
                    std::thread::sleep(Duration::from_millis(2));
                }
                *self.0.aborted.lock().unwrap() += 1;
                return Err(HostError::Failed("aborted".into()));
            }
        };
        let mut headers = Headers::default();
        for (k, v) in set {
            headers.set_response(k, v);
        }
        Ok(Response {
            status,
            status_text: String::new(),
            headers,
            body: body.as_bytes().to_vec(),
            url: request.url.clone(),
            redirected: false,
        }
        .into_stream(request.body_limit(), signal.clone()))
    }
}

fn setup(replies: Vec<Reply>) -> (Core, Arc<Script>, Receiver<()>) {
    let script = Arc::new(Script {
        replies: Mutex::new(replies.into()),
        ..Script::default()
    });
    let grants = "net.fetch https://example.test";
    let owners = (0..WORKERS)
        .map(|_| {
            Some(
                ibex2::host::Host::with_transport(Box::new(Scripted(script.clone())))
                    .endow(ibex2::grant::GrantSet::parse(grants).unwrap()),
            )
        })
        .collect();
    let (wake, woke) = channel();
    let scripted = script.clone();
    let core = Core::with_owners(
        owners,
        grants,
        Box::new(move || {
            let _ = wake.send(());
        }),
    )
    .streams_on(move || ibex2::host::Host::with_transport(Box::new(Scripted(scripted.clone()))));
    (core, script, woke)
}

fn poll(ticket: u64, same_cursor_event: bool, ceiling: u32) -> RequestOut {
    let exact_runner::Answer::Later(mut request) = exact_runner::Answer::stream(
        Request::get("https://example.test/changes?since=0&wait=25")
            .independent_http(ceiling)
            .long_poll("since", "seq"),
    ) else {
        unreachable!()
    };
    if let Some(p) = request.long_poll.as_mut() {
        p.same_cursor_event = same_cursor_event;
    }
    RequestOut {
        ticket,
        target: "live".into(),
        request,
        forced: false,
    }
}

fn next(core: &Core, woke: &Receiver<()>) -> (u64, Outcome) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        core.begin_pump();
        if let Some(done) = core.drain().pop() {
            return (done.0, done.1);
        }
        woke.recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .expect("an outcome within five seconds");
    }
}

fn id(outcome: &Outcome) -> (&str, &str) {
    match outcome {
        Outcome::Message(m) => (m.id.as_str(), m.data.as_str()),
        other => panic!("expected a message, got {other:?}"),
    }
}

/// The messages up to the one with cursor `last`, each with how many it
/// replaced (a reply the runner had not taken yet coalesces, as any stream's).
fn until_cursor(core: &Core, woke: &Receiver<()>, last: &str) -> Vec<(String, u32)> {
    let mut got = Vec::new();
    loop {
        let (ticket, outcome) = next(core, woke);
        assert_eq!(ticket, 1);
        let Outcome::Message(m) = outcome else {
            panic!("expected a message, got {outcome:?}")
        };
        got.push((m.id.clone(), m.coalesced));
        if m.id == last {
            return got;
        }
    }
}

fn asked(script: &Script) -> Vec<String> {
    let asked = script.asked.lock().unwrap();
    asked
        .iter()
        .map(|u| {
            u.trim_start_matches("https://example.test/changes?")
                .to_string()
        })
        .collect()
}

/// Wait until the transport has been asked `n` times.
fn until_asked(script: &Script, n: usize) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while script.asked.lock().unwrap().len() < n {
        assert!(Instant::now() < deadline, "asked {:?}", asked(script));
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn settled(core: &Core) -> bool {
    let state = core.shared.state.lock().unwrap();
    state.counts == [0, 0] && state.bytes == [0, 0] && state.running.is_empty()
}

fn until_settled(core: &Core) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !settled(core) {
        assert!(
            Instant::now() < deadline,
            "the long poll kept its reservation"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

const JSON: (&str, &str) = ("content-type", "application/json");

#[test]
fn each_reply_is_a_message_its_cursor_asked_with_next_and_heartbeats_are_asked_again() {
    let (core, script, woke) = setup(vec![
        Reply::Is(200, vec![JSON], r#"{"seq":1,"changes":["a"]}"#),
        Reply::Is(204, vec![], ""),
        Reply::Is(200, vec![JSON], "  "),
        Reply::Is(200, vec![JSON], r#"{"seq":1,"changes":[]}"#),
        Reply::Is(304, vec![], ""),
        Reply::Is(200, vec![JSON], r#"{"seq":2,"changes":["b"]}"#),
        Reply::Is(200, vec![JSON], r#"{"seq":"x3"}"#),
    ]);
    core.run_owned(poll(1, false, 4096), None).unwrap();
    let got = until_cursor(&core, &woke, "x3");
    let events: u32 = got.iter().map(|(_, c)| 1 + c).sum();
    assert_eq!(events, 3, "the heartbeats delivered nothing: {got:?}");
    until_asked(&script, 8);
    assert_eq!(
        asked(&script),
        [
            "since=0&wait=25",
            "since=1&wait=25",
            "since=1&wait=25",
            "since=1&wait=25",
            "since=1&wait=25",
            "since=1&wait=25",
            "since=2&wait=25",
            "since=x3&wait=25",
        ],
        "one request at a time, each with the cursor the last event gave"
    );
    // Dropping the stream mid-request aborts the request held open.
    core.forget(|t| t != 1);
    let deadline = Instant::now() + Duration::from_secs(5);
    while *script.aborted.lock().unwrap() == 0 {
        assert!(
            Instant::now() < deadline,
            "the held request was not aborted"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
    until_settled(&core);
    core.begin_pump();
    assert!(
        core.drain().is_empty(),
        "nothing after the forget is delivered"
    );
}

#[test]
fn under_same_cursor_event_a_reply_with_the_cursor_asked_with_is_delivered() {
    let (core, script, woke) = setup(vec![
        Reply::Is(200, vec![JSON], r#"{"seq":1,"n":1}"#),
        Reply::Is(200, vec![JSON], r#"{"seq":1,"n":2}"#),
    ]);
    core.run_owned(poll(1, true, 4096), None).unwrap();
    until_asked(&script, 3);
    let (_, newest) = next(&core, &woke);
    let Outcome::Message(m) = &newest else {
        panic!("{newest:?}")
    };
    assert_eq!(
        (m.id.as_str(), m.data.as_str()),
        ("1", r#"{"seq":1,"n":2}"#)
    );
    let earlier = std::iter::from_fn(|| {
        core.begin_pump();
        core.drain().pop()
    });
    assert_eq!(
        1 + m.coalesced as usize + earlier.count(),
        2,
        "both replies were events"
    );
    core.forget(|_| false);
    until_settled(&core);
}

#[test]
fn a_transient_failure_waits_its_retry_after_shown_as_retrying_and_a_drop_ends_the_wait() {
    let (core, script, woke) = setup(vec![
        Reply::Is(503, vec![("retry-after", "0")], ""),
        Reply::Is(200, vec![JSON], r#"{"seq":5}"#),
        Reply::Is(429, vec![("retry-after", "30")], ""),
    ]);
    core.run_owned(poll(1, false, 4096), None).unwrap();
    assert_eq!(
        id(&next(&core, &woke).1).0,
        "5",
        "asked again after the 503"
    );
    until_asked(&script, 3);
    let reader = core.retry_reader();
    let deadline = Instant::now() + Duration::from_secs(5);
    let retry = loop {
        if let Some(r) = reader(1) {
            break r;
        }
        assert!(Instant::now() < deadline, "never shown retrying");
        std::thread::sleep(Duration::from_millis(2));
    };
    assert_eq!(
        (retry.attempt, retry.error.as_str()),
        (1, "HTTP 429"),
        "the 2xx reset the count"
    );
    assert!(
        retry.in_ms > 25_000 && retry.in_ms <= 30_000,
        "{}",
        retry.in_ms
    );
    let dropped = Instant::now();
    core.forget(|_| false);
    until_settled(&core);
    assert!(
        dropped.elapsed() < Duration::from_secs(2),
        "the wait ended with the drop"
    );
    assert!(reader(1).is_none(), "nothing left retrying");
    assert_eq!(script.asked.lock().unwrap().len(), 3, "not asked again");
}

#[test]
fn a_network_failure_backs_off_from_a_second_shown_with_its_error() {
    let (core, script, _woke) = setup(vec![Reply::Down]);
    core.run_owned(poll(1, false, 4096), None).unwrap();
    until_asked(&script, 1);
    let reader = core.retry_reader();
    let deadline = Instant::now() + Duration::from_secs(5);
    let retry = loop {
        if let Some(r) = reader(1) {
            break r;
        }
        assert!(Instant::now() < deadline, "never shown retrying");
        std::thread::sleep(Duration::from_millis(2));
    };
    assert_eq!(retry.attempt, 1);
    assert!(retry.in_ms <= 1200, "{}", retry.in_ms);
    assert!(
        retry.error.contains("connection refused"),
        "{}",
        retry.error
    );
    core.forget(|_| false);
    until_settled(&core);
}

#[test]
fn what_is_not_a_poll_reply_ends_the_stream() {
    let cases: Vec<(Reply, &str)> = vec![
        (Reply::Is(404, vec![], "gone"), "response 404"),
        (Reply::Is(200, vec![], "not json"), "response 200"),
        (Reply::Is(200, vec![JSON], r#"{"other":1}"#), "response 200"),
        (
            Reply::Is(
                200,
                vec![("content-type", "text/event-stream")],
                "data: 1\n\n",
            ),
            "refused event stream",
        ),
        (
            Reply::Is(
                200,
                vec![JSON],
                r#"{"seq":1,"pad":"................................................................"}"#,
            ),
            "refused limit",
        ),
    ];
    for (reply, want) in cases {
        let (core, _script, woke) = setup(vec![reply]);
        core.run_owned(poll(1, false, 48), None).unwrap();
        let (_, end) = next(&core, &woke);
        let ok = match (want, &end) {
            ("response 404", Outcome::Response(r)) => r.status == 404 && r.body == b"gone",
            ("response 200", Outcome::Response(r)) => r.status == 200,
            (
                "refused event stream",
                Outcome::Failed {
                    kind: FailureKind::Refused,
                    message,
                },
            ) => message.contains("event stream"),
            (
                "refused limit",
                Outcome::Failed {
                    kind: FailureKind::Refused,
                    message,
                },
            ) => message.contains("exceeds"),
            _ => false,
        };
        assert!(ok, "{want}: {end:?}");
        until_settled(&core);
    }
}

#[test]
fn a_long_poll_that_is_not_a_bodiless_http_get_is_refused() {
    for request in [
        Request::post_json("https://example.test/changes", "{}"),
        Request::get("wss://example.test/changes"),
    ] {
        let (core, script, woke) = setup(vec![]);
        let exact_runner::Answer::Later(request) =
            exact_runner::Answer::stream(request.independent_http(4096).long_poll("since", "seq"))
        else {
            unreachable!()
        };
        core.run_owned(
            RequestOut {
                ticket: 1,
                target: "live".into(),
                request,
                forced: false,
            },
            None,
        )
        .unwrap();
        let (_, end) = next(&core, &woke);
        assert!(
            matches!(&end, Outcome::Failed { kind: FailureKind::Refused, message } if message.starts_with("exactLongPoll")),
            "{end:?}"
        );
        assert!(script.asked.lock().unwrap().is_empty());
    }
}
