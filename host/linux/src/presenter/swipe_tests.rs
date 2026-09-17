use super::*;
use exact_motion::{HoldEnd, Property, Value};
use exact_runner::DataError;

#[derive(Default)]
struct NoData;
impl DataSource for NoData {
    fn query(
        &mut self,
        name: &str,
        _: &[exact_runner::Value],
    ) -> Result<exact_runner::Value, DataError> {
        Err(DataError::UnknownSource(name.into()))
    }
}
fn source() -> String {
    r#"component App
  state count = 0
  state target = 0
  state showing = true
  action reply writes count, target
    count = count + 1
    target = 180
  action hide writes showing
    showing = false
  action show writes showing
    showing = true
  view
    column width=400 height=500
      when showing
        box testId="row" width=400 height=100 swiperight=reply touch-action="pan-y" opacity=(target == 0 ? 1 : 0.25) transition="translate spring(300, 30, 1)"
          box testId="indicator" swipeIndicator=true opacity=0 scale=0 transition="opacity 100ms ease-out, scale 100ms ease-out" width=10 height=10
          text "swipe me" testId="label"
      button press=hide testId="hide"
        text "hide"
      button press=show testId="show"
        text "show"
      text `${count}` testId="count"
"#.into()
}
fn boot(src: &str) -> Presenter<NoData> {
    let (p, error) = Presenter::boot_with(
        &contract::compile(src).unwrap().encode(),
        NoData,
        (400., 500.),
        1.,
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p
}
fn id(p: &Presenter<NoData>, name: &str) -> ViewId {
    let k = p.host.kernel();
    k.node_by_key(k.find_by_test_id(name)[0]).unwrap().id
}
fn count(p: &Presenter<NoData>) -> &str {
    p.host
        .kernel()
        .node(id(p, "count"))
        .unwrap()
        .props
        .str(PropId::Text)
        .unwrap()
}
fn recognize(p: &mut Presenter<NoData>) {
    assert!(p.pointer_down(20., 40., 0.).unwrap());
    assert!(p.pointer_move(30., 40., 10.).unwrap());
}

#[test]
fn pointer_release_applies_final_sample_then_action_while_held_then_releases_pin() {
    let mut p = boot(&source());
    let row = id(&p, "row");
    recognize(&mut p);
    assert_eq!(
        p.host.presented(row).translate.0,
        0.,
        "recognition starts at the sampled origin"
    );
    assert!(p.collection_interaction().is_some());
    // No intermediate move: release itself must apply the 70px displacement.
    assert!(p.pointer_up(100., 40., 50.).unwrap());
    assert_eq!(count(&p), "1");
    assert!((p.host.presented(row).translate.0 - 65.2).abs() < 0.001);
    assert!(!p.host.engine().is_held(
        exact_kernel::motion::motion_node(p.host.kernel().node(row).unwrap().key),
        Property::Translate
    ));
    assert_eq!(p.collection_interaction(), None);
    assert!(!p.pointer_up(100., 40., 60.).unwrap());
    p.tick(10_000.);
    assert_eq!(p.host.presented(row).translate.0, 0.);
    assert_eq!(p.host.presented(row).opacity, 0.25);
}

#[test]
fn pointer_catches_return_at_recognition_and_zero_delta_never_jumps() {
    let mut p = boot(&source());
    let row = id(&p, "row");
    let first = p
        .host
        .hold_begin(row, Property::Translate, 0.)
        .unwrap()
        .unwrap();
    p.host
        .hold_update(first.token, Value::new(100., 0.), 0.)
        .unwrap();
    p.host.hold_end(first.token, HoldEnd::Cancel, 0.).unwrap();
    p.tick(20.);
    // Hit the moving row, then advance the return before recognition.
    let x = p.host.presented(row).translate.0 + 20.;
    p.pointer_down(x, 40., 20.).unwrap();
    p.tick(40.);
    let caught = p.host.presented(row).translate.0;
    assert!(p.pointer_move(x + 10., 40., 40.).unwrap());
    assert!((p.host.presented(row).translate.0 - caught).abs() < 0.0001);
    assert!(
        !p.host.motion(),
        "holding alone must not schedule animation"
    );
    p.pointer_move(x + 10., 40., 60.).unwrap();
    assert!((p.host.presented(row).translate.0 - caught).abs() < 0.0001);
    p.pointer_cancel(60.).unwrap();
    assert_eq!(count(&p), "0");
}

#[test]
fn display_cancel_and_wheel_win_without_dispatching_reply() {
    let mut p = boot(&source());
    recognize(&mut p);
    p.pointer_move(150., 40., 30.).unwrap();
    p.pointer_cancel(40.).unwrap();
    assert_eq!(p.collection_interaction(), None);
    assert_eq!(count(&p), "0");
    p.tick(10_000.);
    p.pointer_down(20., 40., 10_000.).unwrap();
    p.pointer_move(30., 40., 10_010.).unwrap();
    p.pointer_move(150., 40., 10_030.).unwrap();
    p.wheel_at(20., 40., 0., 40.);
    assert!(!p.pointer_up(150., 40., 10_050.).unwrap());
    assert_eq!(p.collection_interaction(), None);
    assert_eq!(count(&p), "0");
}

#[test]
fn vertical_motion_and_auto_touch_action_never_take_over() {
    for src in [source(), source().replace("touch-action=\"pan-y\"", "")] {
        let mut p = boot(&src);
        let row = id(&p, "row");
        p.pointer_down(20., 40., 0.).unwrap();
        assert!(!p.pointer_move(21., 60., 10.).unwrap());
        assert!(!p.pointer_move(130., 60., 20.).unwrap());
        p.pointer_up(130., 60., 30.).unwrap();
        assert_eq!(p.host.presented(row).translate.0, 0.);
        assert_eq!(count(&p), "0");
    }
    let mut p = boot(&source().replace("touch-action=\"pan-y\"", ""));
    p.pointer_down(20., 40., 0.).unwrap();
    assert!(!p.pointer_move(140., 40., 10.).unwrap());
}

#[test]
fn deletion_remount_and_runtime_replacement_cannot_receive_old_release() {
    let src = source();
    let mut p = boot(&src);
    recognize(&mut p);
    let token = p
        .contact
        .as_ref()
        .unwrap()
        .hold
        .as_ref()
        .unwrap()
        .primary
        .token;
    p.tap(id(&p, "hide")).unwrap();
    assert!(!p.host.has_hold(token));
    assert_eq!(p.collection_interaction(), None);
    p.tap(id(&p, "show")).unwrap();
    assert!(!p.pointer_up(150., 40., 100.).unwrap());
    assert_eq!(count(&p), "0");
    p.pointer_down(20., 40., 100.).unwrap();
    p.pointer_move(30., 40., 110.).unwrap();
    let token = p
        .contact
        .as_ref()
        .unwrap()
        .hold
        .as_ref()
        .unwrap()
        .primary
        .token;
    p.reload(&contract::compile(&src).unwrap().encode(), NoData)
        .unwrap();
    assert!(!p.host.hold_update(token, Value::ZERO, f64::NAN).unwrap());
    assert!(!p.pointer_up(150., 40., 200.).unwrap());
    assert_eq!(count(&p), "0");
}

#[test]
fn action_deletion_and_ordinary_click_release_clean_up_once() {
    let mut p = boot(&source().replace("swiperight=reply", "swiperight=hide"));
    recognize(&mut p);
    p.pointer_up(150., 40., 50.).unwrap();
    assert!(p.host.kernel().find_by_test_id("row").is_empty());
    assert_eq!(p.collection_interaction(), None);
    assert!(!p.host.motion());
    let mut p = boot(&source().replace("swiperight=reply", "press=reply"));
    p.pointer_down(20., 40., 0.).unwrap();
    p.pointer_up(20., 40., 10.).unwrap();
    assert_eq!(count(&p), "1");
    assert_eq!(p.collection_interaction(), None);
}

#[test]
fn companion_holds_and_replaced_primary_are_bounded_and_do_not_release_successor() {
    let mut p = boot(&source());
    recognize(&mut p);
    let row = id(&p, "row");
    let indicator = id(&p, "indicator");
    p.pointer_move(110., 40., 30.).unwrap();
    assert_eq!(p.host.presented(indicator).opacity, 1.);
    let newer = p
        .host
        .hold_begin(row, Property::Translate, 30.)
        .unwrap()
        .unwrap();
    assert!(!p.pointer_up(110., 40., 40.).unwrap());
    assert!(p.host.has_hold(newer.token));
    assert_eq!(count(&p), "0");
    assert!(!p.host.engine().is_held(
        exact_kernel::motion::motion_node(p.host.kernel().node(indicator).unwrap().key),
        Property::Opacity
    ));
    assert_eq!(p.collection_interaction(), None);
}

#[test]
fn agent_contact_phases_still_refuse_instead_of_claiming_hardware_delivery() {
    let mut p = boot(&source());
    let reply = crate::agent::handle(
        &mut p,
        "{\"op\":\"tap\",\"phase\":\"down\",\"x\":20,\"y\":40}",
    );
    assert!(reply.contains("unsupported"));
    assert!(p.contact.is_none());
}

#[test]
fn leftward_catch_and_reverse_use_displayed_completion_threshold() {
    let mut p = boot(&source());
    let row = id(&p, "row");
    let start = p
        .host
        .hold_begin(row, Property::Translate, 0.)
        .unwrap()
        .unwrap();
    p.host
        .hold_update(start.token, Value::new(120., 0.), 0.)
        .unwrap();
    p.host.hold_end(start.token, HoldEnd::Cancel, 0.).unwrap();
    p.tick(10.);
    let x = p.host.presented(row).translate.0 + 100.;
    p.pointer_down(x, 40., 10.).unwrap();
    p.tick(20.);
    let caught = p.host.presented(row).translate.0;
    assert!(
        p.pointer_move(x - 10., 40., 20.).unwrap(),
        "catch directly leftward"
    );
    assert!((p.host.presented(row).translate.0 - caught).abs() < 0.0001);
    // Reverse far enough to close the caught displacement: no false reply.
    p.pointer_up(x - 410., 40., 60.).unwrap();
    assert_eq!(count(&p), "0");
    p.tick(10_000.);
    // Conversely, small displacement can complete when caught presentation
    // remains past the displayed threshold; no raw pointer-down distance gate.
    let start = p
        .host
        .hold_begin(row, Property::Translate, 10_000.)
        .unwrap()
        .unwrap();
    p.host
        .hold_update(start.token, Value::new(100., 0.), 10_000.)
        .unwrap();
    p.host
        .hold_end(start.token, HoldEnd::Cancel, 10_000.)
        .unwrap();
    p.pointer_down(150., 40., 10_000.).unwrap();
    assert!(p.pointer_move(140., 40., 10_010.).unwrap());
    p.pointer_up(135., 40., 10_020.).unwrap();
    assert_eq!(count(&p), "1");
}

#[test]
fn resize_keeps_live_hold_origin_pin_and_latest_target() {
    let mut p = boot(&source());
    let row = id(&p, "row");
    recognize(&mut p);
    p.pointer_move(80., 40., 30.).unwrap();
    let before = p.host.presented(row).translate.0;
    let token = p
        .contact
        .as_ref()
        .unwrap()
        .hold
        .as_ref()
        .unwrap()
        .primary
        .token;
    assert!(p.resize(600., 700.).is_none());
    assert!(p.host.has_hold(token));
    assert_eq!(p.collection_interaction(), Some(row));
    p.pointer_move(80., 40., 40.).unwrap();
    assert_eq!(p.host.presented(row).translate.0, before);
    p.pointer_up(110., 40., 60.).unwrap();
    assert_eq!(count(&p), "1");
    assert_eq!(p.collection_interaction(), None);
    p.tick(10_000.);
    assert_eq!(p.host.presented(row).translate.0, 0.);
    assert_eq!(p.host.presented(row).opacity, 0.25);
}

#[test]
fn navigation_cancels_a_held_view_without_waiting_for_the_next_pointer_event() {
    let src = r#"component App
  state selected = "a"
  state count = 0
  action away writes selected
    selected = "b"
  action back writes selected
    selected = "a"
  action reply writes count
    count = count + 1
  view
    main navigationKey=selected navigationBack="back"
      column navigationKey="a"
        box testId="row" width=400 height=100 swiperight=reply touch-action="pan-y" transition="translate spring(300, 30, 1)"
          text "swipe me"
      column navigationKey="b"
        text "other route"
      button press=away testId="away"
        text "away"
      button press=back testId="back"
        text "back"
      text `${count}` testId="count"
"#;
    let mut p = boot(src);
    recognize(&mut p);
    let token = p
        .contact
        .as_ref()
        .unwrap()
        .hold
        .as_ref()
        .unwrap()
        .primary
        .token;
    p.pointer_move(150., 40., 30.).unwrap();
    assert!(p
        .host
        .dispatch_at(id(&p, "away"), Event::Press, 30.)
        .is_none());
    assert!(p.after_commit().is_none());
    assert!(!p.host.has_hold(token));
    assert!(p.contact.is_none());
    assert_eq!(p.collection_interaction(), None);
    p.host.dispatch_at(id(&p, "back"), Event::Press, 30.);
    p.after_commit();
    assert!(!p.pointer_up(150., 40., 50.).unwrap());
    assert_eq!(count(&p), "0");
}

#[test]
fn displayed_velocity_and_companion_catch_are_continuous_past_resistance_knee() {
    let mut p = boot(&source());
    let row = id(&p, "row");
    let companion = id(&p, "indicator");
    for (view, property, value) in [
        (row, Property::Translate, Value::new(80., 0.)),
        (companion, Property::Opacity, Value::scalar(0.4)),
        (companion, Property::Scale, Value::scalar(0.4)),
    ] {
        let token = p
            .host
            .hold_begin(view, property, 0.)
            .unwrap()
            .unwrap()
            .token;
        p.host.hold_update(token, value, 0.).unwrap();
        p.host.hold_end(token, HoldEnd::Cancel, 0.).unwrap();
    }
    p.tick(10.);
    let x = p.host.presented(row).translate.0 + 20.;
    let before = p.host.presented(companion);
    p.pointer_down(x, 40., 10.).unwrap();
    p.pointer_move(x + 10., 40., 10.).unwrap();
    assert_eq!(p.host.presented(companion).opacity, before.opacity);
    assert_eq!(p.host.presented(companion).scale, before.scale);
    p.pointer_move(x + 30., 40., 30.).unwrap();
    let held = p.contact.as_ref().unwrap().hold.as_ref().unwrap();
    let velocity = held.velocity.estimate(0.03).x;
    assert!(
        (velocity - 200.).abs() < 0.01,
        "displayed velocity={velocity}, not raw1000px/s"
    );
    p.pointer_cancel(30.).unwrap();
}

#[test]
fn malformed_pointer_samples_preserve_contact_and_presentation() {
    let mut p = boot(&source());
    recognize(&mut p);
    let row = id(&p, "row");
    let token = p
        .contact
        .as_ref()
        .unwrap()
        .hold
        .as_ref()
        .unwrap()
        .primary
        .token;
    for (x, time) in [(f32::NAN, 100.), (80., 9.), (80., f64::INFINITY)] {
        assert!(p.pointer_move(x, 40., time).is_err());
        assert!(p.host.has_hold(token));
        assert_eq!(p.host.now(), 10.);
        assert_eq!(p.host.presented(row).translate.0, 0.);
    }
    p.pointer_cancel(10.).unwrap();
}
