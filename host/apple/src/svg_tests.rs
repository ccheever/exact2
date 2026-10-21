//! LLP 1055 D4/D7 on the Apple host: SVG elements become one `svg` scene op,
//! never views; animations lower to CA specs; the engine keeps no frames busy.
use super::*;
use exact_kernel::MonospaceMeasurer;
use exact_plan::Value;
use exact_runner::DataError;

#[derive(Clone, Default)]
struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

const APP: &str = "keyframes draw\n  from stroke-dashoffset=1\n  to stroke-dashoffset=0\nkeyframes breathe\n  from r=3 opacity=0.5\n  to r=9 opacity=0\ncomponent A\n  view\n    column\n      svg testId=\"chart\" width=96 height=32 viewBox=\"0 0 96 32\" overflow=\"visible\" color=\"#16a34a\"\n        polyline points=\"0,32 48,0 96,16\" fill=\"none\" stroke=\"currentcolor\" stroke-width=1.5 pathLength=1 stroke-dasharray=\"1\" animation=\"draw 600ms ease-out both\"\n        circle cx=96 cy=16 r=3 fill=\"none\" stroke=\"currentcolor\" opacity=0 animation=\"breathe 1200ms ease-out 600ms infinite\"\n";

fn scene(batch: &str) -> serde_json::Value {
    let v: serde_json::Value = serde_json::from_str(batch).unwrap();
    v["ops"]
        .as_array()
        .unwrap()
        .iter()
        .find(|op| op["op"] == "svg")
        .expect("an svg op")["scene"]
        .clone()
}

#[test]
fn an_svg_is_one_scene_with_lowered_animations() {
    let plan = contract::compile(APP).unwrap().encode();
    let (host, batch) = Host::boot(
        &plan,
        NoData,
        Box::new(MonospaceMeasurer::default()),
        390.0,
        844.0,
    )
    .unwrap();
    let v: serde_json::Value = serde_json::from_str(&batch).unwrap();
    let creates: Vec<&str> = v["ops"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|op| op["op"] == "create")
        .map(|op| op["kind"].as_str().unwrap())
        .collect();
    assert!(
        creates.contains(&"svg") && !creates.contains(&"svg-element"),
        "{creates:?}"
    );
    assert_eq!(
        v["motion"], false,
        "a lowered infinite pulse keeps no frames busy"
    );
    let s = scene(&batch);
    assert_eq!(s["box"], serde_json::json!([0, 0, 96, 32]));
    let line = &s["els"][0];
    // The polyline's path: move to (0,32), then two lines, all user units.
    assert_eq!(
        line["p"],
        serde_json::json!([0, 0, 32, 1, 48, 0, 1, 96, 16])
    );
    let length = (48f64.hypot(32.0) + 48f64.hypot(16.0)) as f32;
    let dash: Vec<f32> = line["dash"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d.as_f64().unwrap() as f32)
        .collect();
    assert!(
        (dash[0] - length).abs() < 1e-3 && dash.len() == 2,
        "{dash:?}"
    );
    let draw = &line["a"][0];
    assert_eq!(draw["k"], "lineDashPhase");
    assert_eq!(draw["t"], serde_json::json!([0, 1]));
    let v0 = draw["v"][0].as_f64().unwrap() as f32;
    assert!(
        (v0 - length).abs() < 1e-3,
        "the phase starts one path length in"
    );
    assert_eq!(
        draw["c"][0],
        serde_json::json!([0, 0, 0.58, 1]),
        "CSS ease-out"
    );
    let ring = &s["els"][1];
    assert_eq!(ring["pos"], serde_json::json!([96, 16]));
    let keys: Vec<&str> = ring["a"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["k"].as_str().unwrap())
        .collect();
    assert_eq!(keys, ["opacity", "r"]);
    assert_eq!(ring["a"][1]["n"], -1.0, "infinite");
    assert_eq!(ring["a"][1]["dl"], 0.6);
    drop(host);
}

