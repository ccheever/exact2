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
        import.unmapped[0]
            .why
            .contains("the design would refuse it"),
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

/// A token's value in a context of `doc`, as hex.
fn hex_in(doc: &Value, context: &str, path: &[&str]) -> String {
    let mut at = &doc["modifiers"]["mode"]["contexts"][context][0];
    for p in path {
        at = &at[*p];
    }
    at["$value"]["hex"].as_str().unwrap().to_string()
}

#[test]
fn an_edit_reaches_only_the_contexts_it_changed() {
    let red: Value = serde_json::from_str(RED).unwrap();
    // `accent.fill`'s dim dark follows its default: an edit of the black
    // dark alone pins dim to what it was.
    let import = edited_import(|doc| {
        context(doc, "dark-style=black, dark")["accent"]["fill"]["$value"] = red.clone()
    });
    assert!(import.unmapped.is_empty(), "{:?}", import.unmapped);
    let accepted = exact_tamber::apply_all(FIXTURE, &import.ops, &vocab())
        .unwrap()
        .text;
    let (_, doc) = exported(&accepted);
    assert_eq!(
        hex_in(&doc, "dark-style=black, dark", &["accent", "fill"]),
        "#ff0000"
    );
    assert_eq!(
        hex_in(&doc, "dark-style=dim, dark", &["accent", "fill"]),
        "#1d9bf0"
    );
    assert_eq!(
        hex_in(&doc, "dark-style=dim, light", &["accent", "fill"]),
        "#1d9bf0"
    );
    // Two light sides an axis under scheme=dark tells apart: no lines give
    // both, reported.
    let blue = serde_json::json!({"colorSpace": "srgb", "components": [0, 0, 1], "hex": "#0000ff"});
    let import = edited_import(|doc| {
        context(doc, "dark-style=black, light")["accent"]["soft"]["$value"] = red.clone();
        context(doc, "dark-style=dim, light")["accent"]["soft"]["$value"] = blue.clone();
    });
    assert!(import.ops.is_empty(), "{:?}", import.ops);
    assert_eq!(import.unmapped.len(), 1, "{:?}", import.unmapped);
    // A later source in a context wins, as the resolver says.
    let import = edited_import(|doc| {
        let srcs = doc["modifiers"]["mode"]["contexts"]["dark-style=black, dark"]
            .as_array_mut()
            .unwrap();
        srcs.push(
            serde_json::json!({"accent": {"fill": {"$type": "color", "$value": red.clone()}}}),
        );
    });
    assert_eq!(import.ops.len(), 1, "{:?}", import.ops);
}

#[test]
fn a_stripped_copy_reads_against_the_export_it_came_from() {
    let (first, mut doc) = exported(FIXTURE);
    let changed = FIXTURE.replace("accent.fill = #1d9bf0", "accent.fill = #00ff00");
    doc["sets"]["exact"]["sources"][0] = serde_json::json!({});
    // The design as it was: its own export is the one baseline, and an
    // unedited copy proposes nothing.
    let import = reimport(
        &doc.to_string(),
        std::slice::from_ref(&first),
        FIXTURE,
        &vocab(),
    )
    .unwrap();
    assert!(import.ops.is_empty(), "{:?}", import.ops);
    // The design changed since: the first export kept, and the design's own,
    // could each be where the copy came from; its values cannot tell an edit
    // from the other export's, so it is refused.
    let refused = reimport(&doc.to_string(), &[first], &changed, &vocab()).unwrap_err();
    assert!(
        refused[0].message.contains("more than one export"),
        "{refused:?}"
    );
    // A renamed modifier is no export of ours, and no panic.
    doc["modifiers"] = serde_json::json!({"other": doc["modifiers"]["mode"].clone()});
    assert!(!exact_tamber::export::is_own(&doc.to_string()));
}

#[test]
fn a_legacy_colour_exports_its_bytes() {
    let src = include_str!("../corpus/relative.tamber");
    let (_, mut doc) = exported(src);
    let first = doc["modifiers"]["mode"]["default"]
        .as_str()
        .unwrap()
        .to_string();
    let v = context(&mut doc, &first)["absolute"]["calc"]["$value"].clone();
    assert_eq!(v["hex"], "#800000", "{v}");
    assert_eq!(v["components"][0].as_f64().unwrap(), 128.0 / 255.0);
}

