//! Real Presenter contacts; independent from any new helper API or packet shape.
use exact_kernel::{motion::motion_node, NodeKey, PropId};
use exact_linux::{presenter::PainterChoice, Presenter};
use exact_motion::{Property, Value};
use exact_runner::{DataError, DataSource};
use std::path::PathBuf;

#[derive(Default)]
struct Empty;
impl DataSource for Empty {
    fn query(
        &mut self,
        name: &str,
        _: &[exact_runner::Value],
    ) -> Result<exact_runner::Value, DataError> {
        Err(DataError::UnknownSource(name.into()))
    }
}
const APP: &str = r#"component App
  state x = 20
  state y = 10
  state zoom = 2
  state count = 0
  state seenScale = 0
  state seenVx = 0
  state seenX = 0
  state seenY = 0
  state geometryCount = 0
  state boxWidth = 0
  state portWidth = 0
  state left = 20
  state disabled = false
  state showing = true
  state draft = ""
  action geometry(w: number, h: number, pw: number, ph: number) writes geometryCount, boxWidth, portWidth
    geometryCount = geometryCount + 1
    boxWidth = w
    portWidth = pw
  action release(px: number, py: number, s: number, vx: number, vy: number, vs: number) writes x, y, count, seenScale, seenX, seenY, seenVx
    x = px
    y = py
    count = count + 1
    seenX = px
    seenY = py
    seenScale = s
    seenVx = vx
  action fit writes zoom
    zoom = 1
  action shift writes left
    left = 40
  action disable writes disabled
    disabled = true
  action hide writes showing
    showing = false
  action show writes showing
    showing = true
  action edit(value) writes draft
    draft = value
  view
    box width="100%" height="100%"
      when showing
        box testId="clip" position="absolute" left=left top=40 width="60%" height=160 overflow="hidden" box-sizing="border-box" padding=0 border-width=0
          box id="target" testId="target" width="100%" height="100%" box-sizing="border-box" margin=0 padding=0 border-width=0 translate=`${x}px ${y}px` scale=zoom transition="translate spring(300,30,1), scale spring(300,30,1)"
            box testId="handle" position="absolute" left=0 top=0 width="100%" height="100%" transformDragFor="target" transformgeometry=geometry transformrelease=release touch-action="none" disabled=disabled
      column position="absolute" top=300
        button testId="fit" press=fit
          text "fit"
        button testId="shift" press=shift
          text "shift"
        button testId="disable" press=disable
          text "disable"
        button testId="hide" press=hide
          text "hide"
        button testId="show" press=show
          text "show"
        input testId="draft" value=draft change=edit
        text `${count}` testId="count"
        text `${seenScale}` testId="seenScale"
        text `${seenVx}` testId="seenVx"
        text `${seenX}` testId="seenX"
        text `${seenY}` testId="seenY"
        text `${geometryCount}` testId="geometryCount"
        text `${boxWidth}` testId="boxWidth"
        text `${portWidth}` testId="portWidth"
