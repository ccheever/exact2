//! A focused control that a commit disables loses the focus and runs its
//! `blur`, as HTML's focus fixup has it; a control inside a disabled box keeps
//! it; a removed one runs no `blur`.
use super::*;
use exact_runner::{DataError, Value};

#[derive(Default)]
struct NoData;
impl DataSource for NoData {
    fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(name.into()))
    }
}

const APP: &str = r#"component App
  state busy = false
  state off = false
  state gone = false
  state log = ""
  action go
    busy = true
  action shut
    off = true
  action leave
    gone = true
  action note(s: string)
    log = `${log}${s}`
  view
    column
      button press=go disabled=busy focus=note("+") blur=note("-") testId="self"
        text "self"
      column disabled=off testId="box"
        button press=shut focus=note("[") blur=note("]") testId="inner"
          text "inner"
      if not gone
        button press=leave focus=note("<") blur=note(">") testId="leaving"
          text "leaving"
"#;

fn named(p: &Presenter<NoData>, name: &str) -> ViewId {
    let k = p.host.kernel();
    k.node_by_key(k.find_by_test_id(name)[0]).unwrap().id
}

fn log(p: &Presenter<NoData>) -> String {
    p.host
        .runner()
        .slot("log")
        .unwrap()
        .as_str()
        .unwrap()
        .to_owned()
}

