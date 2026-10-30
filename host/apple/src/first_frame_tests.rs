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

/// LLP 1055.001 §3: iOS hands a box's opacity transition to Core Animation
/// at the frame that starts it, and takes it back when it is retargeted.
#[test]
fn core_animation_plays_a_started_transition_until_it_is_retargeted() {
    let mut host = boot();
    host.svg.play_transitions = true;
    let (go, fade) = (id(&host, "go"), id(&host, "fade"));
    let key = exact_kernel::motion::motion_node(host.runner.kernel().node(fade).unwrap().key);
    let pressed = host.dispatch_at(go, Event::Press, 100.0);
    assert!(
        !host.engine.is_played(key, Property::Opacity),
        "it waits for its frame"
    );
    assert!(motion(&pressed), "the display link runs to start it");
    let started = host.tick_at(112.0, 116.0);
    assert!(host.engine.is_played(key, Property::Opacity));
    assert_eq!(opacity(&host), 0.0, "the engine presents the target");
    let played = spec(&started, fade);
    assert_eq!(played["k"], "opacity");
    assert_eq!(played["s"], 0.116);
    assert_eq!(played["d"], 1.0);
    assert_eq!(played["v"], serde_json::json!([1, 0]));
    assert_eq!(played["fill"], 2, "a delay shows its start");
    assert!(!motion(&started), "nothing is sampled: {started}");
    // Turned back 184 ms in: the engine samples the new curve until its
    // frame, and Core Animation lets the old one go in the same batch.
    let back = host.dispatch_at(go, Event::Press, 300.0);
    assert!(!host.engine.is_played(key, Property::Opacity));
    let v: serde_json::Value = serde_json::from_str(&back).unwrap();
    let dropped = v["ops"]
        .as_array()
        .unwrap()
        .iter()
        .rev()
        .find(|op| op["op"] == "animations" && op["id"] == fade);
    assert_eq!(
        dropped.map(|op| op["specs"].clone()),
        Some(serde_json::json!([])),
        "{back}"
    );
    let again = host.tick_at(312.0, 316.0);
    let replayed = spec(&again, fade);
    assert_eq!(replayed["s"], 0.316);
    assert_eq!(replayed["v"][1], 1);
    assert!(
        (replayed["v"][0].as_f64().unwrap() - 0.816).abs() < 1e-3,
        "{replayed}"
    );
}

const PLAYED: &str = r##"component A
  state on = false
  state fogged = false
  action go
    on = not on
  action fog
    fogged = true
  view
    column
      button press=go testId="go"
        text "Go"
      button press=fog testId="fog"
        text "Fog"
      column testId="wrap" filter=(fogged ? "blur(4px)" : "none")
        box testId="fade" width=24 height=24 opacity=(on ? 0 : 1) transition="opacity 1s linear"
      box testId="clipped" width=24 height=24 padding=4 background-clip="content-box" background-color=(on ? "#ff0000" : "#0000ff") transition="background-color 1s linear"
      box testId="round" width=24 height=24 border-radius="50%" background-color=(on ? "#ff0000" : "#0000ff") transition="background-color 1s linear"
      box testId="plain" width=24 height=24 background-color=(on ? "#ff0000" : "#0000ff") transition="background-color 1s linear"
"##;

