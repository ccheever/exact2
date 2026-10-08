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
fn a_frame_task_leaves_the_input_clock_at_the_wall() {
    let mut host = boot();
    host.frame_at(133.0, 120.0);
    assert_eq!(host.engine.now(), 0.120);
    // A touch at 125 ms, behind the frame's target, is not refused.
    let go = id(&host, "go");
    host.dispatch_at(go, Event::Press, 125.0);
    assert_eq!(
        host.engine.now(),
        0.125,
        "the touch's own time, not the runner's"
    );
    let sweep = host.runner.kernel().find_by_test_id("sweep")[0];
    let node = exact_kernel::motion::motion_node(sweep);
    assert!(host
        .engine
        .begin_hold(node, Property::Translate, 0.126, None)
        .is_ok());
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