#[test]
fn round_three_edges() {
    let red: Value = serde_json::from_str(RED).unwrap();
    // The design changed since the export (dim's dark of `accent.fill`), and
    // the file changed only black's dark: dim keeps the design's value.
    let (text, mut doc) = exported(FIXTURE);
    let newer = FIXTURE.replace(
        "  accent.fill = #1d9bf0\n",
        "  accent.fill = #1d9bf0\n    when dark-style=dim: light-dark(#1d9bf0, #00ff00)\n",
    );
    context(&mut doc, "dark-style=black, dark")["accent"]["fill"]["$value"] = red.clone();
    let import = reimport(
        &doc.to_string(),
        std::slice::from_ref(&text),
        &newer,
        &vocab(),
    )
    .unwrap();
    let accepted = exact_tamber::apply_all(&newer, &import.ops, &vocab())
        .unwrap()
        .text;
    let (_, after) = exported(&accepted);
    assert_eq!(
        hex_in(&after, "dark-style=black, dark", &["accent", "fill"]),
        "#ff0000"
    );
    assert_eq!(
        hex_in(&after, "dark-style=dim, dark", &["accent", "fill"]),
        "#00ff00"
    );
    // A source by reference, later in the context, wins.
    let import = edited_import(|doc| {
        doc["sets"]["red"] = serde_json::json!({"sources": [{"accent": {"fill": {"$type": "color", "$value": red.clone()}}}]});
        doc["modifiers"]["mode"]["contexts"]["dark-style=black, dark"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({"$ref": "#/sets/red"}));
    });
    assert!(!import.ops.is_empty(), "{:?}", import);
    // A context the export did not have is reported.
    let import = edited_import(|doc| {
        let c = doc["modifiers"]["mode"]["contexts"]["dark-style=black, dark"].clone();
        doc["modifiers"]["mode"]["contexts"]["dark-style=grey, dark"] = c;
    });
    assert!(
        import
            .unmapped
            .iter()
            .any(|u| u.name == "dark-style=grey, dark"),
        "{:?}",
        import.unmapped
    );
    assert_eq!(
        import.counted.source,
        import.counted.mapped + import.counted.reported
    );
    // A reprint of an 8-bit channel to six places changes nothing.
    let import = edited_import(|doc| {
        let v = &mut context(doc, "dark-style=black, dark")["accent"]["fill"]["$value"];
        let c: Vec<f64> = v["components"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| (x.as_f64().unwrap() * 1e6).round() / 1e6)
            .collect();
        v["components"] = serde_json::json!(c);
    });
    assert!(import.ops.is_empty(), "{:?}", import.ops);
}

#[test]
fn a_legacy_alpha_exports_its_byte() {
    let src = "tamber t\n\ntheme\n  veil = rgb(10 20 30 / 50%)\n";
    let (_, doc) = exported(src);
    let v = &doc["modifiers"]["mode"]["contexts"]["light"][0]["veil"]["$value"];
    assert_eq!(v["alpha"].as_f64().unwrap(), 128.0 / 255.0, "{v}");
}

#[test]
fn an_unchanged_lossy_side_keeps_the_design_value() {
    let src = "tamber t\n\ntheme\n  glow = light-dark(#ffffff, color(srgb-linear 2 2 2))\n";
    let (text, mut doc) = exported(src);
    assert!(
        doc["modifiers"]["mode"]["contexts"]["dark"][0]["glow"]["$extensions"]["exact"]["lossy"]
            .is_string()
    );
    doc["modifiers"]["mode"]["contexts"]["light"][0]["glow"]["$value"] =
        serde_json::from_str(RED).unwrap();
    let import = reimport(&doc.to_string(), &[text], src, &vocab()).unwrap();
    let Op::SetToken { value, .. } = &import.ops[0] else {
        panic!("{import:?}")
    };
    assert_eq!(value, "light-dark(#ff0000, color(srgb-linear 2 2 2))");
}

