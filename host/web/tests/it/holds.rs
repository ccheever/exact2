//! Host admission and release use the shared Engine hold, never authored state.
use exact_motion::{HoldEnd, Property, Value as MotionValue};
use exact_runner::{DataError, DataSource, Event, Value};
use exact_web::Host;
struct NoData;
impl DataSource for NoData {
    fn query(&mut self, s: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(s.into()))
    }
}
const SOURCE: &str = r##"component App
  state target = 0
  state color = "#ff0000"
  state shown = true
  state count = 0
  action retarget
    target = 24
    color = "#0000ff"
  action swipe
    count = count + 1
    shown = false
  view
    column
      button press=retarget testId="retarget"
        text "retarget"
      text `${count}` testId="count"
      when shown
        column testId="row" scale=target background-color=color transition="translate spring(180, 12, 1), scale spring(180, 12, 1)" swiperight=swipe
          text "row"
"##;
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
#[test]
fn actual_browser_sample_survives_commits_and_releases_to_latest_target() {
    let mut host = boot();
    let row = id(&host, "row");
    let (hold, _) = host
        .begin_hold(row, Property::Scale, MotionValue::new(83.0, 0.0), 100.0)
        .unwrap()
        .unwrap();
    assert_eq!(hold.value.x, 83.0);
    assert!(host
        .update_hold(hold.token.serial(), MotionValue::new(91.0, 0.0), 110.0)
        .unwrap()
        .is_some());
    let retarget = id(&host, "retarget");
    let committed = host.dispatch_at(retarget, Event::Press, 120.0);
    assert!(
        !committed.contains("\"op\":\"animate\""),
        "held properties cannot animate: {committed}"
    );
    assert!(host.has_hold(hold.token.serial()));
    let release = host
        .end_hold(
            hold.token.serial(),
            HoldEnd::Release {
                velocity: MotionValue::new(-50.0, 0.0),
            },
            130.0,
        )
        .unwrap()
        .unwrap();
    assert!(release.contains("\"values\":[91,"), "{release}");
    assert!(release.contains(",24]}"), "{release}");
    assert!(!host.has_hold(hold.token.serial()));
    assert!(host
        .end_hold(hold.token.serial(), HoldEnd::Cancel, f64::NAN)
        .unwrap()
        .is_none());
}
#[test]
fn stale_completion_cannot_dispatch_and_action_deletion_invalidates_release() {
    let mut host = boot();
    let row = id(&host, "row");
    let (old, _) = host
        .begin_hold(row, Property::Translate, MotionValue::new(75.0, 0.0), 100.0)
        .unwrap()
        .unwrap();
    let (current, _) = host
        .begin_hold(row, Property::Translate, MotionValue::new(81.0, 0.0), 110.0)
        .unwrap()
        .unwrap();
    assert!(!host.has_hold(old.token.serial()));
    assert!(host
        .update_hold(
            old.token.serial(),
            MotionValue::new(f64::NAN, 0.0),
            f64::NAN
        )
        .unwrap()
        .is_none());
    assert!(host.dispatch_held(old.token.serial(), f64::NAN).is_none());
    assert!(host
        .update_hold(current.token.serial(), MotionValue::new(85.0, 0.0), 120.0)
        .unwrap()
        .is_some());
    let batch = host.dispatch_held(current.token.serial(), 120.0).unwrap();
    assert!(batch.contains("\"op\":\"destroy\""), "{batch}");
    assert!(!host.has_hold(current.token.serial()));
    assert!(host
        .end_hold(current.token.serial(), HoldEnd::Cancel, 120.0)
        .unwrap()
        .is_none());
    let replacement = boot();
    assert!(!replacement.has_hold(current.token.serial()));
}
#[test]
fn malformed_live_sample_keeps_hold_clock_and_presentation() {
    let mut host = boot();
    let row = id(&host, "row");
    let (hold, _) = host
        .begin_hold(row, Property::Opacity, MotionValue::scalar(0.4), 100.0)
        .unwrap()
        .unwrap();
    assert!(host
        .update_hold(hold.token.serial(), MotionValue::new(0.2, 1.0), 110.0)
        .is_err());
    assert!(host.has_hold(hold.token.serial()));
    assert_eq!(host.springs().engine().now(), 0.1);
    assert_eq!(
        host.springs()
            .engine()
            .value(hold.token.node(), Property::Opacity)
            .unwrap()
            .x,
        0.4
    );
}

#[test]
fn equal_target_takeover_cancels_and_same_clock_velocity_release_is_not_deduplicated() {
    let mut host = boot();
    let row = id(&host, "row");
    let retarget = id(&host, "retarget");
    host.dispatch_at(retarget, Event::Press, 100.0);
    let (held, cancel) = host
        .begin_hold(row, Property::Scale, MotionValue::scalar(24.0), 100.0)
        .unwrap()
        .unwrap();
    assert!(cancel.contains("\"values\":[]"), "{cancel}");
    let release = host
        .end_hold(
            held.token.serial(),
            HoldEnd::Release {
                velocity: MotionValue::scalar(17.0),
            },
            100.0,
        )
        .unwrap()
        .unwrap();
    assert!(release.contains("\"op\":\"animate\""), "{release}");
    assert!(release.contains("\"values\":[24,"), "{release}");
}

