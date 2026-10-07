//! Drive the embedded example through the same exported seam as the JVM.
use super::*;
use exact_runner::{CollectionFeedback, CollectionFill, RowMeasurement};
use serde_json::{json, Value};

struct App(u32);

impl App {
    fn boot() -> Self {
        let app = Self(exact_android_create());
        assert_ne!(app.0, 0);
        assert!(exact_android_boot(app.0, 390., 844.) > 32);
        app.transaction();
        app
    }

    fn bytes(&self) -> Vec<u8> {
        EXACT_ANDROID.with(|r| r.borrow().get(self.0).unwrap().borrow().output().to_vec())
    }

    fn input(&self, bytes: &[u8]) -> usize {
        exact_android::session::with_session(
            &EXACT_ANDROID,
            self.0,
            |s| s.bridge.input_write(bytes),
            || panic!("example session was destroyed"),
        )
    }

    fn query(&self, request: Value) -> Value {
        let n = self.input(request.to_string().as_bytes());
        exact_android_agent(self.0, n);
        let answer: Value = serde_json::from_slice(&self.bytes()).unwrap();
        assert!(answer.get("error").is_none(), "{answer}");
        answer
    }

    fn state(&self) -> Value {
        self.query(json!({"op": "state"}))
    }

    fn node(&self, target: &str) -> Value {
        self.query(json!({"op": "tree", "target": target, "shallow": true}))["nodes"][0].clone()
    }

    fn id(&self, target: &str) -> u32 {
        self.node(target)["id"]
            .as_u64()
            .unwrap()
            .try_into()
            .unwrap()
    }

    // Agent reads publish JSON; platform mutations must publish the actual EXA1
    // transaction, including a refusal rather than silently accepting bad input.
    fn transaction(&self) -> Value {
        let bytes = self.bytes();
        assert_eq!(&bytes[..4], b"EXA1");
        let n = u32::from_le_bytes(bytes[28..32].try_into().unwrap()) as usize;
        if n == 0 {
            return json!({});
        }
        serde_json::from_slice(&bytes[bytes.len() - n..]).unwrap()
    }

    fn event(&self, target: &str, kind: u32, payload: &str, now: f64) -> Value {
        let view = self.id(target);
        let n = self.input(payload.as_bytes());
        exact_android_dispatch(self.0, view, kind, n, now);
        self.transaction()
    }

    fn dispatch(&self, target: &str, kind: u32, payload: &str, now: f64) {
        let batch = self.event(target, kind, payload, now);
        assert!(batch.get("error").is_none(), "{batch}");
    }

    fn control(&self, target: &str, kind: u32) -> Value {
        exact_android_control_query(self.0, self.id(target), kind);
        serde_json::from_slice(&self.bytes()).unwrap()
    }

    fn collection(&self) -> Value {
        let state = self.state();
        let lists = state["collections"].as_array().unwrap();
        assert_eq!(lists.len(), 1, "{state}");
        lists[0].clone()
    }

    fn feedback(&self, collection: &Value, offset: f64, now: f64) {
        let decimal = |name: &str| collection[name].as_str().unwrap().parse().unwrap();
        let facts = CollectionFeedback {
            view: collection["view"].as_u64().unwrap().try_into().unwrap(),
            revision: decimal("revision"),
            scroll_sequence: decimal("scrollSequence") + 1,
            offset,
            port_main: 240.,
            port_cross: 390.,
            cross: 390.,
            measurements: collection["rows"]
                .as_array()
                .unwrap()
                .iter()
                .map(|row| RowMeasurement {
                    view: row["view"].as_u64().unwrap().try_into().unwrap(),
                    epoch: row["epoch"].as_str().unwrap().parse().unwrap(),
                    size: 80.,
                })
                .collect(),
            focus_view: None,
            interaction_view: None,
        };
        let bytes = facts
            .encode_with(CollectionFill {
                velocity: 1200.,
                ..CollectionFill::default()
            })
            .unwrap();
        assert_eq!(u32::from_le_bytes(bytes[..4].try_into().unwrap()), 3);
        let n = self.input(&bytes);
        exact_android_collection_feedback(self.0, n, now);
        let batch = self.transaction();
        assert!(batch.get("error").is_none(), "{batch}");
    }
}

impl Drop for App {
    fn drop(&mut self) {
        exact_android_destroy(self.0);
    }
}

#[test]
fn sdk_control_events_reach_authored_typed_state_and_native_queries() {
    let app = App::boot();
    let options = app.control("native-select", 1);
    assert_eq!(options["chosen"], 0);
    assert_eq!(options["options"][2]["disabled"], true);
    assert_eq!(options["options"][1]["label"], "Green");
    let fast = app.id("native-fast");
    let safe = app.id("native-safe");
    assert_eq!(app.control("native-fast", 2)["group"], json!([fast, safe]));

    for (i, (target, kind, payload)) in [
        ("native-checkbox", 24, "true"),
        ("native-switch", 24, "true"),
        ("native-select", 1, "green"),
        ("native-range", 23, "73"),
        ("native-date", 1, "2026-12-24"),
        ("native-time", 1, "09:45"),
        ("native-safe", 1, "safe"),
    ]
    .into_iter()
    .enumerate()
    {
        app.dispatch(target, kind, payload, i as f64 + 1.);
    }
    let slots = app.state()["slots"].clone();
    assert_eq!(slots["enabled"], true);
    assert_eq!(slots["switched"], true);
    assert_eq!(slots["choice"], "green");
    assert_eq!(slots["slider"], 75);
    assert_eq!(slots["date"], "2026-12-24");
    assert_eq!(slots["time"], "09:45");
    assert_eq!(slots["mode"], "safe");
    assert_eq!(app.control("native-select", 1)["chosen"], 1);
    assert_eq!(app.node("native-fast")["props"]["checked"], false);
    assert_eq!(app.node("native-safe")["props"]["checked"], true);
    app.dispatch("native-checkbox", 24, "false", 8.);
    app.dispatch("native-switch", 24, "false", 9.);
    let slots = app.state()["slots"].clone();
    assert_eq!(slots["enabled"], false);
    assert_eq!(slots["switched"], false);
}

