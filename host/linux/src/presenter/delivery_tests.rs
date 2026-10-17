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
