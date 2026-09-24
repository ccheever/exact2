//! One registered numeric height; the browser remains its presentation executor.
use exact_kernel::motion::motion_node;
use exact_motion::{HoldEnd, Property, Value as MotionValue};
use exact_runner::{DataError, DataSource, Event, Value};
use exact_web::Host;

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(name.into()))
    }
}
const SOURCE: &str = r#"component App
  state extent = 180
  state display = "block"
  state shown = true
  action tall writes extent
    extent = 640
  action short writes extent
    extent = 240
  action hide writes display
    display = "none"
  action show writes display
    display = "block"
  action remove writes shown
    shown = false
  view
    column
      button press=tall testId="tall"
        text "Tall"
      button press=short testId="short"
        text "Short"
      button press=hide testId="hide"
        text "Hide"
      button press=show testId="show"
        text "Show"
      button press=remove testId="remove"
        text "Remove"
      column display=display
        when shown
          column testId="panel" height=extent max-height=400 box-sizing="border-box" transition="height spring(180, 12, 1), translate spring(180, 12, 1)"
            text "Panel"
      column testId="other" height=120
        text "Other"
      column testId="automatic"
        text "Automatic"
"#;
fn boot() -> Host<NoData> {
    exact_web::link(exact_web_capabilities::ALL);
    Host::boot(
        &contract::compile(SOURCE).unwrap().encode(),
        NoData,
        Default::default(),
        "/",
    )
    .unwrap()
    .0
}
fn id(host: &Host<NoData>, name: &str) -> u32 {
    let k = host.runner().kernel();
    k.node_by_key(k.find_by_test_id(name)[0]).unwrap().id
}
fn press(host: &mut Host<NoData>, name: &str, now: f64) -> String {
    host.dispatch_at(id(host, name), Event::Press, now)
}
fn node(host: &Host<NoData>, view: u32) -> u64 {
    motion_node(host.runner().kernel().node(view).unwrap().key)
}

#[test]
fn height_requires_explicit_owner_and_rejected_replacement_is_atomic() {
    let mut host = boot();
    let panel = id(&host, "panel");
    assert!(host
        .begin_hold(panel, Property::Height, MotionValue::scalar(180.0), 0.0)
        .unwrap()
        .is_none());
    assert!(!press(&mut host, "tall", 10.0).contains("\"property\":\"height\""));
    host.set_height_owner(Some(panel)).unwrap();
    let (held, _) = host
        .begin_hold(panel, Property::Height, MotionValue::scalar(400.0), 20.0)
        .unwrap()
        .unwrap();
    let now = host.springs().engine().now();
    assert!(host.set_height_owner(Some(id(&host, "automatic"))).is_err());
    assert!(host.set_height_owner(Some(u32::MAX)).is_err());
    assert!(host.has_hold(held.token.serial()));
    assert_eq!(host.springs().engine().now(), now);
    assert_eq!(
        host.springs()
            .engine()
            .value(held.token.node(), Property::Height),
        Some(MotionValue::scalar(400.0))
    );
}

#[test]
fn caught_constrained_height_releases_once_to_latest_authored_target() {
    let mut host = boot();
    let panel = id(&host, "panel");
    host.set_height_owner(Some(panel)).unwrap();
    let start = press(&mut host, "tall", 10.0);
    assert!(start.contains("\"property\":\"height\""), "{start}");
    let (held, _) = host
        .begin_hold(panel, Property::Height, MotionValue::scalar(400.0), 100.0)
        .unwrap()
        .unwrap();
    let committed = press(&mut host, "short", 110.0);
    assert!(!committed.contains("\"op\":\"animate\""), "{committed}");
    assert_eq!(
        host.springs()
            .engine()
            .value(held.token.node(), Property::Height),
        Some(MotionValue::scalar(400.0))
    );
    let release = host
        .end_hold(
            held.token.serial(),
            HoldEnd::Release {
                velocity: MotionValue::scalar(-20.0),
            },
            110.0,
        )
        .unwrap()
        .unwrap();
    assert!(release.contains("\"values\":[400,"), "{release}");
    assert!(release.contains(",240]}"), "{release}");
    assert_eq!(host.springs().playing_count(), 1);
    assert!(host
        .end_hold(held.token.serial(), HoldEnd::Cancel, f64::NAN)
        .unwrap()
        .is_none());
}