/// Each operation as its `when` and value.
fn lines(import: &exact_tamber::import::Import) -> Vec<(Vec<(String, String)>, String)> {
    import
        .ops
        .iter()
        .map(|o| match o {
            Op::SetToken { when, value, .. } => (when.clone(), value.clone()),
            o => panic!("{o:?}"),
        })
        .collect()
}

#[test]
fn a_change_edits_the_line_that_wins_for_its_tuple() {
    let red: Value = serde_json::from_str(RED).unwrap();
    let dim = vec![("dark-style".to_string(), "dim".to_string())];
    // `accent.soft`'s dim dark comes from its `when dark-style=dim` line: the
    // edit lands there, its light side kept as written.
    let import = edited_import(|doc| {
        context(doc, "dark-style=dim, dark")["accent"]["soft"]["$value"] = red.clone()
    });
    assert_eq!(
        lines(&import),
        vec![(dim.clone(), "light-dark(#8ecdf8, #ff0000)".to_string())]
    );
    // Every context of `accent.fill`'s one line, to one value: that line.
    let import = edited_import(|doc| {
        for (_, c) in doc["modifiers"]["mode"]["contexts"]
            .as_object_mut()
            .unwrap()
        {
            c[0]["accent"]["fill"]["$value"] = red.clone();
        }
    });
    assert_eq!(lines(&import), vec![(vec![], "#ff0000".to_string())]);
    // Its dark side in every tuple: the default line, its light side kept.
    let import = edited_import(|doc| {
        for c in ["dark-style=black, dark", "dark-style=dim, dark"] {
            context(doc, c)["accent"]["fill"]["$value"] = red.clone();
        }
    });
    assert_eq!(
        lines(&import),
        vec![(vec![], "light-dark(#1d9bf0, #ff0000)".to_string())]
    );
    // One tuple of a line others share: the narrowest `when` for it alone.
    let import = edited_import(|doc| {
        context(doc, "dark-style=dim, dark")["accent"]["fill"]["$value"] = red.clone()
    });
    assert_eq!(
        lines(&import),
        vec![(dim, "light-dark(#1d9bf0, #ff0000)".to_string())]
    );
}

#[test]
fn a_change_on_an_app_axis_lands_on_its_when_or_the_narrowest_new_one() {
    let src = "tamber t\n\naxes\n  theme = calm | loud\n  density = cozy | tight\n\ntheme\n  ink = #000000\n    when theme=loud: #ff0000\n";
    let (text, doc) = exported(src);
    let blue = serde_json::json!({"colorSpace": "srgb", "components": [0, 0, 1], "hex": "#0000ff"});
    let edit = |contexts: &[&str]| {
        let mut doc = doc.clone();
        for c in contexts {
            doc["modifiers"]["mode"]["contexts"][*c][0]["ink"]["$value"] = blue.clone();
        }
        reimport(&doc.to_string(), std::slice::from_ref(&text), src, &vocab()).unwrap()
    };
    let loud = vec![("theme".to_string(), "loud".to_string())];
    // Every context `when theme=loud` wins: that line.
    let all_loud = [
        "theme=loud, density=cozy, light",
        "theme=loud, density=cozy, dark",
        "theme=loud, density=tight, light",
        "theme=loud, density=tight, dark",
    ];
    assert_eq!(lines(&edit(&all_loud)), vec![(loud, "#0000ff".to_string())]);
    // One of its tuples: a new `when` that tells it from the other.
    let import = edit(&all_loud[2..]);
    assert_eq!(
        lines(&import),
        vec![(
            vec![
                ("theme".into(), "loud".into()),
                ("density".into(), "tight".into())
            ],
            "#0000ff".to_string()
        )]
    );
    exact_tamber::apply_all(src, &import.ops, &vocab()).unwrap();
    // Every tight tuple, across both lines: `density=tight` alone separates them.
    let tight = [
        "theme=calm, density=tight, light",
        "theme=calm, density=tight, dark",
        "theme=loud, density=tight, light",
        "theme=loud, density=tight, dark",
    ];
    let import = edit(&tight);
    assert!(import.unmapped.is_empty(), "{:?}", import.unmapped);
    // `density=tight` alone would tie with `theme=loud` on loud/tight: two
    // lines that each tell their tuple apart.
    assert_eq!(
        lines(&import),
        vec![
            (
                vec![
                    ("theme".into(), "calm".into()),
                    ("density".into(), "tight".into())
                ],
                "#0000ff".to_string()
            ),
            (
                vec![
                    ("theme".into(), "loud".into()),
                    ("density".into(), "tight".into())
                ],
                "#0000ff".to_string()
            ),
        ]
    );
    let accepted = exact_tamber::apply_all(src, &import.ops, &vocab())
        .unwrap()
        .text;
    let (_, after) = exported(&accepted);
    for c in tight {
        assert_eq!(
            after["modifiers"]["mode"]["contexts"][c][0]["ink"]["$value"]["hex"], "#0000ff",
            "{c}"
        );
    }
    assert_eq!(
        after["modifiers"]["mode"]["contexts"]["theme=loud, density=cozy, dark"][0]["ink"]
            ["$value"]["hex"],
        "#ff0000"
    );
}

