//! Large legitimate imports finish in time (E23 round 5's super-linear
//! paths): a DTCG file whose types come down a long alias chain, and a
//! re-import that changes every value of a large design.

use crate::common::vocab;
use exact_tamber::export::{dtcg, reimport};
use exact_tamber::import::dtcg as dtcg_import;
use exact_tamber::{check, parse};
use serde_json::Value;
use std::time::{Duration, Instant};

/// `f`'s result and how long it took.
fn timed<T>(f: impl FnOnce() -> T) -> (T, Duration) {
    let t = Instant::now();
    let out = f();
    (out, t.elapsed())
}

#[test]
fn a_long_alias_chain_types_each_token_once() {
    // c0 a colour; each next one aliases the one before, untyped, so its
    // type is c0's, found down the chain (past 256 links each is reported,
    // its type found all the same).
    const N: usize = 8000;
    let mut doc = serde_json::Map::new();
    doc.insert(
        "c0".into(),
        serde_json::json!({"$type": "color", "$value": "#102030"}),
    );
    for i in 1..N {
        doc.insert(
            format!("c{i}"),
            serde_json::json!({"$value": format!("{{c{}}}", i - 1)}),
        );
    }
    let files = vec![("tokens.json".to_string(), Value::Object(doc).to_string())];
    let (import, took) =
        timed(|| dtcg_import::import(&files, "chain", &parse("tamber app\n").unwrap()));
    let import = import.unwrap();
    assert_eq!(
        (
            import.counted.source,
            import.counted.mapped + import.counted.reported
        ),
        (N, N)
    );
    assert!(import.counted.mapped > 256, "{:?}", import.unmapped.first());
    assert!(
        took < Duration::from_secs(BOUND_CHAIN),
        "{N} aliases took {took:?}"
    );
}

/// A design of `n` colour tokens, each with a line for an axis's other value.
fn design(n: usize) -> String {
    let mut s =
        String::from("tamber t\n\naxes\n  dark-style = black | dim  under scheme=dark\n\ntheme\n");
    for i in 0..n {
        s += &format!("  c{i} = light-dark(#{:06x}, #{:06x})\n    when dark-style=dim: light-dark(#{:06x}, #{:06x})\n", i, i + 1, i, i + 2);
    }
    s
}

/// Every colour in `v` moved (its hex's top bit flipped).
fn moved(v: &mut Value) {
    match v {
        Value::Object(o) => {
            if let Some(Value::Object(val)) = o.get_mut("$value") {
                if let Some(Value::String(h)) = val.get("hex").cloned() {
                    let n = u32::from_str_radix(&h[1..7], 16).unwrap() ^ 0x80_0000;
                    let c = |k: u32| f64::from(k & 0xff) / 255.0;
                    val.insert("hex".into(), Value::String(format!("#{n:06x}")));
                    val.insert(
                        "components".into(),
                        serde_json::json!([c(n >> 16), c(n >> 8), c(n)]),
                    );
                    return;
                }
            }
            for (_, x) in o.iter_mut() {
                moved(x);
            }
        }
        Value::Array(a) => a.iter_mut().for_each(moved),
        _ => {}
    }
}

#[test]
fn re_importing_every_value_of_a_large_design_takes_each_once() {
    const N: usize = 2000;
    let src = design(N);
    let text = dtcg(&check(&src, &vocab()).unwrap(), &vocab()).unwrap();
    let mut doc: Value = serde_json::from_str(&text).unwrap();
    moved(&mut doc["modifiers"]);
    let (import, took) = timed(|| {
        reimport(
            &doc.to_string(),
            std::slice::from_ref(&text),
            &src,
            &vocab(),
        )
    });
    let import = import.unwrap();
    // Every token moves on both its lines; nothing is reported.
    assert_eq!((import.ops.len(), import.unmapped.len()), (2 * N, 0));
    assert!(
        took < Duration::from_secs(BOUND_REIMPORT),
        "{N} tokens took {took:?}"
    );
}

// Bounds with room for a loaded machine's debug build: each walk taken once
// and the proposal judged whole are well under them (2026-10-19, an M-series
// debug build: 0.22 s and 0.51 s), and the quadratic `type_of` and the cubic
// re-import were far over (3.8 s for 4,000 aliases, 4.2 s for 150 tokens).
const BOUND_CHAIN: u64 = 5;
const BOUND_REIMPORT: u64 = 10;

/// A design of `n` colour tokens over 64 contexts (five two-valued axes,
/// light and dark), each token with a `when` for one of the axes.
fn contexts_64(n: usize) -> String {
    let mut s = String::from("tamber t\n\naxes\n");
    for a in 0..5 {
        s += &format!("  a{a} = p | q\n");
    }
    s += "\ntheme\n";
    for i in 0..n {
        s += &format!(
            "  c{i} = light-dark(#{:06x}, #{:06x})\n    when a{}=q: light-dark(#{:06x}, #{:06x})\n",
            i,
            i + 1,
            i % 5,
            i,
            i + 2
        );
    }
    s
}

/// The design's export, and that export with every colour moved.
fn exported(src: &str) -> (String, String) {
    let text = dtcg(&check(src, &vocab()).unwrap(), &vocab()).unwrap();
    let mut doc: Value = serde_json::from_str(&text).unwrap();
    moved(&mut doc["modifiers"]);
    (text, doc.to_string())
}

#[test]
fn the_budget_takes_a_design_near_its_limit_and_refuses_one_past_it() {
    // Near the limits (calibrated to the isolate's 1 GiB, LLP 1121 §10):
    // 500 tokens over 64 contexts re-import (about 76% of `REIMPORT`), and
    // 700 import fresh (about 79% of `IMPORT`).
    let src = contexts_64(500);
    let (text, incoming) = exported(&src);
    let (import, took) = timed(|| reimport(&incoming, std::slice::from_ref(&text), &src, &vocab()));
    assert_eq!(import.unwrap().ops.len(), 1000);
    assert!(took < Duration::from_secs(BOUND_NEAR), "{took:?}");
    let (text, _) = exported(&contexts_64(700));
    let files = vec![("tokens.resolver.json".to_string(), text)];
    let (import, took) =
        timed(|| dtcg_import::import(&files, "fresh", &parse("tamber app\n").unwrap()));
    assert_eq!(import.unwrap().ops.len(), 701);
    assert!(took < Duration::from_secs(BOUND_NEAR), "{took:?}");
    // 3,000 tokens over 64 contexts: refused by the budget's message, not
    // run on into the isolate's RSS limit.
    // (Its export is never built whole here: a client's file is small, and
    // the design's own export is what the budget charges, as it is built.)
    let src = contexts_64(3000);
    let (_, small) = exported(&contexts_64(10));
    let (import, took) = timed(|| reimport(&small, &[], &src, &vocab()));
    let refused = import.unwrap_err();
    assert!(
        refused.iter().any(|d| d.message.contains("expands past")),
        "{refused:?}"
    );
    assert!(took < Duration::from_secs(BOUND_NEAR), "{took:?}");
    // And the server's check of a baseline against it, likewise.
    let refused = exact_tamber::export::own_digest(&src, &vocab()).unwrap_err();
    assert!(refused.iter().any(|d| d.message.contains("expands past")));
}

// A debug build's, with room for a loaded machine; release is far under
// (§10: 0.5 s and 0.4 s near the limits, the refusal in about 1 s).
const BOUND_NEAR: u64 = 30;
