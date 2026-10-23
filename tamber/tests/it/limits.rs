//! The design checker's own bounds (the lead's 13:30Z PRIORITY, rt1's
//! finding; theme2's approval): a value's bytes and nesting
//! (`css::over_limit`), and a length over base names' atoms
//! (`residual::MAX_RESIDUAL_ATOMS`). Each value past one is refused with its
//! own id, quickly, whatever its shape; every value the corpus writes reads.

use crate::common::vocab;
use exact_tamber::{check, Vocab, MAX_RESIDUAL_ATOMS, MAX_VALUE_BYTES, MAX_VALUE_DEPTH};
use std::time::{Duration, Instant};

/// The vocabulary with one base name the design leaves unset, so a length
/// over it stays a residual and nothing folds it.
fn based() -> Vocab {
    Vocab {
        bases: vec!["base.gap".to_string()],
        ..vocab()
    }
}

/// The ids the design is refused with, and how long the check took.
fn refused(src: &str) -> (Vec<&'static str>, Duration) {
    let began = Instant::now();
    let ids = match check(src, &based()) {
        Ok(_) => Vec::new(),
        Err(d) => d.iter().map(|d| d.id).collect(),
    };
    (ids, began.elapsed())
}

fn theme(lines: &[String]) -> String {
    format!("tamber t\n\ntheme\n  ink = #111111\n{}\n", lines.join("\n"))
}

/// A chain of tokens each naming the next three times in a `clamp()`, over
/// a base name: its residual tripled a level, 3^n atoms on main (rt1's
/// finding). Refused at the level that passes the bound, in milliseconds.
#[test]
fn a_clamp_chain_is_refused_where_it_passes_the_bound() {
    let n = 30;
    let mut lines: Vec<String> = (0..n)
        .map(|k| format!("  t{k} = clamp(t{0}, t{0}, t{0})", k + 1))
        .collect();
    lines.push(format!("  t{n} = calc(base.gap + 1px)"));
    let (ids, took) = refused(&theme(&lines));
    assert!(ids.contains(&"tamber-length-size"), "{ids:?}");
    assert!(took < Duration::from_secs(2), "{took:?}");
    // Four levels (3^4 = 81 atoms over clamps) stay under it.
    let mut short: Vec<String> = (0..4)
        .map(|k| format!("  t{k} = clamp(t{0}, t{0}, t{0})", k + 1))
        .collect();
    short.push("  t4 = calc(base.gap + 1px)".into());
    assert_eq!(refused(&theme(&short)).0, Vec::<&str>::new());
}

/// A nest of `clamp()`s inside one value multiplies too: bounded where each
/// function is made.
#[test]
fn a_nest_in_one_value_is_bounded_where_each_function_is_made() {
    let mut v = "base.gap".to_string();
    for _ in 0..12 {
        v = format!("clamp({v}, {v}, {v})");
    }
    if v.len() <= MAX_VALUE_BYTES {
        let (ids, _) = refused(&theme(&[format!("  deep = {v}")]));
        assert!(ids.contains(&"tamber-length-size"), "{ids:?}");
    }
    // As a chain of shorter values it is the length bound that says so.
    let mut lines = vec!["  a0 = calc(base.gap + 1px)".to_string()];
    for k in 1..8 {
        lines.push(format!(
            "  a{k} = clamp(clamp(a{0}, a{0}, a{0}), a{0}, a{0})",
            k - 1
        ));
    }
    let (ids, took) = refused(&theme(&lines));
    assert!(ids.contains(&"tamber-length-size"), "{ids:?}");
    assert!(took < Duration::from_secs(2), "{took:?}");
}

/// A value nested 100,000 deep is refused before the reader recurses (on
/// main it overflowed the stack); 32 levels read, 33 don't.
#[test]
fn deep_nesting_is_refused_before_the_reader_recurses() {
    let deep = format!("{}1px{}", "calc(".repeat(100_000), ")".repeat(100_000));
    let (ids, took) = refused(&theme(&[format!("  deep = {deep}")]));
    assert!(
        ids.iter()
            .any(|id| matches!(*id, "tamber-value-size" | "tamber-value-depth")),
        "{ids:?}"
    );
    assert!(took < Duration::from_secs(2), "{took:?}");
    let nest = |d: usize| format!("{}1px{}", "calc(".repeat(d), ")".repeat(d));
    assert_eq!(
        refused(&theme(&[format!("  ok = {}", nest(MAX_VALUE_DEPTH))])).0,
        Vec::<&str>::new()
    );
    assert_eq!(
        refused(&theme(&[format!("  no = {}", nest(MAX_VALUE_DEPTH + 1))])).0,
        vec!["tamber-value-depth"]
    );
}

