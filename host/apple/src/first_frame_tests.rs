//! LLP 1003.001 on Apple: with the rule on, motion a commit begins waits for
//! the display's next frame; a lowered play is held at its start in Core
//! Animation until then, and a frame task never moves the engine's input
//! clock past the wall.
use super::*;
use exact_kernel::MonospaceMeasurer;
use exact_motion::Property;
use exact_plan::Value;
use exact_runner::{DataError, Event};

#[derive(Clone, Default)]
struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

const APP: &str = r##"keyframes sweep
  from translate="-24px 0px"
  to translate="64px 8px"
component A
  state on = false
  action go
    on = not on
  view
    column
      button press=go testId="go"
        text "Go"
      box testId="sweep" width=24 height=64 animation=(on ? "sweep 1s linear infinite" : "none")
      box testId="fade" width=24 height=24 opacity=(on ? 0 : 1) transition="opacity 1s linear"
"##;

fn boot() -> Host<NoData> {
    let plan = contract::compile(APP).unwrap().encode();
    let (mut host, _) = Host::boot(
        &plan,
        NoData,
        Box::new(MonospaceMeasurer::default()),
        390.0,
        844.0,
    )
    .unwrap();
    host.svg.box_motion = true;
    host.engine.set_lowered_properties(&svg::lowered(true));
    host.start_on_frame(true, 0.0);
    host
}

fn id(host: &Host<NoData>, test_id: &str) -> u32 {
    let k = host.runner.kernel();
    k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().id
}

fn opacity(host: &Host<NoData>) -> f64 {
    let k = host.runner.kernel();
    let key = k.find_by_test_id("fade")[0];
    host.engine
        .value(exact_kernel::motion::motion_node(key), Property::Opacity)
        .unwrap()
        .x
}

/// The last `animations` spec for `view` in a batch.
fn spec(batch: &str, view: u32) -> serde_json::Value {
    let v: serde_json::Value = serde_json::from_str(batch).unwrap();
    v["ops"]
        .as_array()
        .unwrap()
        .iter()
        .rev()
        .find(|op| op["op"] == "animations" && op["id"] == view)
        .map(|op| op["specs"][0].clone())
        .unwrap_or_else(|| panic!("no specs for {view}: {batch}"))
}

fn motion(batch: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(batch).unwrap()["motion"] == true
}

#[test]
fn a_lowered_play_is_held_at_its_start_until_the_frame_starts_it() {
    let mut host = boot();
    let (go, sweep) = (id(&host, "go"), id(&host, "sweep"));
    let on = host.dispatch_at(go, Event::Press, 100.0);
    assert_eq!(spec(&on, sweep)["h"], 0.0, "held in Core Animation: {on}");
    assert!(motion(&on), "the display link runs to start it");
    let tick = host.tick_at(108.0, 116.0);
    let started = spec(&tick, sweep);
    assert!(started["h"].is_null(), "{tick}");
    assert_eq!(started["s"], 0.116);
}

#[test]
fn a_transition_shows_its_start_at_the_first_frame_then_one_interval() {
    let mut host = boot();
    let go = id(&host, "go");
    host.dispatch_at(go, Event::Press, 100.0);
    host.advance(110.0);
    assert_eq!(opacity(&host), 1.0);
    host.tick_at(112.0, 116.0);
    assert_eq!(opacity(&host), 1.0, "the first frame shows the start");
    host.tick_at(128.0, 132.0);
    assert!((opacity(&host) - 0.984).abs() < 1e-9, "then one interval");
}

#[test]
fn a_display_frame_leaves_the_input_clock_at_the_wall() {
    let mut host = boot();
    host.frame_at(133.0, 120.0);
    assert_eq!(host.engine.now(), 0.120);
    // A touch at 125 ms, behind the frame's target, is heard at its own time
    // and a hold just after it is not refused.
    let go = id(&host, "go");
    host.dispatch_at(go, Event::Press, 125.0);
    assert_eq!(
        host.engine.now(),
        0.125,
        "the touch's own time, not the runner's"
    );
    let fade = host.runner.kernel().find_by_test_id("fade")[0];
    let node = exact_kernel::motion::motion_node(fade);
    let held = host.engine.begin_hold(node, Property::Opacity, 0.126, None);
    assert!(matches!(held, Ok(Some(_))), "{held:?}");
}

