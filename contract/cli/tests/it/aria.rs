//! ARIA states the Contract carries by their web names.

use exact_kernel::{Kernel, PropId};
use exact_runner::{DataError, DataSource, Event, Runner, Value};

struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _args: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.to_string()))
    }
}

fn boot(src: &str) -> Runner<NoData> {
    Runner::boot(
        contract::compile(src).unwrap_or_else(|e| panic!("{e}")),
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

fn view_of(r: &Runner<NoData>, test_id: &str) -> u32 {
    let key = r.kernel().find_by_test_id(test_id)[0];
    r.kernel().node_by_key(key).unwrap().id
}

fn prop(r: &Runner<NoData>, test_id: &str, id: PropId) -> Option<String> {
    let node = r.kernel().node(view_of(r, test_id)).unwrap();
    node.props.str(id).map(str::to_string)
}

/// `aria-pressed` makes a button a toggle button (ARIA 1.2): a bool is
/// written as `true`/`false`, and `mixed` is the third state. The forest
/// game's twelve toggle buttons were refused (`button has no attribute
/// aria-pressed`) before.
#[test]
fn aria_pressed_is_a_toggle_buttons_tristate() {
    let mut r = boot(
        "component App\n  state on = false\n  action flip\n    on = not on\n  view\n    column\n      button press=flip aria-pressed=on testId=\"flash\"\n        text \"Flashlight\"\n      button press=flip aria-pressed=\"mixed\" testId=\"some\"\n        text \"Some\"\n      button press=flip aria-pressed=true testId=\"lit\"\n        text \"Lit\"\n      button press=flip testId=\"plain\"\n        text \"Plain\"\n",
    );
    assert_eq!(
        prop(&r, "flash", PropId::AccessibilityPressed).as_deref(),
        Some("false")
    );
    assert_eq!(
        prop(&r, "some", PropId::AccessibilityPressed).as_deref(),
        Some("mixed")
    );
    assert_eq!(
        prop(&r, "lit", PropId::AccessibilityPressed).as_deref(),
        Some("true")
    );
    assert_eq!(prop(&r, "plain", PropId::AccessibilityPressed), None);
    let flash = view_of(&r, "flash");
    r.dispatch(flash, Event::Press).unwrap();
    assert_eq!(
        prop(&r, "flash", PropId::AccessibilityPressed).as_deref(),
        Some("true")
    );
}

#[test]
fn aria_pressed_refuses_a_word_aria_does_not_have_and_a_number() {
    for (value, says) in [
        (
            "\"yes\"",
            "`aria-pressed` takes a bool or \"true\", \"false\" or \"mixed\"",
        ),
        ("3", "`aria-pressed` takes a string"),
    ] {
        let e = contract::compile(&format!(
            "component App\n  view\n    button aria-pressed={value}\n      text \"A\"\n"
        ))
        .unwrap_err()
        .to_string();
        assert!(e.contains(says), "{value}: {e}");
    }
}
