//! @ref LLP 1121 D11 (stage 12 B2) — a length over base names the design
//! leaves unset: kept as one expression, token references inlined and its
//! px parts folded, which each host evaluates in its column
//! (`contract/cli/tests/base_expression.rs`). The compiler checks it in
//! every column and names the one it fails in, a param's range included.
//! What is no length's `calc()`, `min()`, `max()` or `clamp()` over px and
//! numbers is refused. A shadow over a base name is stage 12 B3's
//! (`tamber_base_shadow.rs`).

use exact_kernel::style::scale;
use std::path::PathBuf;

fn app(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("exact2-base-expr-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for (file, text) in files {
        std::fs::write(dir.join(file), text).unwrap();
    }
    dir.join("app.contract")
}

/// A view with one column a token, each its padding.
fn view(tokens: &[&str]) -> String {
    let mut v = String::from("component App\n  view\n    column\n");
    for t in tokens {
        v += &format!("      column padding=\"{t}\"\n        text \"a\"\n");
    }
    v
}

fn refused(name: &str, theme: &str, view: &str) -> (String, String) {
    let design = format!("tamber t\n\ntheme\n{theme}");
    let e = contract::compile_path(&app(
        name,
        &[("app.tamber", &design), ("app.contract", view)],
    ))
    .unwrap_err();
    (e.id.to_string(), e.message)
}

#[test]
fn an_expression_is_kept_with_its_references_inlined_and_px_folded() {
    let design = "tamber t\n\ntheme\n  pad.a: length = calc(space.md + 2px + 2px)\n  pad.b: length = calc(pad.a * 2)\n  pad.c = calc(space.md * 2)\n  pad.d: length = min(space.lg, 20px, 30px)\n  pad.e: length = clamp(2px, space.sm * 3, 40px)\n  pad.f: length = calc(space.md)\n  pad.g: length = calc(space.md - space.md + 3px)\n  pad.h: length = calc(space.lg * 2 + 4px - space.xs / 2)\n";
    let names = [
        "pad.a", "pad.b", "pad.c", "pad.d", "pad.e", "pad.f", "pad.g", "pad.h",
    ];
    let plan = contract::compile_path(&app(
        "kept",
        &[("app.tamber", design), ("app.contract", &view(&names))],
    ))
    .unwrap_or_else(|e| panic!("{e:?}"));
    let value = |name: &str| {
        let t = plan
            .theme
            .iter()
            .find(|t| plan.str(t.name) == name)
            .unwrap();
        plan.str(t.value).to_string()
    };
    for (name, want) in [
        ("pad.a", "calc(space.md + 4px)"),
        ("pad.b", "calc(space.md * 2 + 8px)"),
        // Untyped, it is a length: a base name is a dimension.
        ("pad.c", "calc(space.md * 2)"),
        ("pad.d", "min(space.lg, 20px)"),
        ("pad.e", "clamp(2px, space.sm * 3, 40px)"),
        // A lone base name is the alias; none left, px.
        ("pad.f", "space.md"),
        ("pad.g", "3px"),
        ("pad.h", "calc(space.lg * 2 - space.xs * 0.5 + 4px)"),
    ] {
        assert_eq!(value(name), want, "{name}");
    }
    // Each reads its base names, the inlined ones too (stage 11 D's `bases`).
    let folding = exact_tamber::check_design(
        exact_tamber::parse(design).unwrap(),
        &contract_lower::tamber_vocab(),
    )
    .unwrap();
    let bases = |name: &str| {
        folding
            .tokens
            .iter()
            .find(|t| t.name == name)
            .unwrap()
            .bases
            .clone()
    };
    assert_eq!(bases("pad.b"), ["space.md"]);
    assert_eq!(bases("pad.h"), ["space.lg", "space.xs"]);
    // A read a later term cancels is still read (round 1, Astra 6).
    assert_eq!(bases("pad.g"), ["space.md"]);
}

