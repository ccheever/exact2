//! The checker bound's sweep (the lead's, after round 3): every pass over a
//! design, an import or a re-import that was more than linear in its size,
//! each now linear, charged to an import's budget, or capped. One hostile
//! input each, which must finish in moments (the table is in the commit
//! that made them so).

use crate::common::vocab;
use exact_tamber::{check, parse, Op, Vocab};
use std::time::{Duration, Instant};

/// Run `f`, and fail past `secs` (each case took minutes or more before).
fn within<T>(secs: u64, what: &str, f: impl FnOnce() -> T) -> T {
    let began = Instant::now();
    let out = f();
    let took = began.elapsed();
    assert!(took < Duration::from_secs(secs), "{what}: {took:?}");
    out
}

fn theme(lines: impl IntoIterator<Item = String>) -> String {
    let mut src = String::from("tamber t\n\ntheme\n  ink = #111111\n");
    for l in lines {
        src += &l;
        src.push('\n');
    }
    src
}

/// A write that changes every token's value (each names the one written)
/// lists them by name once: the plan's changed tokens and its motion test.
#[test]
fn a_write_changing_every_token_plans_in_moments() {
    let n = 40_000;
    let src = theme((0..n).map(|k| format!("  t{k} = ink")));
    let files = vec![("app.tamber".to_string(), src)];
    let op = (
        "app.tamber".to_string(),
        Op::SetToken {
            name: "ink".into(),
            when: Vec::new(),
            value: "#222222".into(),
            at: None,
            comment: None,
        },
    );
    let planned = within(20, "plan", || {
        exact_tamber::plan(&files, &[op], &vocab(), false, Default::default())
    });
    assert_eq!(planned.map(|p| p.changed.len()).ok(), Some(n + 1));
}

/// A base name read by every token is looked up once, not against the
/// tokens each time.
#[test]
fn a_base_name_read_everywhere_checks_in_moments() {
    let vocab = Vocab {
        bases: vec!["base.gap".to_string()],
        ..vocab()
    };
    let src = theme((0..40_000).map(|k| format!("  t{k} = base.gap")));
    within(20, "check", || check(&src, &vocab).map(|_| ()).ok());
}

/// Axes of one value each, however many, leave each tuple's digits made
/// once.
#[test]
fn many_one_value_axes_check_in_moments() {
    let mut src = String::from("tamber t\n\naxes\n");
    for a in 0..10_000 {
        src += &format!("  a{a} = v\n");
    }
    src += "\ntheme\n  ink = #111111\n";
    for k in 0..10_000 {
        src += &format!("  t{k} = #222222\n");
    }
    within(20, "check", || check(&src, &vocab()).map(|_| ()).ok());
}

/// Names no token has, beside many icons, are looked up in the icons'
/// names once each.
#[test]
fn unknown_names_beside_many_icons_check_in_moments() {
    let n = 30_000;
    let mut src = theme((0..n).map(|k| format!("  t{k} = u{k}")));
    src += "\nicons\n";
    for k in 0..n {
        src += &format!("  icon.i{k} = icon(sf \"house\")\n");
    }
    within(20, "check", || check(&src, &vocab()).map(|_| ()).err());
}

/// A long `pair=` list is told apart by a set.
#[test]
fn long_pair_lists_check_in_moments() {
    let names: Vec<String> = (0..600).map(|k| format!("p{k}")).collect();
    let list = names.join(",");
    let mut lines: Vec<String> = names.iter().map(|n| format!("  {n} = #ffffff")).collect();
    for k in 0..250 {
        lines.push(format!("  on{k} = #000000  pair={list}"));
    }
    within(20, "check", || {
        check(&theme(lines), &vocab()).map(|_| ()).ok()
    });
}

/// Many dials, each moving one token, are found moving in one pass.
#[test]
fn many_dials_are_found_moving_in_moments() {
    let n = 20_000;
    let mut src = String::from("tamber t\n\ndials\n");
    for k in 0..n {
        src += &format!("  d{k}: color = #102030\n");
    }
    src += "\ntheme\n  ink = #111111\n";
    for k in 0..n {
        src += &format!("  t{k} = d{k}\n");
    }
    within(30, "check", || check(&src, &vocab()).map(|_| ()).ok());
}

