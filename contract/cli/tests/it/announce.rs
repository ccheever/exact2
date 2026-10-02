//! @ref LLP 1016.002 — the device says a topic changed; the answers that
//! watch it are asked again, and nothing else is.
use exact_kernel::Kernel;
use exact_runner::{Answer, DataError, DataSource, Native, Runner, Store, Value};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

struct Device {
    level: Arc<AtomicUsize>,
    asks: usize,
    native: Native,
}

impl DataSource for Device {
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::Unavailable("answers only with the store".into()))
    }
    fn answer(
        &mut self,
        store: &mut Store,
        source: &str,
        _: &[Value],
    ) -> Result<Answer, DataError> {
        self.asks += 1;
        let text = match source {
            "meter" => {
                store.observe_topic("meter");
                format!("level {}", self.level.load(Ordering::SeqCst))
            }
            _ => "still".into(),
        };
        Ok(Answer::Now(Value::record(vec![Value::str(&text)])))
    }
    fn native(&self) -> Option<Native> {
        Some(self.native.clone())
    }
}

const APP: &str = "shape Line\n  text: string\ncomponent App\n  resource meter = meter() as shape Line\n  resource other = other() as shape Line\n  view\n    column\n      text meter.text testId=\"meter\"\n      text other.text testId=\"other\"\n";

fn text(r: &mut Runner<Device>, id: &str) -> String {
    let key = r.kernel().find_by_test_id(id)[0];
    let node = r.kernel().node_by_key(key).unwrap();
    node.props
        .str(exact_kernel::PropId::Text)
        .unwrap_or("")
        .to_string()
}

