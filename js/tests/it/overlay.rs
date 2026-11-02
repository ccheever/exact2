//! An overlay runs synchronously with no effects: every call that would
//! read the device, the network or storage refuses by name, a
//! promise-returning one by rejecting and any other by throwing. The wasm web
//! realm runs this prelude; `host/web/tests/overlay-guards.test.mjs` holds
//! the web JS target to the same calls.
#![cfg(exact_js_engine)]

use exact_js::Module;
use exact_plan::{Plan, Value};
use exact_runner::{Answer, DataSource, Store};

const HBC: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/overlay.hbc"));
const GRANTS: &str = "net.fetch https://fixture.exact.test\nfs.read app:/x\n";

const SRC: &str = r#"
shape Item
  text: string

component App
  resource items = items() as shape Item[]
  mutation captured as shape string
  mutation refused as shape string
  action capture
    send captured = capture()
  action read
    send refused = refusals()
  view
    column
      for item in items key=item.text
        text item.text
"#;

/// The overlay's calls in the fixture's order: each refuses by name, a
/// promise-returning one by rejecting and any other by throwing.
const REFUSED: &[(&str, &str)] = &[
    ("crypto.getRandomValues()", "throws"),
    ("crypto.randomUUID()", "throws"),
    ("crypto.subtle.digest()", "rejects"),
    ("crypto.subtle.generateKey()", "rejects"),
    ("crypto.subtle.sign()", "rejects"),
    ("crypto.subtle.verify()", "rejects"),
    ("crypto.subtle.importKey()", "rejects"),
    ("crypto.subtle.exportKey()", "rejects"),
    ("crypto.subtle.encrypt()", "rejects"),
    ("crypto.subtle.decrypt()", "rejects"),
    ("crypto.subtle.deriveBits()", "rejects"),
    ("fetch()", "rejects"),
    ("store.get()", "throws"),
    ("store.set()", "throws"),
    ("store.forget()", "throws"),
    ("store.keepKey()", "rejects"),
    ("store.key()", "rejects"),
    ("storage.readFile()", "rejects"),
];

fn plan() -> Plan {
    contract::compile(SRC).expect("overlay fixture compiles")
}

fn text(answer: Answer) -> String {
    match answer {
        Answer::Now(value) => value.as_str().expect("a string").to_string(),
        Answer::Later(_) => panic!("expected an answer now"),
    }
}

#[test]
fn every_effect_in_an_overlay_refuses_by_name() {
    let mut module = Module::loaded(HBC.to_vec(), "test.overlay", GRANTS).expect("loads");
    module.set_budget_ms(f64::INFINITY);
    module.bind(&plan());
    let mut store = Store::new(GRANTS, vec![]);
    assert_eq!(
        text(module.answer(&mut store, "capture", &[]).unwrap()),
        "captured"
    );
    let shown = module
        .overlay("items", &[], &Value::list(vec![]), &[])
        .expect("the overlay ran");
    assert!(shown.is_none(), "it shows the answer");
    let refusals = text(module.answer(&mut store, "refusals", &[]).unwrap());
    let lines: Vec<&str> = refusals.lines().collect();
    assert_eq!(lines.len(), REFUSED.len(), "{refusals}");
    for ((name, how), line) in REFUSED.iter().zip(&lines) {
        let expected = format!("{how} {name} is unavailable in an overlay");
        assert!(line.starts_with(&expected), "{expected}…\n{line}");
    }
    assert_eq!(store.reads(), 0, "nothing was read");
}

const NONE_HBC: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/overlay-none.hbc"));
const SOME_HBC: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/overlay-some.hbc"));
const ODD_HBC: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/overlay-odd.hbc"));
const THROWS_HBC: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/overlay-throws.hbc"));
const GETTER_HBC: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/overlay-getter.hbc"));

const NONE_SRC: &str = r#"
shape Item
  text: string

component App
  resource items = items() as shape Item[]
  mutation armed as shape string
  mutation disarmed as shape string
  mutation counted as shape string
  action arm
    send armed = arm()
  action disarm
    send disarmed = disarm()
  action count
    send counted = reads()
  view
    column
      for item in items key=item.text
        text item.text
"#;

fn loaded(bytecode: &[u8]) -> Module {
    let mut module = Module::loaded(bytecode.to_vec(), "test.overlay-none", "").expect("loads");
    module.set_budget_ms(f64::INFINITY);
    module.bind(&contract::compile(NONE_SRC).expect("the no-overlay fixture compiles"));
    module
}

fn many_items() -> Value {
    Value::list(
        (0..300)
            .map(|i| Value::record(vec![Value::str(&format!("item {i}"))]))
            .collect(),
    )
}

/// The overlay's value, if one ran: the answer's 300 rows and the one it adds.
fn overlaid(module: &mut Module) -> Option<Value> {
    module
        .overlay("items", &[], &many_items(), &[])
        .expect("the overlay ran")
        .map(|o| o.value)
}

fn added(value: &Value) -> bool {
    let Value::List(rows) = value else {
        return false;
    };
    rows.len() == 301
        && matches!(rows.iter().last(), Some(Value::Record(r)) if r[0].as_str() == Some("overlaid"))
}

