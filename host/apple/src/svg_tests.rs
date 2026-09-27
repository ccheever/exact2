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