#[test]
fn round_four_edges() {
    let red: Value = serde_json::from_str(RED).unwrap();
    // A dark-only edit of `accent.soft`'s default: one op; its dim line's
    // light is the default's light still, so it is left alone.
    let import = edited_import(|doc| {
        context(doc, "dark-style=black, dark")["accent"]["soft"]["$value"] = red.clone()
    });
    assert_eq!(
        lines(&import),
        vec![(vec![], "light-dark(#8ecdf8, #ff0000)".to_string())]
    );
    // Two new lines never tie: three tuples of a four-tuple design changed.
    let src = "tamber t\n\naxes\n  theme = calm | loud\n  density = cozy | tight\n\ntheme\n  ink = #000000\n";
    let (text, mut doc) = exported(src);
    for c in [
        "theme=calm, density=cozy",
        "theme=calm, density=tight",
        "theme=loud, density=cozy",
    ] {
        for side in ["light", "dark"] {
            doc["modifiers"]["mode"]["contexts"][format!("{c}, {side}")][0]["ink"]["$value"] =
                red.clone();
        }
    }
    let import = reimport(&doc.to_string(), std::slice::from_ref(&text), src, &vocab()).unwrap();
    assert!(import.unmapped.is_empty(), "{:?}", import.unmapped);
    let accepted = exact_tamber::apply_all(src, &import.ops, &vocab())
        .unwrap()
        .text;
    let (_, after) = exported(&accepted);
    assert_eq!(
        after["modifiers"]["mode"]["contexts"]["theme=loud, density=tight, dark"][0]["ink"]
            ["$value"]["hex"],
        "#000000"
    );
    // A new line keeps the other side as the winning line writes it.
    let src = "tamber t\n\naxes\n  theme = calm | loud\n\ntheme\n  ink = light-dark(color-mix(in oklch, red 50%, blue), #111111)\n";
    let (text, mut doc) = exported(src);
    doc["modifiers"]["mode"]["contexts"]["theme=loud, dark"][0]["ink"]["$value"] = red.clone();
    let import = reimport(&doc.to_string(), std::slice::from_ref(&text), src, &vocab()).unwrap();
    assert_eq!(
        lines(&import),
        vec![(
            vec![("theme".into(), "loud".into())],
            "light-dark(color-mix(in oklch, red 50%, blue), #ff0000)".to_string()
        )]
    );
    // A shadowed `when` under scheme=dark follows its base line's new light too.
    let src = "tamber t\n\naxes\n  theme = calm | loud\n  dark-style = black | dim  under scheme=dark\n\ntheme\n  ink = light-dark(#111111, #222222)\n    when dark-style=dim: light-dark(#111111, #555555)\n    when theme=calm dark-style=dim: light-dark(#111111, #666666)\n    when theme=loud dark-style=dim: light-dark(#111111, #777777)\n";
    let (text, mut doc) = exported(src);
    for (c, _) in doc["modifiers"]["mode"]["contexts"]
        .clone()
        .as_object()
        .unwrap()
    {
        if c.ends_with("light") {
            doc["modifiers"]["mode"]["contexts"][c][0]["ink"]["$value"] = red.clone();
        }
    }
    let import = reimport(&doc.to_string(), std::slice::from_ref(&text), src, &vocab()).unwrap();
    assert!(import.unmapped.is_empty(), "{:?}", import.unmapped);
    exact_tamber::apply_all(src, &import.ops, &vocab()).unwrap();
}

