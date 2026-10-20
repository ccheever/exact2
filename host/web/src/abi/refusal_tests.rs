use super::Bridge;
use exact_runner::{Answer, DataError, DataSource, Request, Store, Value};
use serde_json::{json, Value as Json};

struct Data;
impl DataSource for Data {
    fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(name.into()))
    }
    fn answer(&mut self, _: &mut Store, _: &str, _: &[Value]) -> Result<Answer, DataError> {
        Ok(Answer::Later(Request::get("https://example.test/pending")))
    }
    fn replacement(&self, _: &[u8], receipt: &str, _: Vec<u8>) -> Result<Self, DataError> {
        if receipt == "activate" {
            Ok(Data)
        } else {
            Err(DataError::Unavailable("invalid replacement".into()))
        }
    }
    fn activate(&mut self) -> Result<(), DataError> {
        Err(DataError::Unavailable("activation failed".into()))
    }
}

fn output(bridge: &Bridge<Data>, len: u32) -> Json {
    serde_json::from_slice(bridge.output_bytes(len as usize)).unwrap()
}

fn running() -> Bridge<Data> {
    crate::link::link_for_tests();
    let plan = contract::compile(include_str!("../../tests/fixtures/refused-timers.contract"))
        .unwrap()
        .encode();
    let mut bridge = Bridge::new();
    let len = bridge.boot(&plan, Data, 400., 400., "/");
    assert_eq!(output(&bridge, len)["error"], Json::Null);
    bridge
        .host
        .as_mut()
        .unwrap()
        .runner_mut()
        .present_frames(true);
    bridge.advance(10., 0);
    bridge
}

fn preserves_state(refuse: impl FnOnce(&mut Bridge<Data>) -> u32, error: &str) {
    let mut bridge = running();
    check_state(&mut bridge, refuse, error);
}

fn check_state(
    bridge: &mut Bridge<Data>,
    refuse: impl FnOnce(&mut Bridge<Data>) -> u32,
    error: &str,
) {
    let before = output(bridge, bridge.output.len() as u32);
    assert_eq!(before["timer_due_ms"], 1000.);
    assert_eq!(before["frames"], true);
    let len = refuse(bridge);
    let refused = output(bridge, len);
    assert!(
        refused["error"].as_str().is_some_and(|s| s.contains(error)),
        "{refused}"
    );
    assert_eq!(refused["ops"], json!([]), "{refused}");
    for field in ["timers", "timer_due_ms", "frames", "clock"] {
        assert_eq!(refused[field], before[field], "{field}: {refused}");
    }
    let len = bridge.advance(1000., 0);
    assert_eq!(output(bridge, len)["error"], Json::Null);
    assert_eq!(
        bridge.host.as_ref().unwrap().runner().slot("ticks"),
        Some(&Value::Number(1.))
    );
}

macro_rules! invalid_input {
    ($name:ident, $method:ident, $payload:expr, $error:expr) => {
        #[test]
        fn $name() {
            preserves_state(
                |b| {
                    let len = b.input_write($payload);
                    b.$method(len)
                },
                $error,
            );
        }
    };
}

macro_rules! invalid_event {
    ($name:ident, $kind:expr, $error:expr) => {
        #[test]
        fn $name() {
            preserves_state(
                |b| {
                    let len = b.input_write(b"invalid");
                    b.dispatch(0, $kind, len, 500.)
                },
                $error,
            );
        }
    };
}

invalid_event!(invalid_press, 0, "invalid press modifiers");
invalid_event!(invalid_scroll, 13, "invalid scroll");
invalid_event!(invalid_height_release, 15, "invalid height release");
invalid_event!(invalid_transform_geometry, 16, "invalid transform event");
invalid_event!(invalid_transform_release, 17, "invalid transform event");
invalid_event!(invalid_reorder, 18, "invalid reorder event");
invalid_event!(invalid_picked_files, 26, "invalid picked files");
invalid_event!(invalid_checked_change, 24, "invalid checked state");
invalid_event!(invalid_checked_input, 25, "invalid checked state");
invalid_input!(
    invalid_surface_record,
    surface_record,
    b"\xff",
    "surface record: invalid UTF-8"
);
invalid_input!(invalid_topic, changed, b"\xff", "changed: invalid UTF-8");
invalid_input!(
    invalid_place,
    set_place,
    b"en-US",
    "place: expected locale NUL timeZone"
);
invalid_input!(
    invalid_mode_encoding,
    set_mode,
    b"\xff",
    "mode: invalid UTF-8"
);
invalid_input!(
    invalid_mode_pins,
    set_mode,
    b"invalid",
    "mode: write axis=value pins"
);
invalid_input!(rejected_mode_pins, set_mode, b"missing=value", "mode:");
invalid_input!(
    invalid_design_encoding,
    design_rows,
    b"\xff",
    "live: invalid UTF-8"
);
invalid_input!(
    invalid_design_frame,
    design_rows,
    b"invalid",
    "live: not a tamber batch or snapshot"
);
invalid_input!(
    invalid_collection_length,
    collection_feedback,
    b"\x01",
    "collection"
);

#[test]
fn explicit_refusal() {
    preserves_state(
        |b| b.refuse("invalid \"module\"\nreplacement"),
        "invalid \"module\"\nreplacement",
    );
}

#[test]
fn failed_fresh_boot() {
    preserves_state(|b| b.boot(&[], Data, 400., 400., "/"), "boot:");
}

#[test]
fn failed_plan_reload() {
    preserves_state(
        |b| {
            b.input_write(&[]);
            b.boot_plan(0, Data, 400., 400., "/")
        },
        "boot:",
    );
}

#[test]
fn invalid_module_lengths() {
    preserves_state(
        |b| b.boot_module([usize::MAX, 1, 0], Data, false),
        "invalid module generation lengths",
    );
}

#[test]
fn invalid_module_receipt_encoding() {
    preserves_state(
        |b| {
            b.input_write(b"\xff");
            b.boot_module([0, 1, 0], Data, false)
        },
        "invalid utf-8",
    );
}

#[test]
fn rejected_module_replacement() {
    preserves_state(
        |b| {
            b.input_write(b"invalid");
            b.boot_module([0, 7, 0], Data, false)
        },
        "invalid replacement",
    );
}

#[test]
fn failed_module_activation() {
    preserves_state(
        |b| {
            b.input_write(b"activate");
            b.boot_module([0, 8, 0], Data, false)
        },
        "activation failed",
    );
}

#[test]
fn module_replacement_while_pending() {
    let mut bridge = running();
    let kernel = bridge.host.as_ref().unwrap().runner().kernel();
    let view = kernel
        .node_by_key(kernel.find_by_test_id("fetch")[0])
        .unwrap()
        .id;
    bridge.dispatch(view, 0, 0, 10.);
    assert!(bridge.host.as_ref().unwrap().runner().has_pending());
    check_state(
        &mut bridge,
        |b| b.boot_module([0, 0, 0], Data, false),
        "module replacement waits for in-flight requests",
    );
}

#[test]
fn refusal_before_boot_has_no_wake() {
    let mut bridge = Bridge::<Data>::new();
    let len = bridge.refuse("not booted");
    let refused = output(&bridge, len);
    assert_eq!(refused["ops"], json!([]));
    assert_eq!(refused["timers"], false);
    assert!(refused["timer_due_ms"].is_null());
    assert_ne!(refused["frames"], true);
}
