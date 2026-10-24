//! Font rows the runner moves beyond D10's own-row transitions (LLP 1019
//! §13 D10): keyframes, `font-variation-settings` axis by axis, and a
//! transition a node starts when the row it inherits changes.
use exact_kernel::{Kernel, StyleId};
use exact_plan::Value;
use exact_runner::{agent, DataError, DataSource, Runner};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
        panic!("unexpected {name}")
    }
}

/// `decls` before the component, `view` its body.
fn boot(decls: &str, view: &str) -> Runner<NoData> {
    let source = format!(
        r#"
{decls}
component Main
  state bold = false
  action toggle
    bold = !bold
  view
{view}
"#
    );
    let plan = contract::compile(&source).unwrap();
    Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

fn id(r: &Runner<NoData>, test_id: &str) -> u32 {
    let found = agent::handle(
        r,
        &format!(r#"{{"op":"tree","target":"{test_id}","shallow":true}}"#),
    );
    found
        .split("{\"id\":")
        .nth(1)
        .unwrap()
        .split(',')
        .next()
        .unwrap()
        .parse()
        .unwrap()
}

/// A node's computed weight, stretch and variation settings.
fn rows(r: &Runner<NoData>, test_id: &str) -> (u16, f32, String) {
    let node = r.kernel().node(id(r, test_id)).unwrap();
    (
        node.computed_row(StyleId::FontWeight, |s| s.font_weight),
        node.computed_row(StyleId::FontStretch, |s| s.font_stretch),
        node.computed_row(StyleId::FontVariationSettings, |s| {
            s.font_variation_settings.css()
        }),
    )
}

fn own_weight(r: &Runner<NoData>, test_id: &str) -> bool {
    let node = r.kernel().node(id(r, test_id)).unwrap();
    node.style.mask.has(StyleId::FontWeight)
}

#[test]
fn keyframes_breathe_a_weight_forever_over_its_own() {
    let mut r = boot(
        "keyframes breathe\n  from font-weight=200\n  to font-weight=800\n",
        r#"    text "Library" testId="title" font-weight=400 animation="breathe 1s linear infinite alternate""#,
    );
    // The first frame is the first keyframe's.
    assert_eq!(rows(&r, "title").0, 200);
    assert!(r.wants_frames());
    // An infinite animation never settles (LLP 1055 D10).
    assert_eq!(r.text_axes_settle_ms(), None);
    r.advance(250.0).unwrap();
    assert_eq!(rows(&r, "title").0, 350);
    r.advance(1500.0).unwrap();
    // The second iteration runs backwards, from 800 to 200.
    assert_eq!(rows(&r, "title").0, 500);
    assert!(r.wants_frames());
}

#[test]
fn a_finite_animation_plays_over_the_underlying_value_and_hands_it_back() {
    let mut r = boot(
        "keyframes widen\n  50% font-stretch=\"150%\"\n",
        r#"    text "Library" testId="title" font-stretch="75%" animation="widen 400ms linear""#,
    );
    assert_eq!(rows(&r, "title").1, 75.0);
    assert_eq!(r.text_axes_settle_ms(), Some(400.0));
    r.advance(100.0).unwrap();
    assert_eq!(rows(&r, "title").1, 112.5);
    r.advance(200.0).unwrap();
    assert_eq!(rows(&r, "title").1, 150.0);
    r.advance(450.0).unwrap();
    assert_eq!(rows(&r, "title").1, 75.0);
    assert!(!r.wants_frames());
}

#[test]
fn variation_axes_transition_one_by_one() {
    let mut r = boot(
        "",
        r#"    text "Library" testId="title" font-variation-settings=(bold ? "'wght' 700, 'GRAD' 100" : "'wght' 300, 'GRAD' 0") transition="font-variation-settings 400ms linear"
"#,
    );
    assert_eq!(rows(&r, "title").2, r#""wght" 300, "GRAD" 0"#);
    r.act("toggle", vec![]).unwrap();
    assert_eq!(rows(&r, "title").2, r#""wght" 300, "GRAD" 0"#);
    r.advance(200.0).unwrap();
    assert_eq!(rows(&r, "title").2, r#""wght" 500, "GRAD" 50"#);
    r.advance(400.0).unwrap();
    assert_eq!(rows(&r, "title").2, r#""wght" 700, "GRAD" 100"#);
    assert!(!r.wants_frames());
}

#[test]
fn settings_that_name_other_axes_change_at_once() {
    let mut r = boot(
        "",
        r#"    text "Library" testId="title" font-variation-settings=(bold ? "'wdth' 80" : "'wght' 300") transition="all 400ms linear"
"#,
    );
    r.act("toggle", vec![]).unwrap();
    assert_eq!(rows(&r, "title").2, r#""wdth" 80"#);
    assert!(!r.wants_frames());
}

#[test]
fn keyframes_move_each_axis_they_name() {
    let mut r = boot(
        "keyframes shift\n  from font-variation-settings=\"'wght' 100, 'CASL' 0\"\n  to font-variation-settings=\"'wght' 900, 'CASL' 1\"\n",
        r#"    text "Library" testId="title" font-variation-settings="'wght' 400" animation="shift 1s linear""#,
    );
    assert_eq!(rows(&r, "title").2, r#""wght" 100, "CASL" 0"#);
    r.advance(500.0).unwrap();
    assert_eq!(rows(&r, "title").2, r#""wght" 500, "CASL" 0.5"#);
    r.advance(1000.0).unwrap();
    // An axis the node's own settings do not name leaves with the animation.
    assert_eq!(rows(&r, "title").2, r#""wght" 400"#);
}

#[test]
fn a_node_whose_inherited_weight_changes_runs_its_own_transition() {
    let mut r = boot(
        "",
        r#"    column testId="list" font-weight=(bold ? 700 : 300)
      text "Eased" testId="eased" transition="font-weight 400ms linear"
      text "Plain" testId="plain"
"#,
    );
    r.act("toggle", vec![]).unwrap();
    // The parent has no transition: it, and the child without one, land.
    assert_eq!(rows(&r, "list").0, 700);
    assert_eq!(rows(&r, "plain").0, 700);
    assert_eq!(rows(&r, "eased").0, 300);
    r.advance(200.0).unwrap();
    assert_eq!(rows(&r, "eased").0, 500);
    r.advance(400.0).unwrap();
    assert_eq!(rows(&r, "eased").0, 700);
    // At rest it inherits again, so the next change reaches it.
    assert!(!own_weight(&r, "eased"));
    r.act("toggle", vec![]).unwrap();
    r.advance(600.0).unwrap();
    assert_eq!(rows(&r, "eased").0, 500);
    r.advance(800.0).unwrap();
    assert_eq!(rows(&r, "eased").0, 300);
}

#[test]
fn a_moving_parent_hands_its_children_each_frame() {
    let mut r = boot(
        "",
        r#"    column testId="list" font-weight=(bold ? 700 : 300) transition="font-weight 400ms linear"
      text "Eased" testId="eased" transition="font-weight 400ms linear"
      text "Plain" testId="plain"
"#,
    );
    r.act("toggle", vec![]).unwrap();
    r.advance(200.0).unwrap();
    // The child without a transition follows the parent's frames; the one
    // with its own runs the same curve from the same value.
    assert_eq!(rows(&r, "list").0, 500);
    assert_eq!(rows(&r, "plain").0, 500);
    assert_eq!(rows(&r, "eased").0, 500);
    r.advance(400.0).unwrap();
    assert_eq!(rows(&r, "eased").0, 700);
    assert!(!own_weight(&r, "eased"));
}

/// A keyed jump builds the rows it lands on in a commit of its own
/// (`scrollIntoView`): their keyframes start there, as a row any other
/// commit builds does.
#[test]
fn a_row_a_jump_builds_starts_its_keyframes() {
    let mut r = boot(
        "keyframes breathe\n  from font-weight=200\n  to font-weight=800\n",
        r#"    list virtualized=true estimated-item-height=40 height=400
      for i in range(500) key=i
        text `row ${i}` testId=`row-${i}` font-weight=400 animation="breathe 1s linear infinite alternate""#,
    );
    assert_eq!(rows(&r, "row-0").0, 200);
    let list = r.collections()[0].view;
    r.scroll_into_view_at(list, "300", "start", "nearest")
        .unwrap();
    assert_eq!(rows(&r, "row-300").0, 200, "the first keyframe's");
}