#[test]
fn an_announced_topic_asks_again_exactly_the_answers_that_watch_it() {
    let level = Arc::new(AtomicUsize::new(1));
    let native = Native::default();
    let device = || Device {
        level: level.clone(),
        asks: 0,
        native: native.clone(),
    };
    let plan = contract::compile(APP).unwrap();
    let mut r = Runner::boot(
        plan,
        device(),
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert_eq!(text(&mut r, "meter"), "level 1");
    let woken = Arc::new(AtomicUsize::new(0));
    let wake = woken.clone();
    r.listen(Arc::new(move || {
        wake.fetch_add(1, Ordering::SeqCst);
    }));
    let asks = r.data().asks;
    // From the device's own thread, twice before the host applies: one re-ask.
    level.store(7, Ordering::SeqCst);
    let announcer = native.clone();
    std::thread::spawn(move || {
        announcer.changed("meter");
        announcer.changed("meter");
    })
    .join()
    .unwrap();
    assert!(woken.load(Ordering::SeqCst) >= 1, "the host was woken");
    assert!(r.has_announced());
    let (receipts, error) = r.apply_announced();
    assert!(error.is_none());
    assert_eq!(receipts.len(), 1);
    assert_eq!(text(&mut r, "meter"), "level 7");
    assert_eq!(text(&mut r, "other"), "still");
    assert_eq!(r.data().asks, asks + 1, "only the watching answer is asked");
    // A topic nothing watches commits nothing.
    native.changed("weather");
    assert!(r.apply_announced().0.is_empty());
}

/// A worker turn that watches a topic and then yields another request (a
/// long native call) still watches it: the topic was lost with the yield.
#[test]
fn a_topic_watched_before_a_yield_is_kept() {
    use exact_runner::{Outcome, Request, Response};
    struct Yielding {
        asks: usize,
        native: Native,
    }
    impl DataSource for Yielding {
        fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
            Err(DataError::Unavailable("answers only with the store".into()))
        }
        fn answer(
            &mut self,
            _: &mut Store,
            source: &str,
            _: &[Value],
        ) -> Result<Answer, DataError> {
            if source == "waiting" {
                return Ok(Answer::Now(Value::record(vec![Value::str("…")])));
            }
            self.asks += 1;
            Ok(Answer::Later(Request::get("https://first.test/")))
        }
        fn parse(
            &mut self,
            store: &mut Store,
            _: &str,
            _: &[Value],
            outcome: Outcome,
        ) -> Result<Answer, DataError> {
            let Outcome::Response(r) = outcome else {
                unreachable!()
            };
            if r.body == b"first" {
                store.observe_topic("meter");
                return Ok(Answer::Later(Request::get("https://second.test/")));
            }
            Ok(Answer::Now(Value::record(vec![Value::str("landed")])))
        }
        fn native(&self) -> Option<Native> {
            Some(self.native.clone())
        }
    }
    const ONE: &str = "shape Line\n  text: string\ncomponent App\n  resource meter = meter() as shape Line else waiting()\n  view\n    text meter.text\n";
    let native = Native::default();
    let source = Yielding {
        asks: 0,
        native: native.clone(),
    };
    let mut r = Runner::boot(
        contract::compile(ONE).unwrap(),
        source,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    r.listen(Arc::new(|| {}));
    let reply = |body: &[u8]| {
        Outcome::Response(Response {
            status: 200,
            headers: vec![],
            body: body.to_vec(),
        })
    };
    let first = r.take_requests()[0].ticket;
    r.fulfill(first, reply(b"first")).unwrap();
    let second = r.take_requests()[0].ticket;
    r.fulfill(second, reply(b"second")).unwrap();
    let asks = r.data().asks;
    native.changed("meter");
    let (receipts, error) = r.apply_announced();
    assert!(error.is_none());
    assert_eq!(receipts.len(), 1);
    assert_eq!(r.data().asks, asks + 1, "the announcement asked again");
}

#[test]
fn a_refused_answer_restores_the_standing_answers_topics() {
    use exact_runner::{Outcome, Request, Response};
    struct Watching {
        later: bool,
        asks: usize,
        native: Native,
    }
    impl DataSource for Watching {
        fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
            unreachable!()
        }
        fn answer(
            &mut self,
            store: &mut Store,
            _: &str,
            args: &[Value],
        ) -> Result<Answer, DataError> {
            self.asks += 1;
            if args[0].as_bool() == Some(true) {
                if self.later {
                    return Ok(Answer::Later(Request::get("https://meter.test/")));
                }
                store.observe_topic("B");
                return Ok(Answer::Now(Value::Bool(false)));
            }
            store.observe_topic("A");
            Ok(Answer::Now(Value::record(vec![Value::str("standing")])))
        }
        fn parse(
            &mut self,
            store: &mut Store,
            _: &str,
            _: &[Value],
            _: Outcome,
        ) -> Result<Answer, DataError> {
            store.observe_topic("B");
            Ok(Answer::Now(Value::Bool(false)))
        }
        fn native(&self) -> Option<Native> {
            Some(self.native.clone())
        }
    }
    let src = "shape Line\n  text: string\ncomponent App\n  state changed = false\n  resource meter = meter(changed) as shape Line\n  action change\n    changed = true\n  view\n    text meter.text\n";
    for later in [false, true] {
        let native = Native::default();
        let mut r = Runner::boot(
            contract::compile(src).unwrap(),
            Watching {
                later,
                asks: 0,
                native: native.clone(),
            },
            Kernel::with_monospace(),
            Default::default(),
            "/",
        )
        .unwrap();
        r.listen(Arc::new(|| {}));
        if later {
            r.act("change", vec![]).unwrap();
            let ticket = r.take_requests()[0].ticket;
            r.fulfill(
                ticket,
                Outcome::Response(Response {
                    status: 200,
                    headers: vec![],
                    body: vec![],
                }),
            )
            .unwrap();
            // The failed parse's topic must not join the pending answer's topics.
            native.changed("B");
            assert!(r.apply_announced().0.is_empty());
        } else {
            assert!(r.act("change", vec![]).is_err());
            assert_eq!(r.slot("changed"), Some(&Value::Bool(false)));
            let asks = r.data().asks;
            native.changed("A");
            let (receipts, error) = r.apply_announced();
            assert!(error.is_none());
            assert_eq!(receipts.len(), 1);
            assert_eq!(r.data().asks, asks + 1);
            native.changed("B");
            assert!(r.apply_announced().0.is_empty());
        }
    }
}
