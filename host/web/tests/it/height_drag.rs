//! Authored handle generations own a target hold; release commits while held.
use exact_kernel::{motion::motion_node, NodeKey, Op, PropId, PropValue};
use exact_motion::{EngineError, HoldEnd, HoldStart, Property, Value as MotionValue};
use exact_runner::{DataError, DataSource, Event, Value};
use exact_web::{abi::Bridge, Host};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(name.into()))
    }
}
const SOURCE: &str = r#"component App
  state extent = 640
  state count = 0
  state received = 0
  state speed = 0
  state display = "block"
  state reference = "sheet"
  state first = true
  state second = true
  state shown = true
  action snap(height: number, velocity: number) writes extent, count, received, speed
    extent = 240
    count = count + 1
    received = height
    speed = velocity
  action hide writes display
    display = "none"
  action show writes display
    display = "block"
  action unbind writes reference
    reference = "missing"
  action firstOff writes first
    first = false
  action secondOff writes second
    second = false
  action remove writes shown
    shown = false
  action restore writes shown
    shown = true
  view
    column
      button width=80 height=24 press=hide testId="hide"
      button width=80 height=24 press=show testId="show"
      button width=80 height=24 press=unbind testId="unbind"
      button width=80 height=24 press=firstOff testId="firstOff"
      button width=80 height=24 press=secondOff testId="secondOff"
      button width=80 height=24 press=remove testId="remove"
      button width=80 height=24 press=restore testId="restore"
      text `${count}/${received}/${speed}` testId="result"
      column display=display testId="ancestor"
        when shown
          column id="sheet" testId="sheet" height=extent max-height=400 box-sizing="border-box" transition="height spring(180, 12, 1)"
            when first
              column testId="handle" heightDragFor=reference heightrelease=snap
            when second
              column testId="handle2" heightDragFor="sheet" heightrelease=snap
      column id="other" testId="other" height=100 box-sizing="border-box"
        column testId="otherHandle" heightDragFor="other" heightrelease=snap
      column testId="programmatic" height=123
"#;

fn boot_source(source: &str) -> (Host<NoData>, String) {
    exact_web::link(exact_web_capabilities::ALL);
    Host::boot(
        &contract::compile(source).unwrap().encode(),
        NoData,
        Default::default(),
        "/",
    )
    .unwrap()
}
fn boot() -> (Host<NoData>, String) {
    boot_source(SOURCE)
}
fn id(host: &Host<NoData>, name: &str) -> u32 {
    let kernel = host.runner().kernel();
    kernel
        .node_by_key(kernel.find_by_test_id(name)[0])
        .unwrap()
        .id
}
fn key(host: &Host<NoData>, name: &str) -> NodeKey {
    host.runner().kernel().node(id(host, name)).unwrap().key
}
fn press(host: &mut Host<NoData>, name: &str, now: f64) -> String {
    host.dispatch_at(id(host, name), Event::Press, now)
}
fn begin(host: &mut Host<NoData>, name: &str, value: f64, now: f64) -> HoldStart {
    host.begin_height_drag(key(host, name), MotionValue::scalar(value), now)
        .unwrap()
        .unwrap()
        .0
}
fn binding_op(host: &Host<NoData>, handle: &str, target: Option<&str>) -> String {
    format!(
        "{{\"op\":\"height-drag\",\"id\":{},\"target\":{},\"handleKey\":\"{}\",\"targetKey\":{}}}",
        id(host, handle),
        target.map_or("null".into(), |name| id(host, name).to_string()),
        motion_node(key(host, handle)),
        target.map_or("null".into(), |name| format!(
            "\"{}\"",
            motion_node(key(host, name))
        ))
    )
}
fn set_prop(host: &mut Host<NoData>, view: u32, prop: PropId, value: PropValue) {
    let kernel = host.runner_mut().kernel_mut();
    kernel
        .apply(
            0,
            kernel.epoch() + 1,
            &[Op::SetProp {
                id: view,
                prop,
                value,
            }],
        )
        .unwrap();
}

