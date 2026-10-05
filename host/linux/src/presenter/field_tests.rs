//! A text field's selection (x2apps codeedit #2) and a radio group (x2apps
//! survey #2) on the Linux host, against what Chrome does: the selection's
//! edits, moves and `select` events, `setSelectionRange` on an unfocused
//! field; a radio's exclusive check, its arrows and Space, a refused
//! action's snap back, and an unbound group's own state.
use super::field::{byte_at, delete, moved, replace};
use super::*;
use exact_runner::{DataError, FieldSelection, SelectionDirection, Value};

#[derive(Default)]
struct NoData;
impl DataSource for NoData {
    fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(name.into()))
    }
}

const APP: &str = r#"component Forms
  state color = "red"
  state locked = "a"
  state refusals = 0
  state body = ""
  state caret = ""
  state picked = ""
  state heard = ""
  action pick(value: string)
    color = value
  action refuse(value: string)
    refusals = refusals + 1
  action loose(value: string)
    heard = `${heard}${value}`
  action edited(text: string, e: InputEvent)
    body = text
    caret = `${e.selectionStart} ${e.selectionEnd} ${e.selectionDirection}`
  action selected(e: InputEvent)
    picked = `${e.value} ${e.selectionStart}-${e.selectionEnd} ${e.selectionDirection}`
  action jump
    setSelectionRange("editor", 1, 3, "backward")
  action collapse
    setSelectionRange("editor", 2, 2)
  action stray
    setSelectionRange("red", 0, 1)
  view
    column width=400 height=600
      row height=20
        input type="radio" name="color" value="red" checked=color == "red" change=pick testId="red" id="red"
        input type="radio" name="color" value="green" checked=color == "green" change=pick testId="green"
        input type="radio" name="color" value="gray" checked=color == "gray" change=pick disabled=true testId="gray"
        input type="radio" name="color" value="blue" checked=color == "blue" change=pick testId="blue"
      row height=20
        input type="radio" name="locked" value="a" checked=locked == "a" change=refuse testId="locked-a"
        input type="radio" name="locked" value="b" checked=locked == "b" change=refuse testId="locked-b"
      row height=20
        input type="radio" name="loose" value="x" change=loose testId="loose-x"
        input type="radio" name="loose" value="y" change=loose testId="loose-y"
      textarea id="editor" value=body input=edited select=selected testId="editor" rows=3
      button "Jump" press=jump testId="jump" height=30
      button "Collapse" press=collapse testId="collapse" height=30
      button "Stray" press=stray testId="stray" height=30
      text `${color} ${locked} ${refusals} ${heard}` testId="log" height=20
      text `${body}|${caret}|${picked}` testId="field" height=20
"#;