#[test]
fn impossible_sdk_values_are_refused_without_running_authored_actions() {
    let app = App::boot();
    let before = app.state()["slots"].clone();
    for (i, (target, kind, payload)) in [
        ("native-checkbox", 1, "true"),
        ("native-select", 1, "disabled"),
        ("native-select", 1, "missing"),
        ("native-safe", 1, "fast"),
        ("native-date", 1, "24/12/2026"),
        ("native-time", 1, "25:00"),
    ]
    .into_iter()
    .enumerate()
    {
        let batch = app.event(target, kind, payload, i as f64 + 1.);
        assert!(batch["error"].is_string(), "{target}: {batch}");
        assert_eq!(app.state()["slots"], before, "{target}");
    }
}

#[test]
fn toolbar_and_tabs_select_authored_routes_without_resetting_controls_or_list() {
    let app = App::boot();
    let list = app.id("native-list");
    app.dispatch("native-increment", 0, "", 1.);
    app.dispatch("native-checkbox", 24, "true", 2.);
    assert_eq!(app.node("detail-page")["inactive"], true);
    app.dispatch("details", 0, "", 3.);
    assert_eq!(app.state()["slots"]["page"], "details");
    assert_eq!(app.node("home")["inactive"], true);
    assert_ne!(app.node("detail-page")["inactive"], true);
    app.dispatch("native-back", 0, "", 4.);
    assert_eq!(app.state()["slots"]["page"], "home");
    assert_ne!(app.node("home")["inactive"], true);
    app.dispatch("tab-details", 0, "", 5.);
    assert_eq!(app.state()["slots"]["page"], "details");
    app.dispatch("tab-home", 0, "", 6.);
    let slots = app.state()["slots"].clone();
    assert_eq!(slots["page"], "home");
    assert_eq!(slots["count"], 1);
    assert_eq!(slots["enabled"], true);
    assert_eq!(app.id("native-list"), list);
}

#[test]
fn v3_scroll_feedback_mounts_a_bounded_window_and_keeps_row_actions_live() {
    let app = App::boot();
    let initial = app.collection();
    assert_eq!(initial["count"], 1000);
    assert!(initial["rows"].as_array().unwrap().len() < 100);
    app.feedback(&initial, 40_000., 1.);
    let scrolled = app.collection();
    assert_eq!(scrolled["view"], initial["view"]);
    assert_eq!(scrolled["count"], 1000);
    assert_eq!(scrolled["totalExtent"], 80_000);
    let rows = scrolled["rows"].as_array().unwrap();
    assert!(
        rows.len() < 100,
        "only the viewport window crosses the seam"
    );
    assert!(rows.iter().any(|r| r["index"] == 500), "{scrolled}");
    assert!(!rows.iter().any(|r| r["index"] == 0), "{scrolled}");

    // A delayed platform layout from the old window must not undo this scroll.
    app.feedback(&initial, 0., 2.);
    assert_eq!(app.collection(), scrolled);
    app.dispatch("row-action-500", 0, "", 3.);
    assert_eq!(app.state()["slots"]["count"], 1);
    let current = app.collection();
    app.feedback(&current, 0., 4.);
    let returned = app.collection();
    let rows = returned["rows"].as_array().unwrap();
    assert!(rows.len() < 100);
    assert!(rows.iter().any(|r| r["index"] == 0), "{returned}");
    app.dispatch("row-action-0", 0, "", 5.);
    assert_eq!(app.state()["slots"]["count"], 2);
}

#[test]
fn inert_control_group_is_published_as_an_authored_subtree_property() {
    let app = App::boot();
    let batch = app.event("native-lock", 24, "true", 1.);
    let bytes = app.bytes();
    let count = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
    let mut at = 32;
    let mut ops = Vec::<Value>::new();
    for _ in 0..count {
        let opcode = bytes[at];
        let len = u32::from_le_bytes(bytes[at + 1..at + 5].try_into().unwrap()) as usize;
        at += 5;
        if opcode == exact_android::wire::JSON {
            ops.push(serde_json::from_slice(&bytes[at..at + len]).unwrap());
        }
        at += len;
    }
    assert_eq!(app.state()["slots"]["controlsLocked"], true);
    assert_eq!(
        app.node("controls-group")["props"]["inert"],
        true,
        "{batch}"
    );
    let group = app.id("controls-group");
    assert!(
        ops.iter()
            .any(|op| { op["op"] == "props" && op["id"] == group && op["set"]["inert"] == "true" }),
        "{ops:?}"
    );
    app.dispatch("native-lock", 24, "false", 2.);
    assert_eq!(app.node("controls-group")["props"]["inert"], false);
}
