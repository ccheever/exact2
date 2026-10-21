//! LLP 1116.018 D2: where a refusal's message claims the same behaviour, the
//! edit it names is run, and shows what the refused form was for.

use exact_kernel::{Kernel, PropValue};
use exact_plan::Value;
use exact_runner::{DataError, DataSource, Runner};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
        unreachable!("these apps read no data")
    }
}

fn boot(source: &str) -> Runner<NoData> {
    Runner::boot(
        contract::compile(source).unwrap_or_else(|e| panic!("{e}\n{source}")),
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

fn text(r: &Runner<NoData>, id: &str) -> String {
    let node = r
        .kernel()
        .node_by_key(r.kernel().find_by_test_id(id)[0])
        .unwrap();
    match node.props.iter().find_map(|(_, v)| match v {
        PropValue::Str(s) => Some(s.to_string()),
        _ => None,
    }) {
        Some(s) => s,
        None => panic!("`{id}` has no text"),
    }
}

/// A stopwatch the builder wrote as `timer ticker interval=1000 running=on
/// tick=step` (three repairs deleted it): the `task` the message names ticks
/// once a second while `on` holds, and not after.
#[test]
fn the_timer_message_s_task_ticks_while_its_gate_holds() {
    let wrote = "component App\n  state on = false\n  state n = 0\n  action step\n    n = n + 1\n  action toggle\n    on = not on\n  timer ticker interval=1000 running=on tick=step\n  view\n    button press=toggle testId=\"go\"\n      text `${n}` testId=\"n\"\n";
    let e = contract::compile(wrote).unwrap_err();
    assert_eq!(e.id, "syntax-unknown-section", "{e}");
    let edit = "task stepTimer when on\n    every(1000, step)";
    assert!(
        e.message
            .contains("write `task stepTimer when on` with `every(1000, step)` indented under it"),
        "{e}"
    );
    let mut r = boot(&wrote.replace("timer ticker interval=1000 running=on tick=step", edit));
    // `advance` moves the clock to a time, in milliseconds since launch.
    r.advance(3000.0).unwrap();
    assert_eq!(text(&r, "n"), "0", "off: no ticks");
    r.act("toggle", vec![]).unwrap();
    r.advance(6000.0).unwrap();
    assert_eq!(text(&r, "n"), "3", "on: one a second");
    r.act("toggle", vec![]).unwrap();
    r.advance(9000.0).unwrap();
    assert_eq!(text(&r, "n"), "3", "off again: none");
}