#[test]
fn unsupported_height_and_untouched_ancestor_retire_only_height() {
    let mut host = boot();
    let panel = id(&host, "panel");
    host.set_height_owner(Some(panel)).unwrap();
    let (height, _) = host
        .begin_hold(panel, Property::Height, MotionValue::scalar(200.0), 1.0)
        .unwrap()
        .unwrap();
    let (translate, _) = host
        .begin_hold(panel, Property::Translate, MotionValue::new(30.0, 0.0), 1.0)
        .unwrap()
        .unwrap();
    let hidden = press(&mut host, "hide", 2.0);
    assert!(hidden.contains("\"op\":\"retire-motion\""), "{hidden}");
    assert!(!host.has_hold(height.token.serial()));
    assert!(host.has_hold(translate.token.serial()));
    assert_eq!(
        host.springs()
            .engine()
            .value(height.token.node(), Property::Height),
        None
    );
    let same_hidden = host.set_height_owner(Some(panel)).unwrap();
    assert!(same_hidden.contains("\"ops\":[]"), "{same_hidden}");
    assert_eq!(host.springs().engine().now(), 0.002);
    press(&mut host, "show", 3.0);
    assert_eq!(
        host.springs()
            .engine()
            .value(height.token.node(), Property::Height),
        Some(MotionValue::scalar(180.0))
    );
    let running = press(&mut host, "tall", 4.0);
    assert!(running.contains("\"op\":\"animate\""));
    assert_eq!(host.springs().playing_count(), 1);
    // Drive the host's documented mutable-runner seam to clear the height.
    // The next input boundary must consume retirement even without a receipt.
    let k = host.runner_mut().kernel_mut();
    let mut patch = exact_kernel::StyleProps {
        height: exact_kernel::Dimension::Auto,
        ..Default::default()
    };
    patch.mask.set(exact_kernel::StyleId::Height);
    k.apply(
        0,
        k.epoch() + 1,
        &[exact_kernel::Op::SetStyle {
            id: panel,
            patch: Box::new(patch),
        }],
    )
    .unwrap();
    let automatic = host
        .update_hold(translate.token.serial(), MotionValue::new(31.0, 0.0), 5.0)
        .unwrap()
        .unwrap();
    assert!(
        automatic.contains("\"op\":\"retire-motion\""),
        "{automatic}"
    );
    assert_eq!(host.springs().playing_count(), 0);
    assert!(host.has_hold(translate.token.serial()));
}

#[test]
fn replacement_unregistration_and_destroy_drop_only_registered_height() {
    let mut host = boot();
    let panel = id(&host, "panel");
    let other = id(&host, "other");
    let panel_node = node(&host, panel);
    host.set_height_owner(Some(panel)).unwrap();
    let (old, _) = host
        .begin_hold(panel, Property::Height, MotionValue::scalar(200.0), 10.0)
        .unwrap()
        .unwrap();
    let switched = host.set_height_owner(Some(other)).unwrap();
    assert!(switched.contains("\"op\":\"retire-motion\""));
    assert!(!host.has_hold(old.token.serial()));
    assert_eq!(
        host.springs().engine().value(panel_node, Property::Height),
        None
    );
    assert_eq!(
        host.springs()
            .engine()
            .value(node(&host, other), Property::Height),
        Some(MotionValue::scalar(120.0))
    );
    host.set_height_owner(None).unwrap();
    assert_eq!(
        host.springs()
            .engine()
            .value(node(&host, other), Property::Height),
        None
    );
    host.set_height_owner(Some(panel)).unwrap();
    let (held, _) = host
        .begin_hold(panel, Property::Height, MotionValue::scalar(210.0), 10.0)
        .unwrap()
        .unwrap();
    press(&mut host, "remove", 11.0);
    assert!(!host.has_hold(held.token.serial()));
    assert_eq!(
        host.springs().engine().value(panel_node, Property::Height),
        None
    );
    assert!(host
        .update_hold(held.token.serial(), MotionValue::scalar(f64::NAN), f64::NAN)
        .unwrap()
        .is_none());
}

