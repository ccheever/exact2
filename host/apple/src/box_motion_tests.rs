//! LLP 1055.001 on iOS: a box's `translate`, `scale`, `rotate` and
//! `background-color` keyframes go to Core Animation where it plays them as
//! CSS does, and the engine keeps no frames busy for them; the rest are
//! sampled as before. The host here is flipped to iOS's lowering after boot.
use super::*;
use exact_kernel::MonospaceMeasurer;
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
keyframes spin
  from rotate=0
  to rotate=360
keyframes pulse
  from scale=0.5
  to scale=1.2
keyframes glow
  from background-color="#16a34a"
  to background-color="#2563eb"
keyframes fade
  from background-color="#16a34a"
  to background-color="rgba(37, 99, 235, 0.5)"
keyframes drop
  from cy=10
  to cy=30
component A
  state on = false
  action go
    on = not on
  view
    column
      button press=go testId="go"
        text "Go"
      box testId="sweep" width=24 height=64 animation=(on ? "sweep 1s linear infinite" : "none")
      box testId="spin" width=32 height=12 animation=(on ? "spin 1200ms linear infinite" : "none")
      box testId="pulse" width=32 height=32 animation=(on ? "pulse 800ms ease-in-out infinite alternate" : "none")
      box testId="glow" width=64 height=64 background-color="#f1f5f9" animation=(on ? "glow 1s ease infinite alternate" : "none")
      box testId="corner" width=32 height=8 transform-origin="0% 0%" animation=(on ? "spin 2s linear infinite" : "none")
      box testId="pressed" width=32 height=32 press=go animation=(on ? "sweep 1s linear infinite" : "none")
      box testId="holder" width=64 height=64 animation=(on ? "spin 1s linear infinite" : "none")
        button press=go testId="inner"
          text "In"
      box testId="framed" width=64 height=64 border-width=2 border-style="solid" border-color="#0f172a" animation=(on ? "glow 1s ease infinite" : "none")
      box testId="fading" width=64 height=64 animation=(on ? "fade 1s ease infinite" : "none")
      svg width=40 height=40 viewBox="0 0 40 40"
        rect testId="shape" x=10 y=10 width=20 height=20 fill="#0ea5e9" transform-box="fill-box" transform-origin="center" animation=(on ? "spin 1s linear infinite" : "none")
        circle testId="ball" cx=20 cy=10 r=4 fill="#f97316" animation=(on ? "drop 1s linear infinite" : "none")
        circle testId="clipped" cx=20 cy=10 r=4 filter="blur(1px)" animation=(on ? "drop 1s linear infinite" : "none")
        defs
          filter id="soft"
            feGaussianBlur stdDeviation=1
        g filter="url(#soft)"
          rect testId="inside" x=10 y=10 width=8 height=8 transform-box="fill-box" transform-origin="center" animation=(on ? "spin 1s linear infinite" : "none")
        g filter="blur(1px)"
          rect testId="followed" x=10 y=10 width=8 height=8 transform-box="fill-box" transform-origin="center" animation=(on ? "spin 1s linear infinite" : "none")
"##;

fn id(host: &Host<NoData>, test_id: &str) -> u32 {
    let k = host.runner.kernel();
    k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().id
}

/// The `animations` op's specs for `view` in a batch, if any.
fn specs(batch: &str, view: u32) -> Option<Vec<serde_json::Value>> {
    let v: serde_json::Value = serde_json::from_str(batch).unwrap();
    v["ops"]
        .as_array()
        .unwrap()
        .iter()
        .rev()
        .find(|op| op["op"] == "animations" && op["id"] == view)
        .map(|op| op["specs"].as_array().cloned().unwrap_or_default())
}

fn keys(batch: &str, view: u32) -> Vec<String> {
    specs(batch, view)
        .unwrap_or_default()
        .iter()
        .map(|s| s["k"].as_str().unwrap().to_string())
        .collect()
}

fn ios(host: &mut Host<NoData>) {
    host.svg.box_motion = true;
    host.engine.set_lowered_properties(&svg::lowered(true));
}

