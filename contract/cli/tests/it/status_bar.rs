//! LLP 1105 D1: `status-bar-style` and `status-bar-animation` lower to props,
//! a literal outside their sets is refused, and a bound one follows state.

use exact_kernel::{Kernel, PropId};
use exact_plan::Value;
use exact_runner::{DataError, DataSource, Event, Runner};

struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

#[test]
fn the_status_bar_props_lower_and_follow_state() {
    let src = "component App\n  state compact = false\n  action flip\n    compact = not compact\n  view\n    column testId=\"route\" status-bar-style=\"light-content\"\n      button press=flip testId=\"header\" status-bar-style=(compact ? \"dark-content\" : \"auto\") status-bar-animation=\"fade\"\n        text \"Flip\"\n";
    let plan = contract::bake(contract::compile(src).unwrap(), NoData).unwrap();
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let prop = |r: &Runner<NoData>, id: &str, p: PropId| {
        let k = r.kernel();
        k.node_by_key(k.find_by_test_id(id)[0])
            .unwrap()
            .props
            .str(p)
            .map(str::to_string)
    };
    assert_eq!(
        prop(&r, "route", PropId::StatusBarStyle).as_deref(),
        Some("light-content")
    );
    assert_eq!(
        prop(&r, "header", PropId::StatusBarStyle).as_deref(),
        Some("auto")
    );
    assert_eq!(
        prop(&r, "header", PropId::StatusBarAnimation).as_deref(),
        Some("fade")
    );
    let header = {
        let k = r.kernel();
        k.node_by_key(k.find_by_test_id("header")[0]).unwrap().id
    };
    r.dispatch(header, Event::Press).unwrap();
    assert_eq!(
        prop(&r, "header", PropId::StatusBarStyle).as_deref(),
        Some("dark-content")
    );
}

#[test]
fn a_literal_outside_the_set_is_refused() {
    for (attr, value) in [
        ("status-bar-style", "dark"),
        ("status-bar-style", "light"),
        ("status-bar-animation", "slide"),
    ] {
        let src = format!("component App\n  view\n    column {attr}=\"{value}\"\n");
        let error = contract::compile(&src).unwrap_err().to_string();
        assert!(
            error.contains("lower-attr-value") || error.contains(attr),
            "{attr}={value}: {error}"
        );
    }
    for value in ["light-content", "dark-content", "auto"] {
        contract::compile(&format!(
            "component App\n  view\n    column status-bar-style=\"{value}\"\n"
        ))
        .unwrap();
    }
}