/// LLP 1055.000 D7: a gradient paint crosses as its stops, spread and
/// transform; a radial one as both circles.
#[test]
fn gradients_cross_resolved() {
    let app = "component A\n  view\n    column\n      svg width=120 height=80 viewBox=\"0 0 120 80\"\n        defs\n          radialGradient id=\"shade\"\n            stop offset=0 stop-color=\"#ffffff\"\n            stop offset=1 stop-color=\"#f43f5e\"\n        circle cx=60 cy=40 r=32 fill=\"url(#shade)\"\n";
    let plan = contract::compile(app).unwrap().encode();
    let (_, batch) = Host::boot(
        &plan,
        NoData,
        Box::new(MonospaceMeasurer::default()),
        390.0,
        844.0,
    )
    .unwrap();
    let s = scene(&batch);
    let fill = &s["els"][0]["f"];
    assert_eq!(
        fill["rg"],
        serde_json::json!([0.5, 0.5, 0.5, 0.5, 0.5, 0]),
        "{s}"
    );
    // The circle is drawn about the origin, so its gradient is too.
    assert_eq!(fill["t"], serde_json::json!([64, 0, 0, 64, -32, -32]));
    assert_eq!(fill["st"][1][1], serde_json::json!([244, 63, 94, 255]));
}

#[test]
fn a_fixed_svg_reemits_window_geometry_after_resize() {
    let app = r#"component A
  view
    column
      svg width=200 height=400
        svg width=100 height="50vh"
          rect width="100%" height="100%"
      svg width=20 height=20
        rect width=10 height=10
"#;
    let plan = contract::compile(app).unwrap().encode();
    let (mut host, batch) = Host::boot(
        &plan,
        NoData,
        Box::new(MonospaceMeasurer::default()),
        800.0,
        600.0,
    )
    .unwrap();
    let initial = scene(&batch);
    assert_eq!(initial["box"], serde_json::json!([0, 0, 200, 400]));
    assert_eq!(initial["els"][0]["vp"], serde_json::json!([0, 0, 100, 300]));
    let resized = host.resize(800.0, 400.0);
    let next = scene(&resized);
    assert_eq!(next["box"], initial["box"]);
    assert_eq!(next["els"][0]["vp"], serde_json::json!([0, 0, 100, 200]));
    let count = |batch: &str| {
        let v: serde_json::Value = serde_json::from_str(batch).unwrap();
        v["ops"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|op| op["op"] == "svg")
            .count()
    };
    assert_eq!(count(&resized), 1, "the unchanged scene is not sent again");
    assert_eq!(count(&host.resize(800.0, 400.0)), 0);
    let screened = scene(&host.set_screen(Some((800.0, 700.0))));
    assert_eq!(screened["els"][0]["vp"][3], 350);
    assert_eq!(
        count(&host.resize(800.0, 500.0)),
        0,
        "a sheet keeps the screen's basis"
    );
    let unscreened = scene(&host.set_screen(None));
    assert_eq!(unscreened["els"][0]["vp"][3], 250);
}

/// LLP 1115.003 D4: a path whose `d` moves between two paths of the same
/// commands, flat and as a gradient fill.
const MORPH: &str = "component A\n  state up = false\n  action flip\n    up = not up\n  view\n    column\n      button testId=\"go\" press=flip\n        text \"go\"\n      svg width=100 height=100 viewBox=\"0 0 100 100\"\n        path testId=\"flat\" d=(up ? \"M0 0 Q50 100 100 0 Z\" : \"M0 50 Q50 0 100 50 Z\") fill=\"#16a34a\" transition=\"d 500ms cubic-bezier(0.77, 0, 0.175, 1) 900ms\"\n      svg width=100 height=100 viewBox=\"0 0 100 100\"\n        defs\n          linearGradient id=\"g\"\n            stop offset=0 stop-color=\"#000\"\n            stop offset=1 stop-color=\"#fff\"\n        path testId=\"served\" d=(up ? \"M0 0 L100 0\" : \"M0 50 L100 50\") stroke=\"url(#g)\" transition=\"d 400ms ease-in-out\"\n        path testId=\"solid\" d=(up ? \"M0 10 L100 10\" : \"M0 60 L100 60\") stroke=\"#000\" transition=\"d 400ms ease-in-out\"\n";

/// Every `svg` op's scene in a batch, in order.
fn scenes(batch: &str) -> Vec<serde_json::Value> {
    let v: serde_json::Value = serde_json::from_str(batch).unwrap();
    v["ops"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|op| op["op"] == "svg")
        .map(|op| op["scene"].clone())
        .collect()
}

fn morph_host() -> Host<NoData> {
    let plan = contract::compile(MORPH).unwrap().encode();
    let (host, _) = Host::boot(
        &plan,
        NoData,
        Box::new(MonospaceMeasurer::default()),
        390.0,
        844.0,
    )
    .unwrap();
    host
}