/// A module loaded with an overlay (defined at its top level) overlays; one
/// loaded without never has its answer (here 300 rows) encoded for one.
#[test]
fn an_overlay_at_load_overlays_and_none_at_load_encodes_nothing() {
    let mut with = loaded(SOME_HBC);
    assert!(overlaid(&mut with).as_ref().is_some_and(added));
    assert_eq!(with.overlays_encoded(), 1);
    let mut without = loaded(NONE_HBC);
    for _ in 0..3 {
        assert_eq!(overlaid(&mut without), None, "the answer shows");
    }
    assert_eq!(without.overlays_encoded(), 0, "nothing was encoded");
}

/// The fact is each instance's, decided when it loads: a generation loaded
/// again with an overlay overlays, and one loaded again without encodes none.
#[test]
fn a_module_loaded_again_with_or_without_an_overlay_is_decided_afresh() {
    assert_eq!(overlaid(&mut loaded(NONE_HBC)), None);
    let mut second = loaded(SOME_HBC);
    assert!(
        overlaid(&mut second).as_ref().is_some_and(added),
        "the new generation overlays"
    );
    let mut third = loaded(NONE_HBC);
    assert_eq!(overlaid(&mut third), None);
    assert_eq!(
        third.overlays_encoded(),
        0,
        "and the next, without, encodes nothing"
    );
}

/// After load an overlay set or deleted is not followed, as on the JS web
/// target (host/web-js/ts-data.js reads `source.overlay` at install): a module
/// loaded without one still shows the answer once it sets one; one loaded
/// with one still asks it (and the answer shows, the prelude finding none).
#[test]
fn an_overlay_set_or_deleted_after_load_is_not_followed() {
    let mut store = Store::new("", vec![]);
    let mut without = loaded(NONE_HBC);
    assert_eq!(
        text(without.answer(&mut store, "arm", &[]).unwrap()),
        "armed"
    );
    assert_eq!(overlaid(&mut without), None);
    assert_eq!(
        without.overlays_encoded(),
        0,
        "set after load: not followed"
    );
    let mut with = loaded(SOME_HBC);
    assert_eq!(
        text(with.answer(&mut store, "disarm", &[]).unwrap()),
        "disarmed"
    );
    assert_eq!(
        overlaid(&mut with),
        None,
        "the prelude finds none, so the answer shows"
    );
    assert_eq!(
        with.overlays_encoded(),
        1,
        "deleted after load: still asked"
    );
}

/// A proxied `exact` whose `get` supplies `overlay` is read once at load and
/// found; each overlay then reads it as before (twice: the prelude's check and
/// its call).
#[test]
fn an_overlay_a_proxy_supplies_is_found_at_load() {
    let mut store = Store::new("", vec![]);
    let mut module = loaded(ODD_HBC);
    assert_eq!(
        text(module.answer(&mut store, "reads", &[]).unwrap()),
        "1",
        "one read at load"
    );
    for _ in 0..2 {
        assert!(overlaid(&mut module).as_ref().is_some_and(added));
    }
    assert_eq!(
        text(module.answer(&mut store, "reads", &[]).unwrap()),
        "5",
        "and two an overlay"
    );
}

/// A load-time read that throws is no answer: the module keeps the per-call
/// path, never skipping silently, and its overlay runs once it reads.
#[test]
fn a_load_time_read_that_throws_keeps_the_overlay_asked() {
    let mut module = loaded(THROWS_HBC);
    assert!(overlaid(&mut module).as_ref().is_some_and(added));
    assert_eq!(module.overlays_encoded(), 1);
}

/// The bake's entrance (`inspect`) decides the overlay as `load` does: one
/// read, at the same step, and the same answer cached after it.
#[test]
fn the_bake_reads_the_overlay_at_load_as_a_device_does() {
    let mut store = Store::new("", vec![]);
    let mut baked = Module::inspect(ODD_HBC.to_vec()).expect("inspects");
    baked.set_budget_ms(f64::INFINITY);
    baked.bind(&contract::compile(NONE_SRC).expect("the no-overlay fixture compiles"));
    let mut device = loaded(ODD_HBC);
    for module in [&mut baked, &mut device] {
        assert_eq!(
            text(module.answer(&mut store, "reads", &[]).unwrap()),
            "1",
            "one read at load"
        );
        assert!(overlaid(module).as_ref().is_some_and(added));
        assert_eq!(text(module.answer(&mut store, "reads", &[]).unwrap()), "3");
        assert_eq!(module.overlays_encoded(), 1);
    }
    let mut none = Module::inspect(NONE_HBC.to_vec()).expect("inspects");
    none.set_budget_ms(f64::INFINITY);
    none.bind(&contract::compile(NONE_SRC).expect("the no-overlay fixture compiles"));
    assert!(overlaid(&mut none).is_none());
    assert_eq!(
        none.overlays_encoded(),
        0,
        "nothing encoded at the bake either"
    );
}

/// A getter on `exact.overlay` runs inside the overlay's guard at commit:
/// its `store.set` is refused on both of `__exact_overlay`'s reads, not
/// only on the call's.
#[test]
fn a_getters_store_call_is_refused_on_both_reads() {
    let mut store = Store::new("", vec![]);
    let mut module = loaded(GETTER_HBC);
    assert_eq!(
        text(module.answer(&mut store, "arm", &[]).unwrap()),
        "captured"
    );
    assert!(overlaid(&mut module).as_ref().is_some_and(added));
    let tries = text(module.answer(&mut store, "reads", &[]).unwrap());
    let lines: Vec<&str> = tries.lines().collect();
    assert_eq!(lines.len(), 2, "{tries}");
    for line in lines {
        assert!(
            line.starts_with("throws store.set() is unavailable in an overlay"),
            "{line}"
        );
    }
}
