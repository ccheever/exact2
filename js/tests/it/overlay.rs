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
  resource items = items() as shape list<Item>
  mutation captured as shape string
  mutation refused as shape string
  action capture
    send captured = capture()
  action read
    send refused = refusals()
  view
    column
      each item in items key=item.text
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