#[test]
fn registration_and_height_hold_use_the_existing_lossless_binary_bridge() {
    use exact_web::abi::Bridge;
    let plan = contract::compile(SOURCE).unwrap().encode();
    let host = boot();
    let panel = id(&host, "panel");
    let mut bridge = Bridge::new();
    exact_web::link(exact_web_capabilities::ALL);
    bridge.boot(&plan, NoData, 400.0, 800.0, "/");
    let request = |bridge: &mut Bridge<NoData>, op: u32, view: u32, prop: u32, serial: u64| {
        let mut bytes = Vec::new();
        for word in [1u32, op, view, prop] {
            bytes.extend(word.to_le_bytes());
        }
        bytes.extend(serial.to_le_bytes());
        for value in [400.0f64, 0.0, 25.0] {
            bytes.extend(value.to_le_bytes());
        }
        bridge.input_write(&bytes);
        let len = bridge.motion(bytes.len());
        String::from_utf8(bridge.output_bytes(len as usize).to_vec()).unwrap()
    };
    assert!(request(&mut bridge, 0, panel, 4, 0).contains("\"accepted\":false"));
    let registered = request(&mut bridge, 6, panel, 4, 0);
    assert!(registered.contains("\"accepted\":true"), "{registered}");
    let held = request(&mut bridge, 0, panel, 4, 0);
    let token: u64 = held
        .split("\"token\":\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap()
        .parse()
        .unwrap();
    assert!(held.contains("\"value\":[400,0]"), "{held}");
    assert!(request(&mut bridge, 7, 0, 0, 0).contains("error"));
    assert!(request(&mut bridge, 4, 0, 4, token).contains("\"accepted\":true"));
    let clear = request(&mut bridge, 7, 0, 4, 0);
    assert!(clear.contains("\"op\":\"retire-motion\""), "{clear}");
    assert!(request(&mut bridge, 4, 0, 4, token).contains("\"accepted\":false"));
}

#[test]
fn height_input_range_refuses_before_clock_or_hold_mutation_but_velocity_is_signed() {
    let mut host = boot();
    let panel = id(&host, "panel");
    assert!(host
        .begin_hold(panel, Property::Height, MotionValue::scalar(-1.0), f64::NAN)
        .unwrap()
        .is_none());
    host.set_height_owner(Some(panel)).unwrap();
    let (held, _) = host
        .begin_hold(panel, Property::Height, MotionValue::scalar(20.0), 20.0)
        .unwrap()
        .unwrap();
    for (value, error) in [
        (
            MotionValue::scalar(-1.0),
            exact_motion::EngineError::InvalidValueShape,
        ),
        (
            MotionValue::scalar(f32::MAX as f64 * 2.0),
            exact_motion::EngineError::InvalidValueShape,
        ),
        (
            MotionValue::new(1.0, f64::NAN),
            exact_motion::EngineError::NonFinite,
        ),
    ] {
        assert_eq!(
            host.begin_hold(panel, Property::Height, value, 30.0)
                .unwrap_err(),
            error
        );
        assert_eq!(
            host.update_hold(held.token.serial(), value, 30.0)
                .unwrap_err(),
            error
        );
        assert!(host.has_hold(held.token.serial()));
        assert_eq!(host.springs().engine().now(), 0.020);
        assert_eq!(
            host.springs()
                .engine()
                .value(held.token.node(), Property::Height),
            Some(MotionValue::scalar(20.0))
        );
        assert_eq!(host.springs().playing_count(), 0);
    }
    host.end_hold(
        held.token.serial(),
        HoldEnd::Release {
            velocity: MotionValue::scalar(-2000.0),
        },
        20.0,
    )
    .unwrap()
    .unwrap();
    assert!(host
        .update_hold(held.token.serial(), MotionValue::scalar(f64::NAN), f64::NAN)
        .unwrap()
        .is_none());
}

#[test]
fn closing_spring_retains_negative_lobe_rebound_and_all_sample_times_for_dom_clamping() {
    let source = SOURCE
        .replace("extent = 180", "extent = 20")
        .replace("extent = 240", "extent = 0");
    exact_web::link(exact_web_capabilities::ALL);
    let mut host = Host::boot(
        &contract::compile(&source).unwrap().encode(),
        NoData,
        Default::default(),
        "/",
    )
    .unwrap()
    .0;
    let panel = id(&host, "panel");
    host.set_height_owner(Some(panel)).unwrap();
    let (held, _) = host
        .begin_hold(panel, Property::Height, MotionValue::scalar(20.0), 0.0)
        .unwrap()
        .unwrap();
    press(&mut host, "short", 0.0);
    let batch = host
        .end_hold(
            held.token.serial(),
            HoldEnd::Release {
                velocity: MotionValue::scalar(-2000.0),
            },
            0.0,
        )
        .unwrap()
        .unwrap();
    let descriptor = host
        .springs()
        .engine()
        .spring_descriptor(held.token.node(), Property::Height)
        .unwrap();
    assert_eq!(
        (
            descriptor.from.x,
            descriptor.target.x,
            descriptor.velocity.x
        ),
        (20.0, 0.0, -2000.0)
    );
    let frames = host
        .springs()
        .engine()
        .spring_frames(held.token.node(), Property::Height)
        .unwrap();
    let encoded = batch
        .split("\"values\":[")
        .nth(1)
        .unwrap()
        .split(']')
        .next()
        .unwrap();
    let values: Vec<f64> = encoded.split(',').map(|v| v.parse().unwrap()).collect();
    assert_eq!(
        values,
        frames.values.iter().map(|v| v.x).collect::<Vec<_>>()
    );
    let first_negative = values.iter().position(|&v| v < 0.0).unwrap();
    assert!(
        values[first_negative + 1..].iter().any(|&v| v > 0.0),
        "do not stop at first zero crossing"
    );
    assert_eq!(values.last(), Some(&0.0));
}
