//! The Android seam reuses portable native controls, typed events and queries.
use crate::{bridge::Bridge, session::Session, Hooks};
use android_core_data::Core;

const CONTROLS: &str = r#"component App
  state count = 0
  state checked = false
  state amount = 50
  state choice = "blue"
  action increment
    count = count + 1
  action toggle(value: bool)
    checked = value
  action range(value: number)
    amount = value
  action choose(value: string)
    choice = value
  view
    column
      button appearance="auto" press=increment testId="button"
        text `Count ${count}`
      input type="checkbox" checked=checked change=toggle testId="check"
      input type="range" value=amount input=range testId="range"
      select value=choice change=choose testId="select"
        option value="blue"
          text "Blue"
        option value="green"
          text "Green"
"#;

fn boot() -> Session<Core> {
    let plan = contract::compile(CONTROLS).unwrap();
    assert!(!crate::core_eligible(&plan, &Core));
    let bytes = contract::bake(plan, Core).unwrap().encode();
    let mut session = Session::default();
    let n = session.bridge.boot(&bytes, Core, Hooks::none(), 390., 844.);
    session.publish(n);
    assert_eq!(&session.output()[..4], b"EXA1");
    session
}
fn query(b: &mut Bridge<Core>, request: &str) -> String {
    let n = b.input_write(request.as_bytes());
    let n = b.agent(n);
    String::from_utf8(b.output_bytes(n as usize).to_vec()).unwrap()
}
fn id(b: &mut Bridge<Core>, target: &str) -> u32 {
    let text = query(
        b,
        &format!(r#"{{"op":"tree","target":"{target}","shallow":true}}"#),
    );
    text.split("\"roots\":[")
        .nth(1)
        .unwrap()
        .split(']')
        .next()
        .unwrap()
        .parse()
        .unwrap()
}
fn control(s: &mut Session<Core>, view: u32, kind: u32) -> String {
    let n = s.bridge.control_query(view, kind);
    s.publish_agent(n);
    String::from_utf8(s.output().to_vec()).unwrap()
}
fn dispatch(s: &mut Session<Core>, view: u32, kind: u32, value: &str) {
    let n = s.bridge.input_write(value.as_bytes());
    let n = s.bridge.dispatch(view, kind, n, 1.);
    s.publish(n);
    assert_eq!(&s.output()[..4], b"EXA1");
}

#[test]
fn native_button_faces_use_shared_viewless_children_and_mark_changes() {
    let mut s = boot();
    let button = id(&mut s.bridge, "button");
    let face = control(&mut s, button, 0);
    assert!(face.contains("\"title\":\"Count 0\""), "{face}");
    dispatch(&mut s, button, 0, "");
    let flags = u16::from_le_bytes(s.output()[6..8].try_into().unwrap());
    assert_ne!(flags & 256, 0, "viewless changes invalidate native faces");
    let face = control(&mut s, button, 0);
    assert!(face.contains("\"title\":\"Count 1\""), "{face}");
    assert!(query(&mut s.bridge, r#"{"op":"state"}"#).contains("\"count\":1"));
}

#[test]
fn native_checkbox_range_and_select_keep_typed_runner_values() {
    let mut s = boot();
    let check = id(&mut s.bridge, "check");
    dispatch(&mut s, check, 24, "true");
    let range = id(&mut s.bridge, "range");
    dispatch(&mut s, range, 23, "75");
    let select = id(&mut s.bridge, "select");
    let options = control(&mut s, select, 1);
    assert!(options.contains("\"chosen\":0"), "{options}");
    dispatch(&mut s, select, 1, "green");
    let options = control(&mut s, select, 1);
    assert!(options.contains("\"chosen\":1"), "{options}");
    let state = query(&mut s.bridge, r#"{"op":"state"}"#);
    for value in ["\"checked\":true", "\"amount\":75", "\"choice\":\"green\""] {
        assert!(state.contains(value), "{state}");
    }
    let n = s.bridge.control_query(select, 99);
    s.publish_agent(n);
    assert!(String::from_utf8_lossy(s.output()).contains("error"));
    assert_eq!(query(&mut s.bridge, r#"{"op":"state"}"#), state);
}

const SCROLL_GEOMETRY: &str = r#"component App
  state frameX = 0
  state frameY = 0
  state measureX = 0
  state measureY = 0
  action readGeometry
    frameX = frame("target").x
    frameY = frame("target").y
    measureX = measure("target").x
    measureY = measure("target").y
  view
    column width=390 height=844
      button press=readGeometry testId="read" width=50 height=20
        text "Read"
      scroll id="port" testId="port" width=200 height=100 overflow-x="hidden" overflow-y="scroll"
        column width=400 height=400
          column id="target" width=40 height=30 margin-left=60 margin-top=80
"#;

fn geometry_numbers(state: &str) -> [f64; 4] {
    ["frameX", "frameY", "measureX", "measureY"].map(|name| {
        state
            .split(&format!("\"{name}\":"))
            .nth(1)
            .unwrap_or_else(|| panic!("missing geometry {name}: {state}"))
            .split(|c: char| {
                c != '-' && c != '.' && c != 'e' && c != 'E' && c != '+' && !c.is_ascii_digit()
            })
            .next()
            .unwrap()
            .parse()
            .unwrap()
    })
}

#[test]
fn native_scroll_facts_preserve_output_lease_and_action_geometry_general() {
    let plan = contract::compile(SCROLL_GEOMETRY).unwrap();
    // Geometry opcodes use the general owner under the conservative admission
    // rules; this test does not broaden the core adapter's supported vocabulary.
    assert!(!crate::core_eligible(&plan, &Core));
    let bytes = contract::bake(plan, Core).unwrap().encode();
    let mut s = Session::default();
    let n = s.bridge.boot(&bytes, Core, Hooks::none(), 390., 844.);
    assert!(!s.bridge.binary_output());
    s.publish(n);
    assert_eq!(&s.output()[..4], b"EXA1");
    let port = id(&mut s.bridge, "port");
    let read = id(&mut s.bridge, "read");
    dispatch(&mut s, read, 0, "");
    let initial = geometry_numbers(&query(&mut s.bridge, r#"{"op":"state"}"#));
    assert!(initial[0] > 7.5 && initial[1] > 25., "{initial:?}");
    assert_eq!(initial[0], initial[2]);
    assert_eq!(initial[1], initial[3]);

    // Restore a publication lease after the agent query. Neither an accepted
    // scroll note nor a refused one may replace its format, bytes or allocation.
    dispatch(&mut s, read, 0, "");
    assert_scroll_leases_unchanged(&mut s, port);

    // There is no authored scroll handler: the next press still reads where
    // Android actually presents the descendant, for both geometry operations.
    dispatch(&mut s, read, 0, "");
    let moved = geometry_numbers(&query(&mut s.bridge, r#"{"op":"state"}"#));
    assert_eq!(
        moved,
        [
            initial[0] - 7.5,
            initial[1] - 25.,
            initial[2] - 7.5,
            initial[3] - 25.
        ]
    );
    assert!(s.bridge.scrolled(port, 9., 40.));
    assert!(s.bridge.scrolled(port, 11., 50.));
    dispatch(&mut s, read, 0, "");
    let latest = geometry_numbers(&query(&mut s.bridge, r#"{"op":"state"}"#));
    assert_eq!(
        latest,
        [
            initial[0] - 11.,
            initial[1] - 50.,
            initial[2] - 11.,
            initial[3] - 50.
        ]
    );
}

fn assert_scroll_leases_unchanged(s: &mut Session<Core>, port: u32) {
    let wire = s.output().to_vec();
    let wire_address = s.output().as_ptr();
    let owner = s.bridge.output_bytes(usize::MAX).to_vec();
    let owner_address = s.bridge.output_bytes(usize::MAX).as_ptr();
    let binary = s.bridge.binary_output();
    assert!(s.bridge.scrolled(port, 7.5, 25.));
    assert!(!s.bridge.scrolled(port, f64::NAN, 100.));
    assert!(!s.bridge.scrolled(port, 100., f64::INFINITY));
    assert!(!s.bridge.scrolled(0, 100., 100.));
    assert_eq!(s.bridge.binary_output(), binary);
    assert_eq!(s.output(), wire);
    assert_eq!(s.output().as_ptr(), wire_address);
    assert_eq!(s.bridge.output_bytes(usize::MAX), owner);
    assert_eq!(s.bridge.output_bytes(usize::MAX).as_ptr(), owner_address);
}

#[test]
fn native_scroll_facts_preserve_output_lease_and_next_action_core() {
    const CORE_SCROLL: &str = r#"component App
  state count = 0
  action increment
    count = count + 1
  view
    column width=390 height=844
      button appearance="none" press=increment testId="increment" width=50 height=20
        text `Count ${count}`
      scroll testId="port" width=200 height=100 overflow-x="hidden" overflow-y="scroll"
        column width=200 height=400
          text "Scrollable content"
"#;
    let plan = contract::compile(CORE_SCROLL).unwrap();
    assert!(crate::core_eligible(&plan, &Core));
    let bytes = contract::bake(plan, Core).unwrap().encode();
    let mut s = Session::default();
    let n = s.bridge.boot(&bytes, Core, Hooks::none(), 390., 844.);
    assert!(s.bridge.binary_output(), "the eligible plan must boot core");
    s.publish(n);
    assert_eq!(&s.output()[..4], b"EXA1");
    let port = id(&mut s.bridge, "port");
    let increment = id(&mut s.bridge, "increment");
    dispatch(&mut s, increment, 0, "");
    assert!(s.bridge.binary_output());
    assert_scroll_leases_unchanged(&mut s, port);
    dispatch(&mut s, increment, 0, "");
    assert!(
        s.bridge.binary_output(),
        "scroll notes must preserve the selected owner"
    );
    let state = query(&mut s.bridge, r#"{"op":"state"}"#);
    assert!(state.contains("\"count\":2"), "{state}");
}
