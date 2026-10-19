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
