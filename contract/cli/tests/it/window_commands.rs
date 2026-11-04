//! LLP 1069.010: a quit the app holds leaves full screen and fades its
//! window first, by two host commands.

use exact_kernel::Kernel;
use exact_plan::Value;
use exact_runner::{DataError, DataSource, Runner};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

fn boot(src: &str) -> Runner<NoData> {
    let plan = contract::compile(src).unwrap_or_else(|e| panic!("{e}"));
    Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

/// `exitFullscreen()` and `setWindowOpacity(opacity)` reach the host in
/// order; a literal opacity outside 0–1 and any argument to
/// `exitFullscreen` are refused.
#[test]
fn a_held_quit_s_window_commands() {
    let src = "component App\n  action conceal(n: number)\n    exitFullscreen()\n    setWindowOpacity(0)\n    setWindowOpacity(n)\n  view\n    button press=conceal(1)\n      text \"Quit\"\n";
    let mut r = boot(src);
    r.act("conceal", vec![Value::Number(0.5)]).unwrap();
    let commands = r.take_commands();
    let named: Vec<(&str, Vec<Value>)> = commands
        .iter()
        .map(|c| (c.name.as_str(), c.args.clone()))
        .collect();
    assert_eq!(
        named,
        [
            ("exitFullscreen", vec![]),
            ("setWindowOpacity", vec![Value::Number(0.0)]),
            ("setWindowOpacity", vec![Value::Number(0.5)]),
        ]
    );
    for (bad, id) in [
        ("exitFullscreen(1)", "type-exit-fullscreen"),
        ("setWindowOpacity(1.5)", "type-window-opacity"),
        ("setWindowOpacity(-1)", "type-window-opacity"),
        ("setWindowOpacity(\"0\")", "type-window-opacity"),
        ("setWindowOpacity()", "type-window-opacity"),
    ] {
        let error = contract::compile(&src.replace("exitFullscreen()", bad)).unwrap_err();
        assert!(format!("{error:?}").contains(id), "{bad}: {error:?}");
    }
}
