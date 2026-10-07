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
    if e.metaKey and e.code == "KeyW" and not e.repeat
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
    for payload in ["a\nKeyA", "a\nKeyA\nmaybe", "a\nKey-A\nfalse"] {
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
    assert!(e.to_string().contains("`keyup=` supplies `string`"), "{e}");
    // A third parameter is more than `keyup` hands.
    let e = contract::compile(&HINTS.replace(
        "action up(k: string, e: KeyboardEvent)",
        "action up(k: string, e: KeyboardEvent, f: bool)",
    ))
    .unwrap_err();
    assert!(e.to_string().contains("keyup"), "{e}");
}