fn boot() -> Presenter<NoData> {
    let (p, error) = Presenter::boot_with(
        &contract::compile(APP).unwrap().encode(),
        NoData,
        (400., 600.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p
}
fn id(p: &Presenter<NoData>, test_id: &str) -> ViewId {
    let k = p.host().kernel();
    k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().id
}
fn text(p: &Presenter<NoData>, test_id: &str) -> String {
    let k = p.host().kernel();
    let node = k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap();
    node.props.str(PropId::Text).unwrap().to_string()
}
fn key(p: &mut Presenter<NoData>, target: ViewId, code: &str, name: &str) {
    p.type_key(target, code, name, true, false).unwrap();
    p.type_key(target, code, name, false, false).unwrap();
}
fn painted(p: &mut Presenter<NoData>, test_id: &str) -> Vec<u8> {
    let target = id(p, test_id);
    let (x, y, w, h) = p.rect_of(target).unwrap();
    let pixmap = p.frame();
    let mut out = Vec::new();
    for row in y as u32..(y + h) as u32 {
        for col in x as u32..(x + w) as u32 {
            let c = pixmap.pixel(col, row).unwrap();
            out.extend([c.red(), c.green(), c.blue(), c.alpha()]);
        }
    }
    out
}

#[test]
fn a_selection_edits_moves_and_extends_as_htmls() {
    let caret = FieldSelection::caret;
    let range = |start, end, direction| FieldSelection {
        start,
        end,
        direction,
    };
    // Typing replaces the selection and leaves the caret after it.
    let (text, at) = replace("hello", range(1, 3, SelectionDirection::Backward), "x");
    assert_eq!((text.as_str(), at), ("hxlo", caret(2)));
    // UTF-16 offsets: the emoji is two units, one character.
    assert_eq!(byte_at("a😀b", 3), 5);
    assert_eq!(
        delete("a😀b", caret(3), false),
        Some(("ab".into(), caret(1)))
    );
    assert_eq!(delete("ab", caret(0), false), None);
    assert_eq!(delete("ab", caret(0), true), Some(("b".into(), caret(0))));
    assert_eq!(
        delete("abcd", range(1, 3, SelectionDirection::None), false),
        Some(("ad".into(), caret(1)))
    );
    // A plain arrow collapses a range to its end, else moves a character.
    let r = range(1, 3, SelectionDirection::Forward);
    assert_eq!(moved("abcd", r, "ArrowLeft", false, false), Some(caret(1)));
    assert_eq!(moved("abcd", r, "ArrowRight", false, false), Some(caret(3)));
    assert_eq!(
        moved("abcd", caret(0), "ArrowLeft", false, false),
        Some(caret(0))
    );
    // Shift extends: left of the caret is backward, past it again forward.
    let back = moved("abcd", caret(2), "ArrowLeft", true, false).unwrap();
    assert_eq!(back, range(1, 2, SelectionDirection::Backward));
    let more = moved("abcd", back, "ArrowLeft", true, false).unwrap();
    assert_eq!(more, range(0, 2, SelectionDirection::Backward));
    let shrunk = moved("abcd", more, "ArrowRight", true, false).unwrap();
    assert_eq!(shrunk, range(1, 2, SelectionDirection::Backward));
    let home = moved("abcd", caret(2), "Home", true, false).unwrap();
    assert_eq!(home, range(0, 2, SelectionDirection::Backward));
    // A textarea's Home and End are its line's.
    assert_eq!(
        moved("ab\ncd", caret(4), "Home", false, true),
        Some(caret(3))
    );
    assert_eq!(
        moved("ab\ncd", caret(1), "End", false, true),
        Some(caret(2))
    );
    assert_eq!(
        moved("ab\ncd", caret(1), "End", false, false),
        Some(caret(5))
    );
}

#[test]
fn a_fields_input_reports_its_selection_and_set_selection_range_sets_it() {
    let mut p = boot();
    let editor = id(&p, "editor");
    p.type_text(editor, "hello").unwrap();
    assert_eq!(text(&p, "field"), "hello|5 5 none|");
    // A command's selection on a field that has lost the focus: `select`
    // fires, and the focus stays on the button.
    p.tap(id(&p, "jump")).unwrap();
    p.run_commands(NoData::default);
    assert_eq!(text(&p, "field"), "hello|5 5 none|hello 1-3 backward");
    assert_ne!(p.focus(), Some(editor));
    // The same selection again fires nothing.
    p.set_selection_range(&[
        Value::str("editor"),
        Value::Number(1.0),
        Value::Number(3.0),
        Value::str("backward"),
    ]);
    // The next typing there replaces it.
    key(&mut p, editor, "KeyX", "x");
    assert_eq!(text(&p, "field"), "hxlo|2 2 none|hello 1-3 backward");
    // A collapse to where typing left the caret still fires, as Chrome's.
    p.tap(id(&p, "collapse")).unwrap();
    p.run_commands(NoData::default);
    assert_eq!(text(&p, "field"), "hxlo|2 2 none|hxlo 2-2 none");
    // The person's extension fires `select`; a plain move does not.
    key(&mut p, editor, "ArrowRight", "ArrowRight");
    assert_eq!(p.field_selection(editor), FieldSelection::caret(3));
    p.hold_modifier("ShiftLeft", true);
    key(&mut p, editor, "ArrowLeft", "ArrowLeft");
    key(&mut p, editor, "ArrowLeft", "ArrowLeft");
    p.hold_modifier("ShiftLeft", false);
    assert_eq!(text(&p, "field"), "hxlo|2 2 none|hxlo 1-3 backward");
    key(&mut p, editor, "Backspace", "Backspace");
    assert_eq!(text(&p, "field"), "ho|1 1 none|hxlo 1-3 backward");
    // A paste goes in at the caret.
    p.clipboard(editor, "paste", "ey").unwrap();
    assert_eq!(text(&p, "field"), "heyo|3 3 none|hxlo 1-3 backward");
    // A radio is no text field: refused, and nothing moves.
    p.tap(id(&p, "stray")).unwrap();
    p.run_commands(NoData::default);
    assert!(p
        .host()
        .runner()
        .journal()
        .any(|l| l.contains("setSelectionRange \"red\" refused: not a text field")));
}

#[test]
fn a_radio_group_is_exclusive_and_controlled() {
    let mut p = boot();
    let (red, green, blue) = (id(&p, "red"), id(&p, "green"), id(&p, "blue"));
    let unchecked = painted(&mut p, "green");
    p.tap(green).unwrap();
    assert_eq!(text(&p, "log"), "green a 0 ");
    assert!(p.radio_checked(green) && !p.radio_checked(red));
    assert_ne!(
        painted(&mut p, "green"),
        unchecked,
        "checked paints its dot"
    );
    assert_eq!(painted(&mut p, "red"), unchecked);
    // The arrows move the check and the focus, skipping the disabled radio
    // and wrapping.
    key(&mut p, green, "ArrowDown", "ArrowDown");
    assert_eq!(text(&p, "log"), "blue a 0 ");
    assert_eq!(p.focus(), Some(blue));
    key(&mut p, blue, "ArrowRight", "ArrowRight");
    assert_eq!(text(&p, "log"), "red a 0 ");
    key(&mut p, red, "ArrowUp", "ArrowUp");
    assert_eq!(text(&p, "log"), "blue a 0 ");
    // A disabled radio takes no click or value.
    p.tap(id(&p, "gray")).unwrap_or_default();
    assert!(p.set_control_value(id(&p, "gray"), "true").is_err());
    assert_eq!(text(&p, "log"), "blue a 0 ");
    // An action that writes nothing snaps the group back; the checked one
    // takes no click.
    p.tap(id(&p, "locked-b")).unwrap();
    assert_eq!(text(&p, "log"), "blue a 1 ");
    assert!(p.radio_checked(id(&p, "locked-a")) && !p.radio_checked(id(&p, "locked-b")));
    p.tap(id(&p, "locked-a")).unwrap();
    assert_eq!(text(&p, "log"), "blue a 1 ");
    // The agent's `type <radio> true` checks it as a click; false is refused.
    assert!(p.set_control_value(green, "true").is_ok());
    assert_eq!(text(&p, "log"), "green a 1 ");
    let refused = p.set_control_value(green, "false").unwrap_err();
    assert!(refused.contains("a radio is unchecked by checking another of its group"));
}

#[test]
fn an_unbound_radio_group_keeps_its_own_exclusive_state() {
    let mut p = boot();
    let (x, y) = (id(&p, "loose-x"), id(&p, "loose-y"));
    p.tap(x).unwrap();
    assert!(p.radio_checked(x) && !p.radio_checked(y));
    // Space checks a focused unchecked radio; the other unchecks.
    p.set_focus(Some(y), 0.0);
    key(&mut p, y, "Space", " ");
    assert!(p.radio_checked(y) && !p.radio_checked(x));
    key(&mut p, y, "Space", " ");
    assert_eq!(
        text(&p, "log"),
        "red a 0 xy",
        "a checked radio fires nothing"
    );
}