/// Stage 6 G: every kind is exported as its DTCG type (a length as a
/// dimension, a text style as px at the default root size marked lossy, a
/// type as typography with what it leaves unsaid named), re-imported
/// unedited it proposes nothing, and a changed length or type proposes one
/// line for the tuples it changed.
#[test]
fn every_kind_exports_as_its_type_and_round_trips() {
    const KINDS: &str = "tamber kinds\n\naxes\n  density = regular | compact\n\ntheme\n  ink = #111111\n  space.row = 12px\n    when density=compact: 8px\n  size.title = -exact-title1\n  size.rem = 1.25rem\n  opacity.faded: number = 0.5\n  family.text = \"Inter\", system-ui\n  type.body = -exact-body/1.35 \"Inter\"\n  type.bold = italic 700 18px/22px Georgia, serif\n  shadow.card: box-shadow = 0 1px 2px #00000033\n  glow: text-shadow = 0 0 2px currentcolor\n  duration.fast = 150ms\n  easing.out = ease-out\n  easing.curve = cubic-bezier(0.2, 0, 0, 1)\n";
    let (text, doc) = exported(KINDS);
    let light = tokens(&doc, "density=regular, light");
    let get = |name: &str| {
        light
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, t)| t.clone())
            .unwrap()
    };
    assert_eq!(get("space.row")["$type"], "dimension");
    assert_eq!(
        get("space.row")["$value"],
        serde_json::json!({"value": 12.0, "unit": "px"})
    );
    assert_eq!(get("size.rem")["$value"]["unit"], "rem");
    assert_eq!(get("size.title")["$value"]["unit"], "px");
    assert_eq!(
        get("size.title")["$extensions"]["exact"]["lossy"],
        "-exact-title1"
    );
    assert_eq!(get("opacity.faded")["$type"], "number");
    assert_eq!(
        get("family.text")["$value"],
        serde_json::json!(["Inter", "system-ui"])
    );
    let body = get("type.body");
    assert_eq!(body["$type"], "typography");
    assert_eq!(body["$value"]["lineHeight"], 1.35);
    assert_eq!(
        body["$extensions"]["exact"]["unsaid"],
        serde_json::json!(["fontWeight"])
    );
    assert_eq!(get("shadow.card")["$type"], "shadow");
    assert_eq!(get("glow")["$extensions"]["exact"]["kind"], "text-shadow");
    assert_eq!(
        get("duration.fast")["$value"],
        serde_json::json!({"value": 150.0, "unit": "ms"})
    );
    assert_eq!(
        get("easing.out")["$value"],
        serde_json::json!([0.0, 0.0, 0.58, 1.0])
    );
    let import = reimport(&text, std::slice::from_ref(&text), KINDS, &vocab()).unwrap();
    assert!(import.ops.is_empty(), "{:?}", import.ops);
    assert!(import.unmapped.is_empty(), "{:?}", import.unmapped);
    // A length changed in the compact tuple only, both its sides.
    let mut doc = doc;
    for c in ["density=compact, light", "density=compact, dark"] {
        let tree = &mut doc["modifiers"]["mode"]["contexts"][c][0];
        tree["space"]["row"]["$value"] = serde_json::json!({"value": 6, "unit": "px"});
        tree["type"]["body"]["$value"]["lineHeight"] = serde_json::json!(1.5);
    }
    let names: Vec<String> = doc["modifiers"]["mode"]["contexts"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    assert!(
        names.iter().any(|n| n == "density=compact, light"),
        "{names:?}"
    );
    let import = reimport(
        &doc.to_string(),
        std::slice::from_ref(&text),
        KINDS,
        &vocab(),
    )
    .unwrap();
    assert!(import.unmapped.is_empty(), "{:?}", import.unmapped);
    let set: Vec<(String, Vec<(String, String)>, String)> = import
        .ops
        .iter()
        .filter_map(|op| match op {
            Op::SetToken {
                name, when, value, ..
            } => Some((name.clone(), when.clone(), value.clone())),
            _ => None,
        })
        .collect();
    let compact = vec![("density".to_string(), "compact".to_string())];
    assert!(
        set.contains(&("space.row".into(), compact.clone(), "6px".into())),
        "{set:?}"
    );
    // The style kept, its size unchanged; a family's identifiers bare.
    assert!(
        set.contains(&("type.body".into(), compact, "-exact-body/1.5 Inter".into())),
        "{set:?}"
    );
}