#[test]
fn overdue_timer_retargets_live_hold_without_rewinding_its_clock() {
    let source = SOURCE.replace(
        "  view\n",
        "  task clock mount\n    every(100, retarget)\n  view\n",
    );
    exact_web::link(exact_web_capabilities::ALL);
    let (mut host, _) = Host::boot(
        &contract::compile(&source).unwrap().encode(),
        NoData,
        Default::default(),
        "/",
    )
    .unwrap();
    let row = id(&host, "row");
    let (hold, _) = host
        .begin_hold(row, Property::Scale, MotionValue::scalar(83.0), 150.0)
        .unwrap()
        .unwrap();
    host.update_hold(hold.token.serial(), MotionValue::scalar(91.0), 175.0)
        .unwrap()
        .unwrap();
    // The runner still owes the 100ms timer; the hold is already at 175ms.
    assert_eq!(host.runner().now_ms(), 0.0);
    let batch = host.advance(180.0);
    assert!(!batch.contains("\"op\":\"animate\""), "{batch}");
    assert!(batch.contains("24"), "timer must retarget: {batch}");
    assert!(host.has_hold(hold.token.serial()));
    assert!(host.springs().engine().now() >= 0.175);
    assert_eq!(
        host.springs()
            .engine()
            .value(hold.token.node(), Property::Scale),
        Some(MotionValue::scalar(91.0))
    );
    let release = host
        .end_hold(hold.token.serial(), HoldEnd::Cancel, 180.0)
        .unwrap()
        .unwrap();
    assert!(release.contains("\"values\":[91,"), "{release}");
    assert!(release.contains(",24]}"), "{release}");
}

#[test]
#[ignore = "build a pure Rust web dist with EXACT_WEB_LINK=all; set EXACT_MOTION_DIST and CHROME"]
fn real_wasm_swipe_takeover_style_commits_and_deletion() {
    let source = r##"component App
  state draft = ""
  state count = 0
  state shown = true
  state kill = false
  state blocked = false
  state disabled = false
  state tint = "#ff0000"
  state returning = "translate 0s spring(180, 12, 1) 100ms"
  action edit(value)
    draft = value
    tint = "#0000ff"
  action plain
    returning = "none"
  action arm
    kill = true
  action deactivate
    blocked = true
  action disable
    disabled = true
  action enable
    blocked = false
    disabled = false
  action swipe
    count = count + 1
    shown = not kill
  view
    column width=480 gap=10 padding=10
      input value=draft change=edit testId="input" height=32
      text draft testId="echo"
      text `${count}` testId="count"
      button press=plain testId="plain"
        text "No transition"
      button press=arm testId="arm"
        text "Delete on swipe"
      button press=disable testId="disable"
        text "Disable held row"
      button press=deactivate testId="deactivate"
        text "Switch retained route"
      button press=enable testId="enable"
        text "Enable and return"
      column navigationBack="back" navigationKey=(blocked ? "other" : "held")
        column navigationKey="held" testId="held-route"
          when shown
            column testId="row" disabled=disabled swiperight=swipe touch-action="pan-y" width=300 height=90 background-color=tint transition=returning
              text "Swipe" height=30
              text "Reply" testId="indicator" swipeIndicator=true opacity=0 scale=0.5 transition="opacity spring(180, 12, 1), scale spring(180, 12, 1)"
        column navigationKey="other"
          button id="back" press=enable
            text "Back"
"##;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = std::env::temp_dir().join(format!("exact-motion-browser-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let plan = contract::bake(contract::compile(source).unwrap(), NoData).unwrap();
    std::fs::write(dir.join("app.plan"), plan.encode()).unwrap();
    let output = std::process::Command::new("bun")
        .arg("host/web/tests/holds.mjs")
        .env("EXACT_MOTION_TEST", &dir)
        .current_dir(root)
        .output()
        .unwrap();
    eprintln!("{}", String::from_utf8_lossy(&output.stdout));
    std::fs::remove_dir_all(dir).unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn hold_input_drains_other_advanced_properties_without_animating_the_hold() {
    let source = SOURCE
        .replace(
            "background-color=color",
            "background-color=color opacity=(target == 24 ? 0.5 : 1)",
        )
        .replace(
            "scale spring(180, 12, 1)\"",
            "scale spring(180, 12, 1), opacity spring(180, 12, 1)\"",
        );
    exact_web::link(exact_web_capabilities::ALL);
    let mut host = Host::boot(
        &contract::compile(&source).unwrap().encode(),
        NoData,
        Default::default(),
        "/",
    )
    .unwrap()
    .0;
    let row = id(&host, "row");
    let retarget = id(&host, "retarget");
    host.dispatch_at(retarget, Event::Press, 100.0);
    assert_eq!(host.springs().playing_count(), 2);
    let (held, _) = host
        .begin_hold(row, Property::Translate, MotionValue::new(80.0, 0.0), 100.0)
        .unwrap()
        .unwrap();
    let batch = host
        .update_hold(held.token.serial(), MotionValue::new(82.0, 0.0), 20_000.0)
        .unwrap()
        .unwrap();
    assert!(!batch.contains("\"op\":\"animate\""), "{batch}");
    assert_eq!(
        host.springs().playing_count(),
        0,
        "finished scale/opacity must be drained too"
    );
    assert!(host.has_hold(held.token.serial()));
    assert_eq!(
        host.springs()
            .engine()
            .value(held.token.node(), Property::Opacity)
            .unwrap()
            .x,
        0.5
    );
}
