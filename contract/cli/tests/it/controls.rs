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
  action setMuted(on: bool)
    muted = on
    moves = moves + 1
  action refuse(on: bool)
    moves = moves + 1
  action write(value: string)
    draft = value
  action commit(value: string)
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
            "component App\n  state on = false\n  state kind = \"checkbox\"\n  action set(v: bool)\n    on = v\n  action text(v: string)\n    kind = v\n  view\n    column\n      {line}\n"
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

#[test]
fn a_text_fields_type_is_a_choice_between_text_fields_types() {
    // A password's show and hide: the node is a text field either way, its
    // `type` a bound prop, and its `change` carries text to an action whose
    // parameter is written without a type.
    let field = |kind: &str| {
        format!(
            "component App\n  state shown = false\n  state secret = \"\"\n  action toggle\n    shown = not shown\n  action keep(value)\n    secret = value\n  view\n    column\n      input value=secret change=keep type={kind} testId=\"secret\"\n      button press=toggle testId=\"toggle\"\n        text \"Show\"\n"
        )
    };
    let mut r = boot(&field("shown ? \"text\" : \"password\""));
    let kind = |r: &Runner<NoData>| {
        let node = r.kernel().node(view_of(r, "secret")).unwrap();
        assert_eq!(node.node_type, NodeType::TextInput);
        node.props.str(PropId::Type).map(str::to_string)
    };
    assert_eq!(kind(&r).as_deref(), Some("password"));
    let toggle = view_of(&r, "toggle");
    r.dispatch(toggle, Event::Press).unwrap();
    assert_eq!(kind(&r).as_deref(), Some("text"));
    let secret = view_of(&r, "secret");
    r.dispatch(secret, Event::Change("hunter2".into())).unwrap();
    let node = r.kernel().node(view_of(&r, "secret")).unwrap();
    assert_eq!(node.props.str(PropId::Value), Some("hunter2"));
    // A choice that could name a control, or a value the compiler cannot
    // list, is still refused: the kind of node is chosen when the view
    // compiles. An untyped parameter's refusal is at the `type`, which is
    // why nothing types it; a typed one's is lowering's.
    for kind in ["shown ? \"checkbox\" : \"text\"", "secret"] {
        let e = contract::compile(&field(kind)).unwrap_err().to_string();
        assert!(
            e.contains("type-cannot-infer") && e.contains("this `input`'s `type`"),
            "{kind}: {e}"
        );
        let typed = field(kind).replace("keep(value)", "keep(value: string)");
        let e = contract::compile(&typed).unwrap_err().to_string();
        assert!(e.contains("lower-input-type"), "{kind}: {e}");
    }
}

/// Themes from the data seam: `id`, `name`, in the shape's order.
struct Themes;

impl DataSource for Themes {
    fn query(&mut self, source: &str, _args: &[Value]) -> Result<Value, DataError> {
        match source {
            "themes" => Ok(Value::list(
                [("light", "Light"), ("dark", "Dark")]
                    .into_iter()
                    .map(|(id, name)| Value::record(vec![Value::str(id), Value::str(name)]))
                    .collect(),
            )),
            _ => Err(DataError::UnknownSource(source.to_string())),
        }
    }
}

const THEME: &str = r#"shape Theme
  id: string
  name: string

component App
  resource themes = themes() as shape list<Theme>
  state theme = "system"
  state moves = 0
  action setTheme(value: string)
    theme = value
    moves = moves + 1
  action refuse(value: string)
    moves = moves + 1
  view
    column
      select value=theme change=setTheme testId="theme" aria-label="Theme"
        option value="system"
          text "System"
        each t in themes key=t.id
          option t.name value=t.id testId=`theme-${t.id}`
        option "Sepia" value="sepia" disabled=true
      select value=theme input=refuse testId="fixed" aria-label="Fixed"
        option "System" value="system"
        option "Dark" value="dark"
      text `${moves}` testId="moves"
"#;