/// Review round 1 (Astra 2, 3): a background colour goes to Core Animation
/// only where the layer paints it as UIKit shows it, and a played fade an
/// ancestor's new filter would capture at its end comes back to the engine
/// where its curve is, its spec withdrawn in the same batch.
#[test]
fn what_core_animation_cannot_play_as_uikit_draws_it_stays_sampled() {
    let plan = contract::compile(PLAYED).unwrap().encode();
    let (mut host, _) = Host::boot(
        &plan,
        NoData,
        Box::new(MonospaceMeasurer::default()),
        390.0,
        844.0,
    )
    .unwrap();
    host.svg.box_motion = true;
    host.svg.play_transitions = true;
    host.engine.set_lowered_properties(&svg::lowered(true));
    host.start_on_frame(true, 0.0);
    let key = |host: &Host<NoData>, test_id: &str| {
        let k = host.runner.kernel();
        exact_kernel::motion::motion_node(k.find_by_test_id(test_id)[0])
    };
    let go = id(&host, "go");
    host.dispatch_at(go, Event::Press, 100.0);
    host.tick_at(112.0, 116.0);
    let played = |host: &Host<NoData>, test_id: &str, p: Property| {
        host.engine.is_played(key(host, test_id), p)
    };
    assert!(played(&host, "fade", Property::Opacity));
    assert!(played(&host, "plain", Property::BackgroundColor));
    assert!(
        !played(&host, "clipped", Property::BackgroundColor),
        "UIKit draws a clipped background"
    );
    assert!(
        !played(&host, "round", Property::BackgroundColor),
        "a percentage radius can be elliptical"
    );
    let fade = id(&host, "fade");
    let fogged = host.dispatch_at(id(&host, "fog"), Event::Press, 300.0);
    assert!(
        !played(&host, "fade", Property::Opacity),
        "a filter draws it from the model"
    );
    let shown = host
        .engine
        .value(key(&host, "fade"), Property::Opacity)
        .unwrap()
        .x;
    assert!(
        (shown - 0.816).abs() < 1e-3,
        "where the curve is, not its end: {shown}"
    );
    let v: serde_json::Value = serde_json::from_str(&fogged).unwrap();
    let withdrawn = v["ops"]
        .as_array()
        .unwrap()
        .iter()
        .rev()
        .find(|op| op["op"] == "animations" && op["id"] == fade);
    assert_eq!(
        withdrawn.map(|op| op["specs"].clone()),
        Some(serde_json::json!([])),
        "{fogged}"
    );
    assert!(motion(&fogged), "the display drives it again");
}

/// Review round 2 (Astra 3): when the agent takes the clock, what Core
/// Animation played comes back to the engine where its curve is, and its
/// spec goes in the takeover's batch; nothing new is handed over after.
#[test]
fn the_agents_takeover_reclaims_what_core_animation_plays() {
    let mut host = boot();
    host.svg.play_transitions = true;
    let (go, fade) = (id(&host, "go"), id(&host, "fade"));
    let key = exact_kernel::motion::motion_node(host.runner.kernel().node(fade).unwrap().key);
    host.dispatch_at(go, Event::Press, 100.0);
    host.tick_at(112.0, 116.0);
    assert!(host.engine.is_played(key, Property::Opacity));
    let taken = host.start_on_frame(false, 300.0);
    assert!(!host.engine.is_played(key, Property::Opacity));
    assert!((opacity(&host) - 0.816).abs() < 1e-3, "where the curve is");
    let v: serde_json::Value = serde_json::from_str(&taken).unwrap();
    let withdrawn = v["ops"]
        .as_array()
        .unwrap()
        .iter()
        .rev()
        .find(|op| op["op"] == "animations" && op["id"] == fade);
    assert_eq!(
        withdrawn.map(|op| op["specs"].clone()),
        Some(serde_json::json!([])),
        "{taken}"
    );
    host.dispatch_at(go, Event::Press, 400.0);
    host.advance(500.0);
    assert!(
        !host.engine.is_played(key, Property::Opacity),
        "the agent's clock samples"
    );
}

