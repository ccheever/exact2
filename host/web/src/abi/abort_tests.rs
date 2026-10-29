//! A fetch a realm aborted between batches (a worker's let-go): the page's
//! wake asks the host to collect it, and the batch cancels its request and
//! resumes its answer with the abort.
use super::Bridge;
use exact_runner::{Answer, DataError, DataSource, Outcome, Request, Store, Value};
use serde_json::Value as Json;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Default)]
struct Held {
    aborted: Arc<AtomicBool>,
    answered_aborted: Arc<AtomicBool>,
}
impl DataSource for Held {
    fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(name.into()))
    }
    fn answer(&mut self, _: &mut Store, _: &str, _: &[Value]) -> Result<Answer, DataError> {
        let mut request = Request::post_json("https://example.test/held", "{}");
        request.fetch = Some(1);
        Ok(Answer::Later(request))
    }
    fn parse(
        &mut self,
        _: &mut Store,
        _: &str,
        _: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        let aborted = matches!(
            outcome,
            Outcome::Failed {
                kind: exact_runner::FailureKind::Aborted,
                ..
            }
        );
        self.answered_aborted.store(aborted, Ordering::SeqCst);
        Ok(Answer::Now(Value::Bool(aborted)))
    }
    /// The realm's door-23 ticket for the held fetch, once.
    fn take_aborted(&mut self) -> Vec<u64> {
        match self.aborted.swap(false, Ordering::SeqCst) {
            true => vec![1],
            false => Vec::new(),
        }
    }
}

const APP: &str = r#"component App
  mutation m as shape bool
  action hold
    send m = hold()
  view
    column
      button press=hold testId="hold"
        text "Hold"
"#;

fn output(bridge: &Bridge<Held>, len: u32) -> Json {
    serde_json::from_slice(bridge.output_bytes(len as usize)).unwrap()
}

#[test]
fn an_abort_heard_between_batches_is_collected_cancelled_and_answered() {
    crate::link::link_for_tests();
    let plan = contract::compile(APP).unwrap().encode();
    let source = Held::default();
    let (aborted, answered) = (source.aborted.clone(), source.answered_aborted.clone());
    let mut bridge = Bridge::new();
    let len = bridge.boot(&plan, source, 400., 400., "/");
    assert_eq!(output(&bridge, len)["error"], Json::Null);
    let host = bridge.host.as_mut().unwrap();
    let k = host.runner().kernel();
    let hold = k.node_by_key(k.find_by_test_id("hold")[0]).unwrap().id;
    let batch: Json =
        serde_json::from_str(&host.dispatch(hold, exact_runner::Event::Press)).unwrap();
    let ticket = batch["ops"]
        .as_array()
        .unwrap()
        .iter()
        .find(|op| op["op"] == "request")
        .map(|op| op["ticket"].as_u64().unwrap())
        .expect("the POST goes out");
    // Nothing collects it until the page's wake asks.
    aborted.store(true, Ordering::SeqCst);
    let len = bridge.collect_aborts();
    let batch = output(&bridge, len);
    assert_eq!(batch["error"], Json::Null, "{batch}");
    assert!(
        batch["ops"]
            .as_array()
            .unwrap()
            .iter()
            .any(|op| op["op"] == "abort" && op["ticket"] == ticket),
        "the page cancels it: {batch}"
    );
    assert!(
        answered.load(Ordering::SeqCst),
        "its answer resumed aborted"
    );
}
