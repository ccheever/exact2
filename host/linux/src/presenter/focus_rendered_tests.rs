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
    assert!(p.alert_present(Some(key), "confirm", None, 0.));
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
fn alert_focus_stops_at_the_presented_root_for_inert_ancestors() {
    if !neutral_process() {
        return;
    }
    let mut p = nested_alert();
    let target = named(&p, "yes");
    let container = named(&p, "container");
    let root = named(&p, "confirm");
    for (node, allowed) in [(container, true), (root, false), (target, false)] {
        p.blur();
        p.host.apply_test_ops(&[exact_kernel::Op::SetProp {
            id: node,
            prop: PropId::Inert,
            value: exact_kernel::PropValue::Bool(true),
        }]);
        assert_eq!(
            p.focus_command(&[Value::str("yes")]),
            allowed.then_some(target)
        );
        p.host.apply_test_ops(&[exact_kernel::Op::SetProp {
            id: node,
            prop: PropId::Inert,
            value: exact_kernel::PropValue::Bool(false),
        }]);
    }
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
