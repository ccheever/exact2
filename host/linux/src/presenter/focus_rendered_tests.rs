//! `focus()` reads the current tree, not the last acknowledged picture.
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
  state shown = false
  action reveal
    shown = true
    focus("late")
  action poke
    focus("inside")
  action nothing
    shown = shown
  view
    column
      button id="placed" press=reveal testId="reveal"
        text "Reveal"
      button press=poke testId="poke"
        text "Poke"
      if shown
        button id="late" press=nothing testId="late"
          text "Late"
      dialog id="closed"
        button id="inside" press=nothing testId="inside"
          text "Inside"
"#;

fn named(p: &Presenter<NoData>, name: &str) -> ViewId {
    let k = p.host.kernel();
    k.node_by_key(k.find_by_test_id(name)[0]).unwrap().id
}

/// A presenter whose display has taken and acknowledged its first picture.
fn attached() -> Presenter<NoData> {
    let (mut p, error) = Presenter::boot_with(
        &contract::compile(APP).unwrap().encode(),
        NoData,
        (400., 500.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert_eq!(error, None);
    let frame = p.display_frame().unwrap();
    assert!(p.display_complete(&frame));
    p
}

#[test]
fn desktop_activation_restores_selection_and_drops_held_modifiers() {
    let source = r#"component App
  view
    column
      textarea value="one 🦀 two" testId="editor"
      input value="other" testId="other"
"#;
    let (mut p, error) = Presenter::boot_with(
        &crate::test_plan(source),
        NoData,
        (400., 400.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none());
    let editor = named(&p, "editor");
    let other = named(&p, "other");
    p.set_focus(Some(editor), 0.);
    let selection = exact_runner::FieldSelection {
        start: 4,
        end: 6,
        direction: exact_runner::SelectionDirection::Backward,
    };
    p.mark_field(editor, "one 🦀 two", selection, false);
    p.hardware_key("ControlLeft", "Control", true, false);
    p.hardware_key("ShiftLeft", "Shift", true, false);
    p.set_window_active(false);
    p.set_window_active(false);
    assert_eq!(p.focus(), None);
    assert!(!p.modifiers().ctrl && !p.modifiers().shift);
    p.set_window_active(true);
    assert_eq!(p.focus(), Some(editor));
    assert_eq!(p.field_selection(editor), selection);
    p.field_key(editor, "x", 10.);
    assert_eq!(p.field_text(editor), "one x two");

    p.set_window_active(false);
    p.set_focus(Some(other), 20.);
    p.set_window_active(true);
    assert_eq!(p.focus(), Some(other), "an explicit focus change wins");

    p.set_window_active(false);
    p.host.apply_test_ops(&[exact_kernel::Op::SetProp {
        id: other,
        prop: PropId::Disabled,
        value: exact_kernel::PropValue::Bool(true),
    }]);
    p.set_window_active(true);
    assert_eq!(p.focus(), None, "a disabled field must not regain focus");
    p.set_focus(Some(editor), 30.);
    p.set_window_active(false);
    p.replaced();
    p.set_window_active(true);
    assert_eq!(
        p.focus(),
        None,
        "a replacement runtime has no old focus bookmark"
    );
}

#[test]
fn a_node_shown_by_the_same_commit_takes_focus_before_its_picture() {
    let mut p = attached();
    p.tap(named(&p, "reveal")).unwrap();
    p.run_commands(NoData::default);
    assert_eq!(p.focus, Some(named(&p, "late")));
}

#[test]
fn a_node_in_a_closed_dialog_is_not_rendered() {
    let mut p = attached();
    let poke = named(&p, "poke");
    p.tap(poke).unwrap();
    p.run_commands(NoData::default);
    assert_ne!(p.focus, Some(named(&p, "inside")));
    let journal = crate::agent::handle(&mut p, r#"{"op":"logs"}"#);
    assert!(
        journal.contains(r#"focus \"inside\" refused: not rendered"#),
        "{journal}"
    );
}

#[test]
fn focus_refuses_a_descendant_of_a_hidden_canvas_placement() {
    let mut p = attached();
    let target = named(&p, "poke");
    let parent = p.host.kernel().node(target).unwrap().parent.unwrap();
    p.brush
        .placements
        .insert(parent, crate::placement::Placement::Hidden);
    // The placement boundary applies to descendants, not just its root.
    assert_eq!(p.focus_command(&[Value::str("placed")]), None);
    let journal = crate::agent::handle(&mut p, r#"{"op":"logs"}"#);
    assert!(
        journal.contains(r#"focus \"placed\" refused: not rendered"#),
        "{journal}"
    );
}

const NESTED_ALERT: &str = r#"component App
  state count = 0
  action act
    count = count + 1
  view
    column
      canvas width=200 height=100
        view testId="container"
          dialog id="confirm" testId="confirm" role="alertdialog" aria-label="Confirm?"
            text "Choose an action."
            button id="yes" testId="yes" press=act commandfor="confirm" command="close"
              text "Yes"
            button id="no" testId="no" commandfor="confirm" command="close"
              text "Cancel"
"#;

fn nested_alert() -> Presenter<NoData> {
    let (mut p, error) = Presenter::boot_with(
        &crate::test_plan(NESTED_ALERT),
        NoData,
        (400., 500.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert_eq!(error, None);
    let dialog = named(&p, "confirm");
    let key = p.host.kernel().node(dialog).unwrap().key;
    assert!(p.alert_present(Some(key), "confirm", None, None, true, 0.));
    p.frame();
    p
}

#[test]
fn alert_focus_stops_at_the_presented_root_for_hidden_placements() {
    if !neutral_process() {
        return;
    }
    let mut p = nested_alert();
    let target = named(&p, "yes");
    let container = named(&p, "container");
    let root = named(&p, "confirm");
    p.brush
        .placements
        .insert(container, crate::placement::Placement::Hidden);
    let scene = Scene {
        kernel: p.host.kernel(),
        roots: &p.host.roots(),
        hidden: &|id| p.host.route_visibility(id).0,
        presented: &|id| p.host.presented(id),
        paths: &|_| None,
        scroll: &p.scroll,
        page: p.page,
        images: &p.images.bitmaps,
        focus: None,
        selection: None,
        caret_upstream: false,
        pointer: None,
        controls: &p.controls,
        chosen: &p.chosen,
        menu: None,
    };
    let frame = p.brush.paint(&scene, p.viewport).unwrap();
    assert!(frame.boxes.iter().any(|b| b.id == target));
    assert!(!frame.boxes.iter().any(|b| b.id == container));
    p.blur();
    assert_eq!(p.focus_command(&[Value::str("yes")]), Some(target));
    // The root and the target are still inside the boundary.
    for hidden in [root, target] {
        p.blur();
        p.brush
            .placements
            .insert(hidden, crate::placement::Placement::Hidden);
        assert_eq!(p.focus_command(&[Value::str("yes")]), None);
        p.brush.placements.remove(&hidden);
    }
}

#[test]
fn inert_focus_is_checked_after_alert_synchronization() {
    if !neutral_process() {
        return;
    }
    for name in ["container", "confirm", "yes"] {
        let mut p = nested_alert();
        let target = named(&p, "yes");
        let root = named(&p, "confirm");
        let node = named(&p, name);
        p.blur();
        p.host.apply_test_ops(&[exact_kernel::Op::SetProp {
            id: node,
            prop: PropId::Inert,
            value: exact_kernel::PropValue::Bool(true),
        }]);
        p.frame();
        assert!(p.last_frame_succeeded);
        // Admission closes the alert when its root or an ancestor is inert.
        assert_eq!(p.alert_root(), (node == target).then_some(root), "{name}");
        assert_eq!(p.focus_command(&[Value::str("yes")]), None, "{name}");
        assert_ne!(p.focus(), Some(target), "{name}");
    }
}

#[test]
fn an_opened_or_replaced_alert_focuses_its_action_before_display_acknowledgement() {
    if !neutral_process() {
        return;
    }
    let source = r#"component App
  view
    column
      input testId="field" value="unsent draft"
      dialog id="first" testId="first" role="alertdialog"
        text "First notice."
        button commandfor="first" command="close" testId="first-ok"
          text "OK"
      dialog id="next" testId="next" role="alertdialog"
        text "Next notice."
        button commandfor="next" command="close" testId="next-ok"
          text "OK"
"#;
    let (mut p, error) = Presenter::boot_with(
        &crate::test_plan(source),
        NoData,
        (400., 500.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert_eq!(error, None);
    let frame = p.display_frame().unwrap();
    assert!(p.display_complete(&frame));
    let field = named(&p, "field");
    assert_eq!(p.set_focus(Some(field), 0.), None);
    for (name, action) in [("first", "first-ok"), ("next", "next-ok")] {
        let root = named(&p, name);
        let key = p.host.kernel().node(root).unwrap().key;
        assert!(p.alert_present(Some(key), name, None, None, true, 1.));
        assert_eq!(p.focus(), Some(named(&p, action)), "{name}");
        let frame = p.display_frame().unwrap();
        assert!(p.display_complete(&frame));
        assert_eq!(p.focus(), Some(named(&p, action)), "{name}");
    }
    p.key_down("Enter", 2.);
    p.run_commands(|| NoData);
    assert!(p.alert_root().is_none());
    assert_eq!(p.focus(), Some(field));
    let root = named(&p, "first");
    let key = p.host.kernel().node(root).unwrap().key;
    assert!(p.alert_present(Some(key), "first", None, None, true, 3.));
    p.key_down("Escape", 4.);
    assert!(p.alert_root().is_none());
    assert_eq!(p.focus(), Some(field));
}

fn neutral_process() -> bool {
    const CHILD: &str = "EXACT_ALERT_FOCUS_TEST";
    if std::env::var_os(CHILD).is_some() {
        crate::app::show_neutral_look();
        return true;
    }
    let name = std::thread::current().name().unwrap().to_owned();
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", &name, "--nocapture"])
        .env(CHILD, "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    false
}

#[test]
fn popover_follows_the_invoker_in_the_next_display_frame() {
    if !neutral_process() {
        return;
    }
    let source = include_str!("../../tests/authored_dialog.contract").replace(
        "id=\"invoker\"",
        "id=\"invoker\" position=\"absolute\" right=20 top=100",
    );
    let (mut p, error) = Presenter::boot_with(
        &contract::compile(&source).unwrap().encode(),
        NoData,
        (480., 400.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p.serve_overlays();
    p.frame();
    let frame = p.display_frame().unwrap();
    assert!(p.display_complete(&frame));
    p.tap(named(&p, "invoker")).unwrap();
    p.run_commands(|| NoData);
    p.frame();
    let frame = p.display_frame().unwrap();
    assert!(p.display_complete(&frame));
    let root = named(&p, "picker");
    let before = p.rect_of(root).unwrap();
    assert!(p.resize(600., 400.).is_none());
    let frame = p.display_frame().unwrap();
    assert_eq!(p.rect_of(root), Some(before));
    assert!(p.display_complete(&frame));
    let moved = p.rect_of(root).unwrap();
    assert_eq!(moved, (before.0 + 120., before.1, before.2, before.3));
}