/// History's edits and a revert over large designs read each by name once,
/// and a removed token's place is found in one pass.
#[test]
fn history_over_large_designs_is_linear() {
    let n = 30_000;
    let old = parse(&theme((0..n).map(|k| format!("  t{k} = #111111")))).unwrap();
    let new = parse(&theme((0..n).map(|k| format!("  s{k} = #222222")))).unwrap();
    let edits = within(20, "edits", || {
        exact_tamber::history::edits(Some(&old), Some(&new))
    });
    assert!(edits.is_ok_and(|e| e.len() >= 2 * n));
    let revert = within(20, "revert", || {
        exact_tamber::history::revert(&old, &new, &new)
    });
    assert!(revert.conflicts.is_empty(), "{:?}", &revert.conflicts[..1]);
    assert!(revert.ops.len() >= 2 * n);
}

/// A `when` naming many axes is compared with another as a sorted list,
/// not each condition against every other.
#[test]
fn long_whens_compare_in_moments() {
    let a = 3_000;
    let mut src = String::from("tamber t\n\naxes\n");
    for x in 0..a {
        src += &format!("  a{x} = v | w\n");
    }
    let conds = |last: &str| {
        (0..a)
            .map(|x| format!("a{x}={}", if x + 1 == a { last } else { "v" }))
            .collect::<Vec<_>>()
            .join(" ")
    };
    src += &format!(
        "\ntheme\n  ink = #111111\n    when {}: #222222\n",
        conds("v")
    );
    let design = parse(&src).unwrap();
    let when: Vec<(String, String)> = (0..a)
        .rev()
        .map(|x| {
            (
                format!("a{x}"),
                if x + 1 == a { "w" } else { "v" }.to_string(),
            )
        })
        .collect();
    let op = Op::SetToken {
        name: "ink".into(),
        when,
        value: "#333333".into(),
        at: None,
        comment: None,
    };
    within(20, "history", || {
        exact_tamber::history::edits(Some(&design), Some(&design)).map(|e| e.len())
    });
    within(20, "set-token", || {
        exact_tamber::apply(&src, &op, &vocab()).map(|_| ()).err()
    });
}

/// The CSS importers' work is charged to an import's budget, as DTCG's is:
/// many tokens land, or the import is refused with the budget's words.
#[test]
fn a_large_tailwind_import_lands_or_is_refused_in_moments() {
    let mut css = String::from("@theme {\n");
    for k in 0..30_000 {
        css += &format!("  --color-c{k}: var(--color-c{});\n", k + 1);
    }
    css += "  --color-c30000: #102030;\n}\n";
    within(30, "tailwind", || {
        exact_tamber::import::tailwind::import(&css, "tw", &parse("tamber app\n").unwrap())
            .map(|i| i.ops.len())
            .map_err(|d| d[0].id)
    });
}

/// Astryx's other variables are grouped by token once.
#[test]
fn many_astryx_variables_group_in_moments() {
    let side: Vec<String> = (0..30_000)
        .map(|k| format!("\"--spacing-s{k}\": \"{}px\"", k % 40))
        .collect();
    let side = format!("{{{}}}", side.join(","));
    let json =
        format!("{{\"source\": \"t\", \"core\": \"t\", \"light\": {side}, \"dark\": {side}}}");
    within(30, "astryx", || {
        exact_tamber::import::astryx::import(
            &json,
            "astryx",
            &parse("tamber app\n").unwrap(),
            &vocab(),
        )
        .map(|i| i.ops.len())
        .map_err(|d| d[0].id)
    });
}

/// Many shadcn themes are counted against the tuples by a set, and refused.
#[test]
fn many_shadcn_themes_are_refused_in_moments() {
    let mut css = String::from(":root { --background: #ffffff; }\n");
    for k in 0..20_000 {
        css += &format!(".theme-t{k} {{ --background: #000000; }}\n");
    }
    let d = within(20, "shadcn", || {
        exact_tamber::import::shadcn::import(
            &css,
            "shadcn",
            &parse("tamber app\n").unwrap(),
            &vocab(),
        )
        .map(|_| ())
        .unwrap_err()
    });
    assert!(!d.is_empty());
}
