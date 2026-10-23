//! A long poll on work that is no HTTP request, through the bridge (LLP 1016.000 D8).
use super::*;

/// LLP 1016.000 D8 through Apple's bridge: a long poll attached to work that
/// is no HTTP request (here a continuation) is refused before its work runs,
/// and the source parses the refusal.
#[derive(Clone, Default)]
struct PollsOtherWork {
    ran: Arc<Mutex<bool>>,
    parsed: Arc<Mutex<Option<String>>>,
}

impl DataSource for PollsOtherWork {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
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
            *ran.lock().unwrap() = true;
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
        Ok(Answer::Now(Value::str("refused")))
    }
}

#[test]
fn a_long_poll_on_other_work_is_refused_before_its_work_runs() {
    let data = PollsOtherWork::default();
    let (ran, parsed) = (data.ran.clone(), data.parsed.clone());
    let initial = plan(Some("polled"));
    let mut bridge = Bridge::new();
    bridge.input_write(&initial);
    bridge.prepare_plan(initial.len(), data, Hooks::none(), 390.0, 844.0);
    bridge.commit_plan();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while parsed.lock().unwrap().is_none() {
        assert!(
            std::time::Instant::now() < deadline,
            "the refusal never settled"
        );
        bridge.pump(0.0);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let parsed = parsed.lock().unwrap().clone().unwrap();
    assert!(
        parsed.contains("Refused") && parsed.contains("exactLongPoll"),
        "{parsed}"
    );
    assert!(!*ran.lock().unwrap(), "its work never ran");
}
