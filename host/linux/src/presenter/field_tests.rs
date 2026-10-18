//! A text field's selection (x2apps codeedit #2) and a radio group (x2apps
//! survey #2) on the Linux host, against what Chrome does: the selection's
//! edits, moves and `select` events, `setSelectionRange` on an unfocused
//! field; a radio's exclusive check, its arrows and Space, a refused
//! action's snap back, and an unbound group's own state.
use super::field::{byte_at, delete, fit, moved, replace};
use super::*;
use exact_runner::{FieldSelection, SelectionDirection, Value};

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

fn boot() -> Presenter<()> {
    let (p, error) = Presenter::boot_with(
        &crate::test_plan(APP),
        (),
        (400., 600.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p
}
fn id(p: &Presenter<()>, test_id: &str) -> ViewId {
    let k = p.host().kernel();
    k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().id
}
fn text(p: &Presenter<()>, test_id: &str) -> String {
    let k = p.host().kernel();
    let node = k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap();
    node.props.str(PropId::Text).unwrap().to_string()
}
fn key(p: &mut Presenter<()>, target: ViewId, code: &str, name: &str) {
    p.type_key(target, code, name, true, false).unwrap();
    p.type_key(target, code, name, false, false).unwrap();
}
fn painted(p: &mut Presenter<()>, test_id: &str) -> Vec<u8> {
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
    p.run_commands(<()>::default);
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
    p.run_commands(<()>::default);
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
    p.run_commands(<()>::default);
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

#[test]
fn set_range_text_command_preserves_selection_and_reports_one_input() {
    let source = r#"component RangeEdit
  state body = "a😀bc"
  state count = 0
  state caret = ""
  action edited(v: string, e: InputEvent)
    body = v
    count = count + 1
    caret = `${e.selectionStart}-${e.selectionEnd}|${e.inputType}`
  action replace
    setSelectionRange("editor", 3, 5)
    setRangeText("editor", "XY", 1, 3)
  view
    column
      textarea id="editor" testId="editor" value=body input=edited
      button "Replace" testId="replace" press=replace
      text `${body}|${count}|${caret}` testId="result"
"#;
    let (mut p, error) = Presenter::boot_with(
        &crate::test_plan(source),
        (),
        (400., 600.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none());
    let button = id(&p, "replace");
    p.tap(button).unwrap();
    p.run_commands(<()>::default);
    assert_eq!(text(&p, "result"), "aXYbc|1|1-5|insertReplacementText");
    assert_eq!(p.field_text(id(&p, "editor")), "aXYbc");
    assert_ne!(
        p.edited,
        Some(id(&p, "editor")),
        "a script is no pending user change"
    );
}

#[test]
fn set_range_text_modes_utf16_sanitization_and_refusals() {
    let mut p = boot();
    let editor = id(&p, "editor");
    for (mode, a, b) in [
        ("preserve", 1, 4),
        ("select", 1, 2),
        ("start", 1, 1),
        ("end", 2, 2),
    ] {
        p.type_text(editor, "a😀bc").unwrap();
        p.set_selection_range(&[Value::str("editor"), Value::Number(3.), Value::Number(5.)]);
        p.set_range_text(&[
            Value::str("editor"),
            Value::str("X"),
            Value::Number(1.),
            Value::Number(3.),
            Value::str(mode),
        ]);
        assert_eq!(p.field_text(editor), "aXbc");
        assert_eq!(
            p.field_selection(editor),
            FieldSelection {
                start: a,
                end: b,
                direction: SelectionDirection::None
            }
        );
    }
    p.type_text(editor, "see @a.ts now").unwrap();
    p.set_range_text(&[
        Value::str("editor"),
        Value::str("a\r\nb"),
        Value::Number(4.),
        Value::Number(9.),
        Value::str("end"),
    ]);
    assert_eq!(p.field_text(editor), "see a\nb now");
    assert_eq!(p.field_selection(editor), FieldSelection::caret(8));
    let old = p.field_text(editor);
    for args in [
        vec![
            Value::str("editor"),
            Value::str("bad"),
            Value::Number(9.),
            Value::Number(4.),
        ],
        vec![
            Value::str("editor"),
            Value::str("bad"),
            Value::Number(0.),
            Value::Number(1.),
            Value::str("invalid"),
        ],
        vec![Value::str("red"), Value::str("bad")],
        vec![Value::str("missing"), Value::str("bad")],
    ] {
        p.set_range_text(&args);
    }
    assert_eq!(p.field_text(editor), old);
    assert_eq!(
        p.host()
            .runner()
            .journal()
            .filter(|l| l.contains("setRangeText") && l.contains("refused"))
            .count(),
        4
    );
    p.set_selection_range(&[Value::str("editor"), Value::Number(0.), Value::Number(3.)]);
    p.set_range_text(&[Value::str("editor"), Value::str("See")]);
    assert_eq!(p.field_text(editor), "See a\nb now");
}

#[test]
fn range_replacement_url_sanitization_and_select_events_match_chrome() {
    let source = r#"component RangeEdit
  state log = ""
  state heard = ""
  action edited(v: string, e: InputEvent)
    heard = `${v}|${e.selectionStart}-${e.selectionEnd}`
    log = `${log}input `
  action selected(e: InputEvent)
    log = `${log}select `
  view
    column
      input type="url" id="url" testId="url" input=edited select=selected
      text log testId="log"
      text heard testId="heard"
"#;
    let (mut p, error) = Presenter::boot_with(
        &crate::test_plan(source),
        (),
        (400., 600.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none());
    let url = id(&p, "url");
    p.set_range_text(&[
        Value::str("url"),
        Value::str(" \t\nhttps://example.com\r\u{c} "),
        Value::Number(0.),
        Value::Number(0.),
    ]);
    assert_eq!(p.field_text(url), "https://example.com");
    assert_eq!(text(&p, "heard"), "https://example.com|0-0");
    assert_eq!(text(&p, "log"), "input ", "unchanged caret emits no select");
    p.set_range_text(&[
        Value::str("url"),
        Value::str("!"),
        Value::Number(2.),
        Value::Number(2.),
        Value::str("end"),
    ]);
    assert_eq!(text(&p, "log"), "input input ");
    p.run_commands(<()>::default);
    assert_eq!(text(&p, "log"), "input input select ");
    p.set_range_text(&[
        Value::str("url"),
        Value::str("\u{a0}x\u{a0}"),
        Value::Number(0.),
        Value::Number(999.),
        Value::str("end"),
    ]);
    assert_eq!(p.field_text(url), "\u{a0}x\u{a0}");
}

#[test]
fn range_replacement_select_tasks_observe_both_commands() {
    let source = r#"component RangeEdit
  state body = ""
  state log = ""
  action edited(v: string)
    body = v
    log = `${log}input:${v} `
  action selected(e: InputEvent)
    log = `${log}select:${e.value} `
  action replace
    setRangeText("editor", "A", 0, 0, "end")
    setRangeText("editor", "B", 1, 1, "end")
  view
    column
      textarea id="editor" testId="editor" value=body input=edited select=selected
      button "Replace" testId="replace" press=replace
      text log testId="log"
"#;
    let (mut p, error) = Presenter::boot_with(
        &crate::test_plan(source),
        (),
        (400., 600.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none());
    let button = id(&p, "replace");
    p.tap(button).unwrap();
    p.run_commands(<()>::default);
    assert_eq!(text(&p, "log"), "input:A input:AB select:AB select:AB ");
}

#[test]
fn beforeinput_cancels_paste_and_selectionchange_hears_a_caret_move() {
    let source = r#"component App
  state body = "abc"
  state caret = 0
  state kind = ""
  action before(e: InputEvent)
    kind = e.inputType
    if e.inputType == "insertFromPaste"
      preventDefault()
  action input(v: string, e: InputEvent)
    body = v
    kind = e.inputType
  action moved(e: InputEvent)
    caret = e.selectionStart
  view
    column width=400 height=300
      textarea value=body beforeinput=before input=input selectionchange=moved testId="editor"
      text `${body}:${caret}:${kind}` testId="result"
"#;
    let (mut p, error) = Presenter::boot_with(
        &crate::test_plan(source),
        (),
        (400., 300.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none());
    let editor = id(&p, "editor");
    p.press_field(editor, 10000., 10000.);
    p.field_key(editor, "ArrowLeft", 0.);
    p.run_commands(<()>::default);
    assert_eq!(text(&p, "result"), "abc:2:");
    p.paste_field(editor, "PASTE", 0.);
    assert_eq!(p.field_text(editor), "abc");
    assert_eq!(text(&p, "result"), "abc:2:insertFromPaste");
    p.field_key(editor, "x", 0.);
    p.run_commands(<()>::default);
    assert_eq!(text(&p, "result"), "abxc:3:insertText");
    p.field_key(editor, "Backspace", 0.);
    p.run_commands(<()>::default);
    assert_eq!(text(&p, "result"), "abc:2:deleteContentBackward");
}

#[test]
fn agent_typing_selects_all_then_delivers_a_cancelable_insert_text() {
    let source = r#"component App
  state body = "old"
  state events = ""
  state edits = 0
  state changes = 0
  state picked = ""
  state moved = ""
  action before(e: InputEvent)
    events = `${events}before:${e.inputType}:${match e.data { case some(data) => data, case none => "null" }}:${e.cancelable}:${e.value}:${e.selectionStart}-${e.selectionEnd}|`
    if e.data == some("cancel")
      preventDefault()
  action edited(v: string, e: InputEvent)
    body = v
    edits = edits + 1
    events = `${events}input:${e.inputType}:${e.cancelable}:${e.selectionStart}-${e.selectionEnd}|`
  action changed(v: string)
    changes = changes + 1
  action selected(e: InputEvent)
    picked = `${e.value}:${e.selectionStart}-${e.selectionEnd}:${edits}`
  action moving(e: InputEvent)
    moved = `${e.selectionStart}-${e.selectionEnd}`
  view
    column
      textarea testId="editor" value=body beforeinput=before input=edited change=changed select=selected selectionchange=moving
      button "Blur" testId="blur"
      text events testId="events"
      text `${body}|${edits}|${changes}|${picked}|${moved}` testId="result"
"#;
    let (mut p, error) = Presenter::boot_with(
        &crate::test_plan(source),
        (),
        (400., 600.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none());
    p.run_commands(<()>::default);
    assert_eq!(text(&p, "result"), "old|0|0||3-3");
    let editor = id(&p, "editor");
    let reply = crate::agent::handle(
        &mut p,
        &format!(r#"{{"op":"type","id":{editor},"text":"key ArrowLeft"}}"#),
    );
    assert!(!reply.contains("\"error\""), "{reply}");
    assert_eq!(p.focus(), Some(editor));
    assert_eq!(
        text(&p, "events"),
        "before:insertText:key ArrowLeft:true:old:0-3|input:insertText:false:13-13|"
    );
    // The select-all's `select` is heard before the insertion.
    assert_eq!(text(&p, "result"), "key ArrowLeft|1|0|old:0-3:0|13-13");
    p.tap(id(&p, "blur")).unwrap();
    assert_eq!(p.edited, None);
    p.type_text(editor, "cancel").unwrap();
    p.run_commands(<()>::default);
    assert_eq!(p.field_text(editor), "key ArrowLeft");
    assert_eq!(p.edited, None, "a cancelled edit has no pending change");
    assert_eq!(
        text(&p, "events"),
        "before:insertText:key ArrowLeft:true:old:0-3|input:insertText:false:13-13|before:insertText:cancel:true:key ArrowLeft:0-13|"
    );
    assert_eq!(
        text(&p, "result"),
        "key ArrowLeft|1|1|key ArrowLeft:0-13:1|0-13"
    );
    p.tap(id(&p, "blur")).unwrap();
    assert_eq!(p.host.runner().slot("changes"), Some(&Value::Number(1.)));
}

#[test]
fn queued_selection_events_follow_the_edits_focus_command() {
    let source = r#"component App
  state body = "old"
  state focused = false
  state picked = ""
  state moved = ""
  action before(e: InputEvent)
    if e.data == some("cancel")
      focus("other")
      preventDefault()
  action edited(v: string)
    body = v
    focus("other")
  action otherFocused
    focused = true
  action selected(e: InputEvent)
    picked = `${focused}:${e.value}:${e.selectionStart}-${e.selectionEnd}`
  action moving(e: InputEvent)
    moved = `${focused}:${e.value}:${e.selectionStart}-${e.selectionEnd}`
  view
    column
      textarea testId="editor" value=body beforeinput=before input=edited select=selected selectionchange=moving
      input id="other" testId="other" focus=otherFocused
      text picked testId="picked"
      text moved testId="moved"
"#;
    let plan = crate::test_plan(source);
    // The agent's select-all is heard before its edit: its `select` and
    // `selectionchange` read the field as the edit found it, before the
    // edit's focus command. A cancelled edit moves nothing after them.
    for (path, expected, picked) in [
        ("agent", "true:new:3-3", "false:old:0-3"),
        ("keyboard", "true:oldx:4-4", ""),
        ("paste", "true:oldpaste:8-8", ""),
        ("cancel", "false:old:0-3", "false:old:0-3"),
    ] {
        let (mut p, error) = Presenter::boot_with(
            &plan,
            (),
            (400., 600.),
            1.,
            PathBuf::new(),
            PainterChoice::Cpu,
        )
        .unwrap();
        assert!(error.is_none());
        p.run_commands(<()>::default);
        let editor = id(&p, "editor");
        match path {
            "agent" | "cancel" => {
                let value = if path == "agent" { "new" } else { "cancel" };
                let reply = crate::agent::handle(
                    &mut p,
                    &format!(r#"{{"op":"type","id":{editor},"text":"{value}"}}"#),
                );
                assert!(!reply.contains("\"error\""), "{path}: {reply}");
            }
            _ => {
                assert!(p.set_focus(Some(editor), 0.).is_none());
                if path == "keyboard" {
                    p.key_down_with("x", "KeyX", false, 0.);
                } else {
                    p.clipboard(editor, "paste", "paste").unwrap();
                }
                p.run_commands(<()>::default);
            }
        }
        assert_eq!(p.focus(), Some(id(&p, "other")), "{path}");
        assert_eq!(text(&p, "moved"), expected, "{path}");
        assert_eq!(text(&p, "picked"), picked, "{path}");
    }
}

#[test]
fn the_select_alls_handler_and_its_commands_run_before_the_agents_insertion() {
    // On the web the select-all's `select` handler, and the selection it
    // sets, land before the driver's insertion, which then goes where it put
    // the caret: after "old", not over it.
    let source = r#"component App
  state body = "old"
  state picks = 0
  action edited(v: string)
    body = v
  action selected(e: InputEvent)
    picks = picks + 1
    if picks == 0
      setSelectionRange("editor", 3, 3)
  view
    column
      textarea id="editor" testId="editor" value=body input=edited select=selected
      text `${body}|${picks}` testId="result"
"#;
    let (mut p, error) = Presenter::boot_with(
        &crate::test_plan(source),
        (),
        (400., 600.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none());
    p.run_commands(<()>::default);
    let editor = id(&p, "editor");
    let reply = crate::agent::handle(
        &mut p,
        &format!(r#"{{"op":"type","id":{editor},"text":"x"}}"#),
    );
    assert!(!reply.contains("\"error\""), "{reply}");
    // Two `select`s: the select-all's, then the collapse its handler made.
    assert_eq!(text(&p, "result"), "oldx|2");
}

#[test]
fn a_select_handler_that_moves_the_focus_takes_the_agents_insertion_with_it() {
    // The browser inserts the driver's text at the focus: the field the
    // select handler focused (its caret after the value the app wrote), or,
    // focused on a button, nowhere.
    let source = r#"component App
  state body = "old"
  state note = "n"
  state toButton = false
  action edited(v: string)
    body = v
  action noted(v: string)
    note = v
  action selected(e: InputEvent)
    if toButton
      focus("go")
    else
      focus("other")
  action aim
    toButton = true
  view
    column
      textarea id="editor" testId="editor" value=body input=edited select=selected
      input id="other" testId="other" value=note input=noted
      button "Go" id="go" testId="go" press=aim
      text `${body}|${note}` testId="result"
"#;
    for (to_button, expected) in [(false, "old|nx"), (true, "old|n")] {
        let (mut p, error) = Presenter::boot_with(
            &crate::test_plan(source),
            (),
            (400., 600.),
            1.,
            PathBuf::new(),
            PainterChoice::Cpu,
        )
        .unwrap();
        assert!(error.is_none());
        p.run_commands(<()>::default);
        if to_button {
            p.tap(id(&p, "go")).unwrap();
            p.run_commands(<()>::default);
        }
        let editor = id(&p, "editor");
        let reply = crate::agent::handle(
            &mut p,
            &format!(r#"{{"op":"type","id":{editor},"text":"x"}}"#),
        );
        assert!(!reply.contains("\"error\""), "{reply}");
        assert_eq!(text(&p, "result"), expected, "to the button: {to_button}");
    }
}

#[test]
fn the_agents_text_is_cut_to_the_room_maxlength_leaves_at_the_caret() {
    // The select handler leaves a caret at the end, so the insertion keeps
    // the value: Chrome cuts it to maxlength less the kept code units, and
    // with no room fires `beforeinput` (the whole text) but no `input`.
    for (body, typed, expected) in [
        ("hello", "x", "hello|before:x|"),
        ("hel", "xyz", "helxy|before:xyz|input:xy|"),
    ] {
        let end = body.len();
        let source = format!(
            r#"component App
  state body = "{body}"
  state events = ""
  action before(e: InputEvent)
    events = `${{events}}before:${{match e.data {{ case some(data) => data, case none => "null" }}}}|`
  action edited(v: string, e: InputEvent)
    body = v
    events = `${{events}}input:${{match e.data {{ case some(data) => data, case none => "null" }}}}|`
  action selected(e: InputEvent)
    setSelectionRange("editor", {end}, {end})
  view
    column
      textarea id="editor" testId="editor" maxlength=5 value=body beforeinput=before input=edited select=selected
      text `${{body}}|${{events}}` testId="result"
"#
        );
        let (mut p, error) = Presenter::boot_with(
            &crate::test_plan(&source),
            (),
            (400., 600.),
            1.,
            PathBuf::new(),
            PainterChoice::Cpu,
        )
        .unwrap();
        assert!(error.is_none());
        p.run_commands(<()>::default);
        let editor = id(&p, "editor");
        let reply = crate::agent::handle(
            &mut p,
            &format!(r#"{{"op":"type","id":{editor},"text":"{typed}"}}"#),
        );
        assert!(!reply.contains("\"error\""), "{reply}");
        assert_eq!(text(&p, "result"), expected, "{body} + {typed}");
    }
}

#[test]
fn the_agents_fitted_insertion_matches_chromes_value_events_and_caret() {
    // Chrome 151, CDP Input.insertText: endpoints inside a surrogate pair
    // advance past it; the caret counts the original insertion's UTF-16 units.
    let mut actual = Vec::new();
    let mut expected = Vec::new();
    for (tag, body, limit, start, end, typed, value, caret, data) in [
        ("input", "abc", 0, 0, 3, "x", "", 0, Some("")),
        ("textarea", "abcdef", 5, 1, 2, "x", "acdef", 2, Some("")),
        ("textarea", "abcdef", 5, 1, 2, "xy", "acdef", 3, Some("")),
        ("textarea", "abcde", 5, 1, 2, "😀", "acde", 3, Some("")),
        ("textarea", "abcdef", 5, 5, 6, "xy", "abcde", 5, Some("")),
        ("textarea", "abcdef", 5, 1, 2, "", "acdef", 1, Some("")),
        ("textarea", "abcde", 5, 1, 2, "x", "axcde", 2, Some("x")),
        ("textarea", "abcde", 5, 1, 3, "xyz", "axyde", 4, Some("xy")),
        ("input", "abcde", 5, 1, 3, "xyz", "axyde", 4, Some("xy")),
        ("textarea", "😀bc", 4, 0, 1, "x", "xbc", 1, Some("x")),
        ("textarea", "😀bc", 4, 1, 2, "x", "😀bc", 3, None),
        ("textarea", "a😀bc", 5, 1, 2, "x", "axbc", 2, Some("x")),
        ("textarea", "a😀bc", 5, 2, 3, "x", "a😀bc", 4, None),
        ("textarea", "😀bc", 0, 0, 1, "x", "bc", 1, Some("")),
        ("textarea", "abcde", 5, 1, 1, "xyz", "abcde", 4, None),
    ] {
        let source = format!(
            r#"component App
  state body = "{body}"
  state events = ""
  action before(e: InputEvent)
    events = `${{events}}before:${{e.inputType}}:${{match e.data {{ case some(data) => data, case none => "null" }}}}|`
  action edited(v: string, e: InputEvent)
    body = v
    events = `${{events}}input:${{e.inputType}}:${{match e.data {{ case some(data) => data, case none => "null" }}}}:${{e.selectionStart}}-${{e.selectionEnd}}|`
  action selected(e: InputEvent)
    setSelectionRange("editor", {start}, {end})
  view
    column
      {tag} id="editor" testId="editor" maxlength={limit} value=body beforeinput=before input=edited select=selected
      text events testId="events"
"#
        );
        let (mut p, error) = Presenter::boot_with(
            &crate::test_plan(&source),
            (),
            (400., 600.),
            1.,
            PathBuf::new(),
            PainterChoice::Cpu,
        )
        .unwrap();
        assert!(error.is_none(), "{error:?}");
        p.run_commands(<()>::default);
        let editor = id(&p, "editor");
        let reply = crate::agent::handle(
            &mut p,
            &format!(r#"{{"op":"type","id":{editor},"text":"{typed}"}}"#),
        );
        assert!(!reply.contains("\"error\""), "{reply}");
        actual.push((
            p.field_text(editor),
            p.field_selection(editor),
            text(&p, "events"),
            p.edited == Some(editor),
        ));
        let input = data.map_or_else(String::new, |data| {
            format!("input:insertText:{data}:{caret}-{caret}|")
        });
        expected.push((
            value.to_owned(),
            FieldSelection::caret(caret),
            format!("before:insertText:{typed}|{input}"),
            data.is_some(),
        ));
    }
    assert_eq!(actual, expected);
}

#[test]
fn a_rejected_insertion_preserves_an_unchanged_selection_commands_mark() {
    let source = r#"component App
  state picks = 0
  action selected(e: InputEvent)
    picks = picks + 1
  action collapse
    setSelectionRange("editor", 0, 0)
  view
    column
      input id="editor" testId="editor" maxlength=0 select=selected
      button "Collapse" testId="collapse" press=collapse
      text `${picks}` testId="picks"
"#;
    let (mut p, error) = Presenter::boot_with(
        &crate::test_plan(source),
        (),
        (400., 600.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p.run_commands(<()>::default);
    let editor = id(&p, "editor");
    let reply = crate::agent::handle(
        &mut p,
        &format!(r#"{{"op":"type","id":{editor},"text":"x"}}"#),
    );
    assert!(!reply.contains("\"error\""), "{reply}");
    assert_eq!(p.field_text(editor), "");
    assert_eq!(p.field_selection(editor), FieldSelection::caret(0));
    assert_eq!(text(&p, "picks"), "0");
    p.tap(id(&p, "collapse")).unwrap();
    p.run_commands(<()>::default);
    assert_eq!(text(&p, "picks"), "0");
}

#[test]
fn an_insertion_fits_the_room_the_replaced_selection_leaves() {
    let range = |start, end| FieldSelection {
        start,
        end,
        direction: SelectionDirection::None,
    };
    assert_eq!(fit("xyz", "hel", range(3, 3), Some(5)), "xy");
    assert_eq!(fit("x", "hello", range(5, 5), Some(5)), "");
    assert_eq!(
        fit("x", "toolong", range(7, 7), Some(5)),
        "",
        "over the limit, it never grows"
    );
    assert_eq!(fit("xy", "toolong", range(0, 7), Some(5)), "xy");
    assert_eq!(
        fit("😀", "hel", range(3, 3), Some(4)),
        "",
        "a surrogate pair is not split"
    );
    // Inside a surrogate pair `replace` removes nothing, so nothing is room.
    assert_eq!(fit("x", "😀", range(0, 1), Some(2)), "");
    assert_eq!(fit("xyz", "hel", range(3, 3), None), "xyz");
}

#[test]
fn a_focus_handler_that_redirects_once_loses_the_focus_to_select_all() {
    // Chromium's `select()` focuses the field it selects: the editor takes
    // the focus back (its handler hears `focus` again) and the text.
    let source = r#"component App
  state body = "old"
  state note = "n"
  state focuses = 0
  action edited(v: string)
    body = v
  action noted(v: string)
    note = v
  action focused
    focuses = focuses + 1
    if focuses == 0
      focus("other")
  view
    column
      textarea id="editor" testId="editor" value=body input=edited focus=focused
      input id="other" testId="other" value=note input=noted
      text `${body}|${note}|${focuses}` testId="result"
"#;
    let (mut p, error) = Presenter::boot_with(
        &crate::test_plan(source),
        (),
        (400., 600.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none());
    p.run_commands(<()>::default);
    let editor = id(&p, "editor");
    let reply = crate::agent::handle(
        &mut p,
        &format!(r#"{{"op":"type","id":{editor},"text":"x"}}"#),
    );
    assert!(!reply.contains("\"error\""), "{reply}");
    assert_eq!(p.focus(), Some(editor));
    assert_eq!(text(&p, "result"), "x|n|2");
}

#[test]
fn runtime_value_writes_report_only_selection_changes_at_mount_and_commit() {
    let source = r#"component App
  state body = "a😀b"
  state areaMoves = 0
  state inputMoves = 0
  state emptyMoves = 0
  state caret = ""
  action areaMoved(e: InputEvent)
    areaMoves = areaMoves + 1
    caret = `${e.selectionStart}-${e.selectionEnd}`
  action inputMoved(e: InputEvent)
    inputMoves = inputMoves + 1
  action emptyMoved(e: InputEvent)
    emptyMoves = emptyMoves + 1
  action short
    body = "xy"
  action sameLength
    body = "pq"
  view
    column
      textarea id="area" testId="area" value=body selectionchange=areaMoved
      input value=body selectionchange=inputMoved
      input value="" selectionchange=emptyMoved
      button "Short" testId="short" press=short
      button "Same length" testId="same-length" press=sameLength
      text `${areaMoves}|${inputMoves}|${emptyMoves}|${caret}` testId="result"
"#;
    let (mut p, error) = Presenter::boot_with(
        &crate::test_plan(source),
        (),
        (400., 600.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none());
    assert_eq!(text(&p, "result"), "0|0|0|", "selectionchange is queued");
    p.run_commands(<()>::default);
    assert_eq!(text(&p, "result"), "1|1|0|4-4");
    p.tap(id(&p, "short")).unwrap();
    p.run_commands(<()>::default);
    assert_eq!(text(&p, "result"), "2|2|0|2-2");
    p.tap(id(&p, "same-length")).unwrap();
    p.run_commands(<()>::default);
    assert_eq!(text(&p, "result"), "2|2|0|2-2");
    p.set_selection_range(&[Value::str("area"), Value::Number(0.), Value::Number(1.)]);
    p.run_commands(<()>::default);
    assert_eq!(text(&p, "result"), "3|2|0|0-1");
    p.tap(id(&p, "same-length")).unwrap();
    p.run_commands(<()>::default);
    assert_eq!(
        text(&p, "result"),
        "3|2|0|0-1",
        "same value keeps the range"
    );
    p.tap(id(&p, "short")).unwrap();
    p.run_commands(<()>::default);
    assert_eq!(text(&p, "result"), "4|2|0|2-2");
    assert_eq!(p.field_selection(id(&p, "area")), FieldSelection::caret(2));
}

#[test]
fn range_replacement_end_reports_the_moved_caret_once() {
    let source = r#"component App
  state body = "abc"
  state events = ""
  state moves = 0
  state caret = ""
  action edited(v: string, e: InputEvent)
    body = v
    events = `${events}${e.inputType}:${e.selectionStart}-${e.selectionEnd}|`
  action moving(e: InputEvent)
    moves = moves + 1
    caret = `${e.selectionStart}-${e.selectionEnd}`
  view
    column
      textarea id="editor" testId="editor" value=body input=edited selectionchange=moving
      text `${body}|${moves}|${caret}|${events}` testId="result"
"#;
    let (mut p, error) = Presenter::boot_with(
        &crate::test_plan(source),
        (),
        (400., 600.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none());
    p.run_commands(<()>::default);
    p.set_selection_range(&[Value::str("editor"), Value::Number(0.), Value::Number(2.)]);
    p.run_commands(<()>::default);
    p.set_range_text(&[
        Value::str("editor"),
        Value::str("😀"),
        Value::Number(999.),
        Value::Number(999.),
        Value::str("end"),
    ]);
    p.run_commands(<()>::default);
    assert_eq!(text(&p, "result"), "abc😀|3|5-5|insertReplacementText:5-5|");
    p.set_range_text(&[
        Value::str("editor"),
        Value::str(""),
        Value::Number(999.),
        Value::Number(999.),
        Value::str("end"),
    ]);
    p.run_commands(<()>::default);
    assert_eq!(
        text(&p, "result"),
        "abc😀|3|5-5|insertReplacementText:5-5|insertReplacementText:5-5|"
    );
}

#[test]
fn unmarked_and_stale_marks_compare_against_the_reported_caret() {
    let source = r#"component App
  state body = "hello"
  action write
    body = "world"
  view
    column
      textarea markup="markdown" id="editor" testId="editor" value=body
      button "Write" testId="write" press=write
"#;
    let (mut p, error) = Presenter::boot_with(
        &crate::test_plan(source),
        (),
        (400., 600.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none());
    let editor = id(&p, "editor");
    assert!(
        p.field_change_tasks.is_empty(),
        "no plain-field mount event for Markdown"
    );
    p.fields.remove(&editor);
    assert_eq!(p.field_selection(editor), FieldSelection::caret(5));
    p.press_field(editor, 10000., 10000.);
    assert!(
        p.field_change_tasks.is_empty(),
        "pressing at the end did not move it"
    );

    p.fields.remove(&editor);
    p.field_key(editor, "Home", 0.);
    assert_eq!(p.field_selection(editor), FieldSelection::caret(0));
    assert!(p.field_change_tasks.contains(&editor));
    p.run_commands(<()>::default);

    p.fields.remove(&editor);
    p.set_selection_range(&[Value::str("editor"), Value::Number(0.), Value::Number(1.)]);
    assert!(p.field_change_tasks.contains(&editor));
    p.run_commands(<()>::default);
    p.tap(id(&p, "write")).unwrap();
    assert_eq!(p.field_selection(editor), FieldSelection::caret(5));
    assert!(
        p.field_change_tasks.contains(&editor),
        "a runtime write moved the range"
    );
    p.run_commands(<()>::default);

    p.note_field(editor, "old value", FieldSelection::caret(0));
    assert_eq!(p.field_selection(editor), FieldSelection::caret(5));
    p.press_field(editor, 10000., 10000.);
    assert!(
        p.field_change_tasks.is_empty(),
        "the stale mark was not the reported caret"
    );
}

#[test]
fn email_and_number_report_the_web_fallback_without_a_selection_api() {
    let source = r#"component App
  state body = "12"
  state moves = 0
  state heard = ""
  state position = ""
  action moving(e: InputEvent)
    moves = moves + 1
    position = `${e.selectionStart}-${e.selectionEnd}`
  action before(e: InputEvent)
    heard = `${heard}before:${e.selectionStart}-${e.selectionEnd}|`
  action edited(v: string, e: InputEvent)
    heard = `${heard}input:${e.selectionStart}-${e.selectionEnd}|`
  action write
    body = "34"
  view
    column
      input type="email" id="email" testId="email" value=body beforeinput=before input=edited selectionchange=moving select=moving
      input type="number" id="number" testId="number" value=body beforeinput=before input=edited selectionchange=moving select=moving
      button "Write" testId="write" press=write
      text `${moves}|${heard}` testId="result"
      text position testId="position"
"#;
    let (mut p, error) = Presenter::boot_with(
        &crate::test_plan(source),
        (),
        (400., 600.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none());
    p.run_commands(<()>::default);
    assert_eq!(text(&p, "result"), "2|");
    assert_eq!(text(&p, "position"), "2-2");
    for name in ["email", "number"] {
        let field = id(&p, name);
        p.set_selection_range(&[Value::str(name), Value::Number(0.), Value::Number(1.)]);
        assert_eq!(p.field_selection(field), FieldSelection::caret(2));
        assert!(p.set_focus(Some(field), 0.).is_none());
        p.field_key(field, "Home", 0.);
        assert_eq!(p.field_selection(field), FieldSelection::caret(0));
        p.field_key(field, "9", 0.);
        assert_eq!(
            p.field_text(field),
            "912",
            "the private editing caret still moves"
        );
        assert_eq!(p.field_selection(field), FieldSelection::caret(1));
        p.run_commands(<()>::default);
        assert_eq!(text(&p, "position"), "3-3");
    }
    assert_eq!(
        text(&p, "result"),
        "4|before:2-2|input:3-3|before:2-2|input:3-3|"
    );
    p.tap(id(&p, "write")).unwrap();
    p.run_commands(<()>::default);
    assert_eq!(text(&p, "position"), "2-2");
    for name in ["email", "number"] {
        assert_eq!(p.field_text(id(&p, name)), "34");
        assert_eq!(p.field_selection(id(&p, name)), FieldSelection::caret(2));
    }
    assert_eq!(
        text(&p, "result"),
        "6|before:2-2|input:3-3|before:2-2|input:3-3|"
    );
}

#[test]
fn field_sync_visits_only_fields_touched_by_commits() {
    let source = r#"component App
  state body = "ab"
  state count = 0
  state shown = false
  action tick
    count = count + 1
  action write
    body = "cd"
  action toggle
    shown = not shown
  view
    column
      input id="field" testId="field" value=body
      if shown
        input testId="mounted" value="hello"
      button "Tick" testId="tick" press=tick
      button "Write" testId="write" press=write
      button "Toggle" testId="toggle" press=toggle
      text `${count}`
"#;
    let (mut p, error) = Presenter::boot_with(
        &crate::test_plan(source),
        (),
        (400., 600.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none());
    p.run_commands(<()>::default);
    p.set_selection_range(&[Value::str("field"), Value::Number(0.), Value::Number(1.)]);
    p.run_commands(<()>::default);
    for _ in 0..256 {
        assert!(p
            .host
            .dispatch_at(id(&p, "tick"), Event::Press, 0.)
            .is_none());
        assert_eq!(p.host.values.pending_fields(), 0);
        assert!(p.after_commit().is_none());
    }
    let field = id(&p, "field");
    assert_eq!(p.field_selection(field).end, 1);
    assert!(p
        .host
        .dispatch_at(id(&p, "write"), Event::Press, 0.)
        .is_none());
    assert_eq!(p.host.values.pending_fields(), 1);
    assert!(p.after_commit().is_none());
    assert_eq!(p.field_selection(field), FieldSelection::caret(2));
    assert!(p.field_change_tasks.contains(&field));
    p.run_commands(<()>::default);
    p.tap(id(&p, "toggle")).unwrap();
    let mounted = id(&p, "mounted");
    assert_eq!(p.field_selection(mounted), FieldSelection::caret(5));
    assert!(p.field_change_tasks.contains(&mounted));
    p.run_commands(<()>::default);
    p.tap(id(&p, "toggle")).unwrap();
    assert!(!p.fields.contains_key(&mounted));
    assert_eq!(p.host.values.pending_fields(), 0);
}