#[test]
fn a_boxs_transform_and_colour_keyframes_play_in_core_animation() {
    let plan = contract::compile(APP).unwrap().encode();
    let (mut host, _) = Host::boot(
        &plan,
        NoData,
        Box::new(MonospaceMeasurer::default()),
        390.0,
        844.0,
    )
    .unwrap();
    ios(&mut host);
    let go = id(&host, "go");
    let on = host.dispatch_at(go, Event::Press, 100.0);
    assert_eq!(
        keys(&on, id(&host, "sweep")),
        ["transform.translation.x", "transform.translation.y"],
        "{on}"
    );
    assert_eq!(keys(&on, id(&host, "spin")), ["transform.rotation.z"]);
    assert_eq!(keys(&on, id(&host, "pulse")), ["transform.scale"]);
    assert_eq!(keys(&on, id(&host, "glow")), ["backgroundColor"]);
    // A turn in radians, over one linear interval; the y axis its own track.
    let spin = specs(&on, id(&host, "spin")).unwrap();
    let v: Vec<f64> = spin[0]["v"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_f64().unwrap())
        .collect();
    assert_eq!(v.len(), 2);
    assert!((v[1] - std::f64::consts::TAU).abs() < 1e-4, "{v:?}");
    let sweep = specs(&on, id(&host, "sweep")).unwrap();
    assert_eq!(
        sweep[1]["v"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_f64().unwrap())
            .collect::<Vec<_>>(),
        [0.0, 8.0]
    );
    // An SVG element's transform plays on its transform pair's outer
    // layer, and a circle's centre as its layer's position.
    let scene = |on: &str| -> String {
        let v: serde_json::Value = serde_json::from_str(on).unwrap();
        v["ops"]
            .as_array()
            .unwrap()
            .iter()
            .rev()
            .find(|op| op["op"] == "svg")
            .map(|op| op.to_string())
            .unwrap_or_default()
    };
    let svg_op = scene(&on);
    assert!(svg_op.contains("transform.rotation.z"), "{svg_op}");
    assert!(svg_op.contains("position.y"), "{svg_op}");
    // A CSS function filter's picture follows its content on the GPU on
    // iOS, so what turns inside it lowers too.
    for lowered in ["shape", "ball", "followed"] {
        assert!(
            !host.engine.node_sampled(exact_kernel::motion::motion_node(
                host.runner.kernel().node(id(&host, lowered)).unwrap().key
            )),
            "{lowered}"
        );
    }
    // What Core Animation cannot say as CSS does is sampled: a corner
    // origin, a box that takes input or holds one that does, a bordered
    // box's colour, two alphas, a filtered circle's centre (its picture is
    // made for one place), and anything drawn inside a `filter` element's
    // picture.
    for sampled in [
        "corner", "pressed", "holder", "framed", "fading", "clipped", "inside",
    ] {
        assert!(keys(&on, id(&host, sampled)).is_empty(), "{sampled}: {on}");
        assert!(
            host.engine.node_sampled(exact_kernel::motion::motion_node(
                host.runner.kernel().node(id(&host, sampled)).unwrap().key
            )),
            "{sampled}"
        );
    }
    assert!(
        on.contains("\"motion\":true"),
        "the sampled ones keep frames coming"
    );
}

#[test]
fn lowered_box_keyframes_keep_no_frames_coming() {
    let app = r##"keyframes sweep
  from translate="-24px 0px"
  to translate="64px 0px"
keyframes glow
  from background-color="#16a34a"
  to background-color="#2563eb"
component A
  state on = false
  action go
    on = not on
  view
    column
      button press=go testId="go"
        text "Go"
      box testId="bar" width=64 height=64 overflow="hidden"
        box testId="sweep" width=24 height=64 animation=(on ? "sweep 1s linear infinite" : "none")
      box testId="glow" width=64 height=64 animation=(on ? "glow 1s ease infinite alternate" : "none")
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
    ios(&mut host);
    let on = host.dispatch_at(id(&host, "go"), Event::Press, 100.0);
    assert!(on.contains("\"motion\":false"), "{on}");
    let later = host.tick(1400.0);
    assert!(!later.contains("\"op\":\"present\""), "{later}");
    // Off again: the specs go, the row's own values stay.
    let off = host.dispatch_at(id(&host, "go"), Event::Press, 1500.0);
    assert_eq!(specs(&off, id(&host, "sweep")), Some(Vec::new()), "{off}");
}