/// Review round 2 (Astra 2): a played spring's track is made once and kept
/// while the transition stands, and dropped with it.
#[test]
fn a_played_springs_spec_is_made_once() {
    const SPRING: &str = r##"component A
  state on = false
  action go
    on = not on
  view
    column
      button press=go testId="go"
        text "Go"
      box testId="fade" width=24 height=24 opacity=(on ? 0 : 1) transition="opacity -exact-spring(300, 30, 1)"
"##;
    let plan = contract::compile(SPRING).unwrap().encode();
    let (mut host, _) = Host::boot(
        &plan,
        NoData,
        Box::new(MonospaceMeasurer::default()),
        390.0,
        844.0,
    )
    .unwrap();
    host.svg.box_motion = true;
    host.svg.play_transitions = true;
    host.engine.set_lowered_properties(&svg::lowered(true));
    host.start_on_frame(true, 0.0);
    let (go, fade) = (id(&host, "go"), id(&host, "fade"));
    host.dispatch_at(go, Event::Press, 100.0);
    let started = host.tick_at(112.0, 116.0);
    let first = spec(&started, fade);
    assert!(
        first["v"].as_array().unwrap().len() > 2,
        "a spring is its frames: {first}"
    );
    assert_eq!(host.svg.played_specs.len(), 1);
    let later = host.tick_at(140.0, 150.0);
    assert!(
        !later.contains("\"animations\""),
        "an unchanged spec is not sent again: {later}"
    );
    host.advance(5000.0);
    host.tick_at(5000.0, 5016.0);
    assert!(
        host.svg.played_specs.is_empty(),
        "dropped once the spring ended"
    );
}

