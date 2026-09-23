//! LLP 1016 D2 on the web host: a `send` whose source answers later leaves
//! the runner as a `request` op the page runs, and `exact_fulfill`'s reply
//! commits through the same batch path as an event.

use exact_runner::{Answer, DataError, DataSource, Event, Outcome, Request, SurfaceOutcome, Value};
use exact_web::Host;

const SRC: &str = r#"
shape Session
  ok: bool
  username: string

component App
  state who = ""
  mutation session as shape Session
  derive busy = pending(session)
  action setWho(v) writes who
    who = v
  action submit writes session
    send session = login(who)
  view
    column testId="app"
      input value=who change=setWho testId="who"
      button press=submit aria-label="Log in" testId="login"
        text "Log in"
      when busy
        text "Logging in…" testId="busy"
      match session
        case some(s)
          text `Signed in as ${s.username}` testId="signed-in"
        case none
          text "Signed out" testId="signed-out"
"#;

#[derive(Default)]
struct Later;

impl DataSource for Later {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::Unavailable(source.into()))
    }
    fn answer(
        &mut self,
        _: &mut exact_runner::Store,
        _: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        Ok(Answer::Later(
            Request::post_json("https://api.castle.test/graphql", "{\"q\":1}")
                .header("x-who", args[0].as_str().unwrap_or("")),
        ))
    }
    fn parse(
        &mut self,
        _: &mut exact_runner::Store,
        _: &str,
        args: &[Value],
        outcome: exact_runner::Outcome,
    ) -> Result<exact_runner::Answer, DataError> {
        let ok = matches!(outcome, exact_runner::Outcome::Response(ref r) if r.status == 200);
        Ok(exact_runner::Answer::Now(Value::record(vec![
            Value::Bool(ok),
            Value::str(args[0].as_str().unwrap_or("")),
        ])))
    }
    fn grants(&self) -> &'static str {
        "net.fetch https://api.castle.test\n"
    }
}

fn view(host: &Host<Later>, test_id: &str) -> u32 {
    let k = host.runner().kernel();
    k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().id
}

#[test]
fn a_send_leaves_as_a_request_op_and_the_reply_commits() {
    let plan = contract::compile(SRC).unwrap();
    let baked = contract::bake(plan, Later).unwrap();
    let (mut host, first) = Host::boot(&baked.encode(), Later, Default::default(), "/").unwrap();
    assert!(
        first.contains("{\"op\":\"grants\",\"lines\":[\"net.fetch https://api.castle.test\"]}"),
        "{first}"
    );
    assert!(!first.contains("\"op\":\"request\""));

    host.dispatch(view(&host, "who"), Event::Change("ada".into()));
    let batch = host.dispatch(view(&host, "login"), Event::Press);
    assert!(batch.contains("\"op\":\"request\",\"ticket\":1,\"target\":\"session\",\"scope\":null,\"method\":\"POST\",\"url\":\"https://api.castle.test/graphql\",\"headers\":[[\"content-type\",\"application/json\"],[\"x-who\",\"ada\"]],\"body\":\"eyJxIjoxfQ==\",\"cache\":\"default\"}"), "{batch}");
    assert!(batch.contains("Logging in…"), "the view says busy: {batch}");
    assert_eq!(host.runner().pending(), vec![("session".to_string(), 1)]);

    // The reply: one batch, the busy text gone, the signed-in text in.
    let reply = host.fulfill_at(
        1,
        0,
        200,
        "content-type: application/json\nx-served-by: test\n",
        b"{}".to_vec(),
        5.0,
    );
    assert!(reply.contains("Signed in as ada"), "{reply}");
    assert!(
        reply.contains("\"op\":\"destroy\""),
        "the busy text is torn down: {reply}"
    );
    assert!(host.runner().pending().is_empty());
    // A reply for a ticket nobody holds is an empty batch, not an error.
    let late = host.fulfill_at(1, 0, 200, "", Vec::new(), 6.0);
    assert!(
        late.starts_with("{\"ops\":[]") && !late.contains("\"error\":\""),
        "{late}"
    );
    // A failure is data the source shaped.
    host.dispatch(view(&host, "login"), Event::Press);
    let failed = host.fulfill_at(2, 1, 0, "", b"TypeError: Failed to fetch".to_vec(), 7.0);
    assert!(
        failed.contains("\"op\":\"destroy\"") && failed.contains("\"error\":null"),
        "{failed}"
    );
    assert_eq!(
        host.runner().slot("session"),
        Some(&Value::some(Value::record(vec![
            Value::Bool(false),
            Value::str("ada")
        ]))),
        "the failure became the source's own value"
    );
}