/// A path's numbers, to the hundredth.
fn close(path: &serde_json::Value, want: &[f64]) {
    let got: Vec<f64> = path
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n.as_f64().unwrap())
        .collect();
    assert!(
        got.len() == want.len() && got.iter().zip(want).all(|(a, b)| (a - b).abs() < 0.01),
        "{got:?}"
    );
}

fn element(scene: &serde_json::Value, index: usize) -> &serde_json::Value {
    &scene["els"][index]
}

#[test]
fn a_morph_between_paths_of_one_shape_plays_in_core_animation() {
    let mut host = morph_host();
    let go = {
        let k = host.runner.kernel();
        k.node_by_key(k.find_by_test_id("go")[0]).unwrap().id
    };
    let batch = host.dispatch_at(go, exact_runner::Event::Press, 100.0);
    let all = scenes(&batch);
    assert_eq!(all.len(), 1, "the sampled scene has not moved yet");
    let flat = element(&all[0], 0);
    // The model is the target; Core Animation moves from the start to it,
    // after the delay, on the transition's own cubic, holding the start
    // through the delay.
    close(
        &flat["p"],
        &[0., 0., 0., 2., 33.33, 66.67, 66.67, 66.67, 100., 0., 3.],
    );
    let spec = flat["a"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["k"] == "path")
        .unwrap_or_else(|| panic!("a path spec: {flat}"));
    assert_eq!(spec["t"], serde_json::json!([0, 1]));
    assert_eq!(spec["c"], serde_json::json!([[0.77, 0, 0.175, 1]]));
    assert_eq!(spec["d"], 0.5);
    assert_eq!(spec["fill"], 3);
    assert_eq!(spec["v"][1], flat["p"]);
    close(
        &spec["v"][0],
        &[0., 0., 50., 2., 33.33, 16.67, 66.67, 16.67, 100., 50., 3.],
    );
    // A path painted by a gradient keeps its sampled morph (Core Graphics
    // draws its parts), and its scene's other morphs stay with it.
    for test_id in ["served", "solid"] {
        let k = host.runner.kernel();
        let key = exact_kernel::motion::motion_node(k.find_by_test_id(test_id)[0]);
        assert!(
            host.engine.played_path(key).is_none(),
            "{test_id} is sampled"
        );
    }
    let v: serde_json::Value = serde_json::from_str(&batch).unwrap();
    assert_eq!(v["motion"], true, "the sampled morphs keep frames");
}

#[test]
fn a_lowered_morph_keeps_no_frames_busy() {
    let plan = contract::compile(&MORPH.replace("stroke=\"url(#g)\"", "stroke=\"#000\""))
        .unwrap()
        .encode();
    let (mut host, _) = Host::boot(
        &plan,
        NoData,
        Box::new(MonospaceMeasurer::default()),
        390.0,
        844.0,
    )
    .unwrap();
    let go = {
        let k = host.runner.kernel();
        k.node_by_key(k.find_by_test_id("go")[0]).unwrap().id
    };
    let batch = host.dispatch_at(go, exact_runner::Event::Press, 100.0);
    let v: serde_json::Value = serde_json::from_str(&batch).unwrap();
    assert_eq!(
        v["motion"], false,
        "both morphs are Core Animation's: {batch}"
    );
}

#[test]
fn a_morph_retargeted_midway_starts_from_the_path_it_shows() {
    let mut host = morph_host();
    let go = {
        let k = host.runner.kernel();
        k.node_by_key(k.find_by_test_id("go")[0]).unwrap().id
    };
    let _ = host.dispatch_at(go, exact_runner::Event::Press, 100.0);
    // Half way through the 500 ms after its 900 ms delay, back again.
    let batch = host.dispatch_at(go, exact_runner::Event::Press, 1250.0);
    let spec = scenes(&batch)
        .iter()
        .flat_map(|s| s["els"].as_array().unwrap().clone())
        .flat_map(|e| e["a"].as_array().cloned().unwrap_or_default())
        .find(|a| a["k"] == "path")
        .expect("a path spec for the reversal");
    // The cubic's midpoint, not either end: the control point's y is at
    // neither 66.67 nor 16.67.
    let y = spec["v"][0][4].as_f64().unwrap();
    assert!(y > 20.0 && y < 63.0, "{spec}");
    close(
        &spec["v"][1],
        &[0., 0., 50., 2., 33.33, 16.67, 66.67, 16.67, 100., 50., 3.],
    );
}