#[test]
fn boot_registers_one_target_multiple_handles_after_coherent_tree_and_only_changes() {
    let (mut host, batch) = boot();
    let a = host.height_drag_binding(id(&host, "handle")).unwrap();
    assert_eq!(a.handle, key(&host, "handle"));
    assert_eq!(a.target, key(&host, "sheet"));
    assert_eq!(
        host.height_drag_binding(id(&host, "handle2"))
            .unwrap()
            .target,
        a.target
    );
    assert!(host.height_drag_binding(id(&host, "otherHandle")).is_none());
    assert!(
        batch.contains(&binding_op(&host, "handle", Some("sheet"))),
        "{batch}"
    );
    assert!(
        batch.contains(&binding_op(&host, "otherHandle", None)),
        "{batch}"
    );
    assert!(
        batch.find("\"op\":\"roots\"").unwrap() < batch.find("\"op\":\"height-drag\"").unwrap()
    );
    assert!(!host.advance(0.0).contains("\"op\":\"height-drag\""));
    press(&mut host, "firstOff", 1.0);
    assert_eq!(
        host.height_drag_binding(id(&host, "handle2"))
            .unwrap()
            .target,
        a.target
    );
    press(&mut host, "secondOff", 2.0);
    assert!(host.height_drag_binding(id(&host, "otherHandle")).is_some());
    assert_eq!(
        host.springs()
            .engine()
            .value(motion_node(a.target), Property::Height),
        None
    );
}

#[test]
fn constrained_presentation_action_then_release_uses_newest_authored_target_once() {
    let (mut host, _) = boot();
    assert_eq!(
        host.springs()
            .engine()
            .value(motion_node(key(&host, "sheet")), Property::Height),
        Some(MotionValue::scalar(640.0))
    );
    let held = begin(&mut host, "handle", 400.0, 100.0);
    assert_eq!(held.value, MotionValue::scalar(400.0));
    let handle = id(&host, "handle");
    let batch = host
        .dispatch_height_held(held.token.serial(), handle, 360.0, -75.0, 110.0)
        .unwrap()
        .unwrap();
    assert!(batch.contains("1/360/-75"), "{batch}");
    assert!(host.has_hold(held.token.serial()));
    assert_eq!(
        host.springs()
            .engine()
            .value(held.token.node(), Property::Height),
        Some(MotionValue::scalar(360.0))
    );
    assert!(!batch.contains("\"op\":\"animate\""), "{batch}");
    assert!(host
        .dispatch_height_held(held.token.serial(), handle, f64::NAN, f64::NAN, f64::NAN)
        .unwrap()
        .is_none());
    let release = host
        .end_hold(
            held.token.serial(),
            HoldEnd::Release {
                velocity: MotionValue::scalar(-75.0),
            },
            110.0,
        )
        .unwrap()
        .unwrap();
    assert!(release.contains("\"values\":[360,"), "{release}");
    assert!(release.contains(",240]}"), "{release}");
    assert!(host
        .end_hold(held.token.serial(), HoldEnd::Cancel, f64::NAN)
        .unwrap()
        .is_none());
}

#[test]
fn handle_prop_and_untouched_ancestor_changes_cancel_without_action_and_restore() {
    for action in ["hide", "unbind"] {
        let (mut host, _) = boot();
        let held = begin(&mut host, "handle", 300.0, 1.0);
        let batch = press(&mut host, action, 2.0);
        assert!(
            batch.contains(&binding_op(&host, "handle", None)),
            "{batch}"
        );
        assert!(!host.has_hold(held.token.serial()));
        assert!(host
            .dispatch_height_held(
                held.token.serial(),
                id(&host, "handle"),
                f64::NAN,
                f64::NAN,
                f64::NAN
            )
            .unwrap()
            .is_none());
        assert!(!batch.contains("1/"));
        if action == "hide" {
            // The other valid target becomes owner while hidden; removing its
            // handle then restoring the sheet deterministically restores it.
            let other = id(&host, "otherHandle");
            set_prop(
                &mut host,
                other,
                PropId::HeightDragFor,
                PropValue::Str("missing".into()),
            );
            press(&mut host, "show", 3.0);
            assert!(host.height_drag_binding(id(&host, "handle")).is_some());
        }
    }
}

