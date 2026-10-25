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
    let _ = within(20, "history", || {
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
    // (each past the 256 links a chain may have is followed that far: a
    // constant per token, about a quarter of a millisecond in a release
    // build)
    for k in 0..10_000 {
        css += &format!("  --color-c{k}: var(--color-c{});\n", k + 1);
    }
    css += "  --color-c10000: #102030;\n}\n";
    let _ = within(30, "tailwind", || {
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
    let _ = within(30, "astryx", || {
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

/// A design has at most 32 axes and a token at most 64 `when` lines
/// (theme2's 19:26), refused before anything walks them; the importers
/// refuse past them with the same ids.
#[test]
fn axes_and_when_lines_are_capped() {
    use exact_tamber::{MAX_AXES, MAX_WHENS};
    let mut src = String::from("tamber t\n\naxes\n");
    for a in 0..=MAX_AXES {
        src += &format!("  a{a} = v\n");
    }
    src += "\ntheme\n  ink = #111111\n";
    let d = check(&src, &vocab()).unwrap_err();
    assert!(d.iter().any(|d| d.id == "tamber-axes"), "{d:?}");
    let mut src = String::from("tamber t\n\naxes\n  a = v0");
    for v in 1..=MAX_WHENS {
        src += &format!(" | v{v}");
    }
    src += "\n\ntheme\n  ink = #111111\n";
    for v in 0..=MAX_WHENS {
        src += &format!("    when a=v{v}: #22222{}\n", v % 10);
    }
    let d = check(&src, &vocab()).unwrap_err();
    assert!(d.iter().any(|d| d.id == "tamber-whens"), "{d:?}");
    // Pencil's themes are axes: one past the cap is refused with its id.
    let themes: Vec<String> = (0..=MAX_AXES)
        .map(|a| format!("\"t{a}\":[\"x\"]"))
        .collect();
    let pen = format!(
        "{{\"version\":\"2.21\",\"themes\":{{{}}},\"variables\":{{\"v\":{{\"type\":\"color\",\"value\":\"#000000\"}}}}}}",
        themes.join(",")
    );
    let d = exact_tamber::import::dtcg::pen::import_with(
        "x.pen",
        &pen,
        "pen",
        &parse("tamber app\n").unwrap(),
        &vocab(),
    )
    .map(|i| i.ops.len())
    .unwrap_err();
    assert!(d.iter().any(|d| d.id == "tamber-axes"), "{d:?}");
}

/// A transaction of 1024 value writes on a large design is parsed once
/// (theme2's 19:20: a parse per operation made it operations x tokens),
/// and its one restore undoes all of them.
#[test]
fn a_transaction_of_value_writes_parses_the_design_once() {
    let n = 20_000;
    let src = theme((0..n).map(|k| format!("  t{k} = #111111")));
    let ops: Vec<Op> = (0..1024)
        .map(|k| Op::SetToken {
            name: format!("t{}", k * 19),
            when: Vec::new(),
            value: "#222222".into(),
            at: None,
            comment: None,
        })
        .collect();
    let edited = within(10, "1024 set-token", || {
        exact_tamber::apply_all(&src, &ops, &vocab()).unwrap()
    });
    assert_eq!(edited.text.matches("#222222").count(), 1024);
    let undone = exact_tamber::undo(&edited.text, &edited.inverse, &vocab()).unwrap();
    assert_eq!(undone.text, src);
}

/// New tokens (an import's, accepted) are added with one parse too (Astra's
/// round 1: a parse per `add-token` made a large import operations x tokens
/// outside the import's budget).
#[test]
fn a_transaction_of_new_tokens_parses_the_design_once() {
    let n = 20_000;
    let src = theme((0..n).map(|k| format!("  t{k} = #111111")));
    let ops: Vec<Op> = (0..4096)
        .map(|k| Op::AddToken {
            name: format!("new.t{k}"),
            kind: None,
            value: "#222222".into(),
            meta: Vec::new(),
            whens: Vec::new(),
            comment: None,
            before: None,
        })
        .collect();
    let edited = within(10, "4096 add-token", || {
        exact_tamber::apply_all(&src, &ops, &vocab()).unwrap()
    });
    assert_eq!(edited.text.matches("#222222").count(), 4096);
    let undone = exact_tamber::undo(&edited.text, &edited.inverse, &vocab()).unwrap();
    assert_eq!(undone.text, src);
}

/// An import whose axes would pass the design's 32 keeps the cap's id
/// through the landing check (Astra's round 1).
#[test]
fn an_import_past_the_axes_cap_is_refused_with_its_id() {
    let mut src = String::from("tamber t\n\naxes\n");
    for a in 0..exact_tamber::MAX_AXES {
        src += &format!("  a{a} = v\n");
    }
    src += "\ntheme\n  ink = #111111\n";
    let design = parse(&src).unwrap();
    let css = ":root { --background: #ffffff; }\n.theme-blue { --background: #0000ff; }\n";
    let d = exact_tamber::import::shadcn::import(css, "shadcn", &design, &vocab()).unwrap_err();
    assert_eq!(d[0].id, "tamber-axes", "{d:?}");
}

/// An axis of many values is checked for repeats by a set; an import's
/// mixed runs (new tokens, then values) are each parsed once; a run with a
/// bad value at its end is refused after the rest are taken in one parse
/// (Astra's round 2).
#[test]
fn mixed_runs_and_long_axes_apply_in_moments() {
    let n = 20_000;
    let src = theme((0..n).map(|k| format!("  t{k} = #111111")));
    let values: Vec<String> = (0..100_000).map(|k| format!("v{k}")).collect();
    let axis = Op::AddAxis {
        name: "wide".into(),
        values,
        under_dark: false,
    };
    let _ = within(10, "add-axis", || {
        exact_tamber::apply(&src, &axis, &vocab()).map(|_| ())
    });
    let add = |k: usize, value: &str| Op::AddToken {
        name: format!("new.t{k}"),
        kind: None,
        value: value.into(),
        meta: Vec::new(),
        whens: Vec::new(),
        comment: None,
        before: None,
    };
    let set = |k: usize| Op::SetToken {
        name: format!("t{k}"),
        when: Vec::new(),
        value: "#333333".into(),
        at: None,
        comment: None,
    };
    let mut ops: Vec<Op> = (0..1500).map(|k| add(k, "#222222")).collect();
    ops.extend((0..1500).map(set));
    let edited = within(10, "mixed", || {
        exact_tamber::apply_all(&src, &ops, &vocab()).unwrap()
    });
    assert_eq!(edited.text.matches("#333333").count(), 1500);
    let mut bad: Vec<Op> = (0..3000).map(|k| add(k, "#222222")).collect();
    bad.push(add(3000, ""));
    let refused = within(10, "a bad last value", || {
        exact_tamber::apply_all(&src, &bad, &vocab())
    });
    assert!(refused.is_err());
}

/// An import's values and new tokens alternating (its tokens' order) are
/// applied with a few parses (not one each), the new tokens first (Astra's round 3: 6,000
/// alternating operations were 12,000 parses).
#[test]
fn alternating_values_and_new_tokens_apply_in_moments() {
    let n = 20_000;
    let src = theme((0..n).map(|k| format!("  t{k} = #111111")));
    let ops: Vec<Op> = (0..6000)
        .map(|k| match k % 2 {
            0 => Op::SetToken {
                name: format!("t{k}"),
                when: Vec::new(),
                value: "#333333".into(),
                at: None,
                comment: None,
            },
            _ => Op::AddToken {
                name: format!("new.t{k}"),
                kind: None,
                value: "#222222".into(),
                meta: Vec::new(),
                whens: Vec::new(),
                comment: None,
                before: None,
            },
        })
        .collect();
    let edited = within(10, "alternating", || {
        exact_tamber::apply_all(&src, &ops, &vocab()).unwrap()
    });
    assert_eq!(edited.text.matches("#333333").count(), 3000);
    assert_eq!(edited.text.matches("#222222").count(), 3000);
    let undone = exact_tamber::undo(&edited.text, &edited.inverse, &vocab()).unwrap();
    assert_eq!(undone.text, src);
}

/// DTCG themes in many groups are grouped by a map, and sets ranked by
/// precomputed places (Astra's round 3).
#[test]
fn many_dtcg_theme_groups_group_in_moments() {
    use exact_tamber::import::dtcg;
    let themes: Vec<serde_json::Value> = (0..20_000)
        .map(|k| serde_json::json!({"name": format!("t{k}"), "group": format!("g{k}"), "selectedTokenSets": {"core": "enabled"}}))
        .collect();
    let files = vec![
        (
            "core.json".to_string(),
            serde_json::json!({"ink": {"$type": "color", "$value": "#111111"}}).to_string(),
        ),
        (
            "$themes.json".to_string(),
            serde_json::Value::Array(themes).to_string(),
        ),
    ];
    let imported = within(20, "dtcg groups", || {
        dtcg::import(&files, "dt", &parse("tamber app\n").unwrap()).map(|i| i.ops.len())
    });
    assert_eq!(imported.ok(), Some(1));
}

/// A held import's values applied on a design that has since lost one of
/// their tokens take the rest in one parse and refuse at that one, not a
/// parse for each before it (Astra's re-check after round 3).
#[test]
fn values_on_a_token_gone_are_refused_after_one_parse() {
    let n = 20_000;
    let src = theme((0..n).map(|k| format!("  t{k} = #111111")));
    let mut ops: Vec<Op> = (0..6000)
        .map(|k| Op::SetToken {
            name: format!("t{k}"),
            when: Vec::new(),
            value: "#333333".into(),
            at: None,
            comment: None,
        })
        .collect();
    ops.push(Op::SetToken {
        name: "gone".into(),
        when: Vec::new(),
        value: "#333333".into(),
        at: None,
        comment: None,
    });
    let refused = within(10, "a token gone", || {
        exact_tamber::apply_all(&src, &ops, &vocab())
    });
    let d = refused.unwrap_err();
    assert!(d[0].message.contains("gone"), "{d:?}");
}

/// A themed import alternating its tokens' `when` lines and new tokens with
/// theirs keeps its few parses (a guard, the new tokens, the values) where no value can move the `when` indent
/// (Astra's re-check 2).
#[test]
fn alternating_themed_values_and_new_tokens_apply_in_moments() {
    let n = 20_000;
    let mut src = String::from("tamber t\n\naxes\n  tone = a | b\n\ntheme\n");
    for k in 0..n {
        src += &format!("  t{k} = #111111\n    when tone=b: #222222\n");
    }
    let ops: Vec<Op> = (0..6000)
        .map(|k| match k % 2 {
            0 => Op::SetToken {
                name: format!("t{k}"),
                when: vec![("tone".into(), "b".into())],
                value: "#333333".into(),
                at: None,
                comment: None,
            },
            _ => Op::AddToken {
                name: format!("new.t{k}"),
                kind: None,
                value: "#222222".into(),
                meta: Vec::new(),
                whens: vec![(vec![("tone".into(), "b".into())], "#444444".into(), None)],
                comment: None,
                before: None,
            },
        })
        .collect();
    let edited = within(10, "themed alternating", || {
        exact_tamber::apply_all(&src, &ops, &vocab()).unwrap()
    });
    assert_eq!(edited.text.matches("#333333").count(), 3000);
    assert_eq!(edited.text.matches("#444444").count(), 3000);
}
