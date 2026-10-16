//! DTCG export and its own round trip (LLP 1121 D14, stage 5 commit 6).

use crate::common::vocab;
use exact_tamber::export::{dtcg, is_own, reimport};
use exact_tamber::{check, parse, Op};

use serde_json::Value;

fn exported(src: &str) -> (String, Value) {
    let folding = check(src, &vocab()).unwrap();
    let text = dtcg(&folding, &vocab()).unwrap();
    let doc = serde_json::from_str(&text).unwrap();
    (text, doc)
}

const FIXTURE: &str = include_str!("../../../apps/theme-fixture/app.tamber");
const CLONE: &str = include_str!("../corpus/bluesky.tamber");

/// A context's token objects, by name (a group's `$root` the group's).
fn tokens(doc: &Value, context: &str) -> Vec<(String, Value)> {
    fn walk(v: &Value, prefix: &str, out: &mut Vec<(String, Value)>) {
        let obj = v.as_object().unwrap();
        if obj.contains_key("$value") {
            out.push((prefix.to_string(), v.clone()));
            return;
        }
        for (k, v) in obj {
            let name = match k.as_str() {
                "$root" => prefix.to_string(),
                k if k.starts_with('$') => continue,
                k if prefix.is_empty() => k.to_string(),
                k => format!("{prefix}.{k}"),
            };
            walk(v, &name, out);
        }
    }
    let mut out = Vec::new();
    for src in doc["modifiers"]["mode"]["contexts"][context]
        .as_array()
        .unwrap()
    {
        walk(src, "", &mut out);
    }
    out
}

#[test]
fn every_context_is_a_full_tuple_and_a_complete_set() {
    for src in [FIXTURE, CLONE] {
        let folding = check(src, &vocab()).unwrap();
        let (text, doc) = exported(src);
        assert!(is_own(&text));
        assert_eq!(doc["version"], "2025.10");
        let contexts = doc["modifiers"]["mode"]["contexts"].as_object().unwrap();
        assert_eq!(contexts.len(), folding.tuples * 2);
        for (name, _) in contexts {
            let all = tokens(&doc, name);
            assert_eq!(all.len(), folding.tokens.len(), "{name}");
            for (token, t) in &all {
                assert_eq!(t["$type"], "color", "{name} {token}");
                let v = &t["$value"];
                assert!(
                    v["colorSpace"].is_string() && v["components"].as_array().unwrap().len() == 3,
                    "{name} {token}: {v}"
                );
                assert!(v["hex"].as_str().unwrap().len() == 7, "{name} {token}");
            }
        }
        // The header, read by the round trip, and the default context.
        let header = &doc["sets"]["exact"]["sources"][0]["$extensions"]["exact"];
        assert_eq!(
            header["contexts"].as_object().unwrap().len(),
            contexts.len()
        );
        assert!(contexts.contains_key(doc["modifiers"]["mode"]["default"].as_str().unwrap()));
    }
}

#[test]
fn the_fixtures_tuples_roles_and_overrides_are_named() {
    let (_, doc) = exported(FIXTURE);
    let contexts: Vec<&String> = doc["modifiers"]["mode"]["contexts"]
        .as_object()
        .unwrap()
        .keys()
        .collect();
    for c in [
        "dark-style=black, light",
        "dark-style=black, dark",
        "dark-style=dim, light",
        "dark-style=dim, dark",
    ] {
        assert!(contexts.iter().any(|k| *k == c), "{c} in {contexts:?}");
    }
    let dim = tokens(&doc, "dark-style=dim, dark");
    let get = |n: &str| &dim.iter().find(|(k, _)| k == n).unwrap().1;
    assert_eq!(get("surface.page")["$value"]["hex"], "#15202b");
    // A role's value at its fallback, the role named; a role override named so.
    assert_eq!(
        get("text.label")["$extensions"]["exact"]["role"],
        "-exact-label"
    );
    assert_eq!(
        get("text.label")["$extensions"]["exact"]["source"],
        "-exact-label"
    );
    assert_eq!(
        get("AccentColor")["$extensions"]["exact"]["overrides"],
        "AccentColor"
    );
    // An expression's source: the line the tuple's side takes.
    assert_eq!(
        get("surface.page")["$extensions"]["exact"]["source"],
        "light-dark(#ffffff, #15202b)"
    );
}

#[test]
fn re_importing_the_unedited_export_proposes_nothing() {
    for src in [FIXTURE, CLONE] {
        let (text, _) = exported(src);
        let import = reimport(&text, &text, &parse(src).unwrap()).unwrap();
        assert!(import.ops.is_empty(), "{:?}", import.ops);
        assert!(import.unmapped.is_empty(), "{:?}", import.unmapped);
        assert_eq!(import.counted.source, import.counted.mapped);
    }
}

#[test]
fn one_changed_value_proposes_exactly_one_set_token() {
    let (text, mut doc) = exported(FIXTURE);
    let tree = &mut doc["modifiers"]["mode"]["contexts"]["dark-style=dim, dark"][0];
    tree["accent"]["fill"]["$value"] =
        serde_json::json!({"colorSpace": "srgb", "components": [1.0, 0.0, 0.0], "hex": "#ff0000"});
    let edited = serde_json::to_string_pretty(&doc).unwrap();
    let import = reimport(&edited, &text, &parse(FIXTURE).unwrap()).unwrap();
    assert_eq!(import.ops.len(), 1, "{:?}", import.ops);
    let Op::SetToken {
        name, when, value, ..
    } = &import.ops[0]
    else {
        panic!("{:?}", import.ops)
    };
    assert_eq!(name, "accent.fill");
    assert_eq!(when, &vec![("dark-style".to_string(), "dim".to_string())]);
    assert_eq!(value, "light-dark(#1d9bf0, #ff0000)");
    // Accepted, the design exports as the edited file, and re-importing that
    // proposes nothing more: the round trip closes.
    let accepted = exact_tamber::apply_all(FIXTURE, &import.ops, &vocab())
        .unwrap()
        .text;
    let (again, _) = exported(&accepted);
    let import = reimport(&edited, &again, &parse(&accepted).unwrap()).unwrap();
    assert!(import.ops.is_empty(), "{:?}", import.ops);
    // A token the export did not have is reported, not added.
    let mut doc: Value = serde_json::from_str(&text).unwrap();
    for (_, c) in doc["modifiers"]["mode"]["contexts"]
        .as_object_mut()
        .unwrap()
    {
        c[0]["extra"] = serde_json::json!({"$type": "color", "$value": {"colorSpace": "srgb", "components": [0, 0, 0]}});
    }
    let import = reimport(&doc.to_string(), &text, &parse(FIXTURE).unwrap()).unwrap();
    assert!(import.ops.is_empty());
    assert_eq!(import.unmapped.len(), 1);
    assert_eq!(import.unmapped[0].name, "extra");
}
