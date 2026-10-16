//! DTCG export and its own round trip (LLP 1121 D14, stage 5 commit 6).

use crate::common::vocab;
use exact_tamber::export::{dtcg, is_own, reimport};
use exact_tamber::{check, Op};
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
        let import = reimport(&text, std::slice::from_ref(&text), src, &vocab()).unwrap();
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
    let import = reimport(&edited, std::slice::from_ref(&text), FIXTURE, &vocab()).unwrap();
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
    // Accepted, the same file proposes nothing more: the round trip closes.
    let accepted = exact_tamber::apply_all(FIXTURE, &import.ops, &vocab())
        .unwrap()
        .text;
    let import = reimport(&edited, std::slice::from_ref(&text), &accepted, &vocab()).unwrap();
    assert!(import.ops.is_empty(), "{:?}", import.ops);
    // A token the export did not have is reported, not added.
    let mut doc: Value = serde_json::from_str(&text).unwrap();
    for (_, c) in doc["modifiers"]["mode"]["contexts"]
        .as_object_mut()
        .unwrap()
    {
        c[0]["extra"] = serde_json::json!({"$type": "color", "$value": {"colorSpace": "srgb", "components": [0, 0, 0]}});
    }
    let import = reimport(
        &doc.to_string(),
        std::slice::from_ref(&text),
        FIXTURE,
        &vocab(),
    )
    .unwrap();
    assert!(import.ops.is_empty());
    assert_eq!(import.unmapped.len(), 1);
    assert_eq!(import.unmapped[0].name, "extra");
}

/// The fixture's export, edited in place by `f`, re-imported against the
/// unedited one.
fn edited_import(f: impl FnOnce(&mut Value)) -> exact_tamber::import::Import {
    let (text, mut doc) = exported(FIXTURE);
    f(&mut doc);
    reimport(&doc.to_string(), &[text], FIXTURE, &vocab()).unwrap()
}

fn context<'a>(doc: &'a mut Value, name: &str) -> &'a mut Value {
    &mut doc["modifiers"]["mode"]["contexts"][name][0]
}

const RED: &str = r##"{"colorSpace": "srgb", "components": [1, 0, 0], "hex": "#ff0000"}"##;

#[test]
fn an_axis_under_scheme_dark_keeps_its_light_side_on_the_line_without_it() {
    let red: Value = serde_json::from_str(RED).unwrap();
    // The light side, edited in both tuples that show it: the default line,
    // and the `dim` line that must keep the same light side.
    let import = edited_import(|doc| {
        for c in ["dark-style=black, light", "dark-style=dim, light"] {
            context(doc, c)["accent"]["soft"]["$value"] = red.clone();
        }
    });
    assert!(import.unmapped.is_empty(), "{:?}", import.unmapped);
    let lines: Vec<(Vec<(String, String)>, String)> = import
        .ops
        .iter()
        .map(|o| match o {
            Op::SetToken { when, value, .. } => (when.clone(), value.clone()),
            o => panic!("{o:?}"),
        })
        .collect();
    assert_eq!(
        lines,
        vec![
            (vec![], "light-dark(#ff0000, #1a5c8a)".to_string()),
            (
                vec![("dark-style".into(), "dim".into())],
                "light-dark(#ff0000, #2b7bb5)".into()
            ),
        ]
    );
    exact_tamber::apply_all(FIXTURE, &import.ops, &vocab()).unwrap();
    // A token whose dim dark follows the default takes the default's line only.
    let import = edited_import(|doc| {
        for c in ["dark-style=black, light", "dark-style=dim, light"] {
            context(doc, c)["accent"]["fill"]["$value"] = red.clone();
        }
    });
    assert_eq!(import.ops.len(), 1, "{:?}", import.ops);
    // Only the dim tuple's light side: no value of the design, reported.
    let import = edited_import(|doc| {
        context(doc, "dark-style=dim, light")["accent"]["soft"]["$value"] = red.clone()
    });
    assert!(import.ops.is_empty());
    assert!(
        import.unmapped[0].why.contains("varies the dark side only"),
        "{:?}",
        import.unmapped
    );
    // A role's value cannot vary by such an axis: the checker's refusal, reported.
    let import = edited_import(|doc| {
        context(doc, "dark-style=dim, dark")["text"]["label"]["$value"] = red.clone()
    });
    assert!(import.ops.is_empty());
    assert!(
        import.unmapped[0].why.contains("tamber-under-role"),
        "{:?}",
        import.unmapped
    );
}