fn presenter() -> Presenter<NoData> {
    let (p, error) = Presenter::boot_with(
        &contract::compile(APP).unwrap().encode(),
        NoData,
        (400., 500.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert_eq!(error, None);
    p
}

/// Focus `name`, then press it: its action runs and its commit is serviced.
fn focus_and_press(p: &mut Presenter<NoData>, name: &str) {
    let id = named(p, name);
    assert!(p.set_focus(Some(id), 0.).is_none());
    assert!(p.host.dispatch_at(id, Event::Press, 0.).is_none());
    assert!(p.after_commit().is_none());
}

#[test]
fn a_focused_control_its_commit_disables_loses_the_focus_and_runs_its_blur() {
    let mut p = presenter();
    focus_and_press(&mut p, "self");
    assert_eq!(p.focus(), None);
    assert_eq!(log(&p), "+-", "one focus, one blur");
}

#[test]
fn a_control_in_a_box_that_becomes_disabled_keeps_the_focus() {
    let mut p = presenter();
    focus_and_press(&mut p, "inner");
    assert_eq!(p.focus(), Some(named(&p, "inner")));
    assert_eq!(log(&p), "[");
}

#[test]
fn a_removed_control_loses_the_focus_without_its_blur() {
    let mut p = presenter();
    focus_and_press(&mut p, "leaving");
    assert_eq!(p.focus(), None);
    assert_eq!(log(&p), "<");
}

#[test]
fn the_agents_tap_on_a_control_that_disables_itself_blurs_it_once() {
    let mut p = presenter();
    let id = named(&p, "self");
    p.tap(id).unwrap();
    assert_eq!(p.focus(), None);
    assert_eq!(log(&p), "+-", "one focus, one blur");
}

fn boot(source: &str) -> Presenter<NoData> {
    let (p, error) = Presenter::boot_with(
        &contract::compile(source).unwrap().encode(),
        NoData,
        (400., 500.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert_eq!(error, None);
    p
}

#[test]
fn a_field_that_disables_itself_commits_then_blurs_as_a_blur_does() {
    let mut p = boot(
        r#"component App
  state t = ""
  state off = false
  state log = ""
  action typed(v: string)
    t = v
    off = true
  action changed(value: string)
    log = `${log}c`
    focus("next")
  action note(s: string)
    log = `${log}${s}`
  view
    column
      input value=t input=typed change=changed blur=note("-") disabled=off testId="field"
      button id="next" press=note("n") testId="next"
        text "next"
"#,
    );
    let field = named(&p, "field");
    p.type_text(field, "a").unwrap();
    // Its `change` and then its `blur`, each serviced, as `set_focus` blurs
    // a typed field. (The change's `focus("next")` is not honored mid-blur
    // here, as on any blur on this host: a separate item.)
    assert_eq!(log(&p), "c-");
    assert_ne!(p.focus(), Some(field));
}

#[test]
fn a_blur_that_enables_the_control_again_does_not_give_it_the_focus_back() {
    let mut p = boot(
        r#"component App
  state busy = false
  state log = ""
  action go
    busy = true
  action reopen
    busy = false
    log = `${log}-`
  view
    column
      button press=go disabled=busy blur=reopen testId="self"
        text "self"
"#,
    );
    let id = named(&p, "self");
    p.tap(id).unwrap();
    assert_eq!(p.focus(), None);
    assert_eq!(log(&p), "-");
}

#[test]
fn stopping_the_blur_does_not_stop_the_focusout() {
    let mut p = boot(
        r#"component App
  state busy = false
  state log = ""
  action go
    busy = true
  action quiet
    log = `${log}-`
    stopPropagation()
  action outer
    log = `${log}o`
  view
    column focusout=outer
      button press=go disabled=busy blur=quiet testId="self"
        text "self"
"#,
    );
    focus_and_press(&mut p, "self");
    assert_eq!(p.focus(), None);
    assert_eq!(log(&p), "-o");
}

#[test]
fn a_disabled_link_keeps_the_focus_as_the_webs_a_does() {
    let mut p = boot(
        r#"component App
  state off = false
  action shut
    off = true
  view
    column
      link href="/docs" press=shut disabled=off testId="docs"
        text "docs"
"#,
    );
    focus_and_press(&mut p, "docs");
    assert_eq!(p.focus(), Some(named(&p, "docs")));
}

#[test]
fn a_nested_disabled_blur_does_not_give_the_pressed_button_its_focus_back() {
    // The press disables its button and mounts a field that autofocuses; the
    // button's blur enables it again, and the field's focus disables the
    // field: two blurs of this kind in one press.
    let mut p = boot(
        r#"component App
  state busy = false
  state shown = false
  state off = false
  action go
    busy = true
    shown = true
  action reopen
    busy = false
  action shut
    off = true
  view
    column
      button press=go disabled=busy blur=reopen testId="a"
        text "a"
      if shown
        input autofocus=true focus=shut disabled=off testId="b"
"#,
    );
    let a = named(&p, "a");
    p.tap(a).unwrap();
    assert_ne!(p.focus(), Some(a), "the pressed button stays blurred");
}

#[test]
fn a_checkbox_its_input_disables_blurs_before_its_change_runs() {
    let mut p = boot(
        r#"component App
  state off = false
  state log = ""
  action flipped(v: bool)
    off = true
    log = `${log}i`
  action changed(v: bool)
    off = false
    log = `${log}c`
  action note(s: string)
    log = `${log}${s}`
  view
    column
      input type="checkbox" input=flipped change=changed blur=note("b") disabled=off testId="box"
"#,
    );
    let id = named(&p, "box");
    assert!(p.set_focus(Some(id), 0.).is_none());
    p.tap(id).unwrap();
    assert_eq!(
        log(&p),
        "ibc",
        "the blur between input and change, as the web's fixup"
    );
    assert_eq!(p.focus(), None, "enabled again by change, it stays blurred");
}

#[test]
fn a_press_its_ancestor_handles_keeps_the_focus_its_commit_moved() {
    let mut p = boot(
        r#"component App
  state off = false
  action go
    off = true
  view
    column press=go tabindex=0 autofocus=off testId="parent"
      button disabled=off testId="child"
        text "Go"
"#,
    );
    let child = named(&p, "child");
    p.tap(child).unwrap();
    assert_eq!(
        p.focus(),
        Some(named(&p, "parent")),
        "the autofocus the commit gave stands"
    );
}

#[test]
fn the_blurs_focusout_runs_before_an_autofocus_its_commit_offers() {
    let mut p = boot(
        r#"component App
  state busy = false
  state log = ""
  action go
    busy = true
  action note(s: string)
    log = `${log}${s}`
  view
    column focusout=note("o")
      button press=go disabled=busy blur=note("-") testId="self"
        text "self"
      input autofocus=busy focus=note("F") testId="field"
"#,
    );
    let id = named(&p, "self");
    p.tap(id).unwrap();
    assert_eq!(log(&p), "-oF", "blur, focusout, then the autofocus's focus");
}

#[test]
fn a_range_its_input_disables_blurs_before_its_change_runs() {
    // `set_control_value`'s input and change (a range, a select, a picker).
    let mut p = boot(
        r#"component App
  state level = 1
  state off = false
  state log = ""
  action moved(v: number)
    level = v
    off = true
    log = `${log}i`
  action changed(v: number)
    off = false
    log = `${log}c`
  action note(s: string)
    log = `${log}${s}`
  view
    column
      input type="range" min=0 max=10 value=level input=moved change=changed blur=note("b") disabled=off testId="level" width=200
"#,
    );
    let id = named(&p, "level");
    assert!(p.set_focus(Some(id), 0.).is_none());
    p.set_control_value(id, "5").unwrap();
    assert_eq!(
        log(&p),
        "ibc",
        "the blur between input and change, as the web's fixup"
    );
    assert_eq!(p.focus(), None, "enabled again by change, it stays blurred");
}

#[test]
fn a_field_its_change_disables_blurs_before_its_submit_runs() {
    let mut p = boot(
        r#"component App
  state t = ""
  state off = false
  state log = ""
  action typed(v: string)
    t = v
  action changed(value: string)
    off = true
    log = `${log}c`
  action sent
    off = false
    log = `${log}s`
  action note(s: string)
    log = `${log}${s}`
  view
    column
      input value=t input=typed change=changed submit=sent blur=note("b") disabled=off testId="field"
"#,
    );
    let field = named(&p, "field");
    p.type_text(field, "a").unwrap();
    p.key_down("Enter", p.host.now());
    assert_eq!(log(&p), "cbs", "change, the blur, then submit");
    assert_eq!(p.focus(), None);
}

#[test]
fn a_hover_that_disables_the_focused_control_blurs_it_before_the_next_hover() {
    let mut p = boot(
        r#"component App
  state off = false
  state log = ""
  action outer(over: bool)
    off = over
  action inner(over: bool)
    off = false
  action note(s: string)
    log = `${log}${s}`
  view
    column
      button disabled=off blur=note("b") testId="self"
        text "self"
      column hover=outer testId="outer" width=200 height=100
        column hover=inner testId="inner" width=100 height=50
"#,
    );
    let id = named(&p, "self");
    assert!(p.set_focus(Some(id), 0.).is_none());
    p.boxes();
    let inner = p.box_of(named(&p, "inner")).unwrap();
    let (x, y) = inner.center();
    assert!(p.hover_at(Some((x, y)), 1.).is_none());
    assert_eq!(
        log(&p),
        "b",
        "the outer hover disabled it; the inner one enabled it after its blur"
    );
    assert_eq!(p.focus(), None);
}
