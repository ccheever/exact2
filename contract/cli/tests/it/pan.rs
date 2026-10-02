//! @ref LLP 1043.000 §3 D8 — pan is an ordinary typed action, not motion ownership.
use exact_plan::Value;
use exact_runner::{DataError, DataSource, Event, Runner};
struct Empty;
impl DataSource for Empty {
    fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(name.into()))
    }
}
#[test]
fn pan_commits_curried_deltas_and_refuses_nonfinite_before_state_changes() {
    let plan = contract::compile(
        r#"component App
  state x = 0
  state y = 0
  action move(factor: number, dx: number, dy: number)
    x = x + dx * factor
    y = y + dy * factor
  view
    box testId="pan" pan=move(2) left=x top=y
"#,
    )
    .unwrap();
    let mut runner = Runner::boot(
        plan,
        Empty,
        exact_kernel::Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let id = runner
        .kernel()
        .arena()
        .iter_live()
        .find_map(|slot| {
            let n = runner
                .kernel()
                .node_by_key(runner.kernel().arena().key(slot))?;
            (n.props.str(exact_kernel::PropId::TestId) == Some("pan")).then_some(n.id)
        })
        .unwrap();
    runner
        .dispatch(id, Event::pan_payload("120,-40").unwrap())
        .unwrap();
    let before = exact_runner::agent::state(&runner);
    assert!(before.contains("240"));
    assert!(before.contains("-80"));
    for value in [f64::INFINITY, f64::NAN, f64::MAX] {
        assert!(runner.dispatch(id, Event::Pan(value, 1.)).is_err());
    }
    assert_eq!(exact_runner::agent::state(&runner), before);
    for payload in ["NaN,0", "0,inf", "1,2,3", "1e39,0"] {
        assert!(Event::pan_payload(payload).is_none());
    }
}
#[test]
fn pan_requires_two_numeric_payload_parameters_including_component_actions() {
    for params in ["dx: number", "dx: string, dy: number"] {
        let src=format!("component App\n  state x = 0\n  action move({params})\n    x = 1\n  view\n    box pan=move\n");
        assert!(contract::compile(&src).is_err());
    }
}
