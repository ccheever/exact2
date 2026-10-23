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
    // Five levels in one value (364 atoms, about 3 KB): refused (Astra's
    // round 1: the twelve this had were past the bytes, so never ran).
    let mut v = "base.gap".to_string();
    for _ in 0..5 {
        v = format!("clamp({v},{v},{v})");
    }
    assert!(v.len() <= MAX_VALUE_BYTES, "{}", v.len());
    let (ids, _) = refused(&theme(&[format!("  deep = {v}")]));
    assert!(ids.contains(&"tamber-length-size"), "{ids:?}");
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

/// A chain of px tokens within the depth folds to a number, however wide,
/// and never meets the residual bound.
#[test]
fn a_px_chain_within_the_depth_still_folds() {
    let n = 200;
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

/// A sum of many lengths, each within the bound, stops as it passes it
/// (both reviewers' round 1: the bound was checked only on the result).
#[test]
fn a_wide_sum_stops_as_it_passes_the_bound() {
    let mut lines = Vec::new();
    for k in 0..200 {
        let terms: Vec<String> = (0..60)
            .map(|j| format!("min(base.gap, {}px)", k * 100 + j + 1))
            .collect();
        lines.push(format!("  w{k} = calc({})", terms.join(" + ")));
    }
    let names: Vec<String> = (0..200).map(|k| format!("w{k}")).collect();
    lines.push(format!("  all = calc({})", names.join(" + ")));
    let (ids, took) = refused(&theme(&lines));
    assert!(ids.contains(&"tamber-length-size"), "{ids:?}");
    assert!(took < Duration::from_secs(2), "{took:?}");
}

/// A cycle read twice a level is no 2^n (Astra's round 1): refused as a
/// cycle in moments.
#[test]
fn a_cycle_read_twice_a_level_is_refused_in_moments() {
    let mut lines: Vec<String> = (0..30)
        .map(|k| format!("  t{k} = calc(t{0} + t{0})", k + 1))
        .collect();
    lines.push("  t30 = t0".into());
    let (ids, took) = refused(&theme(&lines));
    assert!(ids.contains(&"tamber-cycle"), "{ids:?}");
    assert!(took < Duration::from_secs(2), "{took:?}");
}

/// A chain of names past 256 is refused where it passes, not followed until
/// the stack runs out (both reviewers' round 1).
#[test]
fn a_long_chain_of_names_is_refused_before_the_stack_runs_out() {
    let n = 5000;
    let mut lines: Vec<String> = (0..n).map(|k| format!("  a{k} = a{}", k + 1)).collect();
    lines.push(format!("  a{n} = 4px"));
    let (ids, took) = refused(&theme(&lines));
    assert!(
        ids.contains(&"tamber-reference-depth"),
        "{:?}",
        &ids[..ids.len().min(4)]
    );
    assert!(took < Duration::from_secs(5), "{took:?}");
    // 500 deep folds, on a test thread's 2 MB stack: the bound is short of
    // the stack, and past the importers' 256 links.
    let ok: Vec<String> = (0..500)
        .map(|k| format!("  b{k} = b{}", k + 1))
        .chain(["  b500 = 4px".to_string()])
        .collect();
    assert_eq!(refused(&theme(&ok)).0, Vec::<&str>::new());
    assert!(exact_tamber::MAX_REFERENCE_DEPTH > 500);
}

/// A platform colour's fallback is read again, and the reader counts its
/// levels through it: an escaped quote that hides parentheses from the scan
/// recurses no deeper (Astra's round 1).
#[test]
fn a_platform_colours_fallbacks_count_their_levels() {
    let mut v = "red".to_string();
    for _ in 0..60 {
        v = format!("-exact-platform-color(ios \"x\\\"y\", {v})");
    }
    let began = Instant::now();
    let (ids, _) = refused(&theme(&[format!("  p = {v}")]));
    assert!(!ids.is_empty());
    assert!(began.elapsed() < Duration::from_secs(1));
}

/// A number dial, a range end, an icon and a metadata word past the bound
/// are refused with its id before anything reads them, never echoed (Astra's
/// and Grok's round 1).
#[test]
fn every_kind_of_value_meets_the_bound_first() {
    let big = "0".repeat(MAX_VALUE_BYTES + 1);
    for src in [
        format!("tamber t\n\ndials\n  d: number = {big}\n\ntheme\n  ink = #111111\n"),
        format!("tamber t\n\ndials\n  d: number = 1 in 0..{big}\n\ntheme\n  ink = #111111\n"),
        format!(
            "tamber t\n\ntheme\n  ink = #111111\n\nicons\n  icon.x = icon(sf \"{}\")\n",
            "a".repeat(MAX_VALUE_BYTES + 1)
        ),
        format!(
            "tamber t\n\ntheme\n  ink = #111111 scheme={}\n",
            "a".repeat(MAX_VALUE_BYTES + 1)
        ),
    ] {
        let Err(d) = check(&src, &based()) else {
            panic!("refused: {}", &src[..60])
        };
        assert_eq!(
            d[0].id,
            "tamber-value-size",
            "{:?}",
            d.iter().map(|d| d.id).collect::<Vec<_>>()
        );
        assert!(d.iter().all(|d| d.message.len() < 300), "no value echoed");
    }
}

/// A shadow's pieces read alone keep the depth id (Grok's round 1).
#[test]
fn a_shadow_piece_nested_too_deep_keeps_the_depth_id() {
    let deep = format!(
        "{}1px{}",
        "calc(".repeat(MAX_VALUE_DEPTH + 1),
        ")".repeat(MAX_VALUE_DEPTH + 1)
    );
    let src = theme(&[format!(
        "  s: box-shadow = 0px 0px 2px black, 0px {deep} 0px black"
    )]);
    let (ids, _) = refused(&src);
    assert!(ids.contains(&"tamber-value-depth"), "{ids:?}");
}

/// The importers: two sides each within the bound are no refusal though
/// their joined report is past it; a long name doesn't hide a long value;
/// a Pencil theme's label isn't a value; a reimport refuses as an import
/// does (both reviewers' round 1).
#[test]
fn importers_check_source_values_not_reports() {
    use exact_tamber::import::{astryx, dtcg, shadcn};
    use exact_tamber::parse;
    let empty = parse("tamber app\n").unwrap();
    let side = |v: &str| format!("{{\"--spacing-x\": \"{v}\"}}");
    let (a, b) = (
        format!("calc(1px{})", " + 1px".repeat(340)),
        format!("calc(2px{})", " + 2px".repeat(340)),
    );
    assert!(a.len() < MAX_VALUE_BYTES && a.len() + b.len() > MAX_VALUE_BYTES);
    let json = format!(
        "{{\"source\": \"t\", \"core\": \"t\", \"light\": {}, \"dark\": {}}}",
        side(&a),
        side(&b)
    );
    assert!(astryx::import(&json, "astryx", &empty, &vocab()).is_ok());
    let css = format!(
        ":root {{ --{}: calc(1px{}); }}",
        "n".repeat(300),
        " + 1px".repeat(800)
    );
    let d = shadcn::import(&css, "shadcn", &empty, &vocab()).unwrap_err();
    assert_eq!(d[0].id, "tamber-value-size", "{d:?}");
    let pen = format!("{{\"version\": \"2.21\", \"variables\": {{\"c\": {{\"type\": \"color\", \"value\": [{{\"value\": \"#000000\", \"theme\": {{\"mode\": \"{}\"}}}}]}}}}}}", "m".repeat(5000));
    assert!(
        dtcg::pen::import_with("t.pen", &pen, "pen", &empty, &vocab())
            .map(|_| ())
            .map_err(|d| d[0].id)
            != Err("tamber-value-size")
    );
    let design = "tamber t\n\ntheme\n  ink = #111111\n";
    let folded = check(design, &vocab()).unwrap();
    let export = exact_tamber::export::dtcg(&folded, &vocab()).unwrap();
    let edited = export.replacen("#111111", &format!("calc(1px{})", " + 1px".repeat(800)), 1);
    assert_ne!(edited, export);
    let d =
        exact_tamber::export::reimport_against(&edited, None, &[], design, &vocab()).unwrap_err();
    assert_eq!(d[0].id, "tamber-value-size", "{d:?}");
}