#[test]
fn every_column_is_checked_and_a_refusal_names_its_columns() {
    let expr = "calc(space.md - 12px)";
    // The first value refused, in the table's column order, and every
    // column it is.
    let px: Vec<(&str, f32)> = scale::COLUMNS
        .iter()
        .map(|(n, c)| (*n, scale::evaluate(expr, *c).unwrap()))
        .collect();
    let first = px
        .iter()
        .find(|(_, v)| *v < 0.0)
        .expect("a negative column")
        .1;
    let negative: Vec<&str> = px
        .iter()
        .filter(|(_, v)| *v == first)
        .map(|(n, _)| *n)
        .collect();
    assert!(px.iter().any(|(n, v)| *n == "mac" && *v < 0.0), "{px:?}");
    let (id, message) = refused(
        "negative",
        &format!("  pad: length = {expr}\n"),
        &view(&["pad"]),
    );
    assert_eq!(id, "tamber-kind", "{message}");
    assert!(
        message.contains(&format!(
            "`{first}px` on {} (`{expr}`)",
            negative.join(", ")
        )),
        "{message}"
    );
    // A row that takes a negative length takes it.
    let design = format!("tamber t\n\ntheme\n  pad: length = {expr}\n");
    contract::compile_path(&app(
        "margin",
        &[
            ("app.tamber", &design),
            (
                "app.contract",
                "component App\n  view\n    column margin-top=\"pad\"\n      text \"a\"\n",
            ),
        ],
    ))
    .unwrap_or_else(|e| panic!("{e:?}"));
    // A `when` arm's expression is checked in each column too.
    let design = format!("tamber t\n\naxes\n  tone = plain | wide\n\ntheme\n  pad: length = space.md\n    when tone=wide: {expr}\n");
    let e = contract::compile_path(&app(
        "arm",
        &[("app.tamber", &design), ("app.contract", &view(&["pad"]))],
    ))
    .unwrap_err();
    assert_eq!(e.id, "tamber-kind", "{e:?}");
    assert!(e.message.contains("on mac"), "{}", e.message);
}

#[test]
fn a_params_range_is_checked_in_each_column() {
    let card = |range: &str| {
        format!("component Card\n  params\n    pad: length = \"calc(space.md * 3)\"  range=\"{range}\"\n  view\n    column padding=\"param.pad\"\n      text \"a\"\n")
    };
    let root =
        "use Card from \"./card.contract\"\n\ncomponent App\n  view\n    column\n      Card()\n";
    let over = scale::COLUMNS
        .iter()
        .map(|(n, c)| (*n, scale::evaluate("calc(space.md * 3)", *c).unwrap()))
        .find(|(_, px)| *px > 40.0)
        .expect("a column past 40px");
    let e = contract::compile_path(&app(
        "range",
        &[("app.contract", root), ("card.contract", &card("0..40"))],
    ))
    .unwrap_err();
    assert_eq!(e.id, "tamber-param-range", "{e:?}");
    assert!(
        e.message.contains(&format!("{}px on {}", over.1, over.0)),
        "{}",
        e.message
    );
    contract::compile_path(&app(
        "in-range",
        &[("app.contract", root), ("card.contract", &card("0..100"))],
    ))
    .unwrap_or_else(|e| panic!("{e:?}"));
    // A bare base name (theme2's 01:13 ruling): inside the range in every
    // column, accepted; outside it in one, refused, naming that column.
    let bare = |range: &str| {
        format!("component Card\n  params\n    pad: length = \"space.lg\"  range=\"{range}\"\n  view\n    column padding=\"param.pad\"\n      text \"a\"\n")
    };
    let px: Vec<(&str, f32)> = scale::COLUMNS
        .iter()
        .map(|(n, c)| (*n, scale::evaluate("calc(space.lg)", *c).unwrap()))
        .collect();
    let high = px.iter().map(|(_, v)| *v).fold(f32::MIN, f32::max);
    let low = px.iter().map(|(_, v)| *v).fold(f32::MAX, f32::min);
    assert!(low < high, "{px:?}");
    contract::compile_path(&app(
        "bare-in",
        &[
            ("app.contract", root),
            ("card.contract", &bare(&format!("0..{high}"))),
        ],
    ))
    .unwrap_or_else(|e| panic!("{e:?}"));
    let (column, v) = px.iter().find(|(_, v)| *v > high - 1.0).unwrap();
    let e = contract::compile_path(&app(
        "bare-out",
        &[
            ("app.contract", root),
            ("card.contract", &bare(&format!("0..{}", high - 1.0))),
        ],
    ))
    .unwrap_err();
    assert_eq!(e.id, "tamber-param-range", "{e:?}");
    assert!(
        e.message
            .contains(&format!("`space.lg`, {v}px on {column}, outside its range")),
        "{}",
        e.message
    );
}

