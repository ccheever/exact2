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
