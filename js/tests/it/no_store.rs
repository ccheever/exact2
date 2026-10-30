//! Where the executor offers no store, a let-go answer's continuation, every
//! store operation throws rather than reading as "nothing kept" (LLP 1146 D3),
//! with the castle fixture.
#![cfg(exact_js_engine)]
use super::castle::{later, module, now, store};
use exact_plan::Value;

/// The continuation of a let-go answer runs with no store: reading it
/// throws, as a write does, rather than reading as "nothing kept"
/// (LLP 1146 D3).
#[test]
fn a_let_go_continuation_cannot_read_the_store() {
    use exact_runner::{DataSource, Target};
    let mut m = module();
    m.bind(&contract::compile("component App\n  resource seen = peek() as shape string\n  resource busy = flushing() as shape string\n  view\n    column\n      text seen\n      text busy\n").unwrap());
    let mut s = store();
    s.set("castle.session", "kept").unwrap();
    later(
        m.answer_for(Target::Resource(0), &mut s, "peek", &[])
            .unwrap(),
    );
    m.forgotten(&s, &[]);
    assert_eq!(
        now(m.answer(&mut s, "flushing", &[]).unwrap()),
        Value::str("false store.get: no store here, in background work or the continuation of an answer that was let go. Read the store before the answer's first await. A write that must stand goes after a request with effects (a POST), whose continuation keeps the store.")
    );
}

/// So does every other store operation there, each naming itself (LLP 1146 D3).
#[test]
fn a_let_go_continuation_has_no_store_for_any_operation() {
    use exact_runner::{DataSource, Target};
    let mut m = module();
    m.bind(&contract::compile("component App\n  resource seen = peekAll() as shape string\n  resource busy = flushing() as shape string\n  view\n    column\n      text seen\n      text busy\n").unwrap());
    let mut s = store();
    s.set("castle.session", "kept").unwrap();
    later(
        m.answer_for(Target::Resource(0), &mut s, "peekAll", &[])
            .unwrap(),
    );
    m.forgotten(&s, &[]);
    let said = now(m.answer(&mut s, "flushing", &[]).unwrap());
    let said = said
        .as_str()
        .unwrap()
        .strip_prefix("false ")
        .unwrap()
        .to_owned();
    let ops = [
        "get",
        "has",
        "locked",
        "unlock",
        "protect",
        "unprotect",
        "set",
        "forget",
    ];
    let mut expected: Vec<String> = ops.iter().map(|op| format!("store.{op}: no store here, in background work or the continuation of an answer that was let go")).collect();
    // A kept key is an answer's: with none running, it is refused as outside one.
    expected.push("store.keepKey() called outside an answer".into());
    expected.push("store.key() called outside an answer".into());
    assert_eq!(said.split(" | ").collect::<Vec<_>>(), expected);
    assert_eq!(
        s.get("castle.session"),
        Some("kept"),
        "nothing reached the store"
    );
}
