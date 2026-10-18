//! A dev generation with requests in flight (LLP 1007 §6): refused while
//! they are out, asked again, and after `LET_GO_AFTER` swapped anyway.
use super::*;
use exact_runner::{Answer, DataError, Store, Value};

/// Every answer waits on a continuation the test never pumps: a request
/// always in flight.
#[derive(Default)]
struct Held;
impl DataSource for Held {
    fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(name.into()))
    }
    fn answer(&mut self, _: &mut Store, _: &str, _: &[Value]) -> Result<Answer, DataError> {
        Ok(Answer::Later(exact_runner::Request::continuation(1)))
    }
    fn dispatch(&mut self, _: u64, _: &Store) -> exact_runner::Dispatch {
        exact_runner::Dispatch::Again
    }
    fn replacement(&self, _: &[u8], _: &str, _: Vec<u8>) -> Result<Self, DataError> {
        Ok(Held)
    }
}

const APP: &str = "component App\n  state typed = \"\"\n  resource w = wait(1) as shape number\n  action type(v)\n    typed = v\n  view\n    column width=200 height=200\n      input value=typed change=type testId=\"field\"\n";

fn module() -> Option<crate::delivery::Module> {
    Some(crate::delivery::Module {
        receipt: String::new(),
        bytes: Vec::new(),
    })
}

#[test]
fn an_update_refuses_and_a_dev_generation_lets_requests_go_after_its_window() {
    let plan = crate::test_plan(APP);
    let (mut p, error) = Presenter::boot_with(
        &plan,
        Held,
        (200., 200.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p.painted = true;
    let k = p.host.runner().kernel();
    let field = k.node_by_key(k.find_by_test_id("field")[0]).unwrap().id;
    let now = p.host.now();
    p.host
        .dispatch_at(field, exact_runner::Event::Change("ada".into()), now);
    assert!(p.host.runner().has_pending());
    assert!(matches!(
        p.reload_module(&plan, Held, module(), false),
        Err(HostError::RequestsPending)
    ));

    // The dev loop's generation: refused and held while the window runs,
    // then swapped, carrying what was typed.
    p.set_development(Some("http://127.0.0.1:9/".into()), None);
    let generation = || crate::fetch::Generation {
        identity: "gen".into(),
        plan: plan.clone(),
        module: module(),
    };
    p.pending_dev = Some(generation());
    p.poll_development(|| Held);
    assert!(p.pending_dev.is_some(), "held while the window runs");
    let since = p.dev.as_ref().unwrap().held_since.expect("the wait began");
    p.dev.as_mut().unwrap().held_since = Some(since - crate::host::LET_GO_AFTER);
    p.poll_development(|| Held);
    assert!(p.pending_dev.is_none());
    assert_eq!(
        p.dev.as_ref().unwrap().identity.as_deref(),
        Some("gen"),
        "installed"
    );
    assert!(p.dev.as_ref().unwrap().held_since.is_none());
    assert_eq!(p.host.runner().slot("typed"), Some(&Value::str("ada")));
}

/// LLP 1122 D4 on the Android reader: a host that turns only when woken is
/// told when the development poll wants its next turn, and is woken through
/// the executor's fd when the poll's answer lands.
#[test]
fn a_woken_host_is_told_the_development_polls_turn_and_woken_by_its_answer() {
    let plan = crate::test_plan(APP);
    let (mut p, _) = Presenter::boot_with(
        &plan,
        Held,
        (200., 200.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert_eq!(p.turn_due_ms(p.host.now()), None, "no timer, no dev loop");
    // A port nothing listens on: the poll's answer is a refused connection.
    p.set_development(Some("http://127.0.0.1:9/".into()), None);
    assert_eq!(
        p.turn_due_ms(p.host.now()),
        None,
        "it polls after the first pixel"
    );
    assert!(
        p.dev.as_ref().unwrap().waker.lock().unwrap().is_some(),
        "the executor's waker"
    );
    // Each answer wakes the host (a counting waker stands in for the fd's).
    let woken = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let count = woken.clone();
    *p.dev.as_mut().unwrap().waker.lock().unwrap() = Some(std::sync::Arc::new(move || {
        count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }));
    p.painted = true;
    let due = p.turn_due_ms(p.host.now()).expect("the poll is due");
    assert!(due <= 500.0, "{due}");
    // A tick alone (a quick frame's) starts the due poll: it is due no
    // longer, so a host asking again does not spin on a zero deadline.
    p.tick_development();
    let after = p.turn_due_ms(p.host.now());
    assert!(after.is_none_or(|d| d > 0.0), "{after:?}");
    let start = std::time::Instant::now();
    while woken.load(std::sync::atomic::Ordering::SeqCst) == 0 && start.elapsed().as_secs() < 10 {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(
        woken.load(std::sync::atomic::Ordering::SeqCst) > 0,
        "the poll's answer wakes the host"
    );
}
