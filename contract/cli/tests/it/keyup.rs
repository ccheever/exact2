//! `keyup`, and `KeyboardEvent.code` and `.repeat` (#140): DOM's release of a
//! key, a modifier's included, heard with the same name and record as `key`
//! (keydown), and the physical key and the platform's auto-repeat on both.
//! The events come over the hosts' wire (`Event::of_host_kind`, kinds 6 and
//! 43), as every host writes them.

use exact_kernel::Kernel;
use exact_plan::{EventKind, Value};
use exact_runner::{DataError, DataSource, Event, Runner};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

/// ⌘-held hints and a ⌘W that closes once per press, by the physical key.
const HINTS: &str = r#"component App
  state log = ""
  state hints = false
  state closed = 0
  action down(k: string, e: KeyboardEvent)
    log = `${log}d:${k}:${e.code}:${e.repeat}:${e.metaKey};`
    if k == "Meta"
      hints = true
    if e.metaKey && e.code == "KeyW" && ! e.repeat
      closed = closed + 1
  action up(k: string, e: KeyboardEvent)
    log = `${log}u:${k}:${e.code}:${e.repeat}:${e.metaKey};`
    if k == "Meta"
      hints = false
  action bare(k: string)
    log = `${log}b:${k};`
  view
    column key=down keyup=up testId="list"
      text "row" keyup=bare tabindex=0 testId="row"
"#;

