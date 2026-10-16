use super::*;
use exact_runner::exact_canvas::{Context2d, DrawError, Frame};
use serde_json::{json, Value as Json};

struct Dot;

impl DataSource for Dot {
    fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(name.into()))
    }
    fn canvas_surfaces(&self) -> Vec<(String, usize)> {
        vec![("dot".into(), 0)]
    }
    fn draw_2d(
        &mut self,
        _: &str,
        _: &[Value],
        ctx: &Context2d,
        _: &Frame,
    ) -> Result<bool, DrawError> {
        ctx.fill_rect(0., 0., 4., 4.);
        Ok(false)
    }
}

fn output(bridge: &Bridge<Dot>, len: u32) -> Json {
    serde_json::from_slice(bridge.output_bytes(len as usize)).unwrap()
}

fn preserves_state(refuse: impl FnOnce(&mut Bridge<Dot>, u32) -> u32, error: &str) {
    let plan = contract::compile(
        r#"component App
  state shown = false
  action show
    shown = true
  task timer mount
    every(1000, show)
  view
    column
      button press=show testId="show"
        text "Show"
      box opacity=(shown ? 0.5 : 1) transition="opacity 200ms linear"
      if shown
        canvas surface=dot() width=40 height=40 testId="dot"
"#,
    )
    .unwrap()
    .encode();
    let mut bridge = Bridge::new();
    bridge.boot(&plan, Dot, Hooks::none(), 390., 844.);
    bridge.canvas_defer(true);
    let kernel = bridge.host.as_ref().unwrap().runner().kernel();
    let view = kernel
        .node_by_key(kernel.find_by_test_id("show")[0])
        .unwrap()
        .id;
    let len = bridge.dispatch(view, 0, 0, 10.);
    let made = output(&bridge, len);
    assert_eq!(made["timers"], true, "{made}");
    assert_eq!(made["motion"], true, "{made}");
    assert_eq!(made["canvasOwed"], true, "{made}");
    assert_eq!(made["timer_due_ms"], 1000., "{made}");
    assert!(made["ops"]
        .as_array()
        .unwrap()
        .iter()
        .any(|op| op["op"] == "create"));
    assert!(!made["ops"]
        .as_array()
        .unwrap()
        .iter()
        .any(|op| op["op"] == "canvas2d"));

    let len = refuse(&mut bridge, view);
    let refused = output(&bridge, len);
    assert!(
        refused["error"].as_str().is_some_and(|s| s.contains(error)),
        "{refused}"
    );
    assert_eq!(refused["ops"], json!([]), "{refused}");
    for field in ["timers", "timer_due_ms", "motion", "canvasOwed", "clock"] {
        assert_eq!(refused[field], made[field], "{field}: {refused}");
    }
    let len = bridge.canvas_draw();
    let drawn = output(&bridge, len);
    assert!(
        drawn["ops"]
            .as_array()
            .unwrap()
            .iter()
            .any(|op| op["op"] == "canvas2d"),
        "{drawn}"
    );
    assert_ne!(drawn["canvasOwed"], true, "{drawn}");
}

fn dispatch_refusal(kind: u32, payload: &str, error: &str) {
    preserves_state(
        |bridge, view| {
            let len = bridge.input_write(payload.as_bytes());
            bridge.dispatch(view, kind, len, 500.)
        },
        error,
    );
}

#[test]
fn invalid_press_keeps_state() {
    dispatch_refusal(0, "invalid", "invalid press modifiers");
}

#[test]
fn invalid_picked_files_keeps_state() {
    dispatch_refusal(26, "invalid", "invalid picked files");
}

#[test]
fn invalid_checked_change_keeps_state() {
    dispatch_refusal(24, "invalid", "invalid checked state");
}

#[test]
fn invalid_checked_input_keeps_state() {
    dispatch_refusal(25, "invalid", "invalid checked state");
}

#[test]
fn invalid_host_event_keeps_state() {
    dispatch_refusal(13, "invalid", "invalid scroll");
}

#[test]
fn invalid_reorder_keeps_state() {
    dispatch_refusal(18, "invalid", "invalid reorder event");
}

#[test]
fn unknown_event_keeps_state() {
    dispatch_refusal(u32::MAX, "", "unknown event kind");
}

#[test]
fn invalid_surface_record_keeps_state() {
    preserves_state(
        |bridge, _| {
            let len = bridge.input_write(b"\xff");
            bridge.surface_record(len)
        },
        "surface record: invalid UTF-8",
    );
}

#[test]
fn invalid_place_keeps_state() {
    preserves_state(
        |bridge, _| {
            let len = bridge.input_write(b"en-US");
            bridge.set_place(len)
        },
        "place: expected locale NUL timeZone",
    );
}

#[test]
fn invalid_mode_keeps_state() {
    preserves_state(
        |bridge, _| {
            let len = bridge.input_write(b"invalid");
            bridge.set_mode(len)
        },
        "mode: expected axis=value pins",
    );
}

#[test]
fn truncated_intrinsics_keeps_state() {
    preserves_state(
        |bridge, _| {
            let len = bridge.input_write(&[0]);
            bridge.intrinsics(len)
        },
        "intrinsics: truncated record",
    );
}

#[test]
fn truncated_covers_keeps_state() {
    preserves_state(
        |bridge, _| {
            let len = bridge.input_write(&[0]);
            bridge.covers(len)
        },
        "covers: truncated record",
    );
}

#[test]
fn truncated_colors_keeps_state() {
    preserves_state(
        |bridge, _| {
            let len = bridge.input_write(&[0]);
            bridge.colors(len)
        },
        "colors: truncated record",
    );
}

#[test]
fn failed_fresh_boot_keeps_state() {
    preserves_state(
        |bridge, _| bridge.boot(&[], Dot, Hooks::none(), 390., 844.),
        "boot:",
    );
}

#[test]
fn malformed_view_keeps_state() {
    preserves_state(
        |bridge, _| {
            let len = bridge.input_write(&[0]);
            bridge.view(len)
        },
        "view: malformed",
    );
}

#[test]
fn failed_plan_preparation_keeps_state() {
    preserves_state(
        |bridge, _| bridge.prepare_plan(0, Dot, Hooks::none(), 390., 844.),
        "boot:",
    );
}

#[test]
fn refused_preparation_keeps_state() {
    preserves_state(
        |bridge, _| bridge.refuse_preparation("invalid \"context\""),
        "invalid \"context\"",
    );
}

#[test]
fn missing_prepared_plan_keeps_state() {
    preserves_state(|bridge, _| bridge.commit_plan(), "no prepared plan");
}
