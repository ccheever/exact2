//! A long poll on work that is no HTTP request, through the presenter (LLP 1016.000 D8).
use super::*;
use exact_runner::{Answer, DataError, Store, Value};

/// LLP 1016.000 D8 through the Linux presenter: a long poll attached to work
/// that is no HTTP request (a continuation) is refused before its work runs,
/// and the source parses the refusal.
#[derive(Default)]
struct PollsOtherWork {
    ran: Arc<std::sync::atomic::AtomicBool>,
    parsed: Arc<std::sync::Mutex<Option<String>>>,
}
impl DataSource for PollsOtherWork {
    fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(name.into()))
    }
    fn answer(&mut self, _: &mut Store, _: &str, _: &[Value]) -> Result<Answer, DataError> {
        Ok(Answer::Later(
            exact_runner::Request::continuation(1).long_poll("since", "seq"),
        ))
    }
    fn continuation(
        &mut self,
        _: u64,
    ) -> Option<Box<dyn FnOnce() -> exact_runner::Outcome + Send>> {
        let ran = self.ran.clone();
        Some(Box::new(move || {
            ran.store(true, std::sync::atomic::Ordering::SeqCst);
            exact_runner::Outcome::Storage(vec![])
        }))
    }
    fn parse(
        &mut self,
        _: &mut Store,
        _: &str,
        _: &[Value],
        outcome: exact_runner::Outcome,
    ) -> Result<Answer, DataError> {
        *self.parsed.lock().unwrap() = Some(format!("{outcome:?}"));
        Ok(Answer::Now(Value::Number(0.)))
    }
}

#[test]
fn a_long_poll_on_other_work_is_refused_before_its_work_runs() {
    let source = PollsOtherWork::default();
    let (ran, parsed) = (source.ran.clone(), source.parsed.clone());
    let (mut p, error) = Presenter::boot_with(
        &contract::compile(
            "component App\n  resource polled = polled() as shape number\n  view\n    text \"x\"\n",
        )
        .unwrap()
        .encode(),
        source,
        (200., 200.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while parsed.lock().unwrap().is_none() {
        assert!(
            std::time::Instant::now() < deadline,
            "the refusal never settled"
        );
        p.pump(p.host().now());
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let parsed = parsed.lock().unwrap().clone().unwrap();
    assert!(
        parsed.contains("Refused") && parsed.contains("exactLongPoll"),
        "{parsed}"
    );
    assert!(
        !ran.load(std::sync::atomic::Ordering::SeqCst),
        "its work never ran"
    );
}