#[test]
fn module_replacement_preserves_the_owner_of_an_in_flight_post() {
    let plan = contract::bake(contract::compile(SRC).unwrap(), Later)
        .unwrap()
        .encode();
    let (host, _) = Host::boot(&plan, Later, Default::default(), "/").unwrap();
    let mut bridge = exact_web::abi::Bridge::new();
    bridge.boot(&plan, Later, 390.0, 844.0, "/");
    bridge.input_write(b"ada");
    bridge.dispatch(view(&host, "who"), 1, 3, 0.0);
    bridge.dispatch(view(&host, "login"), 0, 0, 0.0);
    bridge.input_write(b"candidate");
    let len = bridge.boot_module([0, 0, 9], Later);
    assert!(String::from_utf8_lossy(bridge.output_bytes(len as usize)).contains("in-flight"));
    bridge.input_write(b"{}");
    let len = bridge.fulfill(1.0, 0, 200, 0, 2, 5.0);
    assert!(String::from_utf8_lossy(bridge.output_bytes(len as usize)).contains("Signed in as ada"));
}

#[test]
fn storage_batch_preserves_text_and_scope_and_refuses_invalid_utf8() {
    use exact_runner::RequestOut;
    for (payload, encoded) in [
        (
            b"{\"text\":\"hello\"}".to_vec(),
            r#""payload":"{\"text\":\"hello\"}""#,
        ),
        (vec![0xff], r#""payload":"""#),
    ] {
        let mut request = Request::storage(payload);
        request.grants = Some(String::new());
        let mut batch = exact_web::batch::Batch::default();
        batch.request(&RequestOut {
            ticket: 1,
            target: "backup".into(),
            request,
            forced: false,
        });
        let wire = batch.finish(None, 0., None);
        assert!(wire.contains(encoded), "{wire}");
        assert!(
            wire.contains(r#""scope":"""#),
            "an empty scope stays deny-all: {wire}"
        );
        assert!(!wire.contains('�'));
    }
}

#[test]
fn surface_batch_and_outcomes_keep_their_typed_kind() {
    use exact_runner::RequestOut;
    for (request, mode, body) in [
        (Request::capture_surface("world"), "capture", None),
        (
            Request::restore_surface("world", vec![0, 128, 255]),
            "restore",
            Some("AID/"),
        ),
    ] {
        let mut batch = exact_web::batch::Batch::default();
        batch.request(&RequestOut {
            ticket: 7,
            target: "save".into(),
            request,
            forced: false,
        });
        let wire = batch.finish(None, 0., None);
        assert!(wire.contains(&format!(
            r#""op":"surfaceWork","ticket":7,"mode":"{mode}","name":"world""#
        )));
        assert_eq!(body.is_some(), wire.contains(r#""body":"#));
        if let Some(body) = body {
            assert!(wire.contains(&format!(r#""body":"{body}""#)));
        }
    }
    assert_eq!(
        exact_web::host::outcome_from(6, 0, "", vec![1, 2]),
        Outcome::Surface(SurfaceOutcome::Captured(vec![1, 2]))
    );
    assert_eq!(
        exact_web::host::outcome_from(7, 0, "", vec![]),
        Outcome::Surface(SurfaceOutcome::Restored)
    );
    assert!(matches!(
        exact_web::host::outcome_from(6, 0, "", vec![0; exact_runner::MAX_HOST_WORK_BYTES + 1]),
        Outcome::Failed {
            kind: exact_runner::FailureKind::Refused,
            ..
        }
    ));

    let mut mixed = Request::capture_surface("world");
    mixed.storage = Some(b"{}".to_vec());
    let mut batch = exact_web::batch::Batch::default();
    batch.request(&RequestOut {
        ticket: 8,
        target: "save".into(),
        request: mixed,
        forced: false,
    });
    let wire = batch.finish(None, 0., None);
    assert!(wire.contains(r#""op":"surfaceWork""#));
    assert!(wire.contains("surface request combines multiple host-work kinds"));
    assert!(!wire.contains(r#""payload":"#));
}