#[test]
fn every_move_and_release_revalidates_binding_before_clock_even_without_receipt() {
    for release in [false, true] {
        let (mut host, _) = boot();
        let held = begin(&mut host, "handle", 300.0, 10.0);
        let ancestor = id(&host, "ancestor");
        set_prop(&mut host, ancestor, PropId::Inert, PropValue::Bool(true));
        let result = if release {
            host.end_hold(held.token.serial(), HoldEnd::Cancel, f64::NAN)
        } else {
            host.update_hold(held.token.serial(), MotionValue::scalar(f64::NAN), f64::NAN)
        };
        assert!(result.unwrap().is_none());
        assert_eq!(host.springs().engine().now(), 0.010);
        assert!(!host
            .springs()
            .engine()
            .is_held(held.token.node(), Property::Height));
    }
}

#[test]
fn malformed_live_action_and_begin_are_atomic_stale_validation_precedes_values() {
    let (mut host, _) = boot();
    let held = begin(&mut host, "handle", 300.0, 10.0);
    let handle = id(&host, "handle");
    for (height, velocity, now) in [
        (f64::NAN, 0.0, 11.0),
        (-1.0, 0.0, 11.0),
        (1.0, f64::INFINITY, 11.0),
        (1.0, 0.0, 9.0),
        (1.0, 0.0, f64::NAN),
    ] {
        assert!(host
            .dispatch_height_held(held.token.serial(), handle, height, velocity, now)
            .is_err());
        assert_eq!(host.springs().engine().now(), 0.010);
        assert_eq!(
            host.springs()
                .engine()
                .value(held.token.node(), Property::Height),
            Some(MotionValue::scalar(300.0))
        );
    }
    assert_eq!(
        host.begin_height_drag(key(&host, "handle"), MotionValue::new(1.0, 1.0), 11.0)
            .unwrap_err(),
        EngineError::InvalidValueShape
    );
    assert!(host.has_hold(held.token.serial()));
    let bad = NodeKey {
        generation: u32::MAX,
        ..key(&host, "handle")
    };
    assert!(host
        .begin_height_drag(bad, MotionValue::scalar(f64::NAN), f64::NAN)
        .unwrap()
        .is_none());
    assert!(host
        .dispatch_height_held(
            held.token.serial(),
            id(&host, "handle2"),
            f64::NAN,
            f64::NAN,
            f64::NAN
        )
        .unwrap()
        .is_none());
    assert!(host
        .dispatch_height_held(held.token.serial(), handle, 200.0, 0.0, 11.0)
        .unwrap()
        .is_some());
}

#[test]
fn rebegin_reuse_and_action_destruction_retire_generational_active_record() {
    let (mut host, _) = boot();
    let old_key = key(&host, "handle");
    let old = begin(&mut host, "handle", 300.0, 1.0);
    let new = begin(&mut host, "handle2", 290.0, 2.0);
    assert!(!host.has_hold(old.token.serial()));
    assert!(host
        .dispatch_height_held(
            old.token.serial(),
            id(&host, "handle"),
            f64::NAN,
            f64::NAN,
            f64::NAN
        )
        .unwrap()
        .is_none());
    assert!(host.has_hold(new.token.serial()));
    press(&mut host, "remove", 3.0);
    press(&mut host, "restore", 4.0);
    assert_ne!(key(&host, "handle"), old_key);
    assert!(host
        .begin_height_drag(old_key, MotionValue::scalar(f64::NAN), f64::NAN)
        .unwrap()
        .is_none());
    assert!(!host.has_hold(new.token.serial()));
    let deleting = SOURCE
        .replace(
            "writes extent, count, received, speed",
            "writes extent, count, received, speed, shown",
        )
        .replace(
            "    speed = velocity",
            "    speed = velocity\n    shown = false",
        );
    let (mut host, _) = boot_source(&deleting);
    let held = begin(&mut host, "handle", 300.0, 1.0);
    let batch = host
        .dispatch_height_held(held.token.serial(), id(&host, "handle"), 290.0, 0.0, 2.0)
        .unwrap()
        .unwrap();
    assert!(batch.contains("\"op\":\"destroy\""));
    assert!(host
        .end_hold(held.token.serial(), HoldEnd::Cancel, f64::NAN)
        .unwrap()
        .is_none());
}

