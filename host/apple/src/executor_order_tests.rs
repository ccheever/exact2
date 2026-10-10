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

/// `m0`'s work holds the ordered lane; every other continuation is quick.
struct Burst {
    release: Option<Receiver<()>>,
    parsed: Arc<Mutex<Vec<String>>>,
}
impl DataSource for Burst {
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, exact_runner::DataError> {
        Ok(Value::Number(0.))
    }
    fn answer(
        &mut self,
        _: &mut Store,
        name: &str,
        _: &[Value],
    ) -> Result<Answer, exact_runner::DataError> {
        let n: u64 = name[1..].parse().unwrap();
        Ok(Answer::Later(Request::continuation(n + 1)))
    }
    fn continuation(&mut self, token: u64) -> Option<Box<dyn FnOnce() -> Outcome + Send>> {
        let release = (token == 1).then(|| self.release.take().unwrap());
        Some(Box::new(move || {
            if let Some(release) = release {
                release.recv_timeout(Duration::from_secs(5)).unwrap();
            }
            Outcome::Storage(vec![])
        }))
    }
    fn parse(
        &mut self,
        _: &mut Store,
        name: &str,
        _: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, exact_runner::DataError> {
        let line = match outcome {
            Outcome::Failed { message, .. } => format!("{name} refused: {message}"),
            _ => name.to_string(),
        };
        self.parsed.lock().unwrap().push(line);
        Ok(Answer::Now(Value::Number(1.)))
    }
}

/// 146 ordered sends at once through the Bridge, sixteen in flight: the
/// seventeenth is refused at the limit, the next 128 are held and run in
/// their turn, and the 129th held is refused in its place, after them. Every
/// settlement is in issue order (the Bluesky clone's boot; Astra and Grok,
/// round 1: a refusal past the holding cap settled before the held work).
#[test]
fn a_burst_past_the_limit_and_the_holding_cap_settles_in_issue_order() {
    const CHILD: &str = "EXACT_EXECUTOR_BURST_TEST";
    if std::env::var_os(CHILD).is_none() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "abi::executor_order_tests::a_burst_past_the_limit_and_the_holding_cap_settles_in_issue_order"])
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
    const N: usize = 146;
    let mut src = String::from("component App\n");
    for n in 0..N {
        src.push_str(&format!("  mutation s{n} as shape number\n"));
    }
    src.push_str("  action start\n");
    for n in 0..N {
        src.push_str(&format!("    send s{n} = m{n}()\n"));
    }
    src.push_str("  view\n    button press=start testId=\"start\"\n      text \"start\"\n");
    let plan = contract::compile(&src).unwrap().encode();
    let (release, wait) = channel();
    let parsed = Arc::new(Mutex::new(Vec::new()));
    let mut bridge = Bridge::new();
    bridge.boot(
        &plan,
        Burst {
            release: Some(wait),
            parsed: parsed.clone(),
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
    release.send(()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while parsed.lock().unwrap().len() < N && Instant::now() < deadline {
        bridge.pump(0.);
        std::thread::sleep(Duration::from_millis(1));
    }
    let parsed = parsed.lock().unwrap().clone();
    let names: Vec<String> = parsed
        .iter()
        .map(|line| line.split(' ').next().unwrap().to_string())
        .collect();
    assert_eq!(
        names,
        (0..N).map(|n| format!("m{n}")).collect::<Vec<_>>(),
        "{parsed:?}"
    );
    let limit = "refused: native executor admission limit reached";
    assert_eq!(parsed[16], format!("m16 {limit}"));
    assert_eq!(
        parsed[17], "m17",
        "held work runs after the refusal settles"
    );
    assert_eq!(
        parsed[N - 1],
        format!(
            "m{} refused: earlier ordered admission refusal must settle first",
            N - 1
        )
    );
    // Held continuations go sixteen at a time: one past the limit at each
    // lift is refused alone, and the rest run.
    let refused = parsed
        .iter()
        .filter(|line| line.contains(" refused: "))
        .count();
    assert!(refused < 12, "{parsed:?}");
}