#[test]
fn what_is_no_lengths_function_over_px_is_refused() {
    let pad = view(&["pad"]);
    for (name, value, id, said) in [
        (
            "product",
            "calc(space.md * space.sm)",
            "tamber-kind",
            "a length times or over a length",
        ),
        (
            "quotient",
            "calc(space.md / space.sm)",
            "tamber-kind",
            "a length times or over a length",
        ),
        (
            "over-px",
            "calc(space.md * 2px)",
            "tamber-kind",
            "a length times or over a length",
        ),
        ("rem", "calc(space.md + 1rem)", "tamber-kind", "px inside"),
        (
            "percent",
            "calc(space.md + 10%)",
            "tamber-kind",
            "percentage",
        ),
        (
            "number",
            "calc(space.md + 2)",
            "tamber-kind",
            "adds a length and a number",
        ),
        (
            "mixed",
            "min(space.md, 2)",
            "tamber-kind",
            "mixes a length and a number",
        ),
        (
            "number-over",
            "calc(2 / space.md)",
            "tamber-kind",
            "a number over a length",
        ),
    ] {
        let (got, message) = refused(name, &format!("  pad: length = {value}\n"), &pad);
        assert_eq!((got.as_str(), name), (id, name), "{message}");
        assert!(message.contains(said), "{name}: {message}");
    }
    // A shadow's lengths over one are kept (stage 12 B3,
    // `tamber_base_shadow.rs`).
}

/// Outside Exact (the DTCG export) an expression is the neutral column's
/// px, as an alias is, the expression the source it keeps.
#[test]
fn an_expression_exports_the_neutral_columns_px() {
    use exact_kernel::style::metrics::Column;
    let design = "tamber t\n\ntheme\n  card.pad: length = calc(space.md * 2 + 1px)\n";
    let vocab = contract_lower::tamber_vocab();
    let folding = exact_tamber::check_design(exact_tamber::parse(design).unwrap(), &vocab).unwrap();
    let json: serde_json::Value =
        serde_json::from_str(&exact_tamber::export::dtcg(&folding, &vocab).unwrap()).unwrap();
    let want = scale::evaluate("calc(space.md * 2 + 1px)", Column::Neutral).unwrap();
    let text = json.to_string();
    assert!(
        text.contains(&format!("\"value\":{want}"))
            || text.contains(&format!("\"value\":{want}.0")),
        "{text}"
    );
    assert!(text.contains("calc(space.md * 2 + 1px)"), "{text}");
}

