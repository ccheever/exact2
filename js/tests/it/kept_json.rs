//! A list element that is deep-frozen plain data keeps its JSON by identity and is spliced into a later reply
//! (js/src/binary.js `keptReply`, rt1's crossing plan): every reply is what a plain `JSON.stringify` of the
//! answer gives now, whether its elements were kept, mutated, hooked or frozen only on the outside.

#![cfg(exact_js_engine)]

use exact_js::Module;
use exact_plan::{Items, Plan, Value};
use exact_runner::{Answer, DataSource, Store};

const HBC: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/kept-json.hbc"));

const SRC: &str = r#"
shape Inner
  k: string
shape Row
  id: string
  text: string
  n: number
  inner: Inner[]
component App
  resource rows = rows("frozen") as shape Row[]
  resource lengths = lengths() as shape string
  resource proxy = proxy() as shape string
  resource conversions = conversions() as shape string
  resource hookReads = hookReads() as shape string
  view
    text "rows"
"#;

fn module() -> Module {
    let plan: Plan = contract::compile(SRC).expect("the fixture's Contract compiles");
    let mut m = Module::new(HBC.to_vec(), "test.kept-json", "");
    m.set_budget_ms(f64::INFINITY);
    m.bind(&plan);
    m.load().expect("the fixture loads");
    m
}

/// A reply as bytes, or its error's text: two replies compare exactly.
fn rows(m: &mut Module, s: &mut Store, kind: &str) -> Result<Vec<u8>, String> {
    match m.answer(s, "rows", &[Value::str(kind)]) {
        Ok(Answer::Now(v)) => Ok(v.to_bytes()),
        Ok(other) => Err(format!("not answered at once: {other:?}")),
        Err(e) => Err(format!("{e:?}")),
    }
}

#[test]
fn a_reply_is_a_plain_stringify_whether_or_not_its_elements_were_kept() {
    let mut m = module();
    let mut s = Store::new("", []);
    for kind in [
        "frozen",
        "mutable",
        "shallow",
        "hooked",
        "getter",
        "large",
        "nul",
        "nulQuote",
        "dropped",
        "short",
        "boxed",
        "keyDropped",
        "fnHook",
        "wrapped",
        "nulTail",
        "boxedHook",
        "boxedUnspelled",
        "nestedProxy",
        "nulKept",
        "proxyHoles",
        "accessorHook",
        "revocableProxy",
    ] {
        for round in 0..3 {
            let reply = rows(&mut m, &mut s, kind);
            let plain = rows(&mut m, &mut s, &format!("plain {kind}"));
            assert!(reply.is_ok(), "{kind}, round {round}: {reply:?}");
            assert_eq!(reply, plain, "{kind}, round {round}");
            // Hooks, getters and mutations read the counter: a kept text would show the old one.
            rows(&mut m, &mut s, "tick").unwrap();
        }
    }
}

#[test]
fn a_key_beginning_with_u0000_is_read_as_a_key_not_a_kept_element() {
    let mut m = module();
    let mut s = Store::new("", []);
    for round in 0..2 {
        // The frozen rows are kept by now: a key read as a token would splice one of them in, or break the JSON.
        rows(&mut m, &mut s, "frozen").unwrap();
        for kind in ["nulKey", "nulTailKey"] {
            let reply = rows(&mut m, &mut s, kind).unwrap_err();
            assert!(
                reply.contains("` is not in the shape"),
                "{kind}, round {round}: {reply}"
            );
            assert_eq!(
                Err(reply),
                rows(&mut m, &mut s, &format!("plain {kind}")),
                "{kind}, round {round}"
            );
        }
    }
}

/// The rows of a reply, each record's allocation.
fn records(m: &mut Module, s: &mut Store, kind: &str) -> Vec<Items> {
    match m.answer(s, "rows", &[Value::str(kind)]) {
        Ok(Answer::Now(Value::List(rows))) => (rows.as_slice().iter())
            .map(|r| match r {
                Value::Record(items) => items.clone(),
                other => panic!("a row, got {other:?}"),
            })
            .collect(),
        other => panic!("rows, got {other:?}"),
    }
}