#[test]
fn last_auto_handle_clears_registration_programmatic_owner_survives() {
    let single = SOURCE.replace("heightDragFor=\"other\"", "heightDragFor=\"missing\"");
    let (mut host, _) = boot_source(&single);
    let sheet = motion_node(key(&host, "sheet"));
    press(&mut host, "firstOff", 1.0);
    press(&mut host, "secondOff", 2.0);
    assert_eq!(host.springs().engine().value(sheet, Property::Height), None);
    let (mut host, _) = boot_source(&single);
    let own = id(&host, "programmatic");
    host.set_height_owner(Some(own)).unwrap();
    press(&mut host, "firstOff", 1.0);
    press(&mut host, "secondOff", 2.0);
    assert_eq!(
        host.springs()
            .engine()
            .value(motion_node(key(&host, "programmatic")), Property::Height),
        Some(MotionValue::scalar(123.0))
    );
}

#[test]
fn same_height_registration_keeps_automatic_lifetime_and_live_hold() {
    let single = SOURCE.replace("heightDragFor=\"other\"", "heightDragFor=\"missing\"");
    let (mut host, _) = boot_source(&single);
    let sheet = id(&host, "sheet");
    let node = motion_node(key(&host, "sheet"));
    let held = begin(&mut host, "handle", 360.0, 100.0);
    let batch = host.set_height_owner(Some(sheet)).unwrap();
    assert!(batch.contains("\"ops\":[]"), "{batch}");
    assert!(host.springs().engine().has_hold(held.token));
    assert_eq!(host.springs().engine().now(), 0.1);
    assert_eq!(
        host.springs().engine().value(node, Property::Height),
        Some(MotionValue::scalar(360.0))
    );
    press(&mut host, "firstOff", 110.0);
    press(&mut host, "secondOff", 120.0);
    assert_eq!(host.springs().engine().value(node, Property::Height), None);
}

#[test]
fn explicit_height_clear_retires_bindings_until_a_later_receipt() {
    let single = SOURCE.replace("heightDragFor=\"other\"", "heightDragFor=\"missing\"");
    let (mut host, _) = boot_source(&single);
    let node = motion_node(key(&host, "sheet"));
    let held = begin(&mut host, "handle", 360.0, 100.0);
    let batch = host.set_height_owner(None).unwrap();
    assert!(
        batch.contains(&binding_op(&host, "handle", None)),
        "{batch}"
    );
    assert!(
        batch.contains(&binding_op(&host, "handle2", None)),
        "{batch}"
    );
    assert_eq!(host.springs().engine().value(node, Property::Height), None);
    assert!(!host.springs().engine().has_hold(held.token));
    assert_eq!(host.springs().engine().now(), 0.1);
    assert!(host
        .end_hold(held.token.serial(), HoldEnd::Cancel, f64::NAN)
        .unwrap()
        .is_none());
    // An ordinary later receipt can admit the remaining authored handle anew.
    press(&mut host, "firstOff", 110.0);
    assert_eq!(
        host.springs().engine().value(node, Property::Height),
        Some(MotionValue::scalar(640.0))
    );
    let successor = begin(&mut host, "handle2", 400.0, 120.0);
    assert!(host.springs().engine().has_hold(successor.token));
}