/// A value past 4096 bytes is refused, its words giving the size and never
/// the value; one of 4096 reads. Parentheses inside a quoted string are no
/// nesting.
#[test]
fn a_value_past_its_bytes_is_refused_without_echoing_it() {
    let sum = |bytes: usize| {
        let mut v = "calc(1px".to_string();
        while v.len() + 6 + 1 <= bytes {
            v.push_str(" + 1px");
        }
        while v.len() + 1 < bytes {
            v.push(' ');
        }
        v.push(')');
        v
    };
    let at = sum(MAX_VALUE_BYTES);
    assert_eq!(at.len(), MAX_VALUE_BYTES);
    assert_eq!(
        refused(&theme(&[format!("  ok = {at}")])).0,
        Vec::<&str>::new()
    );
    let over = format!("calc(1px{})", " + 1px".repeat(700));
    assert!(over.len() > MAX_VALUE_BYTES);
    let src = theme(&[format!("  no = {over}")]);
    let Err(d) = check(&src, &based()) else {
        panic!("refused")
    };
    assert_eq!(d[0].id, "tamber-value-size");
    assert!(!d[0].message.contains("+ 1px"), "{}", d[0].message);
    assert!(
        d[0].message.contains(&over.len().to_string()),
        "{}",
        d[0].message
    );
    let quoted = format!("\"{}\"", "(".repeat(40));
    assert!(!refused(&theme(&[format!("  family.odd = {quoted}")]))
        .0
        .contains(&"tamber-value-depth"));
}

/// A chain of px tokens, however long, folds to a number and never meets
/// the bound.
#[test]
fn a_px_chain_of_any_length_still_folds() {
    let n = 300;
    let mut lines: Vec<String> = (0..n)
        .map(|k| format!("  p{k} = clamp(p{0}, p{0}, p{0})", k + 1))
        .collect();
    lines.push(format!("  p{n} = 4px"));
    assert_eq!(refused(&theme(&lines)).0, Vec::<&str>::new());
    assert!(MAX_RESIDUAL_ATOMS >= 64);
}

/// Every op that carries a value checks it first (`edit::valid`, rt1's
/// co-sign): a value past the bound is refused with the bound's id, in words
/// that give its size and never echo it, as is a name past it.
#[test]
fn an_edit_past_the_bound_is_refused_before_it_is_read() {
    use exact_tamber::{apply, Op};
    let src = "tamber t\n\ntheme\n  ink = #111111\n";
    let big = format!("calc(1px{})", " + 1px".repeat(200_000));
    let op = Op::SetToken {
        name: "ink".into(),
        when: Vec::new(),
        value: big.clone(),
        at: None,
        comment: None,
    };
    let began = Instant::now();
    let Err(d) = apply(src, &op, &vocab()) else {
        panic!("refused")
    };
    assert_eq!(d[0].id, "tamber-value-size");
    assert!(d[0].message.len() < 200, "{}", d[0].message);
    assert!(began.elapsed() < Duration::from_secs(1));
    let named = Op::SetToken {
        name: "x".repeat(MAX_VALUE_BYTES + 1),
        when: Vec::new(),
        value: "#000".into(),
        at: None,
        comment: None,
    };
    let Err(d) = apply(src, &named, &vocab()) else {
        panic!("refused")
    };
    assert!(d[0].message.len() < 200, "{}", d[0].message);
}

/// Each importer refuses a source value past the bound with that bound's id,
/// whether it would have mapped it or reported it (theme2's condition;
/// `import::bounded`), naming the source's name and never echoing the value.
#[test]
fn each_importer_refuses_a_value_past_the_bound_with_its_id() {
    use exact_tamber::import::{astryx, dtcg, shadcn, tailwind};
    use exact_tamber::parse;
    let empty = parse("tamber app\n").unwrap();
    let big = format!("calc(1px{})", " + 1px".repeat(800));
    assert!(big.len() > MAX_VALUE_BYTES);
    let refused = |r: Result<exact_tamber::import::Import, Vec<exact_tamber::Diagnostic>>,
                   kind: &str| {
        let Err(d) = r else {
            panic!("{kind}: imported")
        };
        assert_eq!(d[0].id, "tamber-value-size", "{kind}: {d:?}");
        assert!(!d[0].message.contains("+ 1px"), "{kind}: {}", d[0].message);
    };
    refused(
        tailwind::import(&format!("@theme {{ --spacing-big: {big}; }}"), "tw", &empty),
        "tailwind",
    );
    refused(
        shadcn::import(
            &format!(":root {{ --big: {big}; }}"),
            "shadcn",
            &empty,
            &vocab(),
        ),
        "shadcn",
    );
    let side = format!("{{\"--spacing-big\": \"{big}\"}}");
    refused(
        astryx::import(
            &format!("{{\"source\": \"t\", \"core\": \"t\", \"light\": {side}, \"dark\": {side}}}"),
            "astryx",
            &empty,
            &vocab(),
        ),
        "astryx",
    );
    let files = vec![(
        "tokens.json".to_string(),
        format!("{{\"big\": {{\"$type\": \"dimension\", \"$value\": \"{big}\"}}}}"),
    )];
    refused(dtcg::import(&files, "dt", &empty), "dtcg");
    let pen = format!("{{\"version\": \"2.21\", \"variables\": {{\"big\": {{\"type\": \"color\", \"value\": \"{big}\"}}}}}}");
    refused(
        dtcg::pen::import_with("big.pen", &pen, "pen", &empty, &vocab()),
        "pen",
    );
}
