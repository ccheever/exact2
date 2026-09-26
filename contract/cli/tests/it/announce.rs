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