#[test]
fn a_copy_a_tool_rewrote_still_reads_against_its_baseline() {
    // `$extensions` stripped everywhere, `hex` dropped: unchanged, nothing.
    fn strip(v: &mut Value) {
        if let Some(o) = v.as_object_mut() {
            o.remove("$extensions");
            o.remove("hex");
            for (_, x) in o.iter_mut() {
                strip(x);
            }
        } else if let Some(a) = v.as_array_mut() {
            a.iter_mut().for_each(strip);
        }
    }
    let import = edited_import(|doc| {
        for (_, c) in doc["modifiers"]["mode"]["contexts"]
            .as_object_mut()
            .unwrap()
        {
            strip(c);
        }
        doc["sets"]["exact"]["sources"][0] = serde_json::json!({});
    });
    assert!(import.ops.is_empty(), "{:?}", import.ops);
    assert!(import.unmapped.is_empty(), "{:?}", import.unmapped);
    // A value with no `hex`, and an alpha no byte holds exactly, still read.
    let import = edited_import(|doc| {
        context(doc, "dark-style=black, dark")["accent"]["fill"]["$value"] =
            serde_json::json!({"colorSpace": "srgb", "components": [1, 0, 0], "alpha": 0.5});
    });
    let Op::SetToken { value, .. } = &import.ops[0] else {
        panic!()
    };
    assert_eq!(value, "light-dark(#1d9bf0, color(srgb 1 0 0 / 0.5))");
    // A token in one context only, and one the file dropped: each reported.
    let import = edited_import(|doc| {
        context(doc, "dark-style=dim, dark")["extra"] = serde_json::json!({"$type": "color", "$value": {"colorSpace": "srgb", "components": [0, 0, 0]}});
        for (_, c) in doc["modifiers"]["mode"]["contexts"]
            .as_object_mut()
            .unwrap()
        {
            c[0]["border"].as_object_mut().unwrap().remove("hairline");
        }
    });
    let mut why: Vec<(&str, &str)> = import
        .unmapped
        .iter()
        .map(|u| (u.name.as_str(), u.why.as_str()))
        .collect();
    why.sort();
    assert_eq!(why.len(), 2, "{why:?}");
    assert!(why[0].0 == "border.hairline" && why[0].1.contains("removed"));
    assert!(why[1].0 == "extra" && why[1].1.contains("new token"));
    assert_eq!(
        import.counted.source,
        import.counted.mapped + import.counted.reported
    );
}

#[test]
fn the_baseline_is_the_export_the_file_came_from() {
    let (first, _) = exported(FIXTURE);
    let changed = FIXTURE.replace("accent.fill = #1d9bf0", "accent.fill = #00ff00");
    let (second, _) = exported(&changed);
    // The first export, unedited, against the changed design: nothing, by
    // its own baseline, kept beside the second's.
    let import = reimport(&first, &[second.clone(), first.clone()], &changed, &vocab()).unwrap();
    assert!(import.ops.is_empty(), "{:?}", import.ops);
    // Without that baseline it is refused, never compared with another.
    let refused = reimport(&first, &[second], &changed, &vocab()).unwrap_err();
    assert!(
        refused[0].message.contains("no baseline for this export"),
        "{refused:?}"
    );
}

#[test]
fn a_relative_colour_exports_as_it_renders() {
    let src = include_str!("../corpus/relative.tamber");
    let (_, doc) = exported(src);
    let first = doc["modifiers"]["mode"]["default"]
        .as_str()
        .unwrap()
        .to_string();
    let mut doc = doc;
    let t = context(&mut doc, &first)["negative"]["chroma"]["$value"].clone();
    assert_eq!(t["hex"], "#808080", "{t}");
}
