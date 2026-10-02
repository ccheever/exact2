//! Real typed actions and generation-bound authored height handles.
use exact_kernel::{motion::motion_node, Dimension, MonospaceMeasurer, NodeKey, PropId};
use exact_linux::Host;
use exact_motion::{HoldEnd, Property, Value};
use exact_runner::{DataError, DataSource, Event};

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
  state target = 180
  state releases = 0
  state seenHeight = 0
  state seenVelocity = 0
  state binding = "panel"
  state first = true
  state second = true
  state disabled = false
  action release(height: number, velocity: number)
    seenHeight = height
    seenVelocity = velocity
    releases = releases + 1
    target = height < 270 ? 180 : height < 500 ? 360 : 640
  action unbind
    binding = ""
  action rebind
    binding = "other"
  action remove
    first = false
  action restore
    first = true
  action disable
    disabled = true
  action enable
    disabled = false
  action clear
    binding = ""
    second = false
  view
    box width=400 height=500 disabled=disabled
      button testId="unbind" press=unbind
        text "unbind"
      button testId="rebind" press=rebind
        text "rebind"
      button testId="remove" press=remove
        text "remove"
      button testId="restore" press=restore
        text "restore"
      button testId="disable" press=disable
        text "disable"
      button testId="enable" press=enable
        text "enable"
      button testId="clear" press=clear
        text "clear"
      box id="panel" testId="panel" position="absolute" bottom=0 width=400 height=target max-height="100%" box-sizing="border-box" transition="height spring(300,30,1)"
        when first
          box testId="first" heightDragFor=binding heightrelease=release height=32 touch-action="none"
            text "drag here"
        when second
          box testId="second" heightDragFor="panel" heightrelease=release height=32 touch-action="none"
            text "or here"
      box id="other" testId="other" height=100 box-sizing="border-box"
        box testId="other-handle" heightDragFor="other" heightrelease=release height=32 touch-action="none"
      text `${releases}` testId="releases"
      text `${seenHeight}` testId="seenHeight"
      text `${seenVelocity}` testId="seenVelocity"
"#;
fn boot(source: &str) -> Host<Empty> {
    let (h, error) = Host::boot(
        &contract::compile(source).unwrap().encode(),
        Empty,
        Box::new(MonospaceMeasurer::default()),
        400.,
        500.,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    h
}
fn key(h: &Host<Empty>, name: &str) -> NodeKey {
    h.kernel().find_by_test_id(name)[0]
}
fn id(h: &Host<Empty>, name: &str) -> u32 {
    h.kernel().node_by_key(key(h, name)).unwrap().id
}
fn press(h: &mut Host<Empty>, name: &str, now: f64) {
    assert!(h.dispatch_at(id(h, name), Event::Press, now).is_none());
}
fn text<'a>(h: &'a Host<Empty>, name: &str) -> &'a str {
    h.kernel()
        .node_by_key(key(h, name))
        .unwrap()
        .props
        .str(PropId::Text)
        .unwrap()
}

#[test]
fn boot_adopts_first_target_multiple_handles_share_and_second_target_cannot_replace_it() {
    let mut h = boot(APP);
    let target = key(&h, "panel");
    let first = key(&h, "first");
    let second = key(&h, "second");
    assert_eq!(h.height_owner(), Some(target));
    assert_eq!(h.height_drag_target(first), Some(target));
    assert_eq!(h.height_drag_target(second), Some(target));
    assert!(h.height_drag_target(key(&h, "other-handle")).is_none());
    let old = h.height_drag_begin(first, target, 0.).unwrap().unwrap();
    let before = h.kernel().export(None).unwrap();
    assert!(h
        .height_drag_begin(key(&h, "other-handle"), key(&h, "other"), f64::NAN)
        .unwrap()
        .is_none());
    assert_eq!(h.height_owner(), Some(target));
    assert!(h.has_hold(old.token));
    assert_eq!(h.kernel().export(None).unwrap(), before);
    let new = h.height_drag_begin(second, target, 0.).unwrap().unwrap();
    assert!(!h.has_hold(old.token));
    assert!(!h
        .dispatch_height_held(old.token, first, f64::NAN, f64::NAN, f64::NAN)
        .unwrap());
    assert!(!h
        .dispatch_height_held(new.token, first, 300., 0., 1.)
        .unwrap());
    assert_eq!(h.now(), 0.);
    assert!(h.has_hold(new.token));
    assert_eq!(text(&h, "releases"), "0");
}

