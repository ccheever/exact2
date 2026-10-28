//! LLP 1133.036 D6 through the executor: a tool's reach to the local network
//! is refused before it is sent, at the first hop, at a redirect's hop and at
//! a WebSocket open, and a person's or the app's own reach is never refused.
use super::super::Core;
use exact_runner::local_network::{self, Reach, State};
use exact_runner::{CallOrigin, FailureKind, Outcome, Request, RequestOut, Response};
use ibex2::{
    boundary::HostError,
    stdlib::{
        abort::AbortSignal,
        fetch::{Headers, StreamingResponse, Transport},
        websocket::{MessageSource, SocketTransport},
    },
};
use std::sync::{
    mpsc::{channel, Receiver},
    Arc, Mutex, MutexGuard,
};
use std::time::{Duration, Instant};

const GRANTS: &str =
    "net.fetch local-network\nnet.fetch https://public.test\nnet.websocket local-network";

/// What reached the transports: each URL opened, in order.
#[derive(Default)]
struct Seen(Mutex<Vec<String>>);

struct Fake(Arc<Seen>);
impl Transport for Fake {
    fn open(
        &self,
        request: &ibex2::stdlib::fetch::Request,
        signal: &AbortSignal,
    ) -> Result<StreamingResponse, HostError> {
        self.0 .0.lock().unwrap().push(request.url.clone());
        // `/to-lan` redirects into the local network.
        let mut headers = Headers::default();
        let redirect = request.url.ends_with("/to-lan");
        if redirect {
            headers.set_response("location", "http://192.168.1.40/json/si");
        }
        Ok(ibex2::stdlib::fetch::Response {
            status: if redirect { 302 } else { 200 },
            status_text: "OK".into(),
            headers,
            body: b"{}".to_vec(),
            url: request.url.clone(),
            redirected: false,
        }
        .into_stream(request.body_limit(), signal.clone()))
    }
}
impl SocketTransport for Fake {
    fn connect(
        &self,
        url: &url::Url,
        _: usize,
        _: &AbortSignal,
    ) -> Result<Box<dyn MessageSource>, HostError> {
        self.0 .0.lock().unwrap().push(url.to_string());
        Err(HostError::Failed("the test's socket opens nothing".into()))
    }
}

/// The host's target prompts ([`local_network::launch`]) for these tests,
/// which take turns since that is process-wide.
fn prompting(prompts: bool) -> MutexGuard<'static, ()> {
    static TURN: Mutex<()> = Mutex::new(());
    let turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    // Before any executor's start, which then leaves the fact to these tests.
    local_network::launch_once(|| false);
    local_network::launch(prompts);
    turn
}

fn setup() -> (Core, Arc<Seen>, Receiver<()>) {
    let seen = Arc::new(Seen::default());
    let host = {
        let seen = seen.clone();
        move || {
            ibex2::host::Host::with_transport(Box::new(Fake(seen.clone())))
                .with_socket_transport(Box::new(Fake(seen.clone())))
        }
    };
    let owners = (0..super::super::WORKERS)
        .map(|_| Some(host().endow(ibex2::grant::GrantSet::parse(GRANTS).unwrap())))
        .collect();
    let (wake, woke) = channel();
    let core = Core::with_owners(
        owners,
        GRANTS,
        Box::new(move || {
            let _ = wake.send(());
        }),
    )
    .streams_on(host);
    (core, seen, woke)
}

fn reach(origin: CallOrigin, state: State) -> Reach {
    Reach {
        origin,
        substitute: Some(state),
    }
}

fn one(url: &str, reach: Reach) -> (Outcome, Vec<String>) {
    let (core, seen, woke) = setup();
    let mut request = Request::get(url);
    if url.starts_with("ws") {
        let exact_runner::Answer::Later(stream) = exact_runner::Answer::stream(request) else {
            unreachable!()
        };
        request = stream;
    }
    request.reach = reach;
    let out = RequestOut {
        ticket: 1,
        target: "test".into(),
        request,
        forced: false,
    };
    core.run(out, None).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        core.begin_pump();
        if let Some((_, outcome, _)) = core.drain().into_iter().next() {
            return (outcome, seen.0.lock().unwrap().clone());
        }
        woke.recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .expect("an outcome");
    }
}

