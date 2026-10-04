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

/// `aria-modal` (LLP 1080.003) lowers to `accessibilityModal` and follows
/// its state; Signal Clone's call screen and menus had no way to say it.
#[test]
fn aria_modal_lowers_and_follows_state() {
    let mut r = boot(
        "component App\n  state open = true\n  action close\n    open = false\n  view\n    column\n      column role=\"dialog\" aria-modal=open testId=\"sheet\"\n        button press=close testId=\"close\"\n          text \"Close\"\n      column testId=\"plain\"\n",
    );
    let modal = |r: &Runner<NoData>, id: &str| {
        let node = r.kernel().node(view_of(r, id)).unwrap();
        node.props.bool(PropId::AccessibilityModal)
    };
    assert_eq!(modal(&r, "sheet"), Some(true));
    assert_eq!(modal(&r, "plain"), None);
    let close = view_of(&r, "close");
    r.dispatch(close, Event::Press).unwrap();
    assert_eq!(modal(&r, "sheet"), Some(false));
}

/// A form's states (onboarding F22) and a menu button's (spreadsheet F20),
/// by their ARIA names: `aria-invalid` and `aria-haspopup` take their words
/// or a bool, `aria-required` a bool, `aria-describedby` ids.
#[test]
fn form_states_and_haspopup_lower_by_their_aria_names() {
    let mut r = boot(
        "component App\n  state bad = false\n  action check\n    bad = true\n  view\n    column\n      input aria-invalid=bad aria-required=true aria-describedby=\"email-error\" testId=\"email\"\n      text \"Enter an email\" id=\"email-error\"\n      button press=check aria-haspopup=\"menu\" aria-expanded=false testId=\"menu\"\n        text \"File\"\n      button press=check aria-haspopup=bad aria-invalid=\"spelling\" testId=\"other\"\n        text \"Other\"\n",
    );
    let required = r
        .kernel()
        .node(view_of(&r, "email"))
        .unwrap()
        .props
        .bool(PropId::AccessibilityRequired);
    assert_eq!(required, Some(true));
    assert_eq!(
        prop(&r, "email", PropId::AccessibilityDescribedBy).as_deref(),
        Some("email-error")
    );
    assert_eq!(
        prop(&r, "email", PropId::AccessibilityInvalid).as_deref(),
        Some("false")
    );
    assert_eq!(
        prop(&r, "menu", PropId::AccessibilityHasPopup).as_deref(),
        Some("menu")
    );
    assert_eq!(
        prop(&r, "other", PropId::AccessibilityHasPopup).as_deref(),
        Some("false")
    );
    assert_eq!(
        prop(&r, "other", PropId::AccessibilityInvalid).as_deref(),
        Some("spelling")
    );
    let menu = view_of(&r, "menu");
    r.dispatch(menu, Event::Press).unwrap();
    assert_eq!(
        prop(&r, "email", PropId::AccessibilityInvalid).as_deref(),
        Some("true")
    );
    for (attr, value, says) in [
        (
            "aria-invalid",
            "\"wrong\"",
            "`aria-invalid` takes a bool or \"true\", \"false\", \"grammar\" or \"spelling\"",
        ),
        (
            "aria-haspopup",
            "\"popup\"",
            "`aria-haspopup` takes a bool or \"true\", \"false\", \"menu\", \"listbox\", \"tree\", \"grid\" or \"dialog\"",
        ),
        ("aria-required", "\"yes\"", "`aria-required` takes a bool"),
    ] {
        let e = contract::compile(&format!(
            "component App\n  view\n    input {attr}={value}\n"
        ))
        .unwrap_err()
        .to_string();
        assert!(e.contains(says), "{attr}={value}: {e}");
    }
}

/// HTML's `tabindex` (LLP 1088 D7.3) and ARIA's `aria-labelledby` (ledger2
/// Rough 3) on any element and on a module tag's box: `tabindex` is the
/// kernel's `tabIndex`, absent when unwritten, and may follow state; its
/// DOM-property spelling is refused naming the attribute.
#[test]
fn tabindex_and_labelledby_bind_by_their_html_names() {
    let mut r = boot(
        "component App\n  state open = false\n  action reveal\n    open = true\n  view\n    column\n      text \"Currency\" id=\"currency-heading\"\n      row role=\"radiogroup\" aria-labelledby=\"currency-heading\" testId=\"group\"\n        box tabindex=0 testId=\"stop\" width=10 height=10\n        button press=reveal tabindex=(open ? 0 : -1) testId=\"delete\"\n          text \"Delete\"\n        box testId=\"plain\" width=10 height=10\n      map-view tabindex=-1 testId=\"module\" width=10 height=10\n",
    );
    let index = |r: &Runner<NoData>, id: &str| {
        r.kernel()
            .node(view_of(r, id))
            .unwrap()
            .props
            .get(PropId::TabIndex)
            .and_then(|v| v.as_int())
    };
    assert_eq!(index(&r, "stop"), Some(0));
    assert_eq!(index(&r, "delete"), Some(-1));
    assert_eq!(index(&r, "plain"), None, "absent is not 0");
    assert_eq!(index(&r, "module"), Some(-1), "a module tag's box takes it");
    assert_eq!(
        prop(&r, "group", PropId::AccessibilityLabelledBy).as_deref(),
        Some("currency-heading")
    );
    let delete = view_of(&r, "delete");
    r.dispatch(delete, Event::Press).unwrap();
    assert_eq!(
        index(&r, "delete"),
        Some(0),
        "a bound tabindex follows state"
    );
    let e = contract::compile("component App\n  view\n    box tabIndex=0 width=1 height=1\n")
        .unwrap_err()
        .to_string();
    assert!(e.contains("`tabIndex` is spelled `tabindex` here"), "{e}");
}
