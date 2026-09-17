use completion_storm_data::Storm;
use exact_kernel::{Kernel, PropId};
use exact_runner::{Event, FailureKind, Outcome, RequestOut, Response, Runner, Value};
use serde_json::json;

fn boot() -> Runner<Storm> {
    let plan = contract::compile(include_str!("../../app.contract")).unwrap();
    let baked = contract::bake(plan, Storm::default()).unwrap();
    Runner::boot(
        baked,
        Storm::default(),
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

fn tap(r: &mut Runner<Storm>, id: &str) {
    let key = r.kernel().find_by_test_id(id)[0];
    let view = r.kernel().node_by_key(key).unwrap().id;
    r.dispatch(view, Event::Press).unwrap();
}

fn text(r: &Runner<Storm>, id: &str) -> String {
    let key = r.kernel().find_by_test_id(id)[0];
    r.kernel()
        .node_by_key(key)
        .unwrap()
        .props
        .str(PropId::Text)
        .unwrap()
        .into()
}

fn response(status: u16, body: serde_json::Value) -> Outcome {
    Outcome::Response(Response {
        status,
        headers: vec![],
        body: body.to_string().into_bytes(),
    })
}

fn open(r: &mut Runner<Storm>, id: u64, count: usize) -> Vec<RequestOut> {
    tap(r, "start-wave");
    let requests = r.take_requests();
    assert_eq!(requests.len(), 1);
    assert!(requests[0].request.url.contains("/api/open?"));
    r.fulfill(
        requests[0].ticket,
        response(200, json!({"id":id,"count":count})),
    )
    .unwrap();
    let requests = r.take_requests();
    assert_eq!(requests.len(), count);
    assert_eq!(r.pending().len(), count);
    requests
}

#[test]
fn bake_is_offline_and_independent_requests_have_distinct_tickets() {
    let mut r = boot();
    assert!(r.take_requests().is_empty());
    assert!(r.pending().is_empty());
    tap(&mut r, "count-128");
    let requests = open(&mut r, 1, 128);
    let mut tickets = requests.iter().map(|r| r.ticket).collect::<Vec<_>>();
    tickets.sort();
    tickets.dedup();
    assert_eq!(tickets.len(), 128);
    assert_eq!(text(&r, "pending-count"), "128 pending lanes");
    assert_eq!(text(&r, "valid-count"), "0 valid successes in this wave");
}

#[test]
fn failures_settle_as_data_and_never_count_as_valid_successes() {
    let mut r = boot();
    tap(&mut r, "count-6");
    let requests = open(&mut r, 7, 6);
    let outcomes = [
        response(200, json!({"wave":7,"lane":0,"value":"wave 7 lane 0"})),
        response(503, json!({"error":"injected"})),
        Outcome::Failed {
            kind: FailureKind::Network,
            message: "closed".into(),
        },
        response(200, json!({"wave":6,"lane":3,"value":"wrong wave"})),
        response(200, json!({"wave":7,"lane":0,"value":"wrong lane"})),
        response(200, json!({"wave":7,"lane":5,"value":"incorrect content"})),
    ];
    for (request, outcome) in requests.iter().zip(outcomes) {
        assert!(r.fulfill(request.ticket, outcome).unwrap().is_some());
    }
    assert!(r.pending().is_empty());
    assert_eq!(text(&r, "valid-count"), "1 valid successes in this wave");
    assert_eq!(
        text(&r, "failed-count"),
        "5 failed or invalid replies in this wave"
    );
    assert_eq!(r.data_ref().accepted(), 6);
}

#[test]
fn replacement_and_unmount_drop_late_replies_without_reusing_results() {
    let mut r = boot();
    tap(&mut r, "count-6");
    let first = open(&mut r, 11, 6);
    let second = open(&mut r, 12, 6);
    for (lane, request) in first.iter().enumerate() {
        assert!(r
            .fulfill(
                request.ticket,
                response(
                    200,
                    json!({"wave":11,"lane":lane,"value":format!("wave 11 lane {lane}")})
                )
            )
            .unwrap()
            .is_none());
    }
    assert_eq!(r.data_ref().accepted(), 0);
    assert_eq!(r.pending().len(), 6);
    tap(&mut r, "navigate-away");
    assert!(r.pending().is_empty());
    assert!(r.kernel().find_by_test_id("lane-0").is_empty());
    for request in &second {
        assert!(r
            .fulfill(request.ticket, response(503, json!({})))
            .unwrap()
            .is_none());
    }
    let third = open(&mut r, 13, 6);
    assert_eq!(text(&r, "valid-count"), "0 valid successes in this wave");
    r.fulfill(
        third[0].ticket,
        response(200, json!({"wave":13,"lane":0,"value":"wave 13 lane 0"})),
    )
    .unwrap();
    assert_eq!(text(&r, "valid-count"), "1 valid successes in this wave");
    assert_eq!(r.data_ref().accepted(), 1);
    tap(&mut r, "interact");
    assert_eq!(r.slot("clicks"), Some(&Value::Number(1.0)));
    assert_eq!(r.pending().len(), 5);
}

#[test]
fn admission_refusals_are_bounded_by_current_tickets_and_settle_as_failures() {
    let mut r = boot();
    tap(&mut r, "count-6");
    let requests = open(&mut r, 9, 6);
    for request in &requests {
        assert_eq!(
            request.request.http,
            exact_runner::HttpScheduling::Independent {
                max_response_bytes: 4096
            }
        );
    }
    for _ in 0..1000 {
        for request in &requests {
            r.refuse_request(request.ticket, "capacity", false);
        }
    }
    assert_eq!(r.pending().len(), 6);
    for _ in 0..6 {
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
    assert!(!r.has_request_refusals(true));
    assert!(!r.has_pending());
    assert_eq!(text(&r, "valid-count"), "0 valid successes in this wave");
    // A late or duplicate refusal adds neither a ticket nor a new completion.
    for request in &requests {
        r.refuse_request(request.ticket, "late capacity", false);
    }
    assert!(r.take_request_refusal(true).is_none());

    let requests = open(&mut r, 10, 6);
    for request in &requests {
        r.refuse_request(request.ticket, "capacity", false);
    }
    tap(&mut r, "navigate-away");
    assert!(!r.has_request_refusals(true));
    assert!(r.take_request_refusal(true).is_none());
}
