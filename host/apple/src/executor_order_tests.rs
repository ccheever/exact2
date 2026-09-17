//! Actual Bridge pump regression: rejected ordered work must not overtake a
//! previously admitted effect, even though refusal never enters an I/O queue.
use super::*;
use exact_runner::{Answer, Outcome, Request, Store, Value};
use std::sync::{
    mpsc::{channel, Receiver},
    Arc, Mutex,
};
use std::time::{Duration, Instant};

struct Ordered {
    release: Option<Receiver<()>>,
    parsed: Arc<Mutex<Vec<String>>>,
    ran_third: Arc<std::sync::atomic::AtomicBool>,
}
impl DataSource for Ordered {
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, exact_runner::DataError> {
        Ok(Value::Number(0.))
    }
    fn answer(
        &mut self,
        _: &mut Store,
        name: &str,
        _: &[Value],
    ) -> Result<Answer, exact_runner::DataError> {
        Ok(Answer::Later(match name {
            "a" => Request::continuation(1),
            "b" => Request::continuation(2).independent_http(4096), // Invalid opt-in is a refusal before effects.
            _ => Request::continuation(3),
        }))
    }
    fn continuation(&mut self, token: u64) -> Option<Box<dyn FnOnce() -> Outcome + Send>> {
        if token == 1 {
            let release = self.release.take().unwrap();
            Some(Box::new(move || {
                release.recv_timeout(Duration::from_secs(5)).unwrap();
                Outcome::Storage(vec![1])
            }))
        } else {
            let third = self.ran_third.clone();
            Some(Box::new(move || {
                third.store(true, std::sync::atomic::Ordering::SeqCst);
                Outcome::Storage(vec![3])
            }))
        }
    }
    fn parse(
        &mut self,
        _: &mut Store,
        name: &str,
        _: &[Value],
        _: Outcome,
    ) -> Result<Answer, exact_runner::DataError> {
        self.parsed.lock().unwrap().push(name.into());
        Ok(Answer::Now(Value::Number(1.)))
    }
}

#[test]
fn rejected_ordered_b_waits_for_held_a_and_c_cannot_bypass_b() {
    let plan = contract::compile(
        r#"component App
  mutation first as shape number
  mutation second as shape number
  mutation third as shape number
  action start writes first, second, third
    send first = a()
    send second = b()
    send third = c()
  action retry writes third
    send third = c()
  view
    column
      button press=start testId="start"
        text "start"
      button press=retry testId="retry"
        text "retry"
"#,
    )
    .unwrap()
    .encode();
    let (release, wait) = channel();
    let parsed = Arc::new(Mutex::new(Vec::new()));
    let ran_third = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mut bridge = Bridge::new();
    bridge.boot(
        &plan,
        Ordered {
            release: Some(wait),
            parsed: parsed.clone(),
            ran_third: ran_third.clone(),
        },
        Hooks::none(),
        390.,
        844.,
    );
    let kernel = bridge.host.as_ref().unwrap().runner().kernel();
    let button = kernel
        .node_by_key(kernel.find_by_test_id("start")[0])
        .unwrap()
        .id;
    bridge.dispatch(button, 0, 0, 0.);
    bridge.pump(0.);
    let early = parsed.lock().unwrap().clone();
    release.send(()).unwrap();
    assert!(
        early.is_empty(),
        "refusal parsed before the earlier effect: {early:?}"
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    while parsed.lock().unwrap().len() < 3 && Instant::now() < deadline {
        bridge.pump(0.);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(*parsed.lock().unwrap(), ["a", "b", "c"]);
    assert!(
        !ran_third.load(std::sync::atomic::Ordering::SeqCst),
        "later work bypassed the refusal barrier"
    );
    // Once the refusal cohort has settled, a new ordered request must recover.
    let kernel = bridge.host.as_ref().unwrap().runner().kernel();
    let retry = kernel
        .node_by_key(kernel.find_by_test_id("retry")[0])
        .unwrap()
        .id;
    bridge.dispatch(retry, 0, 0, 0.);
    let deadline = Instant::now() + Duration::from_secs(5);
    while parsed.lock().unwrap().len() < 4 && Instant::now() < deadline {
        bridge.pump(0.);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(*parsed.lock().unwrap(), ["a", "b", "c", "c"]);
    assert!(ran_third.load(std::sync::atomic::Ordering::SeqCst));
}