"#;
fn boot(source: &str) -> Presenter<Empty> {
    let (p, error) = Presenter::boot_with(
        &contract::compile(source).unwrap().encode(),
        Empty,
        (400., 500.),
        1.,
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p
}
fn key(p: &Presenter<Empty>, name: &str) -> NodeKey {
    p.host().kernel().find_by_test_id(name)[0]
}
fn id(p: &Presenter<Empty>, name: &str) -> u32 {
    p.host().kernel().node_by_key(key(p, name)).unwrap().id
}
fn text<'a>(p: &'a Presenter<Empty>, name: &str) -> &'a str {
    p.host()
        .kernel()
        .node(id(p, name))
        .unwrap()
        .props
        .str(PropId::Text)
        .unwrap()
}
fn press(p: &mut Presenter<Empty>, name: &str) {
    p.tap(id(p, name)).unwrap();
}
fn held(p: &Presenter<Empty>, target: NodeKey) -> [bool; 2] {
    [Property::Translate, Property::Scale]
        .map(|prop| p.host().engine().is_held(motion_node(target), prop))
}
fn recognize(p: &mut Presenter<Empty>) -> NodeKey {
    let target = key(p, "target");
    assert!(p.pointer_down(80., 80., 0.).unwrap());
    assert!(p.pointer_move(86., 86., 10.).unwrap());
    assert_eq!(held(p, target), [true, true]);
    target
}
#[test]
fn boot_geometry_reports_untransformed_boxes_once_and_typing_does_not_repeat_it() {
    let mut p = boot(APP);
    assert_eq!(text(&p, "geometryCount"), "1");
    let actual = p
        .host()
        .kernel()
        .node(id(&p, "target"))
        .unwrap()
        .frame
        .width as f64;
    assert!((actual - 240.).abs() < 0.001);
    assert_eq!(text(&p, "boxWidth").parse::<f64>().unwrap(), actual);
    assert_eq!(text(&p, "portWidth").parse::<f64>().unwrap(), actual);
    p.type_text(id(&p, "draft"), "unrelated").unwrap();
    assert!(p.resize(400., 500.).is_none());
    assert_eq!(text(&p, "geometryCount"), "1");
}
#[test]
fn catch_is_zero_displacement_and_scale_two_pan_stays_in_parent_units() {
    let mut p = boot(APP);
    let target = recognize(&mut p);
    assert_eq!(
        p.host()
            .engine()
            .value(motion_node(target), Property::Translate),
        Some(Value {
            x: 20.,
            y: 10.,
            ..Value::ZERO
        })
    );
    assert!(
        p.host().engine().quiescent(),
        "held-only must not keep a frame loop alive"
    );
    assert!(p.pointer_move(106., 76., 20.).unwrap());
    assert_eq!(
        p.host()
            .engine()
            .value(motion_node(target), Property::Translate),
        Some(Value {
            x: 40.,
            y: 0.,
            ..Value::ZERO
        })
    );
    assert!(p.pointer_up(126., 96., 30.).unwrap());
    assert_eq!(text(&p, "seenX"), "60");
    assert_eq!(text(&p, "seenY"), "20");
    assert_eq!(text(&p, "seenScale"), "2");
    assert_eq!(text(&p, "count"), "1");
    assert_eq!(held(&p, target), [false, false]);
    assert!(p.collection_interaction().is_none());
}
#[test]
fn zoom_control_during_pair_hold_survives_final_sample_and_release() {
    let mut p = boot(APP);
    let target = recognize(&mut p);
    press(&mut p, "fit");
    assert_eq!(held(&p, target), [true, true]);
    assert_eq!(
        p.host()
            .engine()
            .target(motion_node(target), Property::Scale),
        Some(Value::scalar(1.))
    );
    assert_eq!(
        p.host()
            .engine()
            .value(motion_node(target), Property::Scale),
        Some(Value::scalar(2.))
    );
    assert!(p.pointer_up(106., 86., 30.).unwrap());
    assert_eq!(text(&p, "seenScale"), "2");
    p.tick(10_000.);
    assert_eq!(
        p.host()
            .engine()
            .value(motion_node(target), Property::Scale),
        Some(Value::scalar(1.))
    );
}
#[test]
fn same_size_origin_change_cancels_without_duplicate_dimension_event() {
    let mut p = boot(APP);
    let target = recognize(&mut p);
    press(&mut p, "shift");
    assert_eq!(held(&p, target), [false, false]);
    assert!(p.collection_interaction().is_none());
    assert_eq!(text(&p, "geometryCount"), "1");
    assert!(!p.pointer_up(100., 100., 30.).unwrap());
    assert_eq!(text(&p, "count"), "0");
}
#[test]
fn real_resize_cancels_then_publishes_new_untransformed_dimensions() {
    let mut p = boot(APP);
    let target = recognize(&mut p);
    assert!(p.resize(300., 500.).is_none());
    assert_eq!(held(&p, target), [false, false]);
    assert_eq!(text(&p, "geometryCount"), "2");
    assert_eq!(text(&p, "boxWidth"), "180");
    assert_eq!(text(&p, "portWidth"), "180");
    assert!(!p.pointer_up(100., 100., 30.).unwrap());
    assert_eq!(text(&p, "count"), "0");
}
#[test]
fn disable_and_replaced_generation_retire_capture_without_late_action() {
    for action in ["disable", "hide"] {
        let mut p = boot(APP);
        let old = recognize(&mut p);
        press(&mut p, action);
        assert_eq!(held(&p, old), [false, false]);
        let now = p.host().now();
        assert!(!p.pointer_up(f32::NAN, f32::NAN, f64::NAN).unwrap());
        assert_eq!(p.host().now(), now);
        assert_eq!(text(&p, "count"), "0");
        assert!(p.collection_interaction().is_none());
        if action == "hide" {
            press(&mut p, "show");
            assert_ne!(key(&p, "target"), old);
            assert!(p.pointer_down(80., 80., 20.).unwrap());
            assert!(p.pointer_move(86., 86., 30.).unwrap());
            p.pointer_cancel(40.).unwrap();
            assert_eq!(text(&p, "count"), "0");
        }
    }
}
#[test]
fn invalid_live_sample_preserves_both_holds_clock_and_valid_continuation() {
    let mut p = boot(APP);
    let target = recognize(&mut p);
    let now = p.host().now();
    assert!(p.pointer_move(f32::NAN, 90., 20.).is_err());
    assert_eq!(p.host().now(), now);
    assert_eq!(held(&p, target), [true, true]);
    assert!(p.pointer_move(100., 90., 20.).unwrap());
    p.pointer_cancel(30.).unwrap();
    assert_eq!(held(&p, target), [false, false]);
    assert_eq!(text(&p, "count"), "0");
}
#[test]
fn source_geometry_that_is_not_actual_fill_refuses_contact() {
    // A max constraint passes the common source resolver but breaks actual fill.
    let source = APP.replace(
        "id=\"target\" testId=\"target\"",
        "id=\"target\" testId=\"target\" max-width=100",
    );
    let mut p = boot(&source);
    let target = key(&p, "target");
    assert!(p.pointer_down(80., 80., 0.).unwrap());
    assert!(!p.pointer_move(86., 86., 10.).unwrap());
    assert_eq!(held(&p, target), [false, false]);
    assert_eq!(text(&p, "count"), "0");
}

