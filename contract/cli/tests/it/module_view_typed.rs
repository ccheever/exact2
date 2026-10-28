//! LLP 1133.021 §9 items 1 and 2, through `@fixture/pinboard`, a package
//! written beside the app as an installed one: a view's record and list
//! props reach the implementation as JSON under their names; a prop's type
//! is held to the declaration (`contract-view-prop`); and an event the view
//! declares beyond the nine runs its handler with the typed record the
//! implementation sent.

use exact_kernel::PropId;
use exact_runner::{Event, Runner, Value};
use serde_json::json;
use std::path::PathBuf;

const INTERFACE: &str = r#"{
  "kind": "interface",
  "name": "@fixture/pinboard",
  "version": "1.0.0",
  "shapes": {
    "Pin": { "id": "string", "latitude": "number", "longitude": "number", "title": "option<string>" },
    "Camera": { "latitude": "number", "longitude": "number", "zoom": "number" }
  },
  "views": {
    "pin-board": {
      "props": { "latitude": "number", "zoomLevel": "option<number>", "pins": "option<Pin[]>", "focus": "option<Pin>" },
      "events": ["change", { "event": "moveend", "args": ["Camera"] }]
    }
  },
  "names": []
}"#;

const APP: &str = r#"use Pin, Camera from "@fixture/pinboard"

component App
  state zoom = 11
  state moved = ""
  state picked = ""
  derive pins = [Pin(id="ferry", latitude=37.7955, longitude=-122.3937, title=some("Ferry \"Building\"")), Pin(id="pier", latitude=37.8087, longitude=-122.4098, title=none)]
  action ended(c: Camera)
    moved = `${c.latitude} ${c.longitude} ${c.zoom}`
    zoom = c.zoom
  action pick(id: string)
    picked = id
  view
    column
      pin-board testId="board" latitude=37.8 zoomLevel=zoom pins=pins moveend=ended change=pick
"#;

struct App(PathBuf);
impl App {
    fn new(name: &str, app: &str) -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("exact-typed-{name}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        let at = path.join("node_modules/@fixture/pinboard");
        std::fs::create_dir_all(&at).unwrap();
        let manifest = json!({ "name": "@fixture/pinboard", "version": "1.0.0", "exports": { ".": "./exact.json" } });
        std::fs::write(at.join("package.json"), manifest.to_string()).unwrap();
        std::fs::write(at.join("exact.json"), INTERFACE).unwrap();
        let manifest = json!({ "name": "app", "dependencies": { "@fixture/pinboard": "1.0.0" } });
        std::fs::write(path.join("package.json"), manifest.to_string()).unwrap();
        std::fs::write(path.join("app.contract"), app).unwrap();
        Self(path.canonicalize().unwrap())
    }
    fn compile(&self) -> Result<exact_plan::Plan, Vec<contract::CompileError>> {
        contract::compile_path_all(&self.0.join("app.contract"), false).map(|(plan, _)| plan)
    }
}
impl Drop for App {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct NoData;
impl exact_runner::DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, exact_runner::DataError> {
        Err(exact_runner::DataError::UnknownSource(source.into()))
    }
}

fn boot(app: &App) -> Runner<NoData> {
    Runner::boot(
        app.compile().unwrap_or_else(|e| panic!("{e:?}")),
        NoData,
        exact_kernel::Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

fn props(r: &Runner<NoData>) -> serde_json::Value {
    let key = r.kernel().find_by_test_id("board")[0];
    let node = r.kernel().node_by_key(key).unwrap();
    serde_json::from_str(node.props.str(PropId::NativeViewProps).unwrap()).unwrap()
}

#[test]
fn a_record_list_prop_reaches_the_view_as_its_json() {
    let app = App::new("props", APP);
    let r = boot(&app);
    let props = props(&r);
    assert_eq!(props["latitude"], "37.8");
    assert_eq!(props["zoomLevel"], "11");
    let pins: serde_json::Value = serde_json::from_str(props["pins"].as_str().unwrap()).unwrap();
    assert_eq!(
        pins,
        json!([
            { "id": "ferry", "latitude": 37.7955, "longitude": -122.3937, "title": "Ferry \"Building\"" },
            { "id": "pier", "latitude": 37.8087, "longitude": -122.4098 }
        ])
    );
}

#[test]
fn a_prop_is_held_to_its_declared_type() {
    let wrong = App::new(
        "wrong",
        &APP.replace("latitude=37.8 zoomLevel", "latitude=\"north\" zoomLevel"),
    );
    let refused = wrong.compile().unwrap_err();
    assert!(
        refused
            .iter()
            .any(|e| e.id == "contract-view-prop" && e.message.contains("`latitude`")),
        "{refused:?}"
    );
    let record = App::new("record", &APP.replace("pins=pins", "pins=first(pins)"));
    let refused = record.compile().unwrap_err();
    assert!(
        refused
            .iter()
            .any(|e| e.id == "contract-view-prop" && e.message.contains("`pins`")),
        "{refused:?}"
    );
}

#[test]
fn a_declared_event_runs_its_handler_with_the_record_sent() {
    let app = App::new("event", APP);
    let mut r = boot(&app);
    let key = r.kernel().find_by_test_id("board")[0];
    let id = r.kernel().node_by_key(key).unwrap().id;
    let sent = json!({ "exact:event": "moveend", "args": [{ "latitude": 37.79, "longitude": -122.4, "zoom": 13 }] });
    r.dispatch(id, Event::Message(sent.to_string())).unwrap();
    assert_eq!(r.slot("moved"), Some(&Value::str("37.79 -122.4 13")));
    assert_eq!(props(&r)["zoomLevel"], "13");
    // An event the view does not declare runs nothing.
    let other = json!({ "exact:event": "zoomend", "args": [] });
    r.dispatch(id, Event::Message(other.to_string())).unwrap();
    assert_eq!(r.slot("moved"), Some(&Value::str("37.79 -122.4 13")));
    // A handler whose action does not take the event's record is refused.
    let wrong = App::new(
        "arity",
        &APP.replace("action ended(c: Camera)", "action ended(c: Pin)")
            .replace(
                "    moved = `${c.latitude} ${c.longitude} ${c.zoom}`\n    zoom = c.zoom\n",
                "    moved = c.id\n",
            ),
    );
    let refused = wrong.compile().unwrap_err();
    assert!(
        refused.iter().any(|e| e.id == "lower-handler-type"),
        "{refused:?}"
    );
}