fn themed() -> Runner<Themes> {
    Runner::boot(
        contract::compile(THEME).unwrap_or_else(|e| panic!("{e}")),
        Themes,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

fn view_in<D: DataSource>(r: &Runner<D>, test_id: &str) -> u32 {
    let key = r.kernel().find_by_test_id(test_id)[0];
    r.kernel().node_by_key(key).unwrap().id
}

#[test]
fn a_select_is_a_control_whose_options_are_its_children() {
    let r = themed();
    let theme = view_in(&r, "theme");
    let node = r.kernel().node(theme).unwrap();
    assert_eq!(node.node_type, NodeType::Control);
    assert_eq!(node.props.str(PropId::Type), Some("select"));
    assert_eq!(node.props.str(PropId::AccessibilityRole), Some("combobox"));
    // `each` and keys work: the options are nodes, never laid out.
    let choices = r.kernel().select_choices(theme);
    let values: Vec<_> = choices.iter().map(|c| c.value.as_str()).collect();
    assert_eq!(values, ["system", "light", "dark", "sepia"]);
    let labels: Vec<_> = choices.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(labels, ["System", "Light", "Dark", "Sepia"]);
    assert!(choices[3].disabled && !choices[1].disabled);
    let dark = r.kernel().node(view_in(&r, "theme-dark")).unwrap();
    assert_eq!(dark.style.display, exact_kernel::Display::None);
    assert_eq!(dark.props.str(PropId::SemanticTag), Some("option"));
    assert_eq!(
        r.kernel().select_chosen(theme).map(|c| c.label),
        Some("System".into())
    );
}

#[test]
fn a_select_carries_the_chosen_value_and_refuses_what_no_option_has() {
    let mut r = themed();
    let theme = view_in(&r, "theme");
    r.dispatch(theme, Event::Change("dark".into())).unwrap();
    assert_eq!(
        r.kernel().node(theme).unwrap().props.str(PropId::Value),
        Some("dark")
    );
    for (value, why) in [("bogus", "no option"), ("sepia", "disabled")] {
        let refused = r.dispatch(theme, Event::Change(value.into())).unwrap_err();
        match refused {
            RunnerError::InvalidValue { event, reason } => {
                assert_eq!(event, "change");
                assert!(reason.contains(why), "{reason}");
            }
            other => panic!("{other:?}"),
        }
    }
    let refused = r.dispatch(theme, Event::Change(true.into())).unwrap_err();
    assert!(matches!(
        refused,
        RunnerError::InvalidEvent { event: "change" }
    ));
    // An action that writes nothing leaves `value` where it was: the host
    // shows the committed option again (D4).
    let fixed = view_in(&r, "fixed");
    r.dispatch(fixed, Event::Input("system".into())).unwrap();
    assert_eq!(
        r.kernel().node(fixed).unwrap().props.str(PropId::Value),
        Some("dark")
    );
}

#[test]
fn option_and_select_keep_htmls_content_model() {
    let refused = |view: &str| {
        let src = format!("component App\n  state v = \"a\"\n  view\n    column\n{view}");
        contract::compile(&src).unwrap_err().to_string()
    };
    let e = refused("      option \"A\" value=\"a\"\n");
    assert!(e.contains("lower-option-parent"), "{e}");
    let e = refused("      select value=v\n        text \"A\"\n");
    assert!(
        e.contains("lower-option-parent") && e.contains("text"),
        "{e}"
    );
    let e = refused("      select value=v type=\"x\"\n        option \"A\"\n");
    assert!(e.contains("lower-attr-tag"), "{e}");
}

const VOLUME: &str = r#"component App
  state volume = 40
  state scale = 1
  state preview = 0
  action setVolume(value: number)
    volume = value
  action hear(value: number)
    preview = value
  action setScale(value: number)
    scale = value
  view
    column
      input type="range" min=0 max=100 step=5 value=volume input=hear change=setVolume testId="volume" aria-label="Volume"
      input type="range" min=0.85 max=1.25 step=0.01 value=scale input=setScale testId="scale" aria-label="Text size" width="100%"
      text `${preview}` testId="preview"
"#;

#[test]
fn a_range_carries_a_number_clamped_and_snapped_as_html_does() {
    let mut r = boot(VOLUME);
    let volume = view_of(&r, "volume");
    let node = r.kernel().node(volume).unwrap();
    assert_eq!(node.node_type, NodeType::Control);
    assert_eq!(node.props.str(PropId::Type), Some("range"));
    assert_eq!(node.props.str(PropId::AccessibilityRole), Some("slider"));
    assert_eq!(node.props.str(PropId::Min), Some("0"));
    assert_eq!(node.props.str(PropId::Step), Some("5"));
    assert_eq!(node.props.str(PropId::Value), Some("40"));
    assert_eq!(node.style.margin_top, exact_kernel::Dimension::Points(2.0));
    // `input` moves, `change` commits; each payload a number on the step.
    r.dispatch(volume, Event::Input("62".into())).unwrap();
    let preview = r.kernel().node(view_of(&r, "preview")).unwrap();
    assert_eq!(preview.props.str(PropId::Text), Some("60"));
    r.dispatch(volume, Event::Change("140".into())).unwrap();
    let node = r.kernel().node(volume).unwrap();
    assert_eq!(node.props.str(PropId::Value), Some("100"));
    let scale = view_of(&r, "scale");
    r.dispatch(scale, Event::Input("1.1234".into())).unwrap();
    let node = r.kernel().node(scale).unwrap();
    assert_eq!(node.props.str(PropId::Value), Some("1.12"));
    assert_eq!(node.props.str(PropId::Min), Some("0.85"));
    match r
        .dispatch(volume, Event::Change("loud".into()))
        .unwrap_err()
    {
        RunnerError::InvalidValue { reason, .. } => assert!(reason.contains("not a number")),
        other => panic!("{other:?}"),
    }
    // A range's handler takes a number, not the text a field's does.
    let e = contract::compile(
        &VOLUME.replace("action hear(value: number)", "action hear(value: string)"),
    )
    .unwrap_err()
    .to_string();
    assert!(e.contains("number"), "{e}");
}

const DUE: &str = r#"component App
  state due = "2026-10-01"
  state alarm = "07:00"
  state at = ""
  action setDue(value: string)
    due = value
  action setAlarm(value: string)
    alarm = value
  action setAt(value: string)
    at = value
  view
    column
      input type="date" value=due min="2026-01-01" max="2026-12-31" change=setDue testId="due" aria-label="Due"
      input type="time" value=alarm change=setAlarm testId="alarm" aria-label="Alarm"
      input type="datetime-local" value=at change=setAt testId="at" aria-label="At"
"#;

#[test]
fn a_date_carries_htmls_value_format_within_min_and_max() {
    let mut r = boot(DUE);
    let due = view_of(&r, "due");
    let node = r.kernel().node(due).unwrap();
    assert_eq!(node.node_type, NodeType::Control);
    assert_eq!(node.props.str(PropId::Type), Some("date"));
    assert_eq!(node.props.str(PropId::Min), Some("2026-01-01"));
    r.dispatch(due, Event::Change("2026-11-05".into())).unwrap();
    let value = |r: &Runner<NoData>, id| {
        r.kernel()
            .node(id)
            .unwrap()
            .props
            .str(PropId::Value)
            .map(str::to_owned)
    };
    assert_eq!(value(&r, due).as_deref(), Some("2026-11-05"));
    for (text, why) in [
        ("2027-01-01", "after max"),
        ("2025-06-01", "before min"),
        ("2026-02-30", "not a date"),
    ] {
        match r.dispatch(due, Event::Change(text.into())).unwrap_err() {
            RunnerError::InvalidValue { reason, .. } => assert!(reason.contains(why), "{reason}"),
            other => panic!("{other:?}"),
        }
    }
    // A cleared control reports the empty string, as HTML's does.
    r.dispatch(due, Event::Change("".into())).unwrap();
    assert_eq!(value(&r, due).as_deref(), Some(""));
    let alarm = view_of(&r, "alarm");
    r.dispatch(alarm, Event::Change("14:30".into())).unwrap();
    assert!(r.dispatch(alarm, Event::Change("2:30 PM".into())).is_err());
    let at = view_of(&r, "at");
    r.dispatch(at, Event::Change("2026-09-27T14:30".into()))
        .unwrap();
    assert_eq!(value(&r, at).as_deref(), Some("2026-09-27T14:30"));
    assert!(r.dispatch(at, Event::Change(true.into())).is_err());
}