#[test]
fn final_sample_dispatches_typed_action_while_held_then_explicit_release_uses_latest_target() {
    let mut h = boot(APP);
    let handle = key(&h, "first");
    let target = key(&h, "panel");
    let held = h.height_drag_begin(handle, target, 0.).unwrap().unwrap();
    assert!(h
        .dispatch_height_held(held.token, handle, 300., 125., 10.)
        .unwrap());
    assert_eq!(text(&h, "releases"), "1");
    assert_eq!(text(&h, "seenHeight"), "300");
    assert_eq!(text(&h, "seenVelocity"), "125");
    assert!(h.has_hold(held.token));
    assert!(!h
        .dispatch_height_held(held.token, handle, 400., 0., 20.)
        .unwrap());
    assert_eq!(h.now(), 10.);
    assert_eq!(text(&h, "releases"), "1");
    assert_eq!(
        h.kernel().node_by_key(target).unwrap().style.height,
        Dimension::Points(360.)
    );
    assert_eq!(h.kernel().node_by_key(target).unwrap().frame.height, 300.);
    assert_eq!(
        h.engine().target(motion_node(target), Property::Height),
        Some(Value::scalar(360.))
    );
    h.hold_end(
        held.token,
        HoldEnd::Release {
            velocity: Value::scalar(125.),
        },
        10.,
    )
    .unwrap();
    assert!(!h
        .dispatch_height_held(held.token, handle, 400., 0., 20.)
        .unwrap());
    h.tick(10_000.);
    assert_eq!(h.kernel().node_by_key(target).unwrap().frame.height, 360.);
}

#[test]
fn handle_rebind_disable_or_delete_cancels_without_dispatch_even_if_target_stays_numeric() {
    for action in ["rebind", "unbind", "remove", "disable"] {
        let mut h = boot(APP);
        let handle = key(&h, "first");
        let target = key(&h, "panel");
        let held = h.height_drag_begin(handle, target, 0.).unwrap().unwrap();
        h.hold_update(held.token, Value::scalar(300.), 0.).unwrap();
        press(&mut h, action, 1.);
        assert!(!h.has_hold(held.token), "{action}");
        assert!(!h
            .dispatch_height_held(held.token, handle, f64::NAN, f64::NAN, f64::NAN)
            .unwrap());
        assert_eq!(text(&h, "releases"), "0");
        assert_eq!(h.now(), 1.);
        assert!(h.kernel().node_by_key(target).is_some());
    }
}

#[test]
fn last_authored_handle_clears_auto_registration_but_programmatic_only_owner_survives() {
    let source = APP.replace("heightDragFor=\"other\"", "heightDragFor=\"\"");
    let mut h = boot(&source);
    assert!(h.height_owner().is_some());
    press(&mut h, "clear", 0.);
    assert!(h.height_owner().is_none());
    let mut p = boot(
        "component App\n  view\n    box testId=\"panel\" height=180 box-sizing=\"border-box\"\n",
    );
    let panel = key(&p, "panel");
    p.set_height_owner(Some(id(&p, "panel"))).unwrap();
    assert!(p.advance(10.).is_none());
    assert!(p.resize(400., 400.).is_none());
    assert_eq!(p.height_owner(), Some(panel));
}

#[test]
fn stale_handle_generation_foreign_token_and_malformed_live_release_are_atomic() {
    let mut h = boot(APP);
    let target = key(&h, "panel");
    let old = key(&h, "first");
    press(&mut h, "remove", 0.);
    press(&mut h, "restore", 0.);
    let handle = key(&h, "first");
    assert_ne!(old, handle);
    assert!(h
        .height_drag_begin(old, target, f64::NAN)
        .unwrap()
        .is_none());
    let held = h.height_drag_begin(handle, target, 0.).unwrap().unwrap();
    let before = h.kernel().export(None).unwrap();
    for (height, velocity, now) in [
        (-1., 0., 1.),
        (f64::MAX, 0., 1.),
        (300., f64::NAN, 1.),
        (300., 0., -1.),
    ] {
        assert!(h
            .dispatch_height_held(held.token, handle, height, velocity, now)
            .is_err());
        assert_eq!(h.now(), 0.);
        assert_eq!(h.kernel().export(None).unwrap(), before);
        assert!(h.has_hold(held.token));
    }
    let mut replacement = boot(APP);
    assert!(!replacement
        .dispatch_height_held(held.token, handle, f64::NAN, f64::NAN, f64::NAN)
        .unwrap());
}

#[test]
fn release_action_deleting_handle_makes_subsequent_end_stale() {
    let source = APP
        .replace(
            "writes target, releases, seenHeight, seenVelocity",
            "writes target, releases, seenHeight, seenVelocity, first",
        )
        .replace(
            "    releases = releases + 1",
            "    releases = releases + 1\n    first = false",
        );
    let mut h = boot(&source);
    let target = key(&h, "panel");
    let handle = key(&h, "first");
    let held = h.height_drag_begin(handle, target, 0.).unwrap().unwrap();
    assert!(h
        .dispatch_height_held(held.token, handle, 300., -50., 1.)
        .unwrap());
    assert_eq!(text(&h, "releases"), "1");
    assert!(!h.has_hold(held.token));
    assert!(!h.hold_end(held.token, HoldEnd::Cancel, f64::NAN).unwrap());
}