/// Round 1 (Astra): arithmetic at its edges. A sum or a step that isn't
/// finite is refused, as in px; a quotient divides each coefficient (no
/// reciprocal to overflow); a term that underflows to 0 goes, so the text
/// normalises again to itself; an expression past the kernel's `calc()`
/// limits is refused at build, naming the column, wherever a row reads it.
#[test]
fn arithmetic_at_its_edges() {
    let pad = view(&["pad"]);
    for (name, value) in [
        ("sum-inf", "calc(space.md * 1e308 + space.md * 1e308)"),
        ("zero-times-inf", "calc(space.md / 0 * 0)"),
    ] {
        let (id, message) = refused(name, &format!("  pad: length = {value}\n"), &pad);
        assert_eq!(id, "tamber-kind", "{name}: {message}");
        assert!(message.contains("finite"), "{name}: {message}");
    }
    let design = "tamber t\n\ntheme\n  pad.a: length = calc(space.md * 1e-320 / 1e-320)\n  pad.b: length = calc(space.md * 1e-200 * 1e-200 + 2px)\n";
    let plan = contract::compile_path(&app(
        "edges",
        &[
            ("app.tamber", design),
            ("app.contract", &view(&["pad.a", "pad.b"])),
        ],
    ))
    .unwrap_or_else(|e| panic!("{e:?}"));
    let value = |name: &str| {
        let t = plan
            .theme
            .iter()
            .find(|t| plan.str(t.name) == name)
            .unwrap();
        plan.str(t.value).to_string()
    };
    assert_eq!(value("pad.a"), "space.md");
    assert_eq!(value("pad.b"), "2px");
    // Seventeen terms of two values each: past the kernel's 32, and a
    // computed row reads it as a literal does.
    let terms: Vec<String> = (1..=17).map(|n| format!("space.md + {n}px")).collect();
    let design = format!(
        "tamber t\n\ntheme\n  pad: length = min({})\n",
        terms.join(", ")
    );
    let e = contract::compile_path(&app(
        "past-limits",
        &[
            ("app.tamber", &design),
            (
                "app.contract",
                "component App\n  state chosen = \"pad\"\n  view\n    column padding=chosen\n      text \"a\"\n",
            ),
        ],
    ))
    .unwrap_err();
    assert_eq!(e.id, "tamber-kind", "{e:?}");
    assert!(
        e.message.contains("doesn't fold to a length on ios"),
        "{}",
        e.message
    );
}

/// Round 1 (Astra 2): a range's bounds are compared in the kernel's
/// precision, so a column's value at the bound is inside it.
#[test]
fn a_range_bound_is_compared_as_the_kernel_folds() {
    let expr = "calc(space.md * 0.1)";
    let high = scale::COLUMNS
        .iter()
        .map(|(_, c)| scale::evaluate(expr, *c).unwrap())
        .fold(f32::MIN, f32::max);
    let card = format!("component Card\n  params\n    pad: length = \"{expr}\"  range=\"0..{high}\"\n  view\n    column padding=\"param.pad\"\n      text \"a\"\n");
    let root =
        "use Card from \"./card.contract\"\n\ncomponent App\n  view\n    column\n      Card()\n";
    contract::compile_path(&app(
        "bound",
        &[("app.contract", root), ("card.contract", &card)],
    ))
    .unwrap_or_else(|e| panic!("{e:?}"));
}

/// Round 1 (Grok): the text the plan carries parses back in the kernel to
/// what the written expression is, in every column: a negative coefficient,
/// a quotient, a sum inside `min()`, a `clamp()` whose low bound passes its
/// high.
#[test]
fn the_carried_text_folds_as_the_written_one() {
    let written = [
        "calc(0px - space.md)",
        "calc(space.md / 3 - 2px)",
        "min(space.md + 2px, space.lg * 2, 4px + 1px)",
        "max(space.sm * 1.5, space.xs)",
        "clamp(30px, space.md, 2px)",
        "calc(max(space.md, space.lg) * 2 - space.xs / 4)",
    ];
    let mut design = String::from("tamber t\n\ntheme\n");
    for (i, w) in written.iter().enumerate() {
        design += &format!("  pad.t{i}: length = {w}\n");
    }
    let vocab = contract_lower::tamber_vocab();
    let folding = exact_tamber::check_design(exact_tamber::parse(&design).unwrap(), &vocab)
        .unwrap_or_else(|e| panic!("{e:?}"));
    for (i, w) in written.iter().enumerate() {
        let carried = folding
            .tokens
            .iter()
            .find(|t| t.name == format!("pad.t{i}"))
            .unwrap()
            .values[0]
            .css();
        for (column_name, column) in scale::COLUMNS {
            let (a, b) = (
                scale::evaluate(w, column).unwrap(),
                scale::evaluate(&carried, column).unwrap_or_else(|| panic!("{carried}")),
            );
            assert!(
                (a - b).abs() < 1e-3,
                "{column_name}: {w} is {a}, carried {carried} is {b}"
            );
        }
    }
}