fn boot(src: &str) -> Runner<NoData> {
    let plan = contract::compile(src).unwrap_or_else(|e| panic!("{e}"));
    Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

fn id(r: &Runner<NoData>, test_id: &str) -> u32 {
    let k = r.kernel();
    k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().id
}

fn wire(kind: u32, payload: &str) -> Event {
    Event::of_host_kind(kind, payload).unwrap()
}

#[test]
fn capture_handler_compiles_and_dispatches_keyboard_event() {
    let src = HINTS.replace(
        "column key=down keyup=up",
        "column keycapture=down key=down keyup=up",
    );
    let mut r = boot(&src);
    let list = id(&r, "list");
    assert!(r.handlers_of(list).contains(&EventKind::Keycapture));
    r.dispatch(list, wire(44, "Meta+b\nKeyB\nfalse")).unwrap();
    assert_eq!(r.slot("log"), Some(&Value::str("d:b:KeyB:false:true;")));
}

#[test]
fn keyup_hears_a_modifiers_release_and_both_carry_code_and_repeat() {
    let mut r = boot(HINTS);
    let list = id(&r, "list");
    assert_eq!(r.handlers_of(list), vec![EventKind::Key, EventKind::Keyup]);
    r.dispatch(list, wire(6, "Meta+Meta\nMetaLeft\nfalse"))
        .unwrap();
    assert_eq!(r.slot("hints"), Some(&Value::Bool(true)));
    // ⌘W held: the down closes, its repeats do not.
    r.dispatch(list, wire(6, "Meta+w\nKeyW\nfalse")).unwrap();
    r.dispatch(list, wire(6, "Meta+w\nKeyW\ntrue")).unwrap();
    r.dispatch(list, wire(6, "Meta+w\nKeyW\ntrue")).unwrap();
    assert_eq!(r.slot("closed"), Some(&Value::Number(1.0)));
    r.dispatch(list, wire(43, "Meta+w\nKeyW\nfalse")).unwrap();
    // Meta's release: no longer held.
    r.dispatch(list, wire(43, "Meta\nMetaLeft\nfalse")).unwrap();
    assert_eq!(r.slot("hints"), Some(&Value::Bool(false)));
    assert_eq!(
        r.slot("log"),
        Some(&Value::str(
            "d:Meta:MetaLeft:false:true;d:w:KeyW:false:true;d:w:KeyW:true:true;d:w:KeyW:true:true;\
             u:w:KeyW:false:true;u:Meta:MetaLeft:false:false;"
        ))
    );
}

#[test]
fn a_bare_chord_is_code_empty_and_a_keyup_takes_its_name_alone() {
    let mut r = boot(HINTS);
    // A host that cannot tell the physical key sends the chord alone.
    r.dispatch(id(&r, "list"), Event::key("a")).unwrap();
    r.dispatch(id(&r, "row"), wire(43, "Shift+A\nKeyA\nfalse"))
        .unwrap();
    assert_eq!(r.slot("log"), Some(&Value::str("d:a::false:false;b:A;")));
    // A keyup is never a repeat, whatever the wire says.
    r.dispatch(id(&r, "list"), wire(43, "x\nKeyX\ntrue"))
        .unwrap();
    assert!(matches!(r.slot("log"), Some(v) if v.text().ends_with("u:x:KeyX:false:false;")));
}

#[test]
fn a_malformed_key_wire_is_refused_by_name() {
    for payload in [
        "a\nKeyA",
        "a\nKeyA\nmaybe",
        "a\nKey-A\nfalse",
        "a\nKeyA\nfalse\nmaybe",
    ] {
        assert_eq!(
            Event::of_host_kind(6, payload).err(),
            Some("invalid key event")
        );
        assert_eq!(
            Event::of_host_kind(43, payload).err(),
            Some("invalid key event")
        );
    }
}

#[test]
fn keyup_takes_keys_payload_and_record() {
    // `code` is a string and `repeat` a bool.
    let e = contract::compile(&HINTS.replace("e.code == \"KeyW\"", "e.code == 1")).unwrap_err();
    assert_eq!(e.id, "type-operand", "{e}");
    // A keyup action hears the key's name: a number is the wrong payload.
    let e = contract::compile(&HINTS.replace("action bare(k: string)", "action bare(k: number)"))
        .unwrap_err();
    assert_eq!(e.id, "type-handler-payload", "{e}");
    assert!(
        e.to_string()
            .contains("`keyup=` passes its captured arguments, then its payload, a `string`"),
        "{e}"
    );
    // A third parameter is more than `keyup` hands.
    let e = contract::compile(&HINTS.replace(
        "action up(k: string, e: KeyboardEvent)",
        "action up(k: string, e: KeyboardEvent, f: bool)",
    ))
    .unwrap_err();
    assert!(e.to_string().contains("keyup"), "{e}");
}

/// An input method's candidate Enter (`isComposing`), skipped as web code
/// skips it: a composer sends only on an Enter that came after the
/// composition. A host that writes no fourth line is not composing.
const COMPOSER: &str = r#"component App
  state sent = 0
  state log = ""
  action compose(k: string, e: KeyboardEvent)
    log = `${log}${k}:${e.isComposing};`
    if k == "Enter" && ! e.isComposing
      sent = sent + 1
  view
    input key=compose testId="field"
"#;

#[test]
fn is_composing_lets_a_handler_skip_the_candidates_enter() {
    let mut r = boot(COMPOSER);
    let field = id(&r, "field");
    r.dispatch(field, wire(6, "Enter\nEnter\nfalse\ntrue"))
        .unwrap();
    assert_eq!(r.slot("sent"), Some(&Value::Number(0.0)));
    r.dispatch(field, wire(6, "Enter\nEnter\nfalse")).unwrap();
    assert_eq!(r.slot("sent"), Some(&Value::Number(1.0)));
    assert_eq!(r.slot("log"), Some(&Value::str("Enter:true;Enter:false;")));
    // `isComposing` is a bool.
    let e =
        contract::compile(&COMPOSER.replace("! e.isComposing", "e.isComposing == 1")).unwrap_err();
    assert_eq!(e.id, "type-operand", "{e}");
}

/// Held modifiers the web's way (#140): each keydown and keyup sets the
/// flag from the event, and losing the window's focus forgets it, as a
/// page's `window` `blur` listener does, since a key let go in another app
/// sends no keyup. `exactPage().hasFocus` going false arms the task.
const HELD: &str = r#"shape Page
  hasFocus: bool

component App
  resource page = exactPage() as shape Page
  state meta = false
  action down(k: string, e: KeyboardEvent)
    meta = e.metaKey
  action up(k: string, e: KeyboardEvent)
    meta = e.metaKey
  action forget()
    meta = false
  task release when meta && ! page.hasFocus
    after(1, forget)
  view
    column key=down keyup=up testId="list"
      if meta && page.hasFocus
        text "⌘" testId="hint"
"#;

#[test]
fn a_held_modifier_is_forgotten_when_the_window_loses_focus() {
    let mut r = boot(HELD);
    let list = id(&r, "list");
    let hint = |r: &Runner<NoData>| !r.kernel().find_by_test_id("hint").is_empty();
    r.dispatch(list, wire(6, "Meta+Meta\nMetaLeft\nfalse"))
        .unwrap();
    assert!(hint(&r));
    // Another app comes forward with ⌘ still down and ⌘ comes up there: no keyup.
    let blurred = exact_runner::Page {
        has_focus: false,
        ..r.page()
    };
    r.set_page(blurred).unwrap();
    assert!(!hint(&r), "no hint while the window has no focus");
    r.advance(1.0).unwrap();
    assert_eq!(r.slot("meta"), Some(&Value::Bool(false)));
    r.set_page(exact_runner::Page {
        has_focus: true,
        ..blurred
    })
    .unwrap();
    assert!(!hint(&r), "back in front, ⌘ is not held");
    // A ⌘ pressed again is heard as before.
    r.dispatch(list, wire(6, "Meta+Meta\nMetaLeft\nfalse"))
        .unwrap();
    assert!(hint(&r));
}