fn packet(
    bridge: &mut Bridge<NoData>,
    op: u32,
    view: u32,
    serial: u64,
    x: f64,
    y: f64,
    now: f64,
) -> String {
    let mut bytes = Vec::new();
    for word in [1u32, op, view, 4] {
        bytes.extend(word.to_le_bytes());
    }
    bytes.extend(serial.to_le_bytes());
    for value in [x, y, now] {
        bytes.extend(value.to_le_bytes());
    }
    bridge.input_write(&bytes);
    let len = bridge.motion(bytes.len());
    String::from_utf8(bridge.output_bytes(len as usize).to_vec()).unwrap()
}
fn serial(out: &str) -> u64 {
    out.split("\"token\":\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap()
        .parse()
        .unwrap()
}
#[test]
fn binary_height_ops_preserve_key_bits_target_identity_and_once_only_action() {
    let (host, _) = boot();
    let view = id(&host, "handle");
    let handle = motion_node(key(&host, "handle"));
    let mut bridge = Bridge::new();
    bridge.set_links(exact_web::HostLinks::ALL);
    exact_web::link(exact_web_capabilities::ALL);
    bridge.boot(
        &contract::compile(SOURCE).unwrap().encode(),
        NoData,
        400.0,
        800.0,
        "/",
    );
    assert!(packet(
        &mut bridge,
        8,
        view,
        handle ^ (1u64 << 60),
        f64::NAN,
        0.0,
        f64::NAN
    )
    .contains("\"accepted\":false"));
    let begin = packet(&mut bridge, 8, view, handle, 400.0, 0.0, 100.0);
    assert!(
        begin.contains(&format!("\"target\":{}", id(&host, "sheet"))),
        "{begin}"
    );
    assert!(begin.contains("\"value\":[400,0]"), "{begin}");
    let token = serial(&begin);
    // LLP 1057.001 §3: the velocity is measured (400 -> 350 in 10 ms); y is unused.
    assert!(packet(&mut bridge, 9, view, token, 350.0, -50.0, 110.0).contains("y must be zero"));
    let released = packet(&mut bridge, 9, view, token, 350.0, 0.0, 110.0);
    assert!(released.contains("1/350/-5000"), "{released}");
    assert!(
        packet(&mut bridge, 9, view, token, f64::NAN, f64::NAN, f64::NAN)
            .contains("\"accepted\":false")
    );
    assert!(packet(&mut bridge, 2, 0, token, -50.0, 0.0, 110.0).contains(",240]}"));
}

#[test]
fn synthesized_kind15_parses_finite_pair_before_clock_and_remains_separate_from_holds() {
    let (host, _) = boot();
    let handle = id(&host, "handle");
    let mut bridge = Bridge::new();
    bridge.set_links(exact_web::HostLinks::ALL);
    exact_web::link(exact_web_capabilities::ALL);
    bridge.boot(
        &contract::compile(SOURCE).unwrap().encode(),
        NoData,
        400.0,
        800.0,
        "/",
    );
    for payload in ["NaN,0", "1,Infinity", "-1,0", "1,0,2", "1"] {
        bridge.input_write(payload.as_bytes());
        let len = bridge.dispatch(handle, 15, payload.len(), 1000.0);
        assert!(String::from_utf8_lossy(bridge.output_bytes(len as usize))
            .contains("invalid height release"));
    }
    // A later valid capture at 1ms proves malformed synthetic input did not
    // move the engine to 1000ms.
    let held = packet(
        &mut bridge,
        8,
        handle,
        motion_node(key(&host, "handle")),
        300.0,
        0.0,
        1.0,
    );
    assert!(held.contains("\"token\""), "{held}");
    bridge.input_write(b"200,-9");
    let len = bridge.dispatch(handle, 15, 6, 2.0);
    let out = String::from_utf8_lossy(bridge.output_bytes(len as usize));
    assert!(out.contains("1/200/-9"), "{out}");
}

#[test]
fn binding_batch_preserves_every_bit_even_beyond_javascript_safe_integers() {
    let handle = NodeKey {
        index: u32::MAX,
        generation: u32::MAX,
    };
    let target = NodeKey {
        index: 17,
        generation: 0x8000_0001,
    };
    let mut batch = exact_web::batch::Batch::new();
    batch.height_drag(1, handle, Some((target, 2)));
    let out = batch.finish(None, 0.0, None);
    assert!(
        out.contains("\"handleKey\":\"18446744073709551615\""),
        "{out}"
    );
    assert!(
        out.contains(&format!("\"targetKey\":\"{}\"", motion_node(target))),
        "{out}"
    );
    assert_eq!(motion_node(target), 9_223_372_041_149_743_121);
}

#[test]
fn malformed_typed_release_does_not_change_host_clock_or_action_state() {
    let (mut host, _) = boot();
    let handle = id(&host, "handle");
    let error = host.dispatch_at(
        handle,
        Event::HeightRelease {
            height: f64::NAN,
            velocity: 0.0,
        },
        1000.0,
    );
    assert!(error.contains("invalid height release"));
    // dispatch() uses Host's retained clock; malformed dispatch must not have
    // advanced it, including when the event failed before a kernel receipt.
    let batch = host.dispatch(
        handle,
        Event::HeightRelease {
            height: 200.0,
            velocity: 0.0,
        },
    );
    assert!(batch.contains("1/200/0"), "{batch}");
    assert_eq!(host.springs().engine().now(), 0.0);
    let held = begin(&mut host, "handle", 300.0, 1.0);
    assert_eq!(host.springs().engine().now(), 0.001);
    assert!(host.has_hold(held.token.serial()));
}

#[test]
fn overdue_timer_retains_height_hold_and_lowers_unrelated_action_motion() {
    let source = SOURCE.replace("  view\n", "  action tick writes extent, count\n    extent = 240\n    count = count + 1\n  task clock mount\n    every(100, tick)\n  view\n")
        .replace("testId=\"result\"", "testId=\"result\" opacity=(count == 0 ? 1 : 0.5) transition=\"opacity spring(180, 12, 1)\"");
    let (mut host, _) = boot_source(&source);
    let held = begin(&mut host, "handle", 400.0, 150.0);
    let batch = host.advance(175.0);
    assert!(batch.contains("\"property\":\"opacity\""), "{batch}");
    assert!(!batch.contains("\"property\":\"height\""), "{batch}");
    assert!(host.has_hold(held.token.serial()));
    assert_eq!(
        host.springs()
            .engine()
            .value(held.token.node(), Property::Height),
        Some(MotionValue::scalar(400.0))
    );
    let batch = host
        .dispatch_height_held(held.token.serial(), id(&host, "handle"), 350.0, 0.0, 180.0)
        .unwrap()
        .unwrap();
    assert!(batch.contains("2/350/0"), "{batch}");
    let release = host
        .end_hold(held.token.serial(), HoldEnd::Cancel, 180.0)
        .unwrap()
        .unwrap();
    assert!(release.contains(",240]}"), "{release}");
}

#[test]
fn accepted_receipt_cancellation_starts_at_receipt_time_and_retargets_from_elapsed_curve() {
    let (mut host, _) = boot();
    let held = begin(&mut host, "handle", 300.0, 100.0);
    let cancelled = press(&mut host, "unbind", 200.0);
    let returning = host
        .springs()
        .engine()
        .spring_descriptor(held.token.node(), Property::Height)
        .unwrap();
    assert!(cancelled.contains("\"values\":[300,"), "{cancelled}");
    let elapsed = returning.config.sample(
        returning.from.x - returning.target.x,
        returning.velocity.x,
        0.050,
    );
    let expected_origin = returning.target.x + elapsed.displacement;
    host.dispatch_at(
        id(&host, "handle2"),
        Event::HeightRelease {
            height: 240.0,
            velocity: 0.0,
        },
        250.0,
    );
    let retargeted = host
        .springs()
        .engine()
        .spring_descriptor(held.token.node(), Property::Height)
        .unwrap();
    assert_eq!(
        returning.start, 0.200,
        "accepted unbind must not backdate the return"
    );
    assert!(
        (retargeted.from.x - expected_origin).abs() < 1e-9,
        "retargeted from {}, expected 50ms of return = {}",
        retargeted.from.x,
        expected_origin
    );
    assert_eq!(retargeted.start, 0.250);
}

#[test]
fn accepted_cancellation_uses_latest_zero_duration_or_removed_transition() {
    for transition in ["none", "height 0ms linear"] {
        let source = SOURCE.replace("  state reference =", "  state transition = \"height spring(180, 12, 1)\"\n  state reference =")
            .replace("  action unbind writes reference\n    reference = \"missing\"",
                &format!("  action unbind writes reference, transition\n    reference = \"missing\"\n    transition = \"{transition}\""))
            .replace("transition=\"height spring(180, 12, 1)\"", "transition=transition");
        let (mut host, _) = boot_source(&source);
        let held = begin(&mut host, "handle", 300.0, 100.0);
        let cancelled = press(&mut host, "unbind", 200.0);
        assert!(!host.has_hold(held.token.serial()));
        assert_eq!(
            host.springs()
                .engine()
                .value(held.token.node(), Property::Height),
            Some(MotionValue::scalar(640.0)),
            "{transition}: {cancelled}"
        );
        assert!(host
            .springs()
            .engine()
            .spring_descriptor(held.token.node(), Property::Height)
            .is_none());
        assert!(!cancelled.contains("\"op\":\"animate\""), "{cancelled}");
    }
}

#[test]
fn accepted_cancellation_uses_latest_target_spring_and_delay_while_still_held() {
    let source = SOURCE.replace("  state reference =", "  state declaration = \"height spring(180, 12, 1)\"\n  state reference =")
        .replace("  action unbind writes reference\n    reference = \"missing\"",
            "  action unbind writes reference, extent, declaration\n    reference = \"missing\"\n    extent = 240\n    declaration = \"height 0s spring(120, 8, 2) 50ms\"")
        .replace("transition=\"height spring(180, 12, 1)\"", "transition=declaration");
    let (mut host, _) = boot_source(&source);
    let held = begin(&mut host, "handle", 300.0, 100.0);
    let batch = press(&mut host, "unbind", 200.0);
    let returning = host
        .springs()
        .engine()
        .spring_descriptor(held.token.node(), Property::Height)
        .unwrap();
    assert_eq!(returning.start, 0.250);
    assert_eq!(
        returning.from,
        MotionValue::scalar(300.0),
        "release must start at held presentation: {batch}"
    );
    assert_eq!(returning.target, MotionValue::scalar(240.0));
    assert_eq!(returning.velocity, MotionValue::ZERO);
    assert_eq!(
        returning.config,
        exact_motion::SpringConfig {
            stiffness: 120.0,
            damping: 8.0,
            mass: 2.0
        }
    );
    assert!(
        batch.contains("\"delay\":49.999999999999986") || batch.contains("\"delay\":50"),
        "{batch}"
    );
}

#[test]
fn overdue_receipt_cancellation_uses_existing_engine_clock_floor() {
    let source = SOURCE.replace(
        "  view\n",
        "  task clock mount\n    every(200, unbind)\n  view\n",
    );
    let (mut host, _) = boot_source(&source);
    let held = begin(&mut host, "handle", 300.0, 300.0);
    let batch = host.advance(350.0);
    assert!(!batch.contains("\"error\":\""), "{batch}");
    assert!(!host.has_hold(held.token.serial()));
    let returning = host
        .springs()
        .engine()
        .spring_descriptor(held.token.node(), Property::Height)
        .unwrap();
    assert_eq!(returning.start, 0.300);
    assert_eq!(returning.from, MotionValue::scalar(300.0));
}

#[test]
fn invalid_stale_delivery_does_not_use_its_future_clock_for_cancellation() {
    let (mut host, _) = boot();
    let held = begin(&mut host, "handle", 300.0, 100.0);
    let view = id(&host, "handle");
    set_prop(
        &mut host,
        view,
        PropId::HeightDragFor,
        PropValue::Str("missing".into()),
    );
    assert!(host
        .update_hold(held.token.serial(), MotionValue::scalar(f64::NAN), 10_000.0)
        .unwrap()
        .is_none());
    assert_eq!(host.springs().engine().now(), 0.100);
    assert_eq!(
        host.springs()
            .engine()
            .spring_descriptor(held.token.node(), Property::Height)
            .unwrap()
            .start,
        0.100
    );
}

/// LLP 1057.001 §3: the browser's sequence (moves, the shown heights tracked
/// at each move's instant, a last stationary move, then the action) releases
/// at the engine's velocity over what was shown.
#[test]
fn a_measured_release_follows_the_moves_and_tracked_heights() {
    let (host, _) = boot();
    let view = id(&host, "handle");
    let handle = motion_node(key(&host, "handle"));
    let mut bridge = Bridge::new();
    bridge.set_links(exact_web::HostLinks::ALL);
    exact_web::link(exact_web_capabilities::ALL);
    bridge.boot(
        &contract::compile(SOURCE).unwrap().encode(),
        NoData,
        400.0,
        800.0,
        "/",
    );
    let token = serial(&packet(&mut bridge, 8, view, handle, 400.0, 0.0, 100.0));
    for (i, height) in [380.0, 360.0, 340.0, 320.0].into_iter().enumerate() {
        let t = 110.0 + 10.0 * i as f64;
        assert!(packet(&mut bridge, 1, 0, token, height, 0.0, t).contains("\"accepted\":true"));
        assert!(packet(&mut bridge, 12, 0, token, height, 0.0, t).contains("\"accepted\":true"));
    }
    assert!(packet(&mut bridge, 1, 0, token, 320.0, 0.0, 145.0).contains("\"accepted\":true"));
    let released = packet(&mut bridge, 9, view, token, 320.0, 0.0, 150.0);
    let velocity: f64 = released
        .split("1/320/")
        .nth(1)
        .unwrap_or_else(|| panic!("{released}"))
        .split(|c: char| c != '-' && c != '.' && !c.is_ascii_digit())
        .next()
        .unwrap()
        .parse()
        .unwrap();
    assert!(velocity < -1000.0, "{velocity}");
}