#[test]
fn a_repeated_row_decodes_to_the_same_value_across_the_answers_own_envelopes() {
    let mut m = module();
    let mut s = Store::new("", []);
    // Each answer passes a call-begun envelope before its value: those leave the decoder's kept rows
    // alone, so each row is the same value as the reply before's.
    let first = records(&mut m, &mut s, "frozen");
    let second = records(&mut m, &mut s, "frozen");
    assert_eq!(first.len(), 40);
    assert!(first.iter().zip(&second).all(|(a, b)| Items::ptr_eq(a, b)));
    // A reply carrying a captured large string decodes apart from them and leaves them kept too.
    rows(&mut m, &mut s, "large").unwrap();
    let third = records(&mut m, &mut s, "frozen");
    assert!(first.iter().zip(&third).all(|(a, b)| Items::ptr_eq(a, b)));
}

#[test]
fn a_list_behind_a_proxy_has_its_length_read_as_often_as_json_reads_it() {
    let mut m = module();
    let mut s = Store::new("", []);
    let lengths = |m: &mut Module, s: &mut Store| match m.answer(s, "lengths", &[]) {
        Ok(Answer::Now(v)) => v.as_str().unwrap_or_default().parse::<u32>().unwrap(),
        other => panic!("{other:?}"),
    };
    for round in 0..3 {
        let before = lengths(&mut m, &mut s);
        let reply = rows(&mut m, &mut s, "proxiedList");
        let replied = lengths(&mut m, &mut s);
        let plain = rows(&mut m, &mut s, "plain proxiedList");
        let stringified = lengths(&mut m, &mut s);
        assert_eq!(reply, plain, "round {round}");
        // The reply's stringify reads the length as the oracle's plain JSON.stringify does.
        assert_eq!(replied - before, stringified - replied, "round {round}");
    }
}

#[test]
fn an_app_s_proxy_works_as_the_engine_s_while_the_prelude_tracks_it() {
    let mut m = module();
    let mut s = Store::new("", []);
    match m.answer(&mut s, "proxy", &[]) {
        Ok(Answer::Now(v)) => {
            assert_eq!(
                v.as_str(),
                Some("function 1 TypeError TypeError undefined TypeError 3 Proxy 2 false")
            )
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_boxed_string_whose_conversion_throws_fails_the_reply_as_json_would_converting_once() {
    let mut m = module();
    let mut s = Store::new("", []);
    let conversions = |m: &mut Module, s: &mut Store| match m.answer(s, "conversions", &[]) {
        Ok(Answer::Now(v)) => v.as_str().unwrap_or_default().parse::<u32>().unwrap(),
        other => panic!("{other:?}"),
    };
    // Kept rows beside it, so the reply goes through the splice.
    rows(&mut m, &mut s, "frozen").unwrap();
    let before = conversions(&mut m, &mut s);
    let reply = rows(&mut m, &mut s, "boxedThrows").unwrap_err();
    assert!(reply.contains("no text"), "{reply}");
    assert_eq!(conversions(&mut m, &mut s) - before, 1, "converted once");
}

#[test]
fn a_nested_accessor_hook_is_read_once_a_reply_as_plain_json_reads_it() {
    let mut m = module();
    let mut s = Store::new("", []);
    let reads = |m: &mut Module, s: &mut Store| match m.answer(s, "hookReads", &[]) {
        Ok(Answer::Now(v)) => v.as_str().unwrap_or_default().parse::<u32>().unwrap(),
        other => panic!("{other:?}"),
    };
    for round in 0..3 {
        let before = reads(&mut m, &mut s);
        rows(&mut m, &mut s, "accessorHook").unwrap();
        let replied = reads(&mut m, &mut s);
        rows(&mut m, &mut s, "plain accessorHook").unwrap();
        assert_eq!(
            (replied - before, reads(&mut m, &mut s) - replied),
            (1, 1),
            "round {round}"
        );
    }
}