/// D5: a display frame's batch starts what waits, as its tick would, so
/// a transition reaches Core Animation in the frame's own batch.
#[test]
fn a_display_frame_starts_what_waits_in_its_own_batch() {
    for admitted in [false, true] {
        let mut host = boot();
        host.svg.play_transitions = true;
        let (go, fade) = (id(&host, "go"), id(&host, "fade"));
        host.dispatch_at(go, Event::Press, 100.0);
        let framed = if admitted {
            host.admitted_frame(133.0, 120.0)
        } else {
            host.frame_at(133.0, 120.0)
        };
        assert_eq!(spec(&framed, fade)["s"], 0.133, "{framed}");
        assert!(!motion(&framed), "no tick is owed: {framed}");
        assert_eq!(
            host.engine.now(),
            0.120,
            "the input clock stays at the wall"
        );
    }
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
    // The host admits a hold (a drag's) at a wall time behind the frame.
    let fade = id(&host, "fade");
    let held = host.hold_begin(fade, Property::Opacity, 126.0);
    assert!(
        held.contains("\"token\"") && held.contains("\"error\":null"),
        "{held}"
    );
    assert_eq!(host.engine.now(), 0.126);
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
      if shown
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

#[test]
fn a_timer_advance_after_a_display_frame_keeps_the_wall() {
    let mut host = boot();
    host.frame_at(133.0, 120.0);
    host.advance(125.0);
    assert_eq!(host.now_ms, 125.0, "the wall, not the runner's target");
    // The host admits a hold (a drag's) at a wall time behind the frame.
    let fade = id(&host, "fade");
    let held = host.hold_begin(fade, Property::Opacity, 126.0);
    assert!(
        held.contains("\"token\"") && held.contains("\"error\":null"),
        "{held}"
    );
    assert_eq!(host.engine.now(), 0.126);
}

#[test]
fn the_takeover_presents_its_own_instant() {
    let mut host = boot();
    let go = id(&host, "go");
    host.dispatch_at(go, Event::Press, 100.0);
    host.tick_at(110.0, 116.0);
    host.tick_at(140.0, 216.0); // a frame ahead of the wall
    host.start_on_frame(false, 200.0);
    assert!(
        (opacity(&host) - 0.916).abs() < 1e-9,
        "84 ms in at the takeover, not 100"
    );
}

#[test]
fn after_the_takeover_the_hosts_clock_is_the_agents() {
    let mut host = boot();
    let go = id(&host, "go");
    host.dispatch_at(go, Event::Press, 100.0);
    host.start_on_frame(false, 200.0);
    assert_eq!(host.now_ms, 200.0);
    let flipped = host.set_scheme(true);
    assert!(flipped.contains("\"error\":null"), "{flipped}");
}

const ACCORDION: &str = r##"component A
  state open = false
  action toggle
    open = not open
  view
    column width="100%" height="100%" interpolate-size="allow-keywords"
      button testId="toggle" press=toggle
        text "Toggle"
      column testId="body" height=(open ? "auto" : "0px") box-sizing="border-box" overflow="hidden" transition="height 200ms linear"
        box height=120
      box testId="following" width=20 height=20 -exact-layout-transition="200ms linear"
"##;

fn accordion() -> Host<NoData> {
    let plan = contract::compile(ACCORDION).unwrap().encode();
    let (mut host, _) = Host::boot(
        &plan,
        NoData,
        Box::new(MonospaceMeasurer::default()),
        390.0,
        844.0,
    )
    .unwrap();
    host.start_on_frame(true, 0.0);
    host
}

fn height(host: &Host<NoData>, test_id: &str) -> f32 {
    host.runner
        .kernel()
        .node(id(host, test_id))
        .unwrap()
        .frame
        .height
}

#[test]
fn a_curve_a_tick_begins_starts_at_that_tick_s_frame() {
    let mut host = accordion();
    let toggle = id(&host, "toggle");
    host.dispatch_at(toggle, Event::Press, 100.0);
    host.tick_at(110.0, 116.0);
    // The height moves in this tick; its layout moves `following`, whose
    // layout transition the tick itself begins.
    host.tick_at(126.0, 132.0);
    assert!(height(&host, "body") > 0.0);
    let following =
        exact_kernel::motion::motion_node(host.runner.kernel().find_by_test_id("following")[0]);
    assert_eq!(
        host.engine.curve_start(following, Property::Layout),
        Some(0.132),
        "started in the frame that began it, not waiting for the next"
    );
}

#[test]
fn the_takeover_settles_a_height_it_ends() {
    let mut host = accordion();
    let toggle = id(&host, "toggle");
    host.dispatch_at(toggle, Event::Press, 100.0);
    host.tick_at(110.0, 116.0);
    host.tick_at(126.0, 132.0);
    host.start_on_frame(false, 500.0);
    assert_eq!(
        height(&host, "body"),
        120.0,
        "the takeover's tick laid the end out"
    );
}

#[test]
fn a_prepared_host_taken_over_before_its_commit_replaces_its_held_specs_next() {
    const PULSE: &str = r##"keyframes pulse
  from opacity=0.2
  to opacity=1
component A
  view
    box testId="pulse" width=24 height=24 animation="pulse 1s infinite"
"##;
    let plan = contract::compile(PULSE).unwrap().encode();
    let spec = |bridge: &crate::abi::Bridge<NoData>, len: u32| {
        let batch = String::from_utf8_lossy(bridge.output_bytes(len as usize)).into_owned();
        let v: serde_json::Value = serde_json::from_str(&batch).unwrap();
        let op = v["ops"]
            .as_array()
            .unwrap()
            .iter()
            .find(|op| op["op"] == "animations")
            .cloned();
        (op.map(|op| op["specs"][0].clone()), batch)
    };
    let mut bridge = crate::abi::Bridge::new();
    bridge.start_on_frame(true, 0.0);
    bridge.input_write(&plan);
    bridge.prepare_plan(plan.len(), NoData, crate::abi::Hooks::none(), 390., 844.);
    bridge.start_on_frame(false, 50.0);
    let len = bridge.commit_plan();
    let (stored, batch) = spec(&bridge, len);
    assert_eq!(
        stored.expect("specs")["h"],
        0.0,
        "the stored batch, as prepared: {batch}"
    );
    let len = bridge.tick(60.0);
    let (next, batch) = spec(&bridge, len);
    let next = next.unwrap_or_else(|| panic!("the next batch replaces it: {batch}"));
    assert!(next["h"].is_null(), "{batch}");
    assert_eq!(next["s"], 0.05);
}

#[test]
fn an_input_a_live_clock_overtook_is_presented_at_the_inputs_time() {
    let mut host = boot();
    host.start_on_frame(false, 0.0);
    host.runner.set_live_clock(Some(Box::new(|| 2600.0)));
    let go = id(&host, "go");
    let batch = host.dispatch_at(go, Event::Press, 2000.0);
    assert!(!batch.contains("\"error\":\""), "{batch}");
    assert_eq!(
        host.runner.now_ms(),
        2600.0,
        "the action observed the clock"
    );
    // The fade starts at the input's time: a release at 2000 can still end
    // a hold there.
    assert_eq!(host.engine.now(), 2.0);
}

/// A box a timer moves while another timer and a frame task run: one
/// `clock +N` crosses all of them.
const CAPSULE: &str = r##"component A
  state small = false
  state go = false
  state beats = 0
  state frames = 0
  action start
    go = true
  action shrink
    small = true
  action beat
    beats = beats + 1
  action frame
    frames = frames + 1
  task later when go
    after(100, shrink)
  task beating when go
    every(200, beat)
  task framing when go
    every(frame, frame)
  view
    column width="100%" height="100%" position="relative"
      button testId="go" press=start
        text "Go"
      row testId="capsule" position="absolute" right=(small ? 68 : 14) bottom=14 -exact-layout-transition="400ms linear"
        box width=(small ? 48 : 180) height=48
"##;

/// The capsule's presented left edge, and its laid-out one.
fn capsule_x(host: &Host<NoData>) -> (f64, f32) {
    let k = host.runner.kernel();
    let key = k.find_by_test_id("capsule")[0];
    let shown = host
        .engine
        .value(exact_kernel::motion::motion_node(key), Property::Layout)
        .unwrap()
        .x;
    (shown, k.node_by_key(key).unwrap().frame.x)
}

/// `clock +N` as the agent runs it (`Agent.advanceRunner`, `clock_stepped`):
/// the request stops it took.
fn agent_jump(host: &mut Host<NoData>, to: f64) -> usize {
    let mut stops = 0;
    while stops < 4096 && host.runner.timer_due_ms().is_some_and(|d| d <= to) {
        host.advance_until_request(to);
        stops += 1;
    }
    host.advance(to);
    stops
}

#[test]
fn a_box_a_timer_moves_inside_an_agents_jump_starts_moving_at_the_timer() {
    let plan = contract::compile(CAPSULE).unwrap().encode();
    let (mut host, _) = Host::boot(
        &plan,
        NoData,
        Box::new(MonospaceMeasurer::default()),
        390.0,
        844.0,
    )
    .unwrap();
    host.start_on_frame(false, 0.0);
    let go = id(&host, "go");
    host.dispatch_at(go, Event::Press, 0.0);
    assert_eq!(capsule_x(&host), (196.0, 196.0));
    // Right-anchored: it shrinks from 180 to 48 wide at 100 ms and its left
    // edge goes from 196 to 274, half way at 300 ms, whatever else fired.
    assert_eq!(agent_jump(&mut host, 300.0), 1, "no request stopped it");
    let (shown, laid) = capsule_x(&host);
    assert_eq!(laid, 274.0);
    assert!(
        (shown - 235.0).abs() < 1e-6,
        "half way at 300 ms, not {shown}"
    );
    agent_jump(&mut host, 1000.0);
    assert_eq!(capsule_x(&host), (274.0, 274.0));
    // A frame task's every frame for two minutes is one call, not a
    // request stop each (the agent's 4,096 would end at 68 s).
    assert_eq!(agent_jump(&mut host, 121_000.0), 1);
    assert_eq!(host.runner.now_ms(), 121_000.0);
}

/// A box under a layout transition, and two timers that each send.
const SENDS: &str = r##"shape Reply
  text: string
component A
  state go = false
  state small = false
  state seen = ""
  mutation loaded as shape Reply then note
  action start
    go = true
  action load
    small = true
    send loaded = fetchOne("a")
  action loadAgain
    send loaded = fetchOne("b")
  action note
    match loaded
      case some(r)
        seen = `${seen}${r.text}`
      case none
        seen = seen
  task beating when go
    every(1000, load)
  task again when go
    after(1500, loadAgain)
  view
    column width="100%" height="100%" position="relative"
      button testId="go" press=start
        text "Go"
      row testId="capsule" position="absolute" right=(small ? 68 : 14) bottom=14 -exact-layout-transition="400ms linear"
        view width=(small ? 48 : 180) height=48
"##;

/// Each `fetchOne(word)` a request whose reply is `word`.
#[derive(Default)]
struct Fetches;
impl exact_runner::DataSource for Fetches {
    fn query(
        &mut self,
        source: &str,
        _: &[exact_plan::Value],
    ) -> Result<exact_plan::Value, exact_runner::DataError> {
        Err(exact_runner::DataError::UnknownSource(source.into()))
    }
    fn answer(
        &mut self,
        _: &mut exact_runner::Store,
        _: &str,
        args: &[exact_plan::Value],
    ) -> Result<exact_runner::Answer, exact_runner::DataError> {
        let word = args[0].as_str().unwrap_or_default();
        Ok(exact_runner::Answer::Later(exact_runner::Request::get(
            &format!("https://fixture.invalid/{word}"),
        )))
    }
    fn parse(
        &mut self,
        _: &mut exact_runner::Store,
        _: &str,
        _: &[exact_plan::Value],
        outcome: exact_runner::Outcome,
    ) -> Result<exact_runner::Answer, exact_runner::DataError> {
        let exact_runner::Outcome::Storage(bytes) = outcome else {
            panic!("a storage outcome")
        };
        Ok(exact_runner::Answer::Now(exact_plan::Value::record(vec![
            exact_plan::Value::str(std::str::from_utf8(&bytes).unwrap()),
        ])))
    }
}

#[test]
fn a_jump_stepping_for_a_layout_transition_still_stops_at_each_send() {
    let plan = contract::compile(SENDS).unwrap().encode();
    let (mut host, _) = Host::boot(
        &plan,
        Fetches,
        Box::new(MonospaceMeasurer::default()),
        390.0,
        844.0,
    )
    .unwrap();
    host.start_on_frame(false, 0.0);
    let go = id_of(&host, "go");
    host.dispatch_at(go, Event::Press, 0.0);
    // `clock +1800` as the agent runs it: each stop's request is answered
    // before the jump goes on (LLP 1016 D5).
    let mut stops = Vec::new();
    while host.runner.timer_due_ms().is_some_and(|d| d <= 1800.0) {
        host.advance_until_request(1800.0);
        for r in host.take_requests() {
            let word = r.request.url.rsplit('/').next().unwrap().to_string();
            stops.push((host.runner.now_ms(), word.clone()));
            let landed = host.fulfill_all(
                vec![(
                    r.ticket,
                    exact_runner::Outcome::Storage(word.into_bytes()),
                    None,
                )],
                host.runner.now_ms(),
            );
            assert!(landed.contains("\"error\":null"), "{landed}");
        }
    }
    host.advance(1800.0);
    assert_eq!(
        stops,
        [(1000.0, "a".to_string()), (1500.0, "b".to_string())]
    );
    assert_eq!(
        host.runner.slot("seen"),
        Some(&exact_plan::Value::str("ab")),
        "both replies landed"
    );
}

fn id_of<D: exact_runner::DataSource>(host: &Host<D>, test_id: &str) -> u32 {
    let k = host.runner.kernel();
    k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().id
}

#[test]
fn a_delayed_stepped_transition_fills_with_its_uneased_start() {
    for easing in ["steps(4, jump-start)", "linear(0 25%, 1 75%)"] {
        let app = format!(
            r#"component A
  state on = false
  action go
    on = true
  view
    column
      button "Go" testId="go" press=go
      box testId="fade" width=24 height=24 opacity=(on ? 0 : 1) transition="opacity 1s {easing} 500ms"
"#
        );
        let plan = contract::compile(&app).unwrap().encode();
        let (mut host, _) = Host::boot(
            &plan,
            NoData,
            Box::new(MonospaceMeasurer::default()),
            390.0,
            844.0,
        )
        .unwrap();
        host.svg.box_motion = true;
        host.svg.play_transitions = true;
        host.engine.set_lowered_properties(&svg::lowered(true));
        host.start_on_frame(true, 0.0);
        host.dispatch_at(id(&host, "go"), Event::Press, 100.0);
        let started = host.tick_at(112.0, 116.0);
        let played = spec(&started, id(&host, "fade"));
        assert_eq!(
            played["v"][0], 1.0,
            "{easing}: backwards fill holds from during the delay: {played}"
        );
        assert_eq!(played["t"][0], 0.0);
    }
}

#[test]
fn box_transform_transitions_play_without_sampling_and_reclaim_for_the_agent() {
    let app = r##"component A
  state on = false
  action go
    on = not on
  view
    column
      button press=go testId="go"
        text "Go"
      box testId="move" width=100 height=40 translate=(on ? "50% 10px" : "0px 0px") transition="translate 1s linear"
      box testId="turn" width=40 height=40 rotate=(on ? 180 : 0) transition="rotate 1s linear"
      box testId="grow" width=40 height=40 scale=(on ? 2 : 1) transition="scale -exact-spring(200, 20, 1)"
      box testId="hit" width=40 height=40 press=go translate=(on ? "40px 0px" : "0px 0px") transition="translate 1s linear"
      box testId="invoker-parent" width=100 height=40 translate=(on ? "40px 0px" : "0px 0px") transition="translate 1s linear"
        button popovertarget="more"
          text "More"
      box id="more" popover="auto"
        text "Menu"
      box testId="corner" width=40 height=40 transform-origin="0% 0%" rotate=(on ? 180 : 0) transition="rotate 1s linear"
"##;
    let plan = contract::compile(app).unwrap().encode();
    let (mut host, _) = Host::boot(
        &plan,
        NoData,
        Box::new(MonospaceMeasurer::default()),
        390.0,
        844.0,
    )
    .unwrap();
    host.svg.box_motion = true;
    host.svg.play_transitions = true;
    host.engine.set_lowered_properties(&svg::lowered(true));
    host.start_on_frame(true, 0.0);
    host.dispatch_at(id(&host, "go"), Event::Press, 100.0);
    let batch = host.tick_at(112.0, 116.0);
    let key = |host: &Host<NoData>, name| {
        exact_kernel::motion::motion_node(host.runner.kernel().find_by_test_id(name)[0])
    };
    for (name, property, path) in [
        ("move", Property::Translate, "transform.translation.x"),
        ("turn", Property::Rotate, "transform.rotation.z"),
        ("grow", Property::Scale, "transform.scale"),
    ] {
        assert!(
            host.engine.is_played(key(&host, name), property),
            "{name}: {batch}"
        );
        assert_eq!(spec(&batch, id(&host, name))["k"], path);
    }
    assert_eq!(
        spec(&batch, id(&host, "move"))["v"],
        serde_json::json!([0, 50])
    );
    let radians = spec(&batch, id(&host, "turn"))["v"][1].as_f64().unwrap();
    assert!((radians - std::f64::consts::PI).abs() < 1e-5);
    for (name, property) in [
        ("hit", Property::Translate),
        ("invoker-parent", Property::Translate),
        ("corner", Property::Rotate),
    ] {
        assert!(
            !host.engine.is_played(key(&host, name), property),
            "{name} must sample"
        );
    }
    host.start_on_frame(false, 300.0);
    host.tick_at(300.0, 300.0);
    assert!(!host
        .engine
        .is_played(key(&host, "move"), Property::Translate));
    let x = host
        .engine
        .value(key(&host, "move"), Property::Translate)
        .unwrap();
    assert!((x.z - 9.2).abs() < 1e-4, "{x:?}");
}

#[test]
fn sticky_translation_transitions_stay_sampled_for_host_placement() {
    let plan = contract::compile(include_str!("../tests/fixtures/sticky-transition.contract"))
        .unwrap()
        .encode();
    let (mut host, _) = Host::boot(
        &plan,
        NoData,
        Box::new(MonospaceMeasurer::default()),
        300.0,
        500.0,
    )
    .unwrap();
    host.svg.box_motion = true;
    host.svg.play_transitions = true;
    host.engine.set_lowered_properties(&svg::lowered(true));
    host.start_on_frame(true, 0.0);
    host.dispatch_at(id(&host, "move"), Event::Press, 100.0);
    host.tick_at(112.0, 116.0);
    let batch = host.tick_at(612.0, 616.0);
    let node = host.runner.kernel().node(id(&host, "pinned")).unwrap();
    let key = exact_kernel::motion::motion_node(node.key);
    assert!(!host.engine.is_played(key, Property::Translate), "{batch}");
    assert_eq!(host.engine.value(key, Property::Translate).unwrap().x, 50.0);
    assert!(
        motion(&batch),
        "the host needs a sample while placement can change"
    );
}

#[test]
fn canvas_only_live_ticks_keep_drawing_without_firing_timers() {
    use exact_runner::exact_canvas::{Context2d, DrawError, Frame};
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    struct Tile(Rc<RefCell<Vec<f64>>>);
    impl DataSource for Tile {
        fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
            Err(DataError::UnknownSource(source.into()))
        }
        fn canvas_surfaces(&self) -> Vec<(String, usize)> {
            vec![("tile".into(), 0)]
        }
        fn draw_2d(
            &mut self,
            _: &str,
            _: &[Value],
            ctx: &Context2d,
            frame: &Frame,
        ) -> Result<bool, DrawError> {
            self.0.borrow_mut().push(frame.time);
            ctx.clear_rect(0., 0., frame.width, frame.height);
            ctx.fill_rect(frame.time % 100., 0., 4., 4.);
            Ok(true)
        }
    }

    let plan = contract::compile(
        r#"component App
  state ticks = 0
  task timer mount
    every(1000, beat)
  action beat
    ticks = ticks + 1
  view
    canvas surface=tile() width=100 height=48
"#,
    )
    .unwrap()
    .encode();
    for deferred in [false, true] {
        let times = Rc::new(RefCell::new(Vec::new()));
        let (mut host, _) = Host::boot(
            &plan,
            Tile(times.clone()),
            Box::new(MonospaceMeasurer::default()),
            390.,
            844.,
        )
        .unwrap();
        host.set_canvas_deferred(deferred);
        let wall = Rc::new(Cell::new(0.));
        let clock = wall.clone();
        host.runner
            .set_live_clock(Some(Box::new(move || clock.get())));
        times.borrow_mut().clear();
        for at in [100., 116., 132., 2100.] {
            wall.set(at);
            let batch = host.tick_at(at, at + 8.);
            let batch: serde_json::Value = serde_json::from_str(&batch).unwrap();
            assert!(
                batch["ops"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|op| op["op"] == "canvas2d"),
                "{batch}"
            );
        }
        assert_eq!(*times.borrow(), [100., 116., 132., 2100.]);
        assert_eq!(host.runner.slot("ticks"), Some(&Value::Number(0.)));
        host.tick_at(2100., 2108.);
        assert_eq!(
            times.borrow().len(),
            4,
            "duplicate clock values do not draw again"
        );
    }
}
