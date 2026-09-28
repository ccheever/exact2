//! LLP 1069.001 slice 1: `input type="checkbox"` (and `switch`) is the
//! kernel's `Control`, controlled like HTML, its `input`/`change` carrying a
//! bool; a text field's `input` is per keystroke and `change` is commit.

use exact_kernel::{Kernel, NodeType, PropId};
use exact_runner::{ControlValue, DataError, DataSource, Event, Runner, RunnerError, Value};

struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _args: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.to_string()))
    }
}

const SETTINGS: &str = r#"component App
  state muted = false
  state airplane = false
  state draft = ""
  state sent = ""
  state moves = 0
  action setMuted(on: bool) writes muted, moves
    muted = on
    moves = moves + 1
  action refuse(on: bool) writes moves
    moves = moves + 1
  action write(value: string) writes draft
    draft = value
  action commit(value: string) writes sent
    sent = value
  view
    column
      input type="checkbox" switch checked=muted input=setMuted testId="muted" aria-label="Hide Alerts"
      input type="checkbox" checked=airplane change=refuse testId="airplane" aria-label="Airplane Mode" accent-color="light-dark(#34c759, #30d158)"
      input value=draft input=write change=commit testId="draft"
      text sent testId="sent"
"#;

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

fn checked(r: &Runner<NoData>, test_id: &str) -> (Option<bool>, Option<bool>) {
    let node = r.kernel().node(view_of(r, test_id)).unwrap();
    (
        node.props.bool(PropId::Checked),
        node.props.bool(PropId::AccessibilityChecked),
    )
}

#[test]
fn a_checkbox_is_a_control_with_a_role_and_chromes_margins() {
    let r = boot(SETTINGS);
    let muted = r.kernel().node(view_of(&r, "muted")).unwrap();
    assert_eq!(muted.node_type, NodeType::Control);
    assert_eq!(muted.props.str(PropId::Type), Some("checkbox"));
    assert_eq!(muted.props.str(PropId::AccessibilityRole), Some("switch"));
    let airplane = r.kernel().node(view_of(&r, "airplane")).unwrap();
    assert_eq!(
        airplane.props.str(PropId::AccessibilityRole),
        Some("checkbox")
    );
    assert_eq!(
        airplane.style.margin_left,
        exact_kernel::Dimension::Points(4.0)
    );
    assert_eq!(
        airplane.style.margin_top,
        exact_kernel::Dimension::Points(3.0)
    );
    let draft = r.kernel().node(view_of(&r, "draft")).unwrap();
    assert_eq!(draft.node_type, NodeType::TextInput);
    assert_eq!(checked(&r, "muted"), (Some(false), Some(false)));
}

#[test]
fn a_toggle_carries_a_bool_and_the_committed_state_is_authoritative() {
    let mut r = boot(SETTINGS);
    let muted = view_of(&r, "muted");
    r.dispatch(muted, Event::Input(true.into())).unwrap();
    assert_eq!(checked(&r, "muted"), (Some(true), Some(true)));
    // An action that writes nothing leaves `checked` where it was: the host
    // snaps its control back to it.
    let airplane = view_of(&r, "airplane");
    r.dispatch(airplane, Event::Change(true.into())).unwrap();
    assert_eq!(checked(&r, "airplane"), (Some(false), Some(false)));
    // A checkbox's payload is a bool, a text field's text: never coerced.
    let refused = r.dispatch(muted, Event::Input("true".into())).unwrap_err();
    assert!(
        matches!(refused, RunnerError::InvalidEvent { event: "input" }),
        "{refused:?}"
    );
    let draft = view_of(&r, "draft");
    let refused = r
        .dispatch(draft, Event::Change(ControlValue::Checked(true)))
        .unwrap_err();
    assert!(
        matches!(refused, RunnerError::InvalidEvent { event: "change" }),
        "{refused:?}"
    );
}

#[test]
fn a_text_fields_input_is_per_keystroke_and_change_is_commit() {
    let mut r = boot(SETTINGS);
    let draft = view_of(&r, "draft");
    r.dispatch(draft, Event::Input("hel".into())).unwrap();
    r.dispatch(draft, Event::Input("hello".into())).unwrap();
    let sent = r.kernel().node(view_of(&r, "sent")).unwrap();
    assert_eq!(sent.props.str(PropId::Text), Some(""));
    r.dispatch(draft, Event::Change("hello".into())).unwrap();
    let sent = r.kernel().node(view_of(&r, "sent")).unwrap();
    assert_eq!(sent.props.str(PropId::Text), Some("hello"));
}

#[test]
fn the_compiler_names_what_a_control_takes() {
    let refused = |src: &str| contract::compile(src).unwrap_err().to_string();
    let app = |line: &str| {
        format!(
            "component App\n  state on = false\n  state kind = \"checkbox\"\n  action set(v: bool) writes on\n    on = v\n  action text(v: string) writes kind\n    kind = v\n  view\n    column\n      {line}\n"
        )
    };
    // A checkbox's `change` supplies a bool, not the text a field's does.
    let e = refused(&app("input type=\"checkbox\" checked=on change=text"));
    assert!(
        e.contains("type-handler-payload") && e.contains("bool"),
        "{e}"
    );
    // `type` is a literal: the node type is chosen when the view compiles.
    let e = refused(&app("input type=kind checked=on change=set"));
    assert!(e.contains("lower-input-type"), "{e}");
    let e = refused(&app("input value=kind checked=on"));
    assert!(e.contains("lower-attr-tag") && e.contains("checked"), "{e}");
    let e = refused(&app("input switch value=kind input=text"));
    assert!(e.contains("lower-attr-tag") && e.contains("switch"), "{e}");
    // The React names say which of HTML's two they are.
    let e = refused(&app("input value=kind onChangeText=text"));
    assert!(e.contains("`input`"), "{e}");
    assert!(contract::compile(&app(
        "input type=\"checkbox\" role=\"switch\" checked=on change=set appearance=\"none\""
    ))
    .is_ok());
}
