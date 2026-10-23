//! The checker bound's round 3 (Astra, Grok): the walks beside the fold, each
//! bounded or linear. Kind inference and a DTCG composite count the levels
//! they nest with the names they follow; the importers' role analysis is
//! iterative; icons' names are checked once each; and the wire measures a
//! kind and a value as a design on disk does.

use crate::common::vocab;
use exact_tamber::{apply, check, parse, Op};
use std::time::{Duration, Instant};

/// `f` on a thread with the 2 MiB stack a test or a server's thread has.
fn on_small_stack<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new()
        .stack_size(2 << 20)
        .spawn(f)
        .unwrap()
        .join()
        .unwrap()
}

/// A forward chain of 500 tokens whose kinds aren't written, each nesting
/// the next in 32 `calc()`s: within each value's bound and the names'
/// depth, but its kind walk nests 16,000 levels. Refused where names and
/// functions together pass 512 (Astra's round 3).
#[test]
fn a_kind_walk_counts_the_functions_it_enters() {
    let mut src = String::from("tamber t\n\ntheme\n  ink = #111111\n");
    for k in 0..500 {
        let open = "calc(".repeat(32);
        let close = ")".repeat(32);
        src += &format!("  c{k} = {open}c{} + 1px{close}\n", k + 1);
    }
    src += "  c500 = 1px\n";
    let ids = on_small_stack(move || match check(&src, &vocab()) {
        Ok(_) => Vec::new(),
        Err(d) => d.iter().map(|d| d.id).collect::<Vec<_>>(),
    });
    assert!(ids.contains(&"tamber-reference-depth"), "{ids:?}");
}

/// A DTCG composite counts its levels with the aliases it follows: 256
/// tokens, each nesting an alias to the next in 32 arrays, end in moments
/// on a small stack (Astra's round 3: 8,192 levels before its type was
/// refused).
#[test]
fn a_dtcg_composite_counts_its_levels() {
    use exact_tamber::import::dtcg;
    let mut doc = serde_json::Map::new();
    for k in 0..256 {
        let mut v = serde_json::json!(format!("{{t{}}}", k + 1));
        for _ in 0..32 {
            v = serde_json::json!([v]);
        }
        doc.insert(
            format!("t{k}"),
            serde_json::json!({"$type": "dimension", "$value": v}),
        );
    }
    doc.insert(
        "t256".into(),
        serde_json::json!({"$type": "dimension", "$value": "1px"}),
    );
    let files = vec![(
        "tokens.json".to_string(),
        serde_json::Value::Object(doc).to_string(),
    )];
    let began = Instant::now();
    on_small_stack(move || {
        let _ = dtcg::import(&files, "dt", &parse("tamber app\n").unwrap());
    });
    assert!(
        began.elapsed() < Duration::from_secs(10),
        "{:?}",
        began.elapsed()
    );
}

/// The importers' role analysis follows a design's chain of names without
/// a recursion apiece: a chain of 20,000 tokens a role names (which the
/// checker folds, each kind found once) imports on a small stack in moments
/// (Astra's round 3).
#[test]
fn role_analysis_follows_a_long_chain_without_recursing() {
    use exact_tamber::import::shadcn;
    let n = 20_000;
    let mut src = String::from("tamber t\n\ntheme\n  t0 = #ffffff\n");
    for k in 1..n {
        src += &format!("  t{k} = t{}\n", k - 1);
    }
    src += &format!("  -exact-background = t{}\n", n - 1);
    let began = Instant::now();
    let imported = on_small_stack(move || {
        let design = parse(&src).unwrap();
        shadcn::import(
            ":root { --background: #000000; }",
            "shadcn",
            &design,
            &vocab(),
        )
        .is_ok()
    });
    assert!(imported);
    assert!(
        began.elapsed() < Duration::from_secs(10),
        "{:?}",
        began.elapsed()
    );
}

