//! A stream's frames land as commits of the stream's origin (LLP 1133.002
//! D12): a frame of a stream a tool asked asks what it causes as the tool,
//! as a one-shot reply to the tool's request would.
use exact_kernel::Kernel;
use exact_plan::Value;
use exact_runner::{
    Answer, CallOrigin, DataError, DataSource, Message, Outcome, Request, Runner, Store,
};

const SRC: &str = r#"
component App
  state room = "a"
  resource feed = feed(room) as shape string
  resource worker = worker(feed) as shape string
  action join
    room = "b"
  view
    column
      text feed testId="feed"
      text worker testId="worker"
"#;

/// `feed` streams once its room is `b`; `worker` fetches once the feed
/// says anything but `idle`.
struct Src;

impl DataSource for Src {
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
        unreachable!("answers through answer")
    }

    fn grants(&self) -> &str {
        "net.fetch https://x.test"
    }

    fn answer(&mut self, _: &mut Store, source: &str, args: &[Value]) -> Result<Answer, DataError> {
        let a = args
            .first()
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        Ok(match source {
            "feed" if a == "a" => Answer::Now(Value::str("idle")),
            "feed" => Answer::stream(Request::get("https://x.test/events")),
            "worker" if a == "idle" => Answer::Now(Value::str("none")),
            "worker" => Answer::Later(Request::get("https://x.test/work")),
            other => return Err(DataError::UnknownSource(other.into())),
        })
    }

    fn parse(
        &mut self,
        _: &mut Store,
        _: &str,
        _: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        let text = match outcome {
            Outcome::Message(m) => m.data,
            other => other.summary(),
        };
        Ok(Answer::Now(Value::str(&text)))
    }
}

#[test]
fn a_frame_of_a_stream_a_tool_asked_asks_as_the_tool() {
    let plan = contract::compile(SRC).expect("the fixture compiles");
    let mut r = Runner::boot(plan, Src, Kernel::with_monospace(), Default::default(), "/").unwrap();
    r.take_requests();
    r.act_from(CallOrigin::Tool, "join", vec![]).unwrap();
    let out = r.take_requests();
    let feed = out
        .iter()
        .find(|o| o.target == "feed")
        .expect("the feed's stream");
    assert_eq!(feed.request.reach.origin, CallOrigin::Tool);
    let frame = Message {
        event: "message".into(),
        data: "go".into(),
        ..Message::default()
    };
    r.fulfill(feed.ticket, Outcome::Message(frame)).unwrap();
    let out = r.take_requests();
    let work = out
        .iter()
        .find(|o| o.target == "worker")
        .expect("the worker's fetch");
    assert_eq!(work.request.reach.origin, CallOrigin::Tool);
}
