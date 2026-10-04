//! `pointerdown` and `pointerup` (LLP 1005 §3; Charlie, 2026-10-03, the
//! Signal Clone's hold-to-record mic as the consumer): DOM's names for a
//! touch or the primary button going down on a node and coming up, before
//! and apart from `press`; and `pointermove`, each with an optional
//! `PointerEvent` (LLP 1056 §3 stage 3; the paint diary's F1).

use exact_kernel::Kernel;
use exact_plan::{EventKind, Value};
use exact_runner::{DataError, DataSource, Event, PointerEvent, Runner};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

const MIC: &str = r#"component App
  state recording = false
  state sent = 0
  state log = ""
  action begin
    recording = true
    log = `${log}d`
  action end
    recording = false
    sent = sent + 1
    log = `${log}u`
  action tapped
    log = `${log}p`
  view
    button pointerdown=begin pointerup=end press=tapped testId="mic"
      text (recording ? "recording" : "idle")
"#;

#[test]
fn pointer_down_and_up_run_their_actions_beside_press() {
    let plan = contract::compile(MIC).unwrap_or_else(|e| panic!("{e}"));
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let mic = r
        .kernel()
        .node_by_key(r.kernel().find_by_test_id("mic")[0])
        .unwrap()
        .id;
    assert_eq!(
        r.handlers_of(mic),
        vec![
            EventKind::Pointerdown,
            EventKind::Pointerup,
            EventKind::Press
        ]
    );
    r.dispatch(mic, Event::Pointerdown(at("3,4,1,0.5,touch,7")))
        .unwrap();
    assert_eq!(r.slot("recording"), Some(&Value::Bool(true)));
    r.dispatch(mic, Event::Pointerup(at("3,4,0,0,touch,7")))
        .unwrap();
    r.dispatch(mic, Event::Press).unwrap();
    assert_eq!(r.slot("sent"), Some(&Value::Number(1.0)));
    assert_eq!(
        r.slot("log"),
        Some(&Value::str("dup")),
        "DOM's order: down, up, then the click"
    );
}

fn at(line: &str) -> PointerEvent {
    PointerEvent::parse(line).unwrap()
}

/// A parameter of another type is an argument left unbound, not the
/// record: the arity refusal names both declarations.
#[test]
fn a_pointer_handler_takes_a_pointer_event_or_nothing() {
    let src = MIC.replace("action begin\n", "action begin(x: number)\n");
    let e = contract::compile(&src).unwrap_err();
    assert_eq!(e.id, "analyze-handler-arity", "{e}");
    assert!(e.message.contains("for its `PointerEvent`"), "{e}");
}

const PAD: &str = r#"component App
  state points = ""
  state pressing = 0
  action down(tool: string, e: PointerEvent)
    points = `${tool}:${e.pointerType}#${e.pointerId}@${e.offsetX},${e.offsetY}`
    pressing = e.pressure
  action move(e)
    points = `${points} ${e.offsetX},${e.offsetY}/${e.buttons}`
  action up
    points = `${points} up`
  view
    canvas pointerdown=down("pen") pointermove=move pointerup=up testId="pad" width=200 height=100
"#;

#[test]
fn pointer_events_hand_their_record_to_an_action_that_takes_it() {
    let plan = contract::compile(PAD).unwrap_or_else(|e| panic!("{e}"));
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let pad = r
        .kernel()
        .node_by_key(r.kernel().find_by_test_id("pad")[0])
        .unwrap()
        .id;
    r.dispatch(pad, Event::Pointerdown(at("10,20,1,0.75,pen,3")))
        .unwrap();
    r.dispatch(pad, Event::Pointermove(at("12.5,22,1,0.8,pen,3")))
        .unwrap();
    r.dispatch(pad, Event::Pointerup(at("13,23,0,0,pen,3")))
        .unwrap();
    assert_eq!(
        r.slot("points"),
        Some(&Value::str("pen:pen#3@10,20 12.5,22/1 up"))
    );
    assert_eq!(r.slot("pressing"), Some(&Value::Number(0.75)));
}