fn refused(outcome: &Outcome) -> &str {
    match outcome {
        Outcome::Failed {
            kind: FailureKind::Refused,
            message,
        } => message,
        other => panic!("not refused: {other:?}"),
    }
}

#[test]
fn a_tool_s_local_fetch_is_refused_unsent_while_undecided_or_denied() {
    let _turn = prompting(true);
    for state in [State::Prompt, State::Unknown] {
        let (outcome, seen) = one(
            "http://192.168.1.40/json/si",
            reach(CallOrigin::Tool, state),
        );
        assert_eq!(refused(&outcome), "NotAllowedError: person-required");
        assert!(seen.is_empty(), "{seen:?}");
    }
    for origin in [CallOrigin::Wake, CallOrigin::Device] {
        let (outcome, seen) = one("http://wled-a1b2c3.local/", reach(origin, State::Prompt));
        assert_eq!(refused(&outcome), "NotAllowedError: person-required");
        assert!(seen.is_empty(), "{seen:?}");
    }
    let (outcome, seen) = one(
        "http://192.168.1.40/",
        reach(CallOrigin::Tool, State::Denied),
    );
    assert_eq!(
        refused(&outcome),
        "NotAllowedError: local network access is denied"
    );
    assert!(seen.is_empty(), "{seen:?}");
}

#[test]
fn a_granted_tool_an_app_s_own_reach_and_a_public_host_are_sent() {
    let _turn = prompting(true);
    let granted = one(
        "http://192.168.1.40/",
        reach(CallOrigin::Tool, State::Granted),
    );
    assert!(matches!(
        granted.0,
        Outcome::Response(Response { status: 200, .. })
    ));
    let app = one(
        "http://192.168.1.40/",
        reach(CallOrigin::App, State::Denied),
    );
    assert!(matches!(
        app.0,
        Outcome::Response(Response { status: 200, .. })
    ));
    assert_eq!(app.1, ["http://192.168.1.40/"]);
    let public = one(
        "https://public.test/",
        reach(CallOrigin::Tool, State::Prompt),
    );
    assert!(matches!(
        public.0,
        Outcome::Response(Response { status: 200, .. })
    ));
}

#[test]
fn a_redirect_into_the_local_network_is_refused_at_its_hop() {
    let _turn = prompting(true);
    let (outcome, seen) = one(
        "https://public.test/to-lan",
        reach(CallOrigin::Tool, State::Prompt),
    );
    assert_eq!(refused(&outcome), "NotAllowedError: person-required");
    assert_eq!(seen, ["https://public.test/to-lan"]);
}

#[test]
fn a_tool_s_websocket_open_is_refused_before_it_connects() {
    let _turn = prompting(true);
    let (outcome, seen) = one(
        "ws://192.168.1.40/ws",
        reach(CallOrigin::Tool, State::Prompt),
    );
    assert_eq!(refused(&outcome), "NotAllowedError: person-required");
    assert!(seen.is_empty(), "{seen:?}");
    let (_, seen) = one(
        "ws://192.168.1.40/ws",
        reach(CallOrigin::Tool, State::Granted),
    );
    assert_eq!(seen, ["ws://192.168.1.40/ws"]);
}

#[test]
fn where_no_reach_prompts_a_tool_s_local_fetch_is_sent() {
    let _turn = prompting(false);
    let (outcome, seen) = one(
        "http://192.168.1.40/",
        reach(CallOrigin::Tool, State::Prompt),
    );
    assert!(matches!(
        outcome,
        Outcome::Response(Response { status: 200, .. })
    ));
    assert_eq!(seen, ["http://192.168.1.40/"]);
}
