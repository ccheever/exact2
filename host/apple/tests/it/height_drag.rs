//! Authored header gestures use resolved generational bindings and typed release.
use exact_apple::Host;
use exact_kernel::{motion::motion_node, MonospaceMeasurer, NodeKey};
use exact_motion::{Property, Value};
use exact_runner::{DataError, DataSource, Event};

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
fn fixture() -> (Host<NoData>, String) {
    let plan = contract::compile(r#"component App
  state size = 360
  state disabled = false
  state binding = "sheet"
  state showing = true
  state releases = 0
  action release(height: number, velocity: number)
    size = height < 270 ? 180 : height < 500 ? 360 : 640
    releases = releases + 1
  action disable
    disabled = true
  action rebind
    binding = "other"
  action remove
    showing = false
  view
    column width="100%" height="100%"
      button testId="disable" press=disable
        text "disable"
      button testId="rebind" press=rebind
        text "rebind"
      button testId="remove" press=remove
        text "remove"
      text `${releases}` testId="releases"
      when showing
        column id="sheet" testId="sheet" height=size box-sizing="border-box" transition="height spring(180, 12, 1)"
          box testId="header" heightDragFor=binding heightrelease=release disabled=disabled height=32
          box testId="second-header" heightDragFor="sheet" heightrelease=release height=16
          scroll testId="inner" flex=1 min-height=0
            text "scroll remains independent" height=2000
      column id="other" testId="other" height=180 box-sizing="border-box"
        box testId="other-header" heightDragFor="other" heightrelease=release height=32
"#).unwrap().encode();
    Host::boot(
        &plan,
        NoData,
        Box::new(MonospaceMeasurer::default()),
        800.,
        900.,
    )
    .unwrap()
}
fn key(h: &Host<NoData>, name: &str) -> NodeKey {
    h.runner().kernel().find_by_test_id(name)[0]
}
fn id(h: &Host<NoData>, name: &str) -> u32 {
    h.runner().kernel().node_by_key(key(h, name)).unwrap().id
}
fn good(batch: &str) {
    assert!(batch.contains("\"error\":null"), "{batch}");
}
fn token(batch: &str) -> u64 {
    good(batch);
    batch
        .split("\"token\":\"")
        .nth(1)
        .expect(batch)
        .split('"')
        .next()
        .unwrap()
        .parse()
        .unwrap()
}
fn begin(h: &mut Host<NoData>, header: &str) -> u64 {
    token(&h.height_drag_begin(key(h, header), key(h, "sheet"), 0.))
}
#[test]
fn boot_registers_one_target_exports_resolved_keys_and_refuses_second_target() {
    let (mut h, batch) = fixture();
    assert_eq!(h.height_owner(), Some(key(&h, "sheet")));
    for header in ["header", "second-header"] {
        let expected = format!(
            "\"op\":\"height-drag\",\"id\":{},\"target\":{}",
            id(&h, header),
            id(&h, "sheet")
        );
        assert!(batch.contains(&expected), "{batch}");
    }
    let first = begin(&mut h, "header");
    let refused = h.height_drag_begin(key(&h, "other-header"), key(&h, "other"), 100.);
    assert!(!refused.contains("\"token\""));
    assert!(h.has_hold(first));
    assert_eq!(h.engine().now(), 0.);
    assert_eq!(h.height_owner(), Some(key(&h, "sheet")));
}
#[test]
fn final_sample_and_typed_snap_action_run_while_held_then_release_once() {
    let (mut h, _) = fixture();
    let hold = begin(&mut h, "header");
    let owner = motion_node(key(&h, "sheet"));
    good(&h.dispatch_height_held(hold, 540., 600., 10.));
    assert!(h.has_hold(hold));
    assert_eq!(
        h.engine().value(owner, Property::Height),
        Some(Value::scalar(540.))
    );
    assert_eq!(
        h.engine().target(owner, Property::Height),
        Some(Value::scalar(640.))
    );
    good(&h.dispatch_height_held(hold, 200., 0., 100.));
    assert_eq!(h.engine().now(), 0.01);
    assert_eq!(
        h.engine().target(owner, Property::Height),
        Some(Value::scalar(640.))
    );
    // End after dispatch must not dispatch the same release action again.
    good(&h.hold_end(
        hold,
        exact_motion::HoldEnd::Release {
            velocity: Value::scalar(600.),
        },
        10.,
    ));
    assert!(!h.has_hold(hold));
    good(&h.dispatch_height_held(hold, f64::NAN, f64::NAN, f64::NAN));
    assert_eq!(h.engine().now(), 0.01);
}
#[test]
fn changed_handle_binding_and_disabled_header_cancel_before_late_action() {
    for action in ["rebind", "disable"] {
        let (mut h, _) = fixture();
        let hold = begin(&mut h, "header");
        good(&h.height_drag_update(hold, 450., 10.));
        good(&h.dispatch_at(id(&h, action), Event::Press, 10.));
        assert!(!h.has_hold(hold));
        assert_eq!(h.height_owner(), Some(key(&h, "sheet"))); // second handle survives
        good(&h.dispatch_height_held(hold, 600., 100., 500.));
        assert_eq!(h.engine().now(), 0.01);
        assert_eq!(
            h.engine()
                .target(motion_node(key(&h, "sheet")), Property::Height),
            Some(Value::scalar(360.))
        );
    }
}
#[test]
fn stale_generation_and_invalid_live_payload_preflight_before_clock_or_action() {
    let (mut h, _) = fixture();
    let hold = begin(&mut h, "header");
    for (height, velocity) in [(-1., 0.), (f64::MAX, 0.), (200., f64::NAN)] {
        assert!(!h
            .dispatch_height_held(hold, height, velocity, 100.)
            .contains("\"error\":null"));
        assert!(h.has_hold(hold));
        assert_eq!(h.engine().now(), 0.);
    }
    let header = key(&h, "header");
    let target = key(&h, "sheet");
    good(&h.dispatch_at(id(&h, "remove"), Event::Press, 0.));
    assert!(!h.has_hold(hold));
    good(&h.height_drag_begin(header, target, f64::NAN));
    good(&h.height_drag_update(hold, f64::NAN, f64::NAN));
    good(&h.dispatch_height_held(hold, f64::NAN, f64::NAN, f64::NAN));
    assert_eq!(h.engine().now(), 0.);
}

#[test]
fn programmatic_registration_without_authored_handles_is_not_cleared() {
    let plan = contract::compile(
        r#"component App
  state counter = 0
  action increment
    counter = counter + 1
  view
    box testId="panel" height=180 box-sizing="border-box" press=increment
      text `${counter}`
"#,
    )
    .unwrap()
    .encode();
    let (mut h, _) = Host::boot(
        &plan,
        NoData,
        Box::new(MonospaceMeasurer::default()),
        400.,
        800.,
    )
    .unwrap();
    let panel = id(&h, "panel");
    h.set_height_owner(Some(panel)).unwrap();
    good(&h.dispatch_at(panel, Event::Press, 10.));
    assert_eq!(h.height_owner(), Some(key(&h, "panel")));
}

#[test]
fn catch_and_release_use_constrained_height_and_action_deletion_makes_end_stale() {
    let plan = contract::compile(
        r#"component App
  state showing = true
  state seen = 0
  action release(height: number, velocity: number)
    seen = height
    showing = false
  view
    column
      text `${seen}` testId="seen"
      when showing
        box id="sheet" testId="sheet" height=600 max-height=240 box-sizing="border-box"
          box testId="header" heightDragFor="sheet" heightrelease=release
"#,
    )
    .unwrap()
    .encode();
    let (mut h, _) = Host::boot(
        &plan,
        NoData,
        Box::new(MonospaceMeasurer::default()),
        400.,
        800.,
    )
    .unwrap();
    let out = h.height_drag_begin(key(&h, "header"), key(&h, "sheet"), 0.);
    assert!(out.contains("\"x\":240"), "{out}");
    let held = token(&out);
    good(&h.dispatch_height_held(held, 500., -200., 10.));
    let kernel = h.runner().kernel();
    let text = kernel.node_by_key(key(&h, "seen")).unwrap();
    assert_eq!(text.props.str(exact_kernel::PropId::Text), Some("240"));
    assert!(!h.has_hold(held));
    assert_eq!(h.height_owner(), None);
    good(&h.hold_end(
        held,
        exact_motion::HoldEnd::Release {
            velocity: Value::scalar(-200.),
        },
        f64::NAN,
    ));
    assert_eq!(h.engine().now(), 0.01);
}

#[test]
fn accepted_unbind_cancels_at_receipt_time_while_stale_deliveries_cannot_seek() {
    let (mut h, _) = fixture();
    let header = key(&h, "header");
    let target = key(&h, "sheet");
    let hold = token(&h.height_drag_begin(header, target, 100.));
    good(&h.height_drag_update(hold, 450., 100.));
    good(&h.dispatch_at(id(&h, "rebind"), Event::Press, 200.));
    assert_eq!(h.height_owner(), Some(target)); // the second header still owns the target
    assert!(!h.has_hold(hold));
    let curve = h
        .engine()
        .spring_descriptor(motion_node(target), Property::Height)
        .unwrap();
    assert_eq!(curve.start, 0.2);
    assert_eq!(curve.from, Value::scalar(450.));
    assert_eq!(curve.target, Value::scalar(360.));
    assert_eq!(curve.velocity, Value::ZERO);
    assert_eq!(
        h.engine().value(motion_node(target), Property::Height),
        Some(Value::scalar(450.))
    );
    good(&h.height_drag_update(hold, 600., 500.));
    good(&h.dispatch_height_held(hold, 600., 200., 500.));
    good(&h.height_drag_begin(header, target, f64::NAN));
    assert_eq!(h.engine().now(), 0.2);
    assert_eq!(
        h.engine()
            .spring_descriptor(motion_node(target), Property::Height),
        Some(curve)
    );
}

#[test]
fn unbind_uses_latest_none_or_zero_transition_before_cancelling_unchanged_target() {
    for declaration in ["none", "height 0ms linear"] {
        let source = format!(
            r#"component App
  state binding = "sheet"
  state declaration = "height spring(180, 12, 1)"
  action release(height: number, velocity: number)
    binding = binding
  action unbind
    binding = ""
    declaration = "{declaration}"
  view
    column
      button testId="unbind" press=unbind
        text "unbind"
      box id="sheet" testId="sheet" height=360 box-sizing="border-box" transition=declaration
        box testId="header" heightDragFor=binding heightrelease=release
        box heightDragFor="sheet" heightrelease=release
"#
        );
        let plan = contract::compile(&source).unwrap().encode();
        let (mut h, _) = Host::boot(
            &plan,
            NoData,
            Box::new(MonospaceMeasurer::default()),
            400.,
            800.,
        )
        .unwrap();
        let target = key(&h, "sheet");
        let held = token(&h.height_drag_begin(key(&h, "header"), target, 100.));
        good(&h.height_drag_update(held, 450., 100.));
        good(&h.dispatch_at(id(&h, "unbind"), Event::Press, 200.));
        assert_eq!(h.height_owner(), Some(target));
        assert!(!h.has_hold(held));
        assert_eq!(h.engine().now(), 0.2);
        assert_eq!(
            h.engine().value(motion_node(target), Property::Height),
            Some(Value::scalar(360.))
        );
        assert!(h
            .engine()
            .spring_descriptor(motion_node(target), Property::Height)
            .is_none());
        assert!(h.engine().quiescent());
    }
}

#[test]
fn unbind_with_retarget_and_negative_delay_starts_from_held_value_once() {
    let plan = contract::compile(r#"component App
  state binding = "sheet"
  state height = 360
  action release(px: number, velocity: number)
    height = px
  action unbind
    binding = ""
    height = 600
  view
    column
      button testId="unbind" press=unbind
        text "unbind"
      box id="sheet" testId="sheet" height=height box-sizing="border-box" transition="height 1000ms linear -100ms"
        box testId="header" heightDragFor=binding heightrelease=release
        box heightDragFor="sheet" heightrelease=release
"#).unwrap().encode();
    let (mut h, _) = Host::boot(
        &plan,
        NoData,
        Box::new(MonospaceMeasurer::default()),
        400.,
        800.,
    )
    .unwrap();
    let target = key(&h, "sheet");
    let held = token(&h.height_drag_begin(key(&h, "header"), target, 100.));
    good(&h.height_drag_update(held, 450., 100.));
    good(&h.dispatch_at(id(&h, "unbind"), Event::Press, 200.));
    assert!(!h.has_hold(held));
    assert_eq!(h.engine().now(), 0.2);
    assert_eq!(
        h.engine().target(motion_node(target), Property::Height),
        Some(Value::scalar(600.))
    );
    // One release from 450 toward the newly authored 600, sampled 100ms
    // into its 1000ms transition. Never start an old-target return first.
    let actual = h
        .engine()
        .value(motion_node(target), Property::Height)
        .unwrap()
        .x;
    assert!(
        (actual - 465.).abs() < 0.000001,
        "sampled {actual}, expected 465"
    );
}

fn ownership_fixture() -> Host<NoData> {
    let plan = contract::compile(
        r#"component App
  state binding = "sheet"
  state count = 0
  action release(height: number, velocity: number)
    count = count + 1
  action ping
    count = count + 1
  action unbind
    binding = ""
  view
    column
      button testId="ping" press=ping
        text `${count}`
      button testId="unbind" press=unbind
        text "unbind"
      box id="sheet" testId="sheet" height=360 box-sizing="border-box"
        box testId="header" heightDragFor=binding heightrelease=release height=32
      box testId="explicit" height=180 box-sizing="border-box"
"#,
    )
    .unwrap()
    .encode();
    Host::boot(
        &plan,
        NoData,
        Box::new(MonospaceMeasurer::default()),
        800.,
        900.,
    )
    .unwrap()
    .0
}
fn retired_binding(batch: &str, header: u32) {
    assert!(
        batch.contains(&format!(
            "\"op\":\"height-drag\",\"id\":{header},\"target\":null"
        )),
        "{batch}"
    );
}
#[test]
fn different_explicit_owner_retires_export_and_survives_receipt_with_hold() {
    let mut h = ownership_fixture();
    let old = begin(&mut h, "header");
    let explicit = key(&h, "explicit");
    let change = h.set_height_owner(Some(id(&h, "explicit"))).unwrap();
    good(&change.batch);
    assert_eq!(h.height_owner(), Some(explicit));
    assert!(!h.has_hold(old));
    let held = token(&h.hold_begin(id(&h, "explicit"), Property::Height, 100.));
    good(&h.hold_update(held, Value::scalar(230.), 100.));
    good(&h.dispatch_at(id(&h, "ping"), Event::Press, 200.));
    assert_eq!(h.height_owner(), Some(explicit));
    retired_binding(&change.batch, id(&h, "header"));
    assert!(h.has_hold(held));
    assert_eq!(
        h.engine().value(motion_node(explicit), Property::Height),
        Some(Value::scalar(230.))
    );
    good(&h.dispatch_at(id(&h, "unbind"), Event::Press, 300.));
    assert_eq!(h.height_owner(), Some(explicit));
    assert!(h.has_hold(held));
}
#[test]
fn refused_explicit_owner_preserves_auto_provenance_hold_and_clock() {
    let mut h = ownership_fixture();
    let owner = key(&h, "sheet");
    let held = token(&h.height_drag_begin(key(&h, "header"), owner, 100.));
    for invalid in [u32::MAX, id(&h, "header")] {
        assert!(h.set_height_owner(Some(invalid)).is_err());
        assert_eq!(h.height_owner(), Some(owner));
        assert!(h.has_hold(held));
        assert_eq!(h.engine().now(), 0.1);
    }
    good(&h.dispatch_at(id(&h, "unbind"), Event::Press, 200.));
    assert_eq!(h.height_owner(), None);
    assert!(!h.has_hold(held));
}
#[test]
fn same_live_owner_remains_noop_and_preserves_automatic_cleanup() {
    let mut h = ownership_fixture();
    let held = begin(&mut h, "header");
    let change = h.set_height_owner(Some(id(&h, "sheet"))).unwrap();
    good(&change.batch);
    assert!(!change.batch.contains("height-drag"));
    assert!(h.has_hold(held));
    good(&h.dispatch_at(id(&h, "unbind"), Event::Press, 200.));
    assert_eq!(h.height_owner(), None);
    assert!(!h.has_hold(held));
}
#[test]
fn explicit_none_clears_now_and_receipt_can_readopt_authored_handle() {
    let mut h = ownership_fixture();
    let held = begin(&mut h, "header");
    let change = h.set_height_owner(None).unwrap();
    good(&change.batch);
    assert_eq!(h.height_owner(), None);
    assert!(!h.has_hold(held));
    retired_binding(&change.batch, id(&h, "header"));
    good(&h.dispatch_at(id(&h, "ping"), Event::Press, 200.));
    assert_eq!(h.height_owner(), Some(key(&h, "sheet")));
}

#[test]
fn prop_only_target_cannot_preempt_a_real_release_handle() {
    let plan = contract::compile(
        r#"component App
  state released = 0
  action release(height: number, velocity: number)
    released = released + 1
  view
    column
      box id="A" testId="A" height=200 box-sizing="border-box"
        box testId="prop-only" heightDragFor="A"
      box id="B" testId="B" height=200 box-sizing="border-box"
        box testId="real" heightDragFor="B" heightrelease=release
"#,
    )
    .unwrap()
    .encode();
    let (mut h, batch) = Host::boot(
        &plan,
        NoData,
        Box::new(MonospaceMeasurer::default()),
        800.,
        900.,
    )
    .unwrap();
    assert_eq!(h.height_owner(), Some(key(&h, "B")));
    assert!(!batch.contains(&format!(
        "\"op\":\"height-drag\",\"id\":{},",
        id(&h, "prop-only")
    )));
    let held = token(&h.height_drag_begin(key(&h, "real"), key(&h, "B"), 0.));
    good(&h.dispatch_height_held(held, 210., 0., 10.));
}
#[test]
fn release_handler_survives_empty_set_clear_set_idref() {
    let plan = contract::compile(
        r#"component App
  state enabled = false
  state releases = 0
  action toggle
    enabled = !enabled
  action release(height: number, velocity: number)
    releases = releases + 1
  view
    column
      button testId="toggle" press=toggle
        text "toggle"
      box id="sheet" testId="sheet" height=200 box-sizing="border-box"
        box testId="header" heightDragFor=(enabled ? "sheet" : "") heightrelease=release
"#,
    )
    .unwrap()
    .encode();
    let (mut h, _) = Host::boot(
        &plan,
        NoData,
        Box::new(MonospaceMeasurer::default()),
        800.,
        900.,
    )
    .unwrap();
    let header = key(&h, "header");
    assert_eq!(h.height_owner(), None);
    for expected in [true, false, true] {
        good(&h.dispatch_at(id(&h, "toggle"), Event::Press, 0.));
        assert_eq!(key(&h, "header"), header);
        assert_eq!(h.height_owner(), expected.then(|| key(&h, "sheet")));
        if expected {
            let held = begin(&mut h, "header");
            good(&h.hold_end(held, exact_motion::HoldEnd::Cancel, 0.));
        }
    }
}

/// LLP 1057.001 §3: the bridge's release velocity is the engine's, over the
/// heights actually shown: a `max-height` stops the measured motion.
#[test]
fn measured_release_velocity_follows_the_constrained_height_shown() {
    let velocity = |max: &str| {
        let plan = contract::compile(&format!(
            r#"component App
  state size = 200
  state seen = 0
  action release(height: number, velocity: number)
    seen = velocity
  view
    column id="sheet" testId="sheet" height=size {max} box-sizing="border-box" transition="height spring(180, 12, 1)"
      box testId="header" heightDragFor="sheet" heightrelease=release height=32
      text `${{seen}}` testId="seen"
"#
        ))
        .unwrap()
        .encode();
        let (mut h, _) = Host::boot(
            &plan,
            NoData,
            Box::new(MonospaceMeasurer::default()),
            800.,
            900.,
        )
        .unwrap();
        let hold = begin(&mut h, "header");
        for i in 1..=4 {
            good(&h.height_drag_update(hold, 200. + 20. * i as f64, 10. * i as f64));
        }
        good(&h.dispatch_height_measured(hold, 280., 40.));
        let Some(exact_runner::Value::Number(v)) = h.runner().slot("seen") else {
            panic!()
        };
        *v
    };
    let free = velocity("");
    assert!((free - 2000.).abs() < 1e-6, "{free}");
    let clamped = velocity("max-height=230");
    assert!(
        clamped < free / 2.,
        "the shown height stopped at 230: {clamped}"
    );
}