#[test]
fn the_bridge_latch_holds_boots_first_plays_and_the_takeover_starts_them() {
    const PULSE: &str = r##"keyframes pulse
  from opacity=0.2
  to opacity=1
component A
  view
    box testId="pulse" width=24 height=24 animation="pulse 1s infinite"
"##;
    let plan = contract::compile(PULSE).unwrap().encode();
    let boot = |on: bool| {
        let mut bridge = crate::abi::Bridge::new();
        bridge.start_on_frame(on, 0.0);
        let len = bridge.boot(&plan, NoData, crate::abi::Hooks::none(), 390., 844.);
        let batch = String::from_utf8_lossy(bridge.output_bytes(len as usize)).into_owned();
        (bridge, batch)
    };
    let view = |batch: &str| {
        let v: serde_json::Value = serde_json::from_str(batch).unwrap();
        let op = v["ops"]
            .as_array()
            .unwrap()
            .iter()
            .find(|op| op["op"] == "animations")
            .cloned();
        op.map(|op| op["specs"][0].clone())
            .unwrap_or_else(|| panic!("no specs: {batch}"))
    };
    let (_, off) = boot(false);
    assert!(
        view(&off)["h"].is_null(),
        "unchanged with the latch off: {off}"
    );
    let (mut bridge, on) = boot(true);
    assert_eq!(
        view(&on)["h"],
        0.0,
        "boot's play waits for the first frame: {on}"
    );
    assert!(motion(&on));
    let len = bridge.start_on_frame(false, 50.0);
    let taken = String::from_utf8_lossy(bridge.output_bytes(len as usize)).into_owned();
    assert_eq!(view(&taken)["s"], 0.05, "{taken}");
    assert!(view(&taken)["h"].is_null());
}

#[test]
fn the_agents_takeover_starts_what_waits_and_its_seeks_start_nothing() {
    let mut host = boot();
    let (go, sweep) = (id(&host, "go"), id(&host, "sweep"));
    host.dispatch_at(go, Event::Press, 100.0);
    host.advance(150.0);
    assert_eq!(opacity(&host), 1.0, "an advance is no frame");
    let taken = host.start_on_frame(false, 200.0);
    assert_eq!(spec(&taken, sweep)["s"], 0.2, "{taken}");
    host.advance(700.0);
    assert!((opacity(&host) - 0.5).abs() < 1e-9);
}

#[test]
fn an_exit_destroys_after_its_own_end_from_the_frame_that_started_it() {
    const LEAVE: &str = r##"keyframes leave
  to opacity=0
component A
  state shown = true
  action hide
    shown = false
  view
    column
      button press=hide testId="hide"
        text "Hide"
      when shown
        box testId="gone" width=24 height=24 -exact-exit-animation="leave 300ms linear both"
"##;
    let plan = contract::compile(LEAVE).unwrap().encode();
    let (mut host, _) = Host::boot(
        &plan,
        NoData,
        Box::new(MonospaceMeasurer::default()),
        390.0,
        844.0,
    )
    .unwrap();
    host.start_on_frame(true, 0.0);
    let gone = id(&host, "gone");
    let destroyed = |batch: &str| batch.contains(&format!("{{\"op\":\"destroy\",\"id\":{gone}}}"));
    let hide = id(&host, "hide");
    assert!(!destroyed(&host.dispatch_at(hide, Event::Press, 100.0)));
    assert!(
        !destroyed(&host.tick_at(110.0, 116.0)),
        "the exit starts at the frame"
    );
    assert!(
        !destroyed(&host.tick_at(400.0, 405.0)),
        "300 ms from the commit is not its end"
    );
    assert!(
        destroyed(&host.tick_at(410.0, 416.0)),
        "300 ms from the frame is"
    );
}