/// Review round 1 (stage 6 G): a tool that reprints whole numbers changes
/// nothing; a weight the export left unsaid, once edited, is said; a shadow
/// whose colours differ by scheme takes a new blur; a duration in seconds
/// is read as the design writes it; an accepted edit proposes nothing more.
#[test]
fn reprints_unsaid_parts_scheme_shadows_and_units_read_back() {
    const KINDS: &str = "tamber kinds\n\ntheme\n  size.title = -exact-title1\n  type.body = -exact-body/1.35 Inter\n  shadow.card: box-shadow = 0 1px 2px light-dark(#00000033, #ffffff22)\n  duration.fast = 150ms\n  easing.out = ease-out\n";
    let (text, doc) = exported(KINDS);
    // Whole floats as integers, as `JSON.stringify` writes them.
    fn ints(v: &mut Value) {
        match v {
            Value::Number(n)
                if n.as_f64()
                    .is_some_and(|f| f.fract() == 0.0 && f.abs() < 1e15) =>
            {
                *v = serde_json::json!(n.as_f64().unwrap() as i64)
            }
            Value::Array(a) => a.iter_mut().for_each(ints),
            Value::Object(o) => o.values_mut().for_each(ints),
            _ => {}
        }
    }
    let mut reprinted = doc.clone();
    ints(&mut reprinted);
    let import = reimport(
        &reprinted.to_string(),
        std::slice::from_ref(&text),
        KINDS,
        &vocab(),
    )
    .unwrap();
    assert!(
        import.ops.is_empty() && import.unmapped.is_empty(),
        "{:?} {:?}",
        import.ops,
        import.unmapped
    );
    let mut edited = doc.clone();
    for c in ["light", "dark"] {
        let tree = &mut edited["modifiers"]["mode"]["contexts"][c][0];
        tree["type"]["body"]["$value"]["fontWeight"] = serde_json::json!(600);
        tree["shadow"]["card"]["$value"]["blur"] = serde_json::json!({"value": 6, "unit": "px"});
        tree["duration"]["fast"]["$value"] = serde_json::json!({"value": 0.2, "unit": "s"});
    }
    let import = reimport(
        &edited.to_string(),
        std::slice::from_ref(&text),
        KINDS,
        &vocab(),
    )
    .unwrap();
    assert!(import.unmapped.is_empty(), "{:?}", import.unmapped);
    let values: Vec<(String, String)> = import
        .ops
        .iter()
        .filter_map(|op| match op {
            Op::SetToken { name, value, .. } => Some((name.clone(), value.clone())),
            _ => None,
        })
        .collect();
    assert!(
        values.contains(&("type.body".into(), "600 -exact-body/1.35 Inter".into())),
        "{values:?}"
    );
    assert!(
        values.contains(&("duration.fast".into(), "200ms".into())),
        "{values:?}"
    );
    assert!(
        values
            .iter()
            .any(|(n, v)| n == "shadow.card" && v.contains("6px") && v.contains("light-dark(")),
        "{values:?}"
    );
    let accepted = exact_tamber::apply_all(KINDS, &import.ops, &vocab())
        .unwrap()
        .text;
    let again = reimport(
        &edited.to_string(),
        std::slice::from_ref(&text),
        &accepted,
        &vocab(),
    )
    .unwrap();
    assert!(again.ops.is_empty(), "{:?}", again.ops);
}
