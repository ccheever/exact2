//! LLP 1103: a driver fault fails a fetch by its URL's prefix, where a host
//! would hand the request to its transport, as a refused connection does.

use exact_kernel::Kernel;
use exact_runner::{
    agent, Answer, DataError, DataSource, Dispatch, FailureKind, Outcome, Request, Response,
    Runner, Store, Value, Work,
};

/// A source that fetches its URL and lands the body, or `error: <message>`.
struct Recipes;
impl DataSource for Recipes {
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::Unavailable("answers with a request".into()))
    }
    fn answer(&mut self, _: &mut Store, _: &str, args: &[Value]) -> Result<Answer, DataError> {
        let url = args[0].as_str().unwrap_or_default().to_string();
        Ok(Answer::Later(Request::get(&url)))
    }
    fn parse(
        &mut self,
        _: &mut Store,
        _: &str,
        _: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        let text = match outcome {
            Outcome::Response(r) => String::from_utf8_lossy(&r.body).into_owned(),
            Outcome::Failed { kind, message } => format!("error: {kind:?} {message}"),
            other => format!("other: {other:?}"),
        };
        Ok(Answer::Now(Value::record(vec![Value::str(&text)])))
    }
}

const APP: &str = "shape Line\n  text: string\ncomponent App\n  resource a = recipes(\"https://api.test/recipes/1\") as shape Line\n  resource b = recipes(\"https://api.test/users/1\") as shape Line\n  view\n    column\n      text a.text testId=\"a\"\n      text b.text testId=\"b\"\n";

fn text(r: &Runner<Recipes>, id: &str) -> String {
    let key = r.kernel().find_by_test_id(id)[0];
    let node = r.kernel().node_by_key(key).unwrap();
    node.props
        .str(exact_kernel::PropId::Text)
        .unwrap_or("")
        .to_string()
}

/// Every request out, as a host runs it: a fault's work, else a 200 whose
/// body is the URL.
fn run(r: &mut Runner<Recipes>) {
    for out in r.take_requests() {
        let outcome = match r.fault_dispatch(&out) {
            Some(Dispatch::Run(Work::Now(work))) => work(),
            Some(_) => unreachable!("a fault is work now"),
            None => Outcome::Response(Response {
                status: 200,
                headers: vec![],
                body: out.request.url.clone().into_bytes(),
            }),
        };
        r.fulfill(out.ticket, outcome).unwrap();
    }
}

#[test]
fn a_fault_fails_the_fetches_its_prefix_matches_as_a_network_failure() {
    let mut r = Runner::boot(
        contract::compile(APP).unwrap(),
        Recipes,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let reply = agent::handle(
        &r,
        r#"{"op":"faults","fail":"https://api.test/recipes","times":1}"#,
    );
    assert!(reply.contains("\"left\":1"), "{reply}");
    run(&mut r);
    assert!(
        text(&r, "a")
            .starts_with("error: Network fetch failed (driver fault): https://api.test/recipes/1"),
        "{}",
        text(&r, "a")
    );
    assert_eq!(
        text(&r, "b"),
        "https://api.test/users/1",
        "another prefix goes out"
    );
    let logs = agent::handle(&r, r#"{"op":"logs"}"#);
    assert!(
        logs.contains("fetch failed (driver fault): https://api.test/recipes/1"),
        "{logs}"
    );
    let state = agent::handle(&r, r#"{"op":"state"}"#);
    assert!(
        state.contains("\"faults\":[{\"prefix\":\"https://api.test/recipes\",\"times\":1,\"left\":0,\"hits\":1,\"armed\":true}]"),
        "{state}"
    );
    // Spent: a refresh goes out.
    let spent = agent::handle(&r, r#"{"op":"faults","pass":"https://nothing.test/"}"#);
    assert!(spent.contains("no fault was armed"), "{spent}");
    let refused = agent::handle(&r, r#"{"op":"faults","fail":"https://x.test/","times":0}"#);
    assert!(refused.contains("positive integer"), "{refused}");
}

#[test]
fn a_stream_is_not_matched() {
    let mut request = Request::get("https://api.test/recipes/live");
    request.stream = true;
    let r = Runner::boot(
        contract::compile(APP).unwrap(),
        Recipes,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    r.faults().arm("https://api.test/", None).unwrap();
    let mut r = r;
    let out = exact_runner::RequestOut {
        ticket: 99,
        target: "a".into(),
        request,
        forced: false,
    };
    assert!(
        r.fault_dispatch(&out).is_none(),
        "streams are deferred (D5)"
    );
    let plain = exact_runner::RequestOut {
        request: Request::get("https://api.test/recipes/2"),
        ..out
    };
    match r.fault_dispatch(&plain) {
        Some(Dispatch::Run(Work::Now(work))) => assert!(matches!(
            work(),
            Outcome::Failed {
                kind: FailureKind::Network,
                ..
            }
        )),
        _ => panic!("a plain fetch is failed"),
    }
}
