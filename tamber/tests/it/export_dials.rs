//! Re-import's dials (LLP 1121 D14; §5 stage 10b's gate): a changed value
//! whose line is one dial alone is that dial's change, every token it moves
//! listed; one whose line followed others by an expression is a marked pin.

use crate::common::vocab;
use exact_tamber::export::{dtcg, reimport};
use exact_tamber::{check, Op};
use serde_json::{json, Value};

const DESIGN: &str = "tamber t

axes
  dark-style = black | dim  under scheme=dark

dials
  accent: color = #1083fe
  lift: number = 0.1  in 0..0.5

theme
  surface.page = light-dark(#ffffff, #000000)
  accent.fill = accent
  accent.soft = oklch(from accent calc(l + lift) c h)
  surface.raised = oklch(from #3366cc calc(l + lift) c h)
";

/// The design's export, `f` applied to every context's tree, re-imported.
fn edited(f: impl Fn(&str, &mut Value)) -> exact_tamber::import::Import {
    let text = dtcg(&check(DESIGN, &vocab()).unwrap(), &vocab()).unwrap();
    let mut doc: Value = serde_json::from_str(&text).unwrap();
    for (name, c) in doc["modifiers"]["mode"]["contexts"]
        .as_object_mut()
        .unwrap()
    {
        f(name, &mut c[0]);
    }
    reimport(&doc.to_string(), &[text], DESIGN, &vocab()).unwrap()
}

fn red() -> Value {
    json!({"colorSpace": "srgb", "components": [1.0, 0.0, 0.0], "hex": "#ff0000"})
}

#[test]
fn a_changed_accent_is_the_dials_change_with_every_token_it_moves() {
    let import = edited(|_, tree| tree["accent"]["fill"]["$value"] = red());
    assert_eq!(
        import.ops,
        vec![Op::SetDial {
            name: "accent".into(),
            value: "#ff0000".into()
        }],
        "{:?}",
        import.unmapped
    );
    assert_eq!(import.dials.len(), 1);
    let change = &import.dials[0];
    assert_eq!(
        (
            change.dial.as_str(),
            change.value.as_str(),
            change.from.clone()
        ),
        ("accent", "#ff0000", vec!["accent.fill".to_string()])
    );
    // What accepting it moves beyond the variable: the soft accent too, in every tuple.
    for token in ["accent.fill", "accent.soft"] {
        for tuple in ["dark-style=black", "dark-style=dim"] {
            assert!(
                change
                    .impact
                    .iter()
                    .any(|i| i.token == token && i.tuple == tuple && i.before != i.after),
                "{token} {tuple}: {:?}",
                change.impact
            );
        }
    }
    assert!(change.impact.iter().all(|i| i.token != "surface.page"));
    assert!(import.pins.is_empty(), "{:?}", import.pins);
    // Accepted, the same file proposes nothing more.
    let accepted = exact_tamber::apply_all(DESIGN, &import.ops, &vocab())
        .unwrap()
        .text;
    assert!(accepted.contains("accent: color = #ff0000"), "{accepted}");
}

#[test]
fn a_changed_folded_value_is_a_pinned_override_marked_with_what_it_followed() {
    let import = edited(|_, tree| tree["accent"]["soft"]["$value"] = red());
    assert!(import.dials.is_empty());
    assert!(
        matches!(import.ops.as_slice(), [Op::SetToken { name, when, value, .. }] if name == "accent.soft" && when.is_empty() && value == "#ff0000"),
        "{:?} {:?}",
        import.ops,
        import.unmapped
    );
    assert_eq!(import.pins.len(), 1, "{:?}", import.pins);
    assert_eq!(
        (
            import.pins[0].token.as_str(),
            import.pins[0].follows.clone()
        ),
        (
            "accent.soft",
            vec!["accent".to_string(), "lift".to_string()]
        )
    );
}

#[test]
fn a_dial_line_changed_in_one_scheme_only_is_a_pin_not_the_dial() {
    let import = edited(|name, tree| {
        if name.ends_with("dark") {
            tree["accent"]["fill"]["$value"] = red()
        }
    });
    assert!(import.dials.is_empty(), "{:?}", import.dials);
    assert!(
        import
            .ops
            .iter()
            .all(|op| !matches!(op, Op::SetDial { .. })),
        "{:?}",
        import.ops
    );
    assert!(
        import
            .pins
            .iter()
            .any(|p| p.token == "accent.fill" && p.follows == vec!["accent".to_string()]),
        "{:?}",
        import.pins
    );
}

#[test]
fn a_literal_token_changed_is_neither() {
    let import = edited(|_, tree| tree["surface"]["page"]["$value"] = red());
    assert!(
        import.dials.is_empty() && import.pins.is_empty(),
        "{:?} {:?}",
        import.dials,
        import.pins
    );
    assert_eq!(import.ops.len(), 1);
}

/// `design`'s export, `f` applied to every context's tree, re-imported.
fn edited_in(
    design: &str,
    f: impl Fn(&str, &mut Value),
) -> (String, String, exact_tamber::import::Import) {
    let text = dtcg(&check(design, &vocab()).unwrap(), &vocab()).unwrap();
    let mut doc: Value = serde_json::from_str(&text).unwrap();
    for (name, c) in doc["modifiers"]["mode"]["contexts"]
        .as_object_mut()
        .unwrap()
    {
        f(name, &mut c[0]);
    }
    let file = doc.to_string();
    let import = reimport(&file, std::slice::from_ref(&text), design, &vocab()).unwrap();
    (text, file, import)
}

#[test]
fn a_dial_with_one_token_is_its_dial_change_not_an_inert_dial() {
    let design = "tamber t\n\ndials\n  accent: color = #1083fe\n\ntheme\n  accent.fill = accent\n";
    let (text, file, import) = edited_in(design, |_, t| t["accent"]["fill"]["$value"] = red());
    assert_eq!(
        import.ops,
        vec![Op::SetDial {
            name: "accent".into(),
            value: "#ff0000".into()
        }],
        "{:?}",
        import.unmapped
    );
    // Accepted, the same file proposes nothing more: the dial has the value.
    let accepted = exact_tamber::apply_all(design, &import.ops, &vocab())
        .unwrap()
        .text;
    let again = reimport(&file, &[text], &accepted, &vocab()).unwrap();
    assert!(
        again.ops.is_empty() && again.dials.is_empty() && again.unmapped.is_empty(),
        "{again:?}"
    );
}

#[test]
fn a_when_line_that_is_the_dial_moves_the_dial() {
    let design = "tamber t\n\naxes\n  brand = plain | loud\n\ndials\n  accent: color = #1083fe\n\ntheme\n  accent.fill = #000000\n    when brand=loud: accent\n  accent.soft = oklch(from accent calc(l + 0.1) c h)\n";
    let (_, _, import) = edited_in(design, |name, t| {
        if name.contains("brand=loud") {
            t["accent"]["fill"]["$value"] = red()
        }
    });
    assert_eq!(
        import.ops,
        vec![Op::SetDial {
            name: "accent".into(),
            value: "#ff0000".into()
        }],
        "{:?} {:?}",
        import.unmapped,
        import.pins
    );
    assert!(
        import.dials[0]
            .impact
            .iter()
            .any(|i| i.token == "accent.soft" && i.tuple == "brand=plain"),
        "{:?}",
        import.dials[0].impact
    );
}

#[test]
fn two_tokens_on_one_dial_agree_or_are_pinned() {
    let design = "tamber t\n\ndials\n  accent: color = #1083fe\n\ntheme\n  accent.fill = accent\n  accent.line = accent\n  accent.ring = accent\n";
    let (_, _, agree) = edited_in(design, |_, t| {
        t["accent"]["fill"]["$value"] = red();
        t["accent"]["line"]["$value"] = red();
    });
    assert_eq!(agree.ops.len(), 1);
    assert_eq!(
        agree.dials[0].from,
        vec!["accent.fill".to_string(), "accent.line".to_string()]
    );
    let (_, _, apart) = edited_in(design, |_, t| {
        t["accent"]["fill"]["$value"] = red();
        t["accent"]["line"]["$value"] =
            json!({"colorSpace": "srgb", "components": [0.0, 0.0, 1.0], "hex": "#0000ff"});
    });
    // Apart, the dial moves not: each is its literal, a pin marked with the dial.
    assert!(apart.dials.is_empty(), "{:?}", apart.dials);
    assert_eq!(apart.ops.len(), 2, "{:?} {:?}", apart.ops, apart.unmapped);
    assert_eq!(
        apart
            .pins
            .iter()
            .map(|p| (p.token.as_str(), p.follows.clone()))
            .collect::<Vec<_>>(),
        vec![
            ("accent.fill", vec!["accent".to_string()]),
            ("accent.line", vec!["accent".to_string()])
        ]
    );
    // The same colour spelled two ways agrees.
    let (_, _, alike) = edited_in(design, |_, t| {
        t["accent"]["fill"]["$value"] = red();
        t["accent"]["line"]["$value"] =
            json!({"colorSpace": "srgb", "components": [1.0, 0.0, 0.0], "hex": "#FF0000"});
    });
    assert_eq!(alike.dials.len(), 1, "{:?}", alike.unmapped);
}

#[test]
fn a_baseline_whose_contexts_are_mapped_otherwise_is_refused() {
    use exact_tamber::export::reimport_against;
    let text = dtcg(&check(DESIGN, &vocab()).unwrap(), &vocab()).unwrap();
    // The digest, the axes and the names kept; two contexts' tuples swapped.
    let mut forged: Value = serde_json::from_str(&text).unwrap();
    let contexts = forged["sets"]["exact"]["sources"][0]["$extensions"]["exact"]["contexts"]
        .as_object_mut()
        .unwrap();
    let names: Vec<String> = contexts.keys().cloned().collect();
    let (a, b) = (contexts[&names[0]].clone(), contexts[&names[1]].clone());
    assert_ne!(a, b);
    contexts.insert(names[0].clone(), b);
    contexts.insert(names[1].clone(), a);
    let forged = forged.to_string();
    let refused = reimport_against(&forged, Some(&forged), &[], DESIGN, &vocab()).unwrap_err();
    assert!(refused[0].message.starts_with("pull first"), "{refused:?}");
}

#[test]
fn a_new_when_is_marked_with_what_each_side_followed_where_it_holds() {
    let design = "tamber t\n\naxes\n  brand = plain | loud\n  dark-style = black | dim  under scheme=dark\n\ndials\n  day: color = #fafafa\n  night: color = #101010\n\ntheme\n  ink = day\n    when dark-style=dim: light-dark(day, night)\n";
    let (_, _, import) = edited_in(design, |name, t| {
        if name.starts_with("brand=loud") && name.ends_with("dark") {
            t["ink"]["$value"] = red();
        }
    });
    // Each tuple's line: the dark side followed `day` on black, `night` on
    // dim; the light side's `day` is kept on both.
    assert_eq!(
        import
            .pins
            .iter()
            .map(|p| (p.when.clone(), p.follows.clone()))
            .collect::<Vec<_>>(),
        vec![
            (
                vec![
                    ("brand".into(), "loud".into()),
                    ("dark-style".into(), "black".into())
                ],
                vec!["day".to_string()]
            ),
            (
                vec![
                    ("brand".into(), "loud".into()),
                    ("dark-style".into(), "dim".into())
                ],
                vec!["night".to_string()]
            ),
        ],
        "{:?}",
        import.ops
    );
}

#[test]
fn an_explicit_baseline_is_the_only_one_and_must_have_the_designs_axes() {
    use exact_tamber::export::reimport_against;
    let text = dtcg(&check(DESIGN, &vocab()).unwrap(), &vocab()).unwrap();
    let mut doc: Value = serde_json::from_str(&text).unwrap();
    for (_, c) in doc["modifiers"]["mode"]["contexts"]
        .as_object_mut()
        .unwrap()
    {
        c[0]["accent"]["soft"]["$value"] = red();
    }
    let ok = reimport_against(&doc.to_string(), Some(&text), &[], DESIGN, &vocab()).unwrap();
    assert_eq!(ok.pins.len(), 1);
    // A baseline whose axes are not the design's: refused, pull first.
    let other = DESIGN.replace(
        "dark-style = black | dim",
        "dark-style = black | dim | grey",
    );
    let theirs = dtcg(&check(&other, &vocab()).unwrap(), &vocab()).unwrap();
    let refused = reimport_against(&theirs, Some(&theirs), &[], DESIGN, &vocab()).unwrap_err();
    assert!(refused[0].message.starts_with("pull first"), "{refused:?}");
}

#[test]
fn round_3_a_baseline_whose_references_multiply_is_refused_quickly() {
    use exact_tamber::export::reimport_against;
    let text = dtcg(&check(DESIGN, &vocab()).unwrap(), &vocab()).unwrap();
    // The real header and contexts; a context's sources a tree of repeated
    // references, eight to each next level, ten levels deep.
    let mut forged: Value = serde_json::from_str(&text).unwrap();
    for level in 0..10 {
        let next = if level == 9 {
            json!([{"x": {"$type": "number", "$value": 1}}])
        } else {
            json!(vec![json!({"$ref": format!("#/sets/l{}", level + 1)}); 8])
        };
        forged["sets"][format!("l{level}")] = json!({ "sources": next });
    }
    let first = forged["modifiers"]["mode"]["contexts"]
        .as_object()
        .unwrap()
        .keys()
        .next()
        .unwrap()
        .clone();
    forged["modifiers"]["mode"]["contexts"][&first] = json!([{"$ref": "#/sets/l0"}]);
    let forged = forged.to_string();
    let started = std::time::Instant::now();
    let refused = reimport_against(&text, Some(&forged), &[], DESIGN, &vocab()).unwrap_err();
    assert!(started.elapsed().as_secs() < 5);
    assert!(
        refused.iter().any(|d| d.message.contains("expand")),
        "{refused:?}"
    );
}

#[test]
fn round_3_a_colour_with_and_without_its_hex_agrees() {
    let design = "tamber t\n\ndials\n  accent: color = #1083fe\n\ntheme\n  accent.fill = accent\n  accent.line = accent\n  accent.ring = accent\n";
    let (_, _, import) = edited_in(design, |_, t| {
        t["accent"]["fill"]["$value"] = red();
        t["accent"]["line"]["$value"] =
            json!({"colorSpace": "srgb", "components": [1.0, 0.0, 0.0]});
    });
    assert_eq!(
        import.dials.len(),
        1,
        "{:?} {:?}",
        import.ops,
        import.unmapped
    );
}

#[test]
fn round_3_a_bare_role_name_is_followed_and_a_default_edit_keeps_its_branches() {
    let design = "tamber t\n\ntheme\n  background = #000000\n  ink = background\n";
    let (_, _, import) = edited_in(design, |_, t| t["ink"]["$value"] = red());
    assert_eq!(
        import.pins.len(),
        1,
        "{:?} {:?}",
        import.ops,
        import.unmapped
    );
    assert!(
        import.pins[0]
            .follows
            .iter()
            .any(|f| f.ends_with("background")),
        "{:?}",
        import.pins
    );
    let design = "tamber t\n\naxes\n  brand = plain | loud\n\ndials\n  accent: color = #1083fe\n\ntheme\n  ink = #000000\n    when brand=loud: accent\n  accent.ring = accent\n";
    let (_, _, import) = edited_in(design, |name, t| {
        if name.starts_with("brand=plain") {
            t["ink"]["$value"] = red();
        }
    });
    assert!(!import.ops.is_empty(), "{:?}", import.unmapped);
    assert!(import.pins.is_empty(), "{:?} {:?}", import.ops, import.pins);
}

#[test]
fn round_4_a_baseline_whose_references_copy_a_large_tree_is_refused_before_copying() {
    use exact_tamber::export::reimport_against;
    let text = dtcg(&check(DESIGN, &vocab()).unwrap(), &vocab()).unwrap();
    let mut forged: Value = serde_json::from_str(&text).unwrap();
    // Six levels of eight references to one token carrying 4000 bytes (under the checker's
    // 4096-byte bound on a string, so the budget is what refuses): 1 GiB copied, unbudgeted.
    for level in 0..6 {
        let next = if level == 5 {
            json!([{"x": {"$type": "number", "$value": 1, "$description": "d".repeat(4000)}}])
        } else {
            json!(vec![json!({"$ref": format!("#/sets/l{}", level + 1)}); 8])
        };
        forged["sets"][format!("l{level}")] = json!({ "sources": next });
    }
    let first = forged["modifiers"]["mode"]["contexts"]
        .as_object()
        .unwrap()
        .keys()
        .next()
        .unwrap()
        .clone();
    forged["modifiers"]["mode"]["contexts"][&first] = json!([{"$ref": "#/sets/l0"}]);
    let started = std::time::Instant::now();
    let refused =
        reimport_against(&text, Some(&forged.to_string()), &[], DESIGN, &vocab()).unwrap_err();
    assert!(started.elapsed().as_secs() < 10);
    assert!(
        refused.iter().any(|d| d.message.contains("expands past")),
        "{refused:?}"
    );
    // A string past the bound is refused before any reference is followed.
    let mut long = forged.clone();
    long["sets"]["l5"]["sources"][0]["x"]["$description"] = json!("d".repeat(256 << 10));
    let refused =
        reimport_against(&text, Some(&long.to_string()), &[], DESIGN, &vocab()).unwrap_err();
    assert_eq!(refused[0].id, "tamber-value-size", "{refused:?}");
}

/// Judged whole (option B), a dial's change is known before its literal
/// fallback could stand in the proposal: `a`'s change is the dial's, and
/// `b`, which reads the dial too, takes its own literal beside it.
#[test]
fn a_dials_change_and_another_tokens_literal_land_together() {
    let design = "tamber t\n\ndials\n  accent: color = #000000\n\ntheme\n  a = accent\n  b = light-dark(accent, #ffffff)\n";
    let grey =
        |hex: &str, c: f64| json!({"colorSpace": "srgb", "components": [c, c, c], "hex": hex});
    let (_, _, import) = edited_in(design, |_, t| {
        t["a"]["$value"] = grey("#111111", 17.0 / 255.0);
        t["b"]["$value"] = grey("#222222", 34.0 / 255.0);
    });
    assert!(import.unmapped.is_empty(), "{:?}", import.unmapped);
    assert!(
        import.ops.contains(&Op::SetDial {
            name: "accent".into(),
            value: "#111111".into()
        }),
        "{:?}",
        import.ops
    );
    assert!(
        import.ops.iter().any(
            |o| matches!(o, Op::SetToken { name, value, .. } if name == "b" && value == "#222222")
        ),
        "{:?}",
        import.ops
    );
}

/// Judged whole, a token settled in one round is asked again when a later
/// round drops another's lines: `a`'s change cannot be made (its dark side
/// follows `b`), and `c`, which mixes `a` and `b`, was settled on a folding
/// with `a`'s change in it. Asked again without it, `c` cannot show the file's
/// colours (its pins keep its light side's expression), so it is reported, not
/// imported on a folding that will not ship.
#[test]
fn a_token_settled_beside_a_dropped_change_is_asked_again() {
    let design = "tamber t\n\ntheme\n  a = light-dark(#000000, b)\n  b = #ffffff\n  c = light-dark(color-mix(in srgb, a 50%, b), #000000)\n";
    let hex = |h: &str, c: f64| json!({"colorSpace": "srgb", "components": [c, c, c], "hex": h});
    let (_, _, import) = edited_in(design, |context, t| {
        if context.contains("light") {
            t["a"]["$value"] = hex("#ffffff", 1.0);
        }
        t["b"]["$value"] = hex("#000000", 0.0);
        if context.contains("dark") {
            t["c"]["$value"] = hex("#111111", 17.0 / 255.0);
        }
    });
    let reported: Vec<&str> = import.unmapped.iter().map(|u| u.name.as_str()).collect();
    assert_eq!(reported, ["a", "c"], "{:?}", import.unmapped);
    assert_eq!(
        import.ops,
        [Op::SetToken {
            name: "b".into(),
            when: Vec::new(),
            value: "#000000".into(),
            at: None,
            comment: None
        }]
    );
}
