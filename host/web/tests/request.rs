//! LLP 1016 D2 on the web host: a `send` whose source answers later leaves
//! the runner as a `request` op the page runs, and `exact_fulfill`'s reply
//! commits through the same batch path as an event.

use exact_runner::{Answer, DataError, DataSource, Event, Request, Value};
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
      button press=submit label="Log in" testId="login"
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
    fn answer(&mut self, _: &str, args: &[Value]) -> Result<Answer, DataError> {
        Ok(Answer::Later(
            Request::post_json("https://api.castle.test/graphql", "{\"q\":1}")
                .header("x-who", args[0].as_str().unwrap_or("")),
        ))
    }
    fn parse(
        &mut self,
        _: &str,
        args: &[Value],
        outcome: exact_runner::Outcome,
    ) -> Result<Value, DataError> {
        let ok = matches!(outcome, exact_runner::Outcome::Response(ref r) if r.status == 200);
        Ok(Value::record(vec![
            Value::Bool(ok),
            Value::str(args[0].as_str().unwrap_or("")),
        ]))
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
    let (mut host, first) = Host::boot(&baked.encode(), Later).unwrap();
    assert!(
        first.contains("{\"op\":\"grants\",\"lines\":[\"net.fetch https://api.castle.test\"]}"),
        "{first}"
    );
    assert!(!first.contains("\"op\":\"request\""));

    host.dispatch(view(&host, "who"), Event::Change("ada".into()));
    let batch = host.dispatch(view(&host, "login"), Event::Press);
    assert!(batch.contains("\"op\":\"request\",\"ticket\":1,\"target\":\"session\",\"method\":\"POST\",\"url\":\"https://api.castle.test/graphql\",\"headers\":[[\"content-type\",\"application/json\"],[\"x-who\",\"ada\"]],\"body\":\"eyJxIjoxfQ==\",\"cache\":\"default\"}"), "{batch}");
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