#[test]
fn delivered_async_receipt_revalidates_untouched_target_before_late_release() {
    use exact_runner::{Answer, Outcome, Request, Response, Store};
    struct Deferred;
    impl DataSource for Deferred {
        fn query(
            &mut self,
            _: &str,
            _: &[exact_runner::Value],
        ) -> Result<exact_runner::Value, DataError> {
            unreachable!()
        }
        fn answer(
            &mut self,
            _: &mut Store,
            _: &str,
            _: &[exact_runner::Value],
        ) -> Result<Answer, DataError> {
            Ok(Answer::Later(Request::get(
                "http://127.0.0.1/height-binding-test",
            )))
        }
        fn parse(
            &mut self,
            _: &mut Store,
            _: &str,
            _: &[exact_runner::Value],
            outcome: Outcome,
        ) -> Result<Answer, DataError> {
            assert!(matches!(outcome, Outcome::Response(_)));
            Ok(Answer::Now(exact_runner::Value::Bool(true)))
        }
    }
    let source = r#"component App
  mutation permission as shape bool
  state count = 0
  derive disabled = match permission { case some(value) => value, case none => false }
  action check
    send permission = check()
  action release(height: number, velocity: number)
    count = count + 1
  view
    box width=400 height=500
      button press=check testId="check"
        text "check"
      box disabled=disabled
        box id="panel" testId="panel" height=180 box-sizing="border-box"
          box heightDragFor="panel" heightrelease=release testId="handle" height=32
      text `${count}` testId="count"
"#;
    let (mut h, error) = Host::boot(
        &contract::compile(source).unwrap().encode(),
        Deferred,
        Box::new(MonospaceMeasurer::default()),
        400.,
        500.,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    let panel = h.kernel().find_by_test_id("panel")[0];
    let handle = h.kernel().find_by_test_id("handle")[0];
    let check = h
        .kernel()
        .node_by_key(h.kernel().find_by_test_id("check")[0])
        .unwrap()
        .id;
    assert!(h.dispatch_at(check, Event::Press, 0.).is_none());
    let ticket = h.take_requests().pop().unwrap().ticket;
    let held = h.height_drag_begin(handle, panel, 0.).unwrap().unwrap();
    h.hold_update(held.token, Value::scalar(250.), 10.).unwrap();
    // No executor/network needed: deliver the real pending Request's outcome.
    assert!(h
        .fulfill_all(
            vec![(
                ticket,
                Outcome::Response(Response {
                    status: 200,
                    headers: vec![],
                    body: vec![],
                }),
                None,
            )],
            20.
        )
        .is_none());
    assert_eq!(
        h.kernel().node_by_key(panel).unwrap().style.height,
        Dimension::Points(180.)
    );
    assert!(!h.has_hold(held.token));
    assert!(!h
        .dispatch_height_held(held.token, handle, f64::NAN, f64::NAN, f64::NAN)
        .unwrap());
    assert_eq!(h.now(), 20.);
    assert_eq!(
        h.kernel()
            .node_by_key(h.kernel().find_by_test_id("count")[0])
            .unwrap()
            .props
            .str(PropId::Text),
        Some("0")
    );
}

#[test]
fn invalidated_handle_cancels_at_receipt_time_while_second_handle_keeps_owner() {
    let mut h = boot(APP);
    let target = key(&h, "panel");
    let first = key(&h, "first");
    let second = key(&h, "second");
    let held = h.height_drag_begin(first, target, 100.).unwrap().unwrap();
    h.hold_update(held.token, Value::scalar(300.), 100.)
        .unwrap();
    press(&mut h, "unbind", 200.);
    assert_eq!(h.height_owner(), Some(target));
    assert_eq!(h.height_drag_target(second), Some(target));
    assert!(!h.has_hold(held.token));
    let returning = h
        .engine()
        .spring_descriptor(motion_node(target), Property::Height)
        .unwrap();
    assert_eq!(
        returning.start, 0.2,
        "cancel must not start at old hold clock0.1"
    );
    assert_eq!(returning.from, Value::scalar(300.));
    assert_eq!(h.kernel().node_by_key(target).unwrap().frame.height, 300.);
    assert!(!h
        .dispatch_height_held(held.token, first, 300., 0., 500.)
        .unwrap());
    assert_eq!(h.now(), 200.);
    assert_eq!(
        h.engine()
            .spring_descriptor(motion_node(target), Property::Height),
        Some(returning)
    );
}

#[test]
fn invalidation_uses_same_receipt_transition_and_latest_target_before_cancelling() {
    for transition in ["none", "height 0ms linear"] {
        for target_px in [180, 360] {
            let source = APP
                .replace("  state target = 180", "  state target = 180\n  state motion = \"height spring(300,30,1)\"")
                .replace("transition=\"height spring(300,30,1)\"", "transition=motion")
                .replace("  action unbind\n    binding = \"\"",
                    &format!("  action unbind\n    binding = \"\"\n    motion = \"{transition}\"\n    target = {target_px}"));
            let mut h = boot(&source);
            let target = key(&h, "panel");
            let first = key(&h, "first");
            let second = key(&h, "second");
            let held = h.height_drag_begin(first, target, 100.).unwrap().unwrap();
            h.hold_update(held.token, Value::scalar(300.), 100.)
                .unwrap();
            press(&mut h, "unbind", 200.);
            assert_eq!(h.height_drag_target(second), Some(target));
            assert!(!h.has_hold(held.token));
            assert_eq!(h.engine().now(), 0.2);
            assert!(
                !h.engine().is_active(motion_node(target), Property::Height),
                "{transition}/{target_px}"
            );
            assert_eq!(
                h.engine().value(motion_node(target), Property::Height),
                Some(Value::scalar(target_px as f64))
            );
            assert_eq!(
                h.kernel().node_by_key(target).unwrap().frame.height,
                target_px as f32
            );
        }
    }
}

#[test]
fn temporary_authored_handle_does_not_take_ownership_of_programmatic_registration() {
    let source = APP
        .replace("state binding = \"panel\"", "state binding = \"\"")
        .replace("state second = true", "state second = false")
        .replace("heightDragFor=\"other\"", "heightDragFor=\"\"")
        .replace(
            "  action restore\n    first = true",
            "  action restore\n    first = true\n    binding = \"panel\"",
        );
    let mut h = boot(&source);
    let panel = key(&h, "panel");
    assert!(h.height_owner().is_none());
    h.set_height_owner(Some(id(&h, "panel"))).unwrap();
    press(&mut h, "restore", 10.);
    let handle = key(&h, "first");
    assert_eq!(h.height_drag_target(handle), Some(panel));
    let held = h.height_drag_begin(handle, panel, 10.).unwrap().unwrap();
    h.hold_update(held.token, Value::scalar(300.), 10.).unwrap();
    press(&mut h, "clear", 20.);
    assert!(!h.has_hold(held.token));
    assert!(h.height_drag_target(handle).is_none());
    assert_eq!(
        h.height_owner(),
        Some(panel),
        "authored handle must not take the explicit owner's lifetime"
    );
    assert_eq!(
        h.engine().target(motion_node(panel), Property::Height),
        Some(Value::scalar(180.))
    );
}

#[test]
fn prop_only_panel_a_cannot_claim_owner_before_panel_b_with_release_handler() {
    let source = APP.replacen(" heightrelease=release", "", 2);
    let mut h = boot(&source);
    let a = key(&h, "panel");
    let prop_only = key(&h, "first");
    let b = key(&h, "other");
    let actual = key(&h, "other-handle");
    // Resolver deliberately stays property-only; the host owns admission policy.
    assert_eq!(h.kernel().height_drag_target(prop_only), Some(a));
    assert_eq!(h.height_owner(), Some(b));
    assert_eq!(h.height_drag_target(prop_only), None);
    assert!(h
        .height_drag_begin(prop_only, a, f64::NAN)
        .unwrap()
        .is_none());
    let held = h.height_drag_begin(actual, b, 0.).unwrap().unwrap();
    assert!(h
        .dispatch_height_held(held.token, actual, 120., 0., 10.)
        .unwrap());
    assert_eq!(text(&h, "releases"), "1");
}

#[test]
fn newly_created_release_handler_is_admitted_and_destroyed_declaration_is_forgotten() {
    let source = APP
        .replace("state first = true", "state first = false")
        .replace("state second = true", "state second = false")
        .replace("heightDragFor=\"other\"", "heightDragFor=\"\"");
    let mut h = boot(&source);
    assert!(h.height_owner().is_none());
    for at in [10., 20., 30.] {
        press(&mut h, "restore", at);
        let handle = key(&h, "first");
        let panel = key(&h, "panel");
        let held = h.height_drag_begin(handle, panel, at).unwrap().unwrap();
        press(&mut h, "remove", at);
        assert!(!h.has_hold(held.token));
        assert!(h.height_drag_target(handle).is_none());
        assert!(h.height_owner().is_none());
    }
}