#[test]
fn macos_samples_them() {
    let plan = contract::compile(APP).unwrap().encode();
    let (mut host, _) = Host::boot(
        &plan,
        NoData,
        Box::new(MonospaceMeasurer::default()),
        390.0,
        844.0,
    )
    .unwrap();
    assert!(!host.svg.box_motion || cfg!(any(target_os = "ios", target_os = "tvos")));
    let on = host.dispatch_at(id(&host, "go"), Event::Press, 100.0);
    if !cfg!(any(target_os = "ios", target_os = "tvos")) {
        assert!(keys(&on, id(&host, "sweep")).is_empty(), "{on}");
        assert!(on.contains("\"motion\":true"));
    }
}

/// LLP 1053.000.000 D4: a glass group ignores the opacity Core Animation
/// plays between it and its glass, so an opacity animation inside a group is
/// sampled and reaches the host as values; outside one, it is lowered.
#[test]
fn opacity_inside_a_glass_group_is_sampled() {
    let app = r##"keyframes dim
  from opacity=1
  to opacity=0.2
component A
  state on = false
  action go
    on = not on
  view
    column
      button press=go testId="go"
        text "Go"
      row glassGroup=12
        box testId="grouped" width=40 height=40 backgroundMaterial="glass" animation=(on ? "dim 1s linear infinite" : "none")
      box testId="free" width=40 height=40 backgroundMaterial="glass" animation=(on ? "dim 1s linear infinite" : "none")
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
    let on = host.dispatch_at(id(&host, "go"), Event::Press, 100.0);
    assert_eq!(keys(&on, id(&host, "free")), ["opacity"], "{on}");
    assert!(keys(&on, id(&host, "grouped")).is_empty(), "{on}");
    let sampled = |host: &Host<NoData>, t: &str| {
        host.engine.node_sampled(exact_kernel::motion::motion_node(
            host.runner.kernel().node(id(host, t)).unwrap().key,
        ))
    };
    assert!(sampled(&host, "grouped"));
    assert!(!sampled(&host, "free"));
    let grouped = id(&host, "grouped");
    let later = host.tick(600.0);
    let v: serde_json::Value = serde_json::from_str(&later).unwrap();
    assert!(
        v["ops"]
            .as_array()
            .unwrap()
            .iter()
            .any(|op| op["op"] == "present"
                && op["id"] == grouped
                && op.to_string().contains("opacity")),
        "{later}"
    );
}

/// LLP 1053.000.000 D4: an opacity animation still lowered when a group is
/// above it (a group set after it started) is switched to sampling by the
/// pass over every running animation, and its Core Animation spec withdrawn.
#[test]
fn a_lowered_opacity_animation_under_a_group_is_switched_to_sampling() {
    let app = r##"keyframes dim
  from opacity=1
  to opacity=0.2
component A
  state on = false
  action go
    on = not on
  view
    column
      button press=go testId="go"
        text "Go"
      row glassGroup=12
        box testId="grouped" width=40 height=40 backgroundMaterial="glass" animation=(on ? "dim 1s linear infinite" : "none")
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
    host.dispatch_at(id(&host, "go"), Event::Press, 100.0);
    let view = id(&host, "grouped");
    let node = exact_kernel::motion::motion_node(host.runner.kernel().node(view).unwrap().key);
    host.engine.set_node_sampled(node, false);
    assert_eq!(
        svg_lower::glass_sampling(host.runner.kernel(), &mut host.engine),
        [view]
    );
    assert!(host.engine.node_sampled(node));
    assert!(svg_lower::glass_sampling(host.runner.kernel(), &mut host.engine).is_empty());
}
