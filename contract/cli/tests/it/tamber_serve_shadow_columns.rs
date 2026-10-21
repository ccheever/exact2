//! Per-column shadow values on the wire (LLP 1121 stage 12, `tamber/1.16`,
//! theme2's 20:50 ruling): a shadow token's value over unset base names
//! gains D1's `columns` (`{value}`, the whole shadow's text as that column's
//! host reads it) on `explain`'s value, each `when` line and each of
//! `resolve`'s tuples, and each such shadow in a tuple gains
//! `columns: {col: {value, lengths}}`. No terminal column for a shadow (the
//! terminal draws none); a length keeps its. A shadow with no length over an
//! unset base has no `columns` key. Every expected value is computed here
//! from the kernel's per-column evaluator, never a copied px.
#![cfg(unix)]

use crate::tamber_serve::{app, Serve};
use exact_kernel::style::metrics::Column;
use exact_kernel::style::scale;
use serde_json::{json, Value};

const DESIGN: &str = "tamber t

axes
  density = roomy | tight

theme
  card.shadow: box-shadow = 0 space.md calc(space.sm * 2) #000
    when density=tight: 0 1px 2px #000
  lift: box-shadow = 0 1px 2px #000
    when density=tight: inset 0 space.sm 4px #000, 0 1px 2px #000, 0 space.xs space.md #000
  late: box-shadow = 0 space.md 1px #000
    when density=roomy: 0 space.xs 1px #000
  glow: text-shadow = 0 space.xs space.sm #ff0000
  space.lg = 20px
  set.shadow: box-shadow = 0 space.lg 2px #000
  plain.shadow: box-shadow = 0 1px 2px #000
  named: box-shadow = 0 space.md 4px -exact-platform-color(ios named:space.md, #000)
  pad = calc(space.md * 2)
  flip: box-shadow = calc(space.md * -0) calc(min(space.md, 0px) * -1) space.md #000
";

/// The four columns a shadow is answered in, by the wire's key.
fn drawing() -> impl Iterator<Item = (&'static str, Column)> {
    scale::COLUMNS.into_iter().filter(|(k, _)| *k != "terminal")
}

/// An expression's px in `column`, from the kernel's evaluator.
fn px(expr: &str, column: Column) -> f32 {
    scale::evaluate(&format!("calc({expr})"), column).unwrap()
}

/// The same as a number on the wire: the f32's shortest text, read back.
fn n(expr: &str, column: Column) -> f64 {
    px(expr, column).to_string().parse().unwrap()
}

fn token(s: &mut Serve, method: &str, name: &str) -> Value {
    let (v, _) = s.ask(json!({"method": method, "token": name}));
    v["ok"].clone()
}

/// Each drawing column's `{value}`, from `text` given the column, and no
/// terminal.
fn columns(text: impl Fn(Column) -> String) -> Value {
    Value::Object(
        drawing()
            .map(|(k, c)| (k.to_string(), json!({ "value": text(c) })))
            .collect(),
    )
}

#[test]
fn a_shadow_over_base_names_answers_each_column_but_the_terminal() {
    let dir = app("shadow-columns", DESIGN);
    let mut s = Serve::start(&dir, &[]);
    let card = |c: Column| {
        format!(
            "0px {}px {}px #000000ff",
            px("space.md", c),
            px("space.sm * 2", c)
        )
    };
    // The neutral look's and the Mac's differ for space.md: no one px.
    assert_ne!(px("space.md", Column::Neutral), px("space.md", Column::Mac));
    // explain: the default's own; the tight line, px only, none.
    let e = token(&mut s, "explain", "card.shadow");
    assert_eq!(e["columns"], columns(card), "{e}");
    assert!(e["whens"][0].get("columns").is_none(), "{e}");
    // resolve: roomy's tuple the token's and the shadow's columns, tight's none.
    let r = token(&mut s, "resolve", "card.shadow");
    let roomy = &r["tuples"][0];
    assert_eq!(roomy["columns"], columns(card), "{r}");
    let shadow = &roomy["shadows"][0];
    for (key, c) in drawing() {
        let lengths = [0.0, n("space.md", c), n("space.sm * 2", c)];
        assert_eq!(
            shadow["columns"][key],
            json!({"value": card(c), "lengths": lengths}),
            "{key} {r}"
        );
    }
    assert!(shadow["columns"].get("terminal").is_none(), "{r}");
    // (B3's fields beside, unchanged)
    assert_eq!(shadow["lengths"], json!([0.0, null, null]), "{r}");
    let tight = &r["tuples"][1];
    assert!(tight.get("columns").is_none(), "{r}");
    assert!(tight["shadows"][0].get("columns").is_none(), "{r}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A when line over its own bases, and a default the first tuple doesn't
/// choose: each line folded per column from its own value, by the first
/// tuple choosing it; a list's shadow over none has no columns of its own.
#[test]
fn each_line_and_each_shadow_of_a_list_answers_its_own() {
    let dir = app("shadow-columns-lines", DESIGN);
    let mut s = Serve::start(&dir, &[]);
    let line = |c: Column| {
        format!(
            "inset 0px {}px 4px #000000ff, 0px 1px 2px #000000ff, 0px {}px {}px #000000ff",
            px("space.sm", c),
            px("space.xs", c),
            px("space.md", c)
        )
    };
    let e = token(&mut s, "explain", "lift");
    assert!(e.get("columns").is_none(), "{e}");
    assert_eq!(e["whens"][0]["columns"], columns(line), "{e}");
    let r = token(&mut s, "resolve", "lift");
    assert!(r["tuples"][0].get("columns").is_none(), "{r}");
    let tight = &r["tuples"][1];
    assert_eq!(tight["columns"], columns(line), "{r}");
    for (key, c) in drawing() {
        assert_eq!(
            tight["shadows"][0]["columns"][key],
            json!({"value": format!("inset 0px {}px 4px #000000ff", px("space.sm", c)), "lengths": [0.0, n("space.sm", c), 4.0]}),
            "{key} {r}"
        );
        assert!(tight["shadows"][1].get("columns").is_none(), "{r}");
        assert_eq!(
            tight["shadows"][2]["columns"][key]["lengths"],
            json!([0.0, n("space.xs", c), n("space.md", c)]),
            "{key} {r}"
        );
    }
    // A default the first tuple doesn't choose (roomy's line is tuple 0's):
    // its columns are tight's, read per column, not tuple 0's.
    let late = |base: &'static str| move |c: Column| format!("0px {}px 1px #000000ff", px(base, c));
    let e = token(&mut s, "explain", "late");
    assert_eq!(e["columns"], columns(late("space.md")), "{e}");
    assert_eq!(e["whens"][0]["columns"], columns(late("space.xs")), "{e}");
    let r = token(&mut s, "resolve", "late");
    assert_eq!(r["tuples"][0]["columns"], columns(late("space.xs")), "{r}");
    assert_eq!(r["tuples"][1]["columns"], columns(late("space.md")), "{r}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_text_shadow_and_a_platform_colour_named_for_a_base() {
    let dir = app("shadow-columns-text", DESIGN);
    let mut s = Serve::start(&dir, &[]);
    let glow = |c: Column| {
        format!(
            "0px {}px {}px #ff0000ff",
            px("space.xs", c),
            px("space.sm", c)
        )
    };
    let e = token(&mut s, "explain", "glow");
    assert_eq!(e["columns"], columns(glow), "{e}");
    let r = token(&mut s, "resolve", "glow");
    assert_eq!(r["tuples"][0]["kind"], "text-shadow");
    assert_eq!(r["tuples"][0]["columns"], columns(glow), "{r}");
    assert_eq!(
        r["tuples"][0]["shadows"][0]["columns"]["mac"],
        json!({"value": glow(Column::Mac), "lengths": [0.0, n("space.xs", Column::Mac), n("space.sm", Column::Mac)]}),
        "{r}"
    );
    // The colour's words are kept whole in every column, its `space.md` too.
    let named = |c: Column| {
        format!(
            "0px {}px 4px -exact-platform-color(ios named:space.md, #000000ff)",
            px("space.md", c)
        )
    };
    let r = token(&mut s, "resolve", "named");
    assert_eq!(r["tuples"][0]["columns"], columns(named), "{r}");
    assert_eq!(
        r["tuples"][0]["shadows"][0]["columns"]["ios"]["value"],
        named(Column::Ios),
        "{r}"
    );
    let e = token(&mut s, "explain", "named");
    assert_eq!(e["columns"], columns(named), "{e}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// px only, or over a base the app sets (folded to px): no `columns` key
/// at all, anywhere.
#[test]
fn a_shadow_over_no_unset_base_has_no_columns() {
    let dir = app("shadow-columns-none", DESIGN);
    let mut s = Serve::start(&dir, &[]);
    for name in ["set.shadow", "plain.shadow"] {
        let r = token(&mut s, "resolve", name);
        let tuple = &r["tuples"][0];
        assert!(tuple.get("columns").is_none(), "{name} {r}");
        assert!(tuple["shadows"][0].get("columns").is_none(), "{name} {r}");
        let e = token(&mut s, "explain", name);
        assert!(e.get("columns").is_none(), "{name} {e}");
    }
    let r = token(&mut s, "resolve", "set.shadow");
    assert_eq!(
        r["tuples"][0]["shadows"][0]["lengths"],
        json!([0.0, 20.0, 2.0])
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// theme2's Q1: a length over a base still answers the terminal's column;
/// a shadow never does.
#[test]
fn a_length_keeps_the_terminal_column_and_a_shadow_has_none() {
    let dir = app("shadow-columns-terminal", DESIGN);
    let mut s = Serve::start(&dir, &[]);
    let pad = token(&mut s, "resolve", "pad");
    assert!(
        pad["tuples"][0]["columns"]["terminal"]["value"].is_string(),
        "{pad}"
    );
    let e = token(&mut s, "explain", "pad");
    assert!(e["columns"]["terminal"]["value"].is_string(), "{e}");
    let card = token(&mut s, "resolve", "card.shadow");
    let tuple = &card["tuples"][0];
    assert_eq!(
        tuple["columns"]
            .as_object()
            .map(|o| o.keys().cloned().collect::<Vec<_>>()),
        Some(vec![
            "ios".into(),
            "mac".into(),
            "material".into(),
            "neutral".into()
        ]),
        "{card}"
    );
    assert!(tuple["shadows"][0]["columns"].get("terminal").is_none());
    let e = token(&mut s, "explain", "card.shadow");
    assert!(e["columns"].get("terminal").is_none(), "{e}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A zero is never negative (rt1's D1 carry-over): `calc(space.md * -0)`
/// compiles to a px length of -0, and a length over a base may fold to
/// zero; each is `0px` in every column's text and 0 among its numbers, and
/// B3's own `lengths` write it 0 too (theme2's 18:45, *1.16*).
#[test]
fn a_zero_is_never_negative() {
    let dir = app("shadow-columns-zero", DESIGN);
    let mut s = Serve::start(&dir, &[]);
    let r = token(&mut s, "resolve", "flip");
    let tuple = &r["tuples"][0];
    let lengths = &tuple["shadows"][0]["lengths"];
    assert!(
        !lengths[0].as_f64().unwrap().is_sign_negative(),
        "B3's lengths: {r}"
    );
    for (key, c) in drawing() {
        let text = format!("0px 0px {}px #000000ff", px("space.md", c));
        assert_eq!(tuple["columns"][key]["value"], text, "{key} {r}");
        let one = &tuple["shadows"][0]["columns"][key];
        assert_eq!(one["value"], text, "{key} {r}");
        assert_eq!(
            one["lengths"],
            json!([0.0, 0.0, n("space.md", c)]),
            "{key} {r}"
        );
        for n in one["lengths"].as_array().unwrap() {
            assert!(!n.as_f64().unwrap().is_sign_negative(), "{key} {r}");
        }
    }
    assert!(!tuple["columns"].to_string().contains("-0"), "{r}");
    assert!(
        !tuple["shadows"][0]["columns"].to_string().contains("-0"),
        "{r}"
    );
    let e = token(&mut s, "explain", "flip");
    assert!(!e["columns"].to_string().contains("-0"), "{e}");
    let _ = std::fs::remove_dir_all(&dir);
}
