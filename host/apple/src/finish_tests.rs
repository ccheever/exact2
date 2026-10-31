//! A batch says whether the app must finish work an answer started when that
//! changes, so the session keeps the app running from a request with
//! effects' send until it lands and the storage its reply's turn started is
//! persisted (LLP 1146 D2, LLP 1097 D10).
use super::*;
use exact_kernel::MonospaceMeasurer;
use exact_plan::Value;
use exact_runner::{Answer, DataError, Event, Outcome, Request, Response, Store};

const APP: &str = r##"shape Row
  n: number
component A
  state n = 0
  resource row = row(n) as shape Row
  action bump
    n = n + 1
  view
    button press=bump testId="bump"
      text `row ${row.n}`
"##;

/// `row` asks with a POST, one per argument.
#[derive(Clone, Default)]
struct Posts;
impl DataSource for Posts {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
    fn answer(&mut self, _: &mut Store, _: &str, args: &[Value]) -> Result<Answer, DataError> {
        let url = format!("https://example.test/row/{:?}", args[0]);
        Ok(Answer::Later(Request::post_json(&url, "{}")))
    }
    fn parse(
        &mut self,
        _: &mut Store,
        _: &str,
        _: &[Value],
        _: Outcome,
    ) -> Result<Answer, DataError> {
        Ok(Answer::Now(Value::record(vec![Value::Number(1.0)])))
    }
}

/// `row` asks with a POST, and its reply's turn starts one storage
/// operation, which lands when the host answers it.
#[derive(Clone, Default)]
struct PostsThenStores {
    left: u64,
    out: bool,
}
impl DataSource for PostsThenStores {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
    fn answer(&mut self, _: &mut Store, _: &str, args: &[Value]) -> Result<Answer, DataError> {
        Posts.answer(&mut Store::default(), "row", args)
    }
    fn parse(
        &mut self,
        _: &mut Store,
        _: &str,
        _: &[Value],
        _: Outcome,
    ) -> Result<Answer, DataError> {
        self.left += 1;
        Ok(Answer::Now(Value::record(vec![Value::Number(1.0)])))
    }
    fn background(&mut self, _: &Store) -> Option<Request> {
        if self.out || self.left == 0 {
            return None;
        }
        self.out = true;
        Some(Request::continuation(exact_runner::BACKGROUND))
    }
    fn background_landed(&mut self, _: &Store, _: Outcome) -> Result<Option<Request>, DataError> {
        self.out = false;
        self.left -= 1;
        Ok(None)
    }
    fn background_state(&self) -> Option<exact_runner::BackgroundState> {
        Some(exact_runner::BackgroundState {
            queued: self.left - u64::from(self.out),
            in_flight: u64::from(self.out),
            ..Default::default()
        })
    }
}

fn said(batch: &str) -> Option<bool> {
    let v: serde_json::Value = serde_json::from_str(batch).unwrap();
    v.get("mustFinish").map(|must| must.as_bool().unwrap())
}

fn ok() -> Outcome {
    Outcome::Response(Response {
        status: 200,
        headers: Vec::new(),
        body: b"{}".to_vec(),
    })
}

#[test]
fn a_batch_says_the_app_must_finish_from_a_post_s_send_until_it_lands() {
    let plan = contract::compile(APP).unwrap().encode();
    let (mut host, boot) = Host::boot(
        &plan,
        Posts,
        Box::new(MonospaceMeasurer::default()),
        390.0,
        844.0,
    )
    .unwrap();
    assert_eq!(said(&boot), Some(true), "the first POST is out: {boot}");
    let _ = host.take_requests();
    let k = host.runner().kernel();
    let bump = k.node_by_key(k.find_by_test_id("bump")[0]).unwrap().id;
    let moved = host.dispatch_at(bump, Event::Press, 0.0);
    assert_eq!(
        said(&moved),
        None,
        "the newer POST is out, so nothing changed: {moved}"
    );
    let newer = host.take_requests().pop().expect("the newer POST").ticket;
    let landed = host.fulfill_all(vec![(newer, ok(), None)], 0.0);
    assert_eq!(said(&landed), Some(false), "{landed}");
}

#[test]
fn a_batch_says_the_app_must_finish_until_the_storage_a_reply_started_lands() {
    let plan = contract::compile(APP).unwrap().encode();
    let (mut host, boot) = Host::boot(
        &plan,
        PostsThenStores::default(),
        Box::new(MonospaceMeasurer::default()),
        390.0,
        844.0,
    )
    .unwrap();
    assert_eq!(said(&boot), Some(true), "the POST is out: {boot}");
    let post = host.take_requests().pop().expect("the POST").ticket;
    let landed = host.fulfill_all(vec![(post, ok(), None)], 0.0);
    assert_eq!(
        said(&landed),
        None,
        "its reply's storage follows it, so nothing changed: {landed}"
    );
    let storage = host.take_requests().pop().expect("the storage").ticket;
    let stored = host.fulfill_all(vec![(storage, Outcome::Storage(Vec::new()), None)], 0.0);
    assert_eq!(said(&stored), Some(false), "{stored}");
}