#[test]
fn unrelated_typing_preserves_contact_and_negative_pan_velocity_is_parent_space() {
    let mut p = boot(APP);
    let target = recognize(&mut p);
    p.type_text(id(&p, "draft"), "while held").unwrap();
    assert_eq!(held(&p, target), [true, true]);
    assert_eq!(text(&p, "geometryCount"), "1");
    assert!(p.pointer_move(146., 86., 150.).unwrap());
    assert!(p.pointer_move(126., 86., 170.).unwrap());
    assert!(p.pointer_up(106., 86., 190.).unwrap());
    let vx = text(&p, "seenVx").parse::<f64>().unwrap();
    assert!((vx + 1000.).abs() < 0.001, "not divided by scale2: {vx}");
}

#[test]
fn ancestor_scroll_changes_mapping_without_repeating_identical_dimensions() {
    let start = APP.find("      when showing").unwrap();
    let end = APP.find("      column position").unwrap();
    let body = APP[start..end]
        .lines()
        .map(|line| format!("    {line}\n"))
        .collect::<String>();
    let source = format!("{}      scroll testId=\"scroller\" width=400 height=240\n        box height=900 width=400 position=\"relative\"\n{}{}", &APP[..start], body, &APP[end..]);
    let mut p = boot(&source);
    let target = recognize(&mut p);
    p.wheel(id(&p, "scroller"), 0., 40.).unwrap();
    assert_eq!(held(&p, target), [false, false]);
    assert_eq!(text(&p, "geometryCount"), "1");
    assert!(!p.pointer_up(100., 100., 30.).unwrap());
    assert_eq!(text(&p, "count"), "0");
}

#[test]
fn geometry_callback_deletion_clears_owner_and_does_not_dispatch_again() {
    let source = APP
        .replace(
            "writes geometryCount, boxWidth, portWidth",
            "writes geometryCount, boxWidth, portWidth, showing",
        )
        .replace(
            "    portWidth = pw",
            "    portWidth = pw\n    showing = false",
        );
    let mut p = boot(&source);
    assert!(p.host().kernel().find_by_test_id("target").is_empty());
    assert!(p.host().transform_bindings().is_empty());
    assert_eq!(text(&p, "geometryCount"), "1");
    assert!(p.resize(300., 500.).is_none());
    assert_eq!(text(&p, "geometryCount"), "1");
}

#[test]
fn malformed_live_terminal_does_not_consume_contact_or_clock() {
    let mut p = boot(APP);
    let target = recognize(&mut p);
    let now = p.host().now();
    assert!(p.pointer_up(f32::NAN, 100., 20.).is_err());
    assert_eq!(p.host().now(), now);
    assert_eq!(held(&p, target), [true, true]);
    assert!(p.pointer_up(106., 86., 30.).unwrap());
    assert_eq!(text(&p, "count"), "1");
}
