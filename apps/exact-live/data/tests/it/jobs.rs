//! App-local protocol relocation; real Runner tickets, no HTTP/server execution.
#[path = "../../src/jobs.rs"]
mod app_jobs;

use app_jobs::{JobOrigins, Jobs};
use completion_storm_data::Storm;
use exact_kernel::{Kernel, PropId};
use exact_plan::Value;
use exact_runner::{
    Answer, DataSource, Event, FailureKind, HttpScheduling, Outcome, Request, RequestOut, Response,
    Runner, Store,
};

const CONFIG: &str =
    "DATA=https://jobs-data.example.test\nCONTROL=https://jobs-control.example.test\n";
fn source() -> Jobs {
    Jobs::new(JobOrigins::parse(CONFIG).unwrap())
}
fn response(status: u16, body: impl Into<String>) -> Outcome {
    Outcome::Response(Response {
        status,
        headers: vec![],
        body: body.into().into_bytes(),
    })
}
fn good(wave: u64, lane: usize) -> Outcome {
    response(
        200,
        format!(r#"{{"wave":{wave},"lane":{lane},"value":"wave {wave} lane {lane}"}}"#),
    )
}
fn later(answer: Answer) -> Request {
    let Answer::Later(request) = answer else {
        panic!("real asynchronous request expected")
    };
    request
}
fn completion(wave: f64, lane: f64) -> Vec<Value> {
    vec![
        Value::Number(wave),
        Value::Number(lane),
        Value::Number(128.),
        Value::Bool(true),
    ]
}
fn boot() -> Runner<Jobs> {
    let plan =
        contract::compile(include_str!("../../../../completion-storm/app.contract")).unwrap();
    let baked = contract::bake(plan, source()).unwrap();
    Runner::boot(
        baked,
        source(),
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}
fn tap(r: &mut Runner<Jobs>, id: &str) {
    let key = r.kernel().find_by_test_id(id)[0];
    r.dispatch(r.kernel().node_by_key(key).unwrap().id, Event::Press)
        .unwrap();
}
fn text(r: &Runner<Jobs>, id: &str) -> String {
    let key = r.kernel().find_by_test_id(id)[0];
    r.kernel()
        .node_by_key(key)
        .unwrap()
        .props
        .str(PropId::Text)
        .unwrap()
        .into()
}
fn open(r: &mut Runner<Jobs>, wave: u64) -> Vec<RequestOut> {
    tap(r, "start-wave");
    let requests = r.take_requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0].request.url,
        "https://jobs-control.example.test/api/open?count=128&errors=0"
    );
    assert_eq!(requests[0].request.method, "POST");
    r.fulfill(
        requests[0].ticket,
        response(200, format!(r#"{{"id":{wave},"count":128}}"#)),
    )
    .unwrap();
    let requests = r.take_requests();
    assert_eq!(requests.len(), 128);
    assert_eq!(r.pending().len(), 128);
    for (lane, request) in requests.iter().enumerate() {
        assert_eq!(
            request.request.url,
            format!("https://jobs-data.example.test/api/hold?wave={wave}&lane={lane}")
        );
        assert_eq!(
            request.request.http,
            HttpScheduling::Independent {
                max_response_bytes: 4096
            }
        );
        assert_eq!(
            request.request.headers,
            [("cache-control".into(), "no-store".into())]
        );
    }
    requests
}

#[test]
fn delegated_requests_and_grants_use_the_same_configured_origins() {
    let mut jobs = source();
    assert_eq!(
        jobs.grants(),
        "net.fetch https://jobs-data.example.test\nnet.fetch https://jobs-control.example.test\nnet.websocket wss://jobs-control.example.test"
    );
    assert_ne!(
        jobs.grants(),
        Storm::default().grants(),
        "old fixture grants cannot authorize new origins"
    );
    let mut original = Storm::default();
    let cases = [
        (
            "openWave",
            vec![Value::Number(128.), Value::Number(25.)],
            "https://jobs-control.example.test/api/open?count=128&errors=25",
        ),
        (
            "releaseWave",
            vec![Value::Number(42.)],
            "https://jobs-control.example.test/api/release?wave=42",
        ),
        (
            "fixtureStats",
            vec![],
            "https://jobs-control.example.test/api/stats",
        ),
        (
            "completion",
            completion(42., 7.),
            "https://jobs-data.example.test/api/hold?wave=42&lane=7",
        ),
    ];
    for (name, args, expected_url) in cases {
        let mut expected = later(original.answer(&mut Store::default(), name, &args).unwrap());
        expected.url = expected_url.into();
        let actual = later(jobs.answer(&mut Store::default(), name, &args).unwrap());
        assert_eq!(actual, expected, "only the URL may change for {name}");
        let origin = actual.url.split("/api/").next().unwrap();
        assert!(jobs
            .grants()
            .lines()
            .any(|g| g == format!("net.fetch {origin}")));
    }
}

#[test]
fn isolated_defaults_never_share_the_old_fixture() {
    let jobs = Jobs::new(
        JobOrigins::parse("DATA=http://127.0.0.1:4339\nCONTROL=http://127.0.0.1:4340").unwrap(),
    );
    assert_eq!(
        jobs.grants(),
        "net.fetch http://127.0.0.1:4339\nnet.fetch http://127.0.0.1:4340\nnet.websocket ws://127.0.0.1:4340"
    );
    let configured =
        Jobs::new(JobOrigins::parse(include_str!("../../../job-origins.txt")).unwrap());
    assert_eq!(Jobs::default().grants(), configured.grants());
}

#[test]
fn origin_configuration_fails_closed_without_network_or_normalization() {
    for config in [
        "",
        "DATA=https://data.test",
        "DATA=https://same.test\nCONTROL=https://same.test",
        "DATA=http://127.0.0.1:4319\nCONTROL=http://127.0.0.1:4340",
        "DATA=http://127.0.0.1:4339\nCONTROL=http://127.0.0.1:4320",
        "DATA=http://public.test:4339\nCONTROL=https://control.test",
        "DATA=https://2130706433\nCONTROL=https://control.test",
        "DATA=https://127.1\nCONTROL=https://control.test",
        "DATA=https://user@data.test\nCONTROL=https://control.test",
        "DATA=https://data.test/\nCONTROL=https://control.test",
        "DATA=https://data.test?q=1\nCONTROL=https://control.test",
        "DATA=https://data.test#fragment\nCONTROL=https://control.test",
        "DATA=https://data.test:0\nCONTROL=https://control.test",
        "DATA=https://data.test:65536\nCONTROL=https://control.test",
        "DATA=https://data.test:443\nCONTROL=https://data.test",
        "DATA=https://data.test\nDATA=https://other.test\nCONTROL=https://control.test",
        "DATA=https://data.test\nCONTROL=https://control.test\nOTHER=https://extra.test",
    ] {
        assert!(JobOrigins::parse(config).is_err(), "must refuse {config:?}");
    }
    assert!(JobOrigins::parse("# fixed build inputs\nDATA=https://data.example.test:8443\nCONTROL=https://control.example.test\n").is_ok());
}

#[test]
fn exact_origin_endpoint_boundary_and_every_other_http_field_are_preserved() {
    let config = JobOrigins::parse(CONFIG).unwrap();
    let mut request = Request::post_json(
        "http://127.0.0.1:4319/api/hold?wave=7&lane=9&note=%2F%26",
        "{\"kept\":true}",
    )
    .header("x-test", "value")
    .independent_http(4096);
    request.method = "PATCH".into();
    request.grants = Some("net.fetch https://narrow.example.test".into());
    let mut expected = request.clone();
    expected.url = "https://jobs-data.example.test/api/hold?wave=7&lane=9&note=%2F%26".into();
    assert_eq!(config.request(request).unwrap(), expected);
    for url in [
        "http://127.0.0.1:43190/api/hold?wave=7",
        "http://127.0.0.1:4319.evil.test/api/hold",
        "http://127.0.0.1:4319/api/holding",
        "http://127.0.0.1:4319/api/hold/extra",
        "http://127.0.0.1:4319/api/stats",
        "http://127.0.0.1:4320/api/hold",
        "https://other.test/api/hold",
        "http://127.0.0.1:4319/api/hold#fragment",
    ] {
        assert!(
            config.request(Request::get(url)).is_err(),
            "must refuse {url}"
        );
    }
    let mut storage = Request::get("http://127.0.0.1:4319/api/hold");
    storage.storage = Some(vec![1, 2]);
    assert!(config.request(storage).is_err());
    let mut continuation = Request::get("http://127.0.0.1:4319/api/hold");
    continuation.continuation = Some(1);
    assert!(config.request(continuation).is_err());
}

#[test]
fn answering_and_parsing_retain_the_same_storm_owner() {
    let mut jobs = source();
    let args = completion(7., 0.);
    assert!(matches!(
        jobs.answer(&mut Store::default(), "completion", &args)
            .unwrap(),
        Answer::Later(_)
    ));
    assert_eq!(
        jobs.query("counters", &[]).unwrap(),
        Value::record(vec![Value::Number(1.), Value::Number(0.)])
    );
    assert!(matches!(
        jobs.parse(&mut Store::default(), "completion", &args, good(7, 0))
            .unwrap(),
        Answer::Now(_)
    ));
    assert_eq!(
        jobs.query("counters", &[]).unwrap(),
        Value::record(vec![Value::Number(1.), Value::Number(1.)])
    );
}

#[test]
fn real_runner_128_tickets_and_mixed_outcomes_settle_with_honest_validity() {
    let mut r = boot();
    assert!(r.take_requests().is_empty());
    tap(&mut r, "count-128");
    let requests = open(&mut r, 7);
    let tickets: std::collections::BTreeSet<_> = requests.iter().map(|r| r.ticket).collect();
    assert_eq!(tickets.len(), 128);
    assert_eq!(text(&r, "pending-count"), "128 pending lanes");
    for (lane, request) in requests.iter().enumerate() {
        let outcome = match lane % 4 {
            0 => good(7, lane),
            1 => response(503, "{}"),
            2 => response(200, "{broken"),
            _ => Outcome::Failed {
                kind: FailureKind::Refused,
                message: "local capacity".into(),
            },
        };
        assert!(r.fulfill(request.ticket, outcome).unwrap().is_some());
    }
    assert!(r.pending().is_empty());
    assert_eq!(text(&r, "valid-count"), "32 valid successes in this wave");
    assert_eq!(
        text(&r, "failed-count"),
        "96 failed or invalid replies in this wave"
    );
}

#[test]
fn replacement_and_unmount_reject_old_tickets_and_remount_uses_new_origins() {
    let mut r = boot();
    tap(&mut r, "count-128");
    let first = open(&mut r, 1);
    let second = open(&mut r, 2);
    for (lane, request) in first.iter().enumerate() {
        assert!(r.fulfill(request.ticket, good(1, lane)).unwrap().is_none());
    }
    assert_eq!(r.pending().len(), 128);
    tap(&mut r, "navigate-away");
    assert!(r.pending().is_empty());
    for (lane, request) in second.iter().enumerate() {
        assert!(r.fulfill(request.ticket, good(2, lane)).unwrap().is_none());
    }
    let third = open(&mut r, 3);
    assert_eq!(text(&r, "valid-count"), "0 valid successes in this wave");
    r.fulfill(third[0].ticket, good(3, 0)).unwrap();
    assert_eq!(text(&r, "valid-count"), "1 valid successes in this wave");
    tap(&mut r, "interact");
    assert_eq!(r.slot("clicks"), Some(&Value::Number(1.)));
}

#[test]
fn admission_refusals_remain_bounded_and_terminal() {
    let mut r = boot();
    tap(&mut r, "count-128");
    let requests = open(&mut r, 9);
    for _ in 0..3 {
        for request in &requests {
            r.refuse_request(request.ticket, "capacity", false);
        }
    }
    for _ in 0..128 {
        let (ticket, outcome) = r.take_request_refusal(true).unwrap();
        assert!(matches!(
            outcome,
            Outcome::Failed {
                kind: FailureKind::Refused,
                ..
            }
        ));
        r.fulfill(ticket, outcome).unwrap();
    }
    assert!(!r.has_pending());
    assert!(!r.has_request_refusals(true));
    assert_eq!(text(&r, "valid-count"), "0 valid successes in this wave");
    assert_eq!(
        text(&r, "failed-count"),
        "128 failed or invalid replies in this wave"
    );
}