/// Gallery F20: a `press` action that takes one more parameter hears the
/// click's `MouseEvent` — the modifiers held, for shift-click range
/// selection and ⌘-click — and a pointer's record carries them too.
#[test]
fn a_press_hears_the_modifiers_held() {
    let src = r#"component App
  state log = ""
  action pick(id: string, e: MouseEvent)
    log = `${log}${id}${e.shiftKey ? "+shift" : ""}${e.metaKey ? "+meta" : ""};`
  action down(e: PointerEvent)
    log = `${log}down${e.altKey ? "+alt" : ""};`
  view
    button press=pick("a") pointerdown=down testId="a"
      text log
"#;
    let plan = contract::compile(src).unwrap_or_else(|e| panic!("{e}"));
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let a = r
        .kernel()
        .node_by_key(r.kernel().find_by_test_id("a")[0])
        .unwrap()
        .id;
    r.dispatch(a, Event::Press).unwrap();
    r.dispatch(a, Event::press("Shift+Meta+").unwrap()).unwrap();
    r.dispatch(a, Event::Pointerdown(at("1,1,1,0.5,mouse,1,Alt")))
        .unwrap();
    assert_eq!(r.slot("log"), Some(&Value::str("a;a+shift+meta;down+alt;")));
    assert!(Event::press("Hyper").is_none());
    assert_eq!(Event::press(""), Some(Event::Press));
}

/// A board's input as a canvas editor hears it (studio diary R3, R17, R19,
/// R22): a `contextmenu` at its point, a `wheel` (⌘-scroll and a pinch,
/// a Control-held wheel) that it claims with `preventDefault()`, files
/// dropped as `doc:` handles, and `beforeunload` asking before the window
/// closes while there are unsaved changes.
#[test]
fn a_board_hears_its_menu_point_its_wheel_its_drop_and_its_unload() {
    const BOARD: &str = r#"component App
  state zoom = 1
  state menu = ""
  state dropped = ""
  state dirty = true
  action openMenu(e: PointerEvent)
    menu = `${e.offsetX},${e.offsetY},${e.buttons}`
  action wheeled(e: WheelEvent)
    if e.ctrlKey or e.metaKey
      zoom = zoom - e.deltaY / 100
      preventDefault()
  action took(e: DragEvent)
    dropped = `${length(e.files)} ${join(e.files, "+")} ${e.offsetX}`
  action leaving
    if dirty
      preventDefault()
  view
    box testId="world" width=400 height=300 contextmenu=openMenu wheel=wheeled drop=took beforeunload=leaving
"#;
    let plan = contract::compile(BOARD).unwrap_or_else(|e| panic!("{e}"));
    let mut r = Runner::boot(plan, NoData, Kernel::with_monospace(), Default::default(), "/").unwrap();
    let world = r.kernel().node_by_key(r.kernel().find_by_test_id("world")[0]).unwrap().id;
    let event = |kind: u32, payload: &str| Event::of_host_kind(kind, payload).unwrap();
    r.dispatch(world, event(10, "40,8,2,0.5,mouse,1")).unwrap();
    assert_eq!(r.slot("menu"), Some(&Value::str("40,8,2")));
    // A keyboard's menu key: no point, DOM's record at the origin.
    r.dispatch(world, Event::Contextmenu).unwrap();
    assert_eq!(r.slot("menu"), Some(&Value::str("0,0,0")));
    // A plain wheel scrolls: nothing claims it.
    r.dispatch(world, event(36, "10,10,0,120,0")).unwrap();
    assert_eq!(r.slot("zoom"), Some(&Value::Number(1.0)));
    assert!(!r.take_commands().iter().any(|c| c.name == "preventDefault"));
    // A pinch out (Control held, a negative deltaY) zooms and is claimed.
    r.dispatch(world, event(36, "10,10,0,-50,0,Control")).unwrap();
    assert_eq!(r.slot("zoom"), Some(&Value::Number(1.5)));
    assert!(r.take_commands().iter().any(|c| c.name == "preventDefault"));
    r.dispatch(world, event(37, "12,30,Shift\ndoc:/3/plan.board\ndoc:/4/b.board")).unwrap();
    assert_eq!(r.slot("dropped"), Some(&Value::str("2 doc:/3/plan.board+doc:/4/b.board 12")));
    assert!(Event::of_host_kind(37, "12,30\n/etc/passwd").is_err(), "only minted handles drop");
    r.take_commands();
    r.dispatch(world, event(35, "")).unwrap();
    assert!(r.take_commands().iter().any(|c| c.name == "preventDefault"), "unsaved: the window stays");
}
