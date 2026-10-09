//! Actual Bridge pump regression: rejected ordered work must not overtake a
//! previously admitted effect, even though refusal never enters an I/O queue,
//! and later ordered work waits for the refusal to settle, then runs.
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
        // Whether `c`'s work had run when `b`'s refusal settled.
        let ran = self.ran_third.load(std::sync::atomic::Ordering::SeqCst);
        let mut parsed = self.parsed.lock().unwrap();
        parsed.push(if name == "b" && ran {
            "b after c ran".into()
        } else {
            name.into()
        });
        drop(parsed);
        Ok(Answer::Now(Value::Number(1.)))
    }
}

#[test]
fn rejected_ordered_b_waits_for_held_a_and_c_cannot_bypass_b() {
    // This fixture needs one admitted executor. Parallel Bridge tests share
    // the process-wide worker cap, including retired workers still exiting;
    // isolate admission so this test observes ordering, not unrelated overload.
    const CHILD: &str = "EXACT_EXECUTOR_ORDER_TEST";
    if std::env::var_os(CHILD).is_none() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "abi::executor_order_tests::rejected_ordered_b_waits_for_held_a_and_c_cannot_bypass_b"])
            .env(CHILD, "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    let plan = contract::compile(
        r#"component App
  mutation first as shape number
  mutation second as shape number
  mutation third as shape number
  action start
    send first = a()
    send second = b()
    send third = c()
  action retry
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
    // `c` was held behind `b`'s refusal, not refused with it: it ran once
    // `b` settled, never before (the Bluesky clone's poisoned lane).
    assert_eq!(*parsed.lock().unwrap(), ["a", "b", "c"]);
    assert!(ran_third.load(std::sync::atomic::Ordering::SeqCst));
    ran_third.store(false, std::sync::atomic::Ordering::SeqCst);
    // A new ordered request is admitted as before.
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