/// Icons' names are checked against the tokens' and each other once each
/// (Astra's round 3: a scan apiece).
#[test]
fn many_icons_check_in_moments() {
    let n = 60_000;
    let mut src = String::from("tamber t\n\ntheme\n  ink = #111111\n");
    for k in 0..n {
        src += &format!("  t{k} = #111111\n");
    }
    src += "\nicons\n";
    for k in 0..n {
        src += &format!("  icon.i{k} = icon(sf \"house\")\n");
    }
    let began = Instant::now();
    let folded = check(&src, &vocab());
    assert!(
        began.elapsed() < Duration::from_secs(10),
        "{:?}",
        began.elapsed()
    );
    assert_eq!(folded.map(|f| f.icons.len()).map_err(|d| d.len()), Ok(n));
}

/// Astryx's own tokens (`local`) are source values too (Astra's round 3).
#[test]
fn astryx_local_values_meet_the_bound() {
    use exact_tamber::import::astryx;
    let big = format!("calc(1px{})", " + 1px".repeat(800));
    let json = format!(
        "{{\"source\": \"t\", \"core\": \"t\", \"light\": {{}}, \"dark\": {{}}, \"local\": {{\"--astryx-theme-t-color-x\": \"{big}\"}}}}"
    );
    let d = astryx::import(&json, "astryx", &parse("tamber app\n").unwrap(), &vocab()).unwrap_err();
    assert_eq!(d[0].id, "tamber-value-size", "{d:?}");
    assert!(d[0].message.len() < 200, "{}", d[0].message);
}

/// A kind past the bound gets the same words on the wire as on disk, and a
/// value is measured trimmed on both (both reviewers' round 3).
#[test]
fn the_wire_measures_a_kind_and_a_value_as_a_design_does() {
    let src = "tamber t\n\ntheme\n  ink = #111111\n";
    let long = "k".repeat(5000);
    let Err(disk) = check(&format!("{src}  paper: {long} = #ffffff\n"), &vocab()) else {
        panic!("refused")
    };
    let add = |kind: Option<String>, value: String| Op::AddToken {
        name: "paper".into(),
        kind,
        value,
        meta: Vec::new(),
        whens: Vec::new(),
        comment: None,
        before: None,
    };
    let Err(wire) = apply(src, &add(Some(long), "#ffffff".into()), &vocab()) else {
        panic!("refused")
    };
    assert_eq!(
        (disk[0].id, &disk[0].message),
        (wire[0].id, &wire[0].message)
    );
    let padded = format!("{}#ffffff", " ".repeat(4096));
    assert!(check(&format!("{src}  paper = {padded}\n"), &vocab()).is_ok());
    let applied = apply(src, &add(None, padded), &vocab());
    assert!(applied.is_ok(), "{:?}", applied.err());
}

/// Role spellings in quoted families don't each read the role's value
/// again in the importers' role analysis (Astra's round 3 re-check: 81
/// million edges from 40,000 spellings).
#[test]
fn role_spellings_are_read_once_each_in_role_analysis() {
    let spell = |k: usize| {
        let mut s: Vec<char> = "secondary-background".chars().collect();
        for (i, c) in s.iter_mut().enumerate() {
            if k >> (i % 16) & 1 == 1 {
                *c = c.to_ascii_uppercase();
            }
        }
        s.into_iter().collect::<String>()
    };
    let mut lines = Vec::new();
    // The role's value: about 4 KB of words, each a node to follow.
    let words: Vec<String> = (0..700).map(|k| format!("w{k}")).collect();
    lines.push(format!(
        "  -exact-secondary-background = {}",
        words.join(" ")
    ));
    for j in 0..200 {
        let family: Vec<String> = (0..190).map(|k| spell(j * 190 + k)).collect();
        lines.push(format!("  f{j}: family = \"{}\"", family.join(" ")));
    }
    let src = format!("tamber t\n\ntheme\n  ink = #111111\n{}\n", lines.join("\n"));
    let design = parse(&src).unwrap();
    let began = Instant::now();
    let _ = exact_tamber::import::shadcn::import(
        ":root { --secondary: #102030; --card: #ffffff; }",
        "shadcn",
        &design,
        &vocab(),
    );
    assert!(
        began.elapsed() < Duration::from_secs(20),
        "{:?}",
        began.elapsed()
    );
}