/// Round 2 (Grok): a base name is no font size a row takes. Bare, tamber
/// keeps it as a type's size (stage 11 D: its `bases` names it) and the
/// build refuses it where a `font` row reads it, as the kernel's longhands
/// take no base; an expression over one tamber refuses itself.
#[test]
fn a_base_name_is_no_font_size_a_row_takes() {
    let text = "component App\n  view\n    text \"a\" font=\"title\"\n";
    for (name, theme, said) in [
        (
            "type-bare",
            "  title = 700 space.lg/1.2 system-ui\n",
            "no `font` longhand reads",
        ),
        (
            "type-expr",
            "  big: length = calc(space.lg * 2)\n  title: type = 700 big/1.2 system-ui\n",
            "no font size takes",
        ),
    ] {
        let (id, message) = refused(name, theme, text);
        assert_eq!(id, "tamber-kind", "{name}: {message}");
        assert!(message.contains(said), "{name}: {message}");
    }
}

/// Round 2 (Astra): a fold past f32's range (a host's `calc()` saturates)
/// is refused at build, so a computed row never meets it; and outside
/// Exact an expression sums as a host's `calc()` does, the terms then px.
#[test]
fn a_fold_past_f32_is_refused_and_exports_sum_as_hosts_do() {
    let design = "tamber t\n\ntheme\n  pad: length = calc(space.md * 1e38)\n";
    let e = contract::compile_path(&app(
        "saturates",
        &[
            ("app.tamber", design),
            (
                "app.contract",
                "component App\n  state chosen = \"pad\"\n  view\n    column padding=chosen\n      text \"a\"\n",
            ),
        ],
    ))
    .unwrap_err();
    assert!(e.message.contains("doesn't fold to a length"), "{e:?}");
    use exact_kernel::style::metrics::Column;
    let expr = "calc(space.md * 1e16 - space.sm * 2e16 + 1px)";
    let want = scale::evaluate(expr, Column::Neutral).unwrap();
    let vocab = contract_lower::tamber_vocab();
    let design = format!("tamber t\n\ntheme\n  card.pad: length = {expr}\n");
    let folding =
        exact_tamber::check_design(exact_tamber::parse(&design).unwrap(), &vocab).unwrap();
    let value = folding
        .tokens
        .iter()
        .find(|t| t.name == "card.pad")
        .map(|t| t.values[0].clone());
    let Some(exact_tamber::Value::Length(exact_tamber::Length::Base(r))) = value else {
        panic!("not an expression: {value:?}");
    };
    let neutral = |n: &str| match scale::of(n, Column::Neutral) {
        Some(scale::ScaleValue::Px(px)) => Some(px),
        _ => None,
    };
    assert_eq!(r.fold(&neutral), Some(f64::from(want)));
}

/// Round 3 (Astra): a ranged param a column doesn't fold is refused by the
/// params' own check too, which the design server runs without lowering
/// the theme.
#[test]
fn an_unfoldable_ranged_param_is_refused_by_the_params_check() {
    let root =
        "use Card from \"./card.contract\"\n\ncomponent App\n  view\n    column\n      Card()\n";
    let card = "component Card\n  params\n    pad: length = \"calc(space.md * 1e38)\"  range=\"0..40\"\n  view\n    column padding=\"param.pad\"\n      text \"a\"\n";
    let path = app(
        "unfolded-param",
        &[("app.contract", root), ("card.contract", card)],
    );
    let errors = contract::compile_path_source_all(&path, root, false)
        .err()
        .expect("refused");
    assert!(
        errors
            .iter()
            .any(|e| e.id == "tamber-param-range"
                && e.message.contains("doesn't fold to a length on")),
        "{errors:?}"
    );
    // Said once: the theme's own refusal doesn't repeat it (re-check, Astra).
    assert_eq!(
        errors
            .iter()
            .filter(|e| e.message.contains("doesn't fold to a length"))
            .count(),
        1,
        "{errors:?}"
    );
}
