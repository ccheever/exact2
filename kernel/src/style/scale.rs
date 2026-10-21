//! The base scale (LLP 1121 stage 11 B; the schema's `scale`): nine names an
//! app's views read with no design defining them, `space.xs` to `space.xl`
//! and `radius.sm` to `radius.full`, each a length in this process's column.
//!
//! - The token lookup falls back to them (`theme::measure`): a design that
//!   defines one wins, app-wide, as any token does (ruling 4); one it leaves
//!   unset reads this table. The compiler knows the nine names, so a view
//!   reading one compiles with no design (`contract_lower::theme`), and the
//!   plan keeps nothing for them: the kernel already holds every column.
//! - Every cell names its source (a metrics row, a stretch of one theme2
//!   ruled, a measurement or a guideline, `docs/scale-measured.md`), which
//!   the build resolves. This is
//!   the one table every reader of the base takes (theme2's condition (c)):
//!   the design server's `describe`, `resolve` and `explain` read it under
//!   `tamber/1.14` (rt1's stage 10 D), never a copy.
//! - Units are px (points), never scaled by Dynamic Type (ruling 6; QUEUE
//!   holds the opt-in). radius.full is [`RADIUS_FULL`], one oversized length
//!   every host clamps into a pill by CSS's corner-overlap rule (theme2's C
//!   ruling). The terminal's spacings are cells (`ch`), its radii 0.
//!
//! @ref LLP 1121 stage 11 B

pub use crate::generated::{ScaleCell, ScaleRow, ScaleSource, ScaleValue, SCALE};
use crate::style::metrics::Column;

/// radius.full's length, named once: larger than any box, so every host's
/// corner-overlap clamp (CSS Backgrounds 3 §5.5) makes it half the box's
/// smaller side, a pill; never `50%`, which is an ellipse.
pub const RADIUS_FULL: f64 = 9999.0;

/// The nine base names, in the table's order.
pub fn names() -> impl Iterator<Item = &'static str> {
    SCALE.iter().map(|r| r.name)
}

/// The base row `name` names, exactly.
pub fn row(name: &str) -> Option<&'static ScaleRow> {
    SCALE.iter().find(|r| r.name == name)
}

/// Whether `name` is a base name.
pub fn is_base(name: &str) -> bool {
    row(name).is_some()
}

/// A row's cell in a column.
pub fn cell(row: &ScaleRow, column: Column) -> ScaleCell {
    match column {
        // No web column: every web build draws the neutral look (theme2).
        Column::Web | Column::Neutral => row.neutral,
        Column::Ios => row.ios,
        Column::Mac => row.macos,
        Column::Material => row.material,
        Column::Terminal => row.terminal,
    }
}

/// Base name `name`'s value in a column.
pub fn of(name: &str, column: Column) -> Option<ScaleValue> {
    row(name).map(|r| cell(r, column).value)
}

/// A value as the CSS text a row reads, as a token's length text is:
/// `16px`, the terminal's `1ch`, radius.full's [`RADIUS_FULL`] in px.
pub fn css(value: ScaleValue) -> String {
    match value {
        ScaleValue::Px(n) => format!("{}px", exact_num::Shortest(n)),
        ScaleValue::Cells(n) => format!("{}ch", exact_num::Shortest(n)),
        ScaleValue::Full => format!("{}px", exact_num::Shortest(RADIUS_FULL)),
    }
}

/// Base name `name`'s CSS text in this process's column.
pub fn text(name: &str) -> Option<String> {
    of(name, crate::style::metrics::column()).map(css)
}

/// The base names `expr` reads, each a whole word of it, in order, each
/// once: a length token's expression over base names the design leaves
/// unset (LLP 1121 D11, stage 12 B2), which the compiler writes with each
/// operator spaced (`calc(space.md * 2 + 4px)`).
pub fn named_in(expr: &str) -> Vec<&'static str> {
    let mut out = Vec::new();
    for word in words(expr) {
        if let Some(r) = row(word).filter(|r| !out.contains(&r.name)) {
            out.push(r.name);
        }
    }
    out
}

/// Each run of a name's or a number's characters in `text`.
fn words(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')))
        .filter(|w| !w.is_empty())
}

/// Whether `text` has white space outside any parentheses: a sum, which
/// a product's operand must be grouped as.
fn spaced(text: &str) -> bool {
    let mut depth = 0u32;
    text.chars().any(|c| {
        match c {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            _ => {}
        }
        depth == 0 && c.is_whitespace()
    })
}

/// `expr` with each base name in it replaced by `base`'s text for it (one
/// with white space outside parentheses grouped in them); `None` when
/// `base` gives none for one.
pub fn substitute_with(expr: &str, mut base: impl FnMut(&str) -> Option<String>) -> Option<String> {
    let mut out = String::with_capacity(expr.len());
    let mut rest = expr;
    while let Some(word) = words(rest).next() {
        let at = rest.find(word).expect("a word of it");
        out.push_str(&rest[..at]);
        match row(word) {
            Some(r) => {
                let text = base(r.name)?;
                match spaced(text.trim()) {
                    false => out.push_str(text.trim()),
                    true => out.push_str(&format!("({})", text.trim())),
                }
            }
            None => out.push_str(word),
        }
        rest = &rest[at + word.len()..];
    }
    out.push_str(rest);
    Some(out)
}

/// `expr` with each base name in it replaced by its length in `column`
/// (the terminal's in cells, `1ch`, which the kernel's `calc()` reads as
/// px).
pub fn substitute(expr: &str, column: Column) -> Option<String> {
    substitute_with(expr, |name| of(name, column).map(css))
}

/// A length's px as the kernel's `calc()` folds it (`calc(16px * 2)` is
/// 32); `None` for a text that folds to no px.
pub fn fold(text: &str) -> Option<f32> {
    match crate::style::calc::parse(text) {
        // Past f32's range the calc saturates: no length a row reads back.
        Ok(Some(crate::style::Dimension::Points(n))) if n.is_finite() && n.abs() < f32::MAX => {
            Some(n)
        }
        _ => None,
    }
}

/// Expression `expr`'s px in `column`: what that column's host reads.
pub fn evaluate(expr: &str, column: Column) -> Option<f32> {
    fold(&substitute(expr, column)?)
}

/// Whether a shadow's item is a length, as [`fold_lengths`] and the JS
/// target read one (LLP 1121 stage 12 B3): px (`4px`, unitless `0`), a bare
/// base name, or a `calc()`, `min()`, `max()` or `clamp()`, in any case.
/// Any other function, a colour's, is none, whatever its words.
pub fn is_length_item(item: &str) -> bool {
    match item.split_once('(') {
        Some((f, _)) => ["calc", "min", "max", "clamp"]
            .iter()
            .any(|m| f.eq_ignore_ascii_case(m)),
        None => {
            item == "0"
                || is_base(item)
                || item
                    .strip_suffix("px")
                    .is_some_and(|n| exact_num::parse_f64(n).is_ok())
        }
    }
}

/// A shadow's text with each length over base names folded to px (LLP 1121
/// stage 12 B3): split at white space and commas outside parentheses, an
/// item naming a base is substituted through `base` ([`substitute_with`])
/// and folded by the kernel's `calc()` ([`fold`]) to `<px>px`, when it is
/// a bare word or a `calc()`, `min()`, `max()` or `clamp()`; every other
/// item (px, `inset`, any other function, a colour's) and every separator is
/// kept byte for byte, so the row's codec reads the text as a literal's.
/// `None` when `base` gives none for a base, or an item doesn't fold.
pub fn fold_lengths(text: &str, mut base: impl FnMut(&str) -> Option<String>) -> Option<String> {
    let mut out = String::with_capacity(text.len());
    let mut item = |part: &str, out: &mut String| -> Option<()> {
        // Only a length folds ([`is_length_item`]). Any other item (a
        // colour, a platform colour whose asset is named `space.md`) is
        // kept whole, whatever its words, a base name inside one the
        // codec's to refuse.
        if !is_length_item(part) || named_in(part).is_empty() {
            out.push_str(part);
            return Some(());
        }
        // A bare base name is no math function: `calc()` makes it one.
        let px = fold(&format!("calc({})", substitute_with(part, &mut base)?))?;
        out.push_str(&format!("{}px", exact_num::Shortest32(px)));
        Some(())
    };
    let (mut depth, mut start) = (0u32, 0);
    for (i, c) in text.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            _ if depth == 0 && (c.is_whitespace() || c == ',') => {
                item(&text[start..i], &mut out)?;
                out.push(c);
                start = i + c.len_utf8();
            }
            _ => {}
        }
    }
    item(&text[start..], &mut out)?;
    Some(out)
}

/// The table's columns, by the name a reader of the table keys it by (stage
/// 10 D's `describe`, `resolve` and `explain`). `Column::Web` reads the
/// neutral one ([`cell`]).
pub const COLUMNS: [(&str, Column); 5] = [
    ("ios", Column::Ios),
    ("mac", Column::Mac),
    ("neutral", Column::Neutral),
    ("material", Column::Material),
    ("terminal", Column::Terminal),
];

/// The scale column a shipped target reads (theme2's 16:32 ruling): Apple's
/// phones and their kin iOS's, the Mac's own, the web and Linux the neutral
/// look's (D17), Android's reader Material's, the terminal its cells. The
/// design server sends it (`describe`'s `targets`), so no client keeps a map.
pub fn column_for(target: &str) -> Option<&'static str> {
    Some(match target {
        "ios" | "tvos" | "visionos" | "watchos" => "ios",
        "macos" | "mac" => "mac",
        "web" | "linux" | "windows" => "neutral",
        "android" => "material",
        "terminal" => "terminal",
        _ => return None,
    })
}

/// A cell's source as words: `metric grouped-row-gap`, `stretch of
/// grouped-label-inset: <why>`, `measured <id>`, `guideline <id>`, `full`,
/// `terminal cells`.
pub fn source(cell: ScaleCell) -> String {
    match cell.source {
        ScaleSource::Metric(m) => format!("metric {m}"),
        ScaleSource::Stretch { metric, why } => format!("stretch of {metric}: {why}"),
        ScaleSource::Measured(id) => format!("measured {id}"),
        ScaleSource::Guideline(id) => format!("guideline {id}"),
        ScaleSource::Full => "full".into(),
        ScaleSource::Terminal => "terminal cells".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generated::{MetricValue, METRICS};
    use crate::style::metrics::value;

    fn px(v: ScaleValue) -> f64 {
        match v {
            ScaleValue::Px(n) | ScaleValue::Cells(n) => n,
            ScaleValue::Full => RADIUS_FULL,
        }
    }

    #[test]
    fn the_nine_names_and_nothing_else() {
        assert_eq!(
            names().collect::<Vec<_>>(),
            [
                "space.xs",
                "space.sm",
                "space.md",
                "space.lg",
                "space.xl",
                "radius.sm",
                "radius.md",
                "radius.lg",
                "radius.full"
            ]
        );
        assert!(
            is_base("space.md")
                && !is_base("space.header")
                && !is_base("Space.md")
                && !is_base("radius.xl")
        );
    }

    // theme2's (a): every cell equals its source's value, read again here
    // from the metrics table the kernel holds.
    #[test]
    fn every_cell_holds_its_source() {
        for r in SCALE {
            for (column_name, column) in COLUMNS {
                let c = cell(r, column);
                match c.source {
                    ScaleSource::Metric(m) | ScaleSource::Stretch { metric: m, .. } => {
                        let metric = METRICS
                            .iter()
                            .find(|x| x.name == m)
                            .unwrap_or_else(|| panic!("{} {column_name}: no metric {m}", r.name));
                        assert_eq!(
                            value(metric, column),
                            MetricValue::Number(px(c.value)),
                            "{} {column_name} = {m}",
                            r.name
                        );
                    }
                    ScaleSource::Full => {
                        assert_eq!((r.name, c.value), ("radius.full", ScaleValue::Full))
                    }
                    ScaleSource::Terminal => {
                        assert_eq!(column, Column::Terminal);
                        assert!(matches!(c.value, ScaleValue::Cells(_)));
                    }
                    ScaleSource::Measured(_) | ScaleSource::Guideline(_) => {
                        assert!(matches!(c.value, ScaleValue::Px(_)))
                    }
                }
            }
            // The terminal draws no corner.
            if r.name.starts_with("radius.") {
                assert_eq!(r.terminal.value, ScaleValue::Cells(0.0), "{}", r.name);
            }
        }
    }

    // Stage 13 D5: the Mac's space.xl is the one stretch theme2 ruled, its
    // metric's value, with its reason.
    #[test]
    fn the_macs_space_xl_is_a_stretch() {
        let xl = SCALE.iter().find(|r| r.name == "space.xl").unwrap();
        assert!(matches!(
            xl.macos.source,
            ScaleSource::Stretch { metric: "grouped-label-inset", why } if !why.is_empty()
        ));
        assert_eq!(xl.macos.value, ScaleValue::Px(30.0));
        let stretches = SCALE
            .iter()
            .flat_map(|r| COLUMNS.iter().map(move |(_, c)| cell(r, *c)))
            .filter(|c| matches!(c.source, ScaleSource::Stretch { .. }))
            .count();
        assert_eq!(stretches, 1);
        assert!(source(xl.macos).starts_with("stretch of grouped-label-inset: "));
    }

    // theme2's (a): within each look, xs ≤ sm ≤ md ≤ lg ≤ xl and radius
    // sm ≤ md ≤ lg, so a borrowed value cannot invert the scale;
    // radius.full is left out (theme2's C ruling).
    #[test]
    fn each_looks_steps_are_in_order() {
        for (column_name, column) in COLUMNS {
            for steps in [
                &["space.xs", "space.sm", "space.md", "space.lg", "space.xl"][..],
                &["radius.sm", "radius.md", "radius.lg"][..],
            ] {
                let values: Vec<f64> = steps.iter().map(|n| px(of(n, column).unwrap())).collect();
                assert!(
                    values.windows(2).all(|w| w[0] <= w[1]),
                    "{column_name}: {steps:?} are {values:?}"
                );
            }
        }
    }

    // Every measured or guideline id has its entry in docs/scale-measured.md,
    // whose `Binds:` line binds this step to this value exactly (a shared
    // entry's other numbers do not stand in; round 1, Astra 4). The build
    // refuses the same.
    #[test]
    fn every_measured_and_guideline_id_is_documented_with_its_number() {
        let doc = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../docs/scale-measured.md"
        ))
        .expect("docs/scale-measured.md");
        for r in SCALE {
            for (column_name, column) in COLUMNS {
                let c = cell(r, column);
                let (ScaleSource::Measured(id) | ScaleSource::Guideline(id)) = c.source else {
                    continue;
                };
                let entry = doc
                    .split("\n### ")
                    .find(|s| s.starts_with(&format!("`{id}`")))
                    .unwrap_or_else(|| panic!("{} {column_name}: no `### `{id}`` entry", r.name));
                let binding = format!("`{} = {}`", r.name, exact_num::Shortest(px(c.value)));
                let binds = entry
                    .lines()
                    .find(|l| l.starts_with("Binds: "))
                    .unwrap_or_else(|| panic!("`{id}` has no `Binds:` line"));
                assert!(
                    binds.contains(&binding),
                    "{} {column_name}: `{id}`'s `Binds:` line has no {binding}: {binds}",
                    r.name
                );
            }
        }
    }

    // The token lookup's fallback (theme2's (c)): a base name no entry
    // defines reads this table in the process's column, bound theme or none;
    // one the design defines is the design's, even where a row may not name
    // it; and only a row that takes a length reads one.
    #[test]
    fn the_token_lookup_falls_back_to_the_base_scale() {
        use crate::style::theme::{self, Kind};
        let length = |k: Kind, _| k == Kind::Length;
        let expected = text("space.md").unwrap();
        assert_eq!(
            theme::measure("space.md", length),
            Some((Kind::Length, expected.as_str().into()))
        );
        assert_eq!(
            theme::measure("radius.full", length),
            Some((Kind::Length, "9999px".into()))
        );
        assert_eq!(theme::measure("space.header", length), None);
        assert_eq!(theme::measure("space.md", |k, _| k == Kind::Number), None);
        let defined = theme::kinded([
            ("space.md", "40px", false, Kind::Length, false),
            ("space.lg", "50px", false, Kind::Length, true),
        ]);
        let _scope = theme::bind(&defined);
        assert_eq!(
            theme::measure("space.md", length),
            Some((Kind::Length, "40px".into()))
        );
        // Defined, and varying where the row may not take it: the design's, not the base's.
        assert_eq!(
            theme::measure("space.lg", |k, varies| k == Kind::Length && !varies),
            None
        );
        assert_eq!(
            theme::measure("space.sm", length).map(|(_, v)| v),
            text("space.sm").map(|t| t.as_str().into())
        );
        // The read is recorded as a token's, so an edit that defines it later sets the row again.
        let (_, names) = theme::recorded(|| theme::measure("space.xs", length));
        assert_eq!(names, vec![Box::<str>::from("space.xs")]);
    }

    // An alias (round 1, theme2's (a)): a length whose value is a bare base
    // name reads the base, the design's own definition first; both reads are
    // recorded, so an edit of either sets the row again; a base name set to
    // itself reads the scale.
    #[test]
    fn an_alias_reads_the_base_the_designs_own_first() {
        use crate::style::theme::{self, Kind};
        let length = |k: Kind, _| k == Kind::Length;
        let read = |entries: &[(&str, &str)], name: &str| {
            let table = theme::kinded(
                entries
                    .iter()
                    .map(|(n, v)| (*n, *v, false, Kind::Length, false)),
            );
            let _scope = theme::bind(&table);
            theme::recorded(|| theme::measure(name, length).map(|(_, v)| v.to_string()))
        };
        let (value, names) = read(&[("card.pad", "space.md")], "card.pad");
        assert_eq!(value, text("space.md"));
        assert_eq!(
            names,
            vec![Box::<str>::from("card.pad"), Box::<str>::from("space.md")]
        );
        assert_eq!(
            read(
                &[("card.pad", "space.md"), ("space.md", "40px")],
                "card.pad"
            )
            .0
            .as_deref(),
            Some("40px")
        );
        assert_eq!(
            read(&[("space.md", "space.lg")], "space.md").0,
            text("space.lg")
        );
        assert_eq!(
            read(&[("space.md", "space.md")], "space.md").0,
            text("space.md")
        );
    }

    // Stage 12 B2: a length token over base names reads each base as an
    // alias does, the design's own definition first, both reads recorded,
    // and folds to px in the process's column.
    #[test]
    fn an_expression_token_folds_in_the_process_column() {
        use crate::style::theme::{self, Kind};
        let length = |k: Kind, _| k == Kind::Length;
        let read = |entries: &[(&str, &str)], name: &str| {
            let table = theme::kinded(
                entries
                    .iter()
                    .map(|(n, v)| (*n, *v, false, Kind::Length, false)),
            );
            let _scope = theme::bind(&table);
            theme::recorded(|| theme::measure(name, length).map(|(_, v)| v.to_string()))
        };
        let column = crate::style::metrics::column();
        let want = |expr: &str| {
            Some(format!(
                "{}px",
                exact_num::Shortest32(evaluate(expr, column).unwrap())
            ))
        };
        let (value, names) = read(&[("card.pad", "calc(space.md * 2 + 4px)")], "card.pad");
        assert_eq!(value, want("calc(space.md * 2 + 4px)"));
        assert_eq!(
            names,
            vec![Box::<str>::from("card.pad"), Box::<str>::from("space.md")]
        );
        assert_eq!(
            read(
                &[
                    ("card.pad", "min(space.md, space.lg)"),
                    ("space.md", "40px")
                ],
                "card.pad"
            )
            .0
            .as_deref(),
            want("min(40px, space.lg)").as_deref()
        );
        // A base the design defines as an alias of another reads that one.
        assert_eq!(
            read(
                &[("card.pad", "calc(space.md / 2)"), ("space.md", "space.xl")],
                "card.pad"
            )
            .0,
            want("calc(space.xl / 2)")
        );
        // One that folds to no px is no token's value: the row reads its text.
        assert_eq!(
            read(
                &[("card.pad", "calc(space.md + 1px)"), ("space.md", "1rem")],
                "card.pad"
            )
            .0,
            None
        );
    }

    // No web column (theme2): `Column::Web`, a process that says no column
    // and Material's fallback, reads the neutral cell, for every name.
    #[test]
    fn the_web_column_reads_the_neutral_cell() {
        for r in SCALE {
            assert_eq!(cell(r, Column::Web), r.neutral, "{}", r.name);
            assert_eq!(
                of(r.name, Column::Web),
                of(r.name, Column::Neutral),
                "{}",
                r.name
            );
        }
    }

    // Stage 12 B2: an expression over base names is each column's, its
    // bases substituted and folded by the kernel's own `calc()`: `min()`,
    // `max()` and `clamp()` too, and the terminal's cells as px.
    #[test]
    fn an_expression_reads_each_columns_bases() {
        let expr = "calc(space.md * 2 + 4px)";
        assert_eq!(named_in(expr), ["space.md"]);
        assert_eq!(
            named_in("min(space.lg, space.md * 3, space.lg)"),
            ["space.lg", "space.md"]
        );
        assert!(named_in("calc(space.md2 + x.space.md)").is_empty());
        assert_eq!(
            substitute(expr, Column::Mac).as_deref(),
            Some("calc(10px * 2 + 4px)")
        );
        assert_eq!(evaluate(expr, Column::Mac), Some(24.0));
        assert_eq!(
            evaluate(expr, Column::Terminal),
            Some(2.0 * crate::style::cells::COLUMN + 4.0)
        );
        for (column_name, column) in COLUMNS {
            let base = |n: &str| match of(n, column).unwrap() {
                ScaleValue::Px(n) => n as f32,
                ScaleValue::Cells(n) => n as f32 * crate::style::cells::COLUMN,
                ScaleValue::Full => RADIUS_FULL as f32,
            };
            let (md, lg) = (base("space.md"), base("space.lg"));
            for (expr, want) in [
                ("min(space.md, 4px)", md.min(4.0)),
                ("max(space.md, space.lg)", md.max(lg)),
                (
                    "clamp(2px, space.md * 3, 30px)",
                    (md * 3.0).clamp(2.0, 30.0),
                ),
                (
                    "calc(max(space.md * 2, 4px) - 1px)",
                    (md * 2.0).max(4.0) - 1.0,
                ),
                ("calc(radius.full / 2)", base("radius.full") / 2.0),
            ] {
                assert_eq!(evaluate(expr, column), Some(want), "{column_name}: {expr}");
            }
        }
        // A base the design defines is read as `base` gives it.
        assert_eq!(
            substitute_with(expr, |_| Some("calc(1px + 2px)".into())).as_deref(),
            Some("calc(calc(1px + 2px) * 2 + 4px)")
        );
        assert_eq!(
            substitute_with(expr, |_| Some("1px + 2px".into())).as_deref(),
            Some("calc((1px + 2px) * 2 + 4px)")
        );
        assert_eq!(substitute_with(expr, |_| None), None);
    }

    // Stage 12 B3: a shadow's lengths over base names, each folded to px
    // in a column, every other item and separator kept byte for byte.
    #[test]
    fn a_shadows_lengths_fold_and_the_rest_is_kept() {
        let mac = |text: &str| fold_lengths(text, |n| of(n, Column::Mac).map(css));
        let px = |expr: &str| {
            format!(
                "{}px",
                exact_num::Shortest32(evaluate(expr, Column::Mac).unwrap())
            )
        };
        assert_eq!(
            mac("0px space.md calc(space.sm * 2 + 1px) #00000040").unwrap(),
            format!(
                "0px {} {} #00000040",
                px("calc(space.md)"),
                px("calc(space.sm * 2 + 1px)")
            )
        );
        // A list, `inset`, a colour's parentheses and spaces inside them,
        // and a nested function: each kept or folded as one item.
        let text = "inset 0 min(space.md, max(2px, space.xs)) 3px rgb(0 0 0 / 40%),  0px 1px space.lg light-dark(#fff, #000)";
        assert_eq!(
            mac(text).unwrap(),
            format!(
                "inset 0 {} 3px rgb(0 0 0 / 40%),  0px 1px {} light-dark(#fff, #000)",
                px("min(space.md, max(2px, space.xs))"),
                px("calc(space.lg)")
            )
        );
        // Over no base name: the text itself.
        assert_eq!(mac("0px 1px 3px #000").as_deref(), Some("0px 1px 3px #000"));
        // The terminal's cells are px, as in a length's expression.
        assert_eq!(
            fold_lengths("0px space.md 1px #000", |n| of(n, Column::Terminal)
                .map(css)),
            Some(format!("0px {}px 1px #000", crate::style::cells::COLUMN))
        );
        // A colour function is kept whole, a base name inside it too, for
        // the codec to refuse (round 1, Grok 4).
        assert_eq!(
            mac("0px space.md 1px light-dark(space.md, #000)"),
            Some(format!(
                "0px {} 1px light-dark(space.md, #000)",
                px("calc(space.md)")
            ))
        );
        assert_eq!(
            mac("0px 1px 1px RGB(space.sm 0 0)").as_deref(),
            Some("0px 1px 1px RGB(space.sm 0 0)")
        );
        // Any function but a length's (round 2, Astra 1): a platform colour
        // whose asset is named for a base, a contrast colour; a length's
        // in any case.
        for text in [
            "0px 1px 2px -exact-platform-color(ios named:space.md, #000)",
            "0px 1px 2px contrast-color(space.md)",
        ] {
            assert_eq!(mac(text).as_deref(), Some(text));
        }
        assert!([
            "0",
            "4px",
            "-1.5px",
            "space.md",
            "calc(space.md * 2)",
            "MIN(1px, space.sm)"
        ]
        .iter()
        .all(|i| is_length_item(i)));
        assert!(![
            "#000",
            "black",
            "space.mdx",
            "-exact-platform-color(ios named:space.md, #000)",
            "rgb(0 0 0)",
            "1rem"
        ]
        .iter()
        .any(|i| is_length_item(i)));
        assert_eq!(
            mac("0px CALC(space.md) 1px #000"),
            Some(format!("0px {} 1px #000", px("calc(space.md)")))
        );
        // A base `base` gives none for, or one that doesn't fold: none.
        assert_eq!(fold_lengths("0px space.md 1px #000", |_| None), None);
        assert_eq!(
            fold_lengths("0px space.md 1px #000", |_| Some("1rem".into())),
            None
        );
    }

    // Stage 12 B3: a shadow entry over base names reads each base as an
    // alias does, the design's own first, every read recorded, and its
    // lengths fold to px in the process's column; a row that may not take a
    // varying token reads no varying base either.
    #[test]
    fn a_shadow_token_folds_its_lengths_in_the_process_column() {
        use crate::style::theme::{self, Kind};
        let shadow = |k: Kind, _| k == Kind::BoxShadow;
        let read = |entries: &[(&str, &str, Kind, bool)], takes: &dyn Fn(Kind, bool) -> bool| {
            let table = theme::kinded(
                entries
                    .iter()
                    .map(|(n, v, k, varies)| (*n, *v, false, *k, *varies)),
            );
            let _scope = theme::bind(&table);
            theme::recorded(|| theme::measure("card.shadow", takes).map(|(_, v)| v.to_string()))
        };
        let column = crate::style::metrics::column();
        let px = |expr: &str| {
            format!(
                "{}px",
                exact_num::Shortest32(evaluate(expr, column).unwrap())
            )
        };
        const SHADOW: &str = "0px space.md calc(space.sm * 2) #000000ff";
        let (value, names) = read(&[("card.shadow", SHADOW, Kind::BoxShadow, false)], &shadow);
        assert_eq!(
            value,
            Some(format!(
                "0px {} {} #000000ff",
                px("calc(space.md)"),
                px("calc(space.sm * 2)")
            ))
        );
        assert_eq!(
            names,
            ["card.shadow", "space.md", "space.sm"].map(Box::<str>::from)
        );
        assert!(crate::style::BoxShadows::parse(value.as_deref().unwrap()).is_some());
        // The design's own space.md first.
        let own = [
            ("card.shadow", SHADOW, Kind::BoxShadow, false),
            ("space.md", "40px", Kind::Length, false),
        ];
        assert_eq!(
            read(&own, &shadow).0,
            Some(format!("0px 40px {} #000000ff", px("calc(space.sm * 2)")))
        );
        // A varying base, where the row reads no varying token: no value.
        let varying = [
            ("card.shadow", SHADOW, Kind::BoxShadow, false),
            ("space.md", "40px", Kind::Length, true),
        ];
        assert_eq!(
            read(&varying, &|k, varies| shadow(k, varies) && !varies).0,
            None
        );
    }

    #[test]
    fn a_value_is_read_as_a_length_text() {
        assert_eq!(css(ScaleValue::Px(16.0)), "16px");
        assert_eq!(css(ScaleValue::Px(5.6)), "5.6px");
        assert_eq!(css(ScaleValue::Cells(2.0)), "2ch");
        assert_eq!(css(ScaleValue::Full), "9999px");
        assert_eq!(of("space.md", Column::Mac), Some(ScaleValue::Px(10.0)));
        assert_eq!(
            of("space.md", Column::Terminal),
            Some(ScaleValue::Cells(1.0))
        );
        assert_eq!(of("radius.full", Column::Ios), Some(ScaleValue::Full));
        assert_eq!(of("space.header", Column::Ios), None);
    }
    #[test]
    fn each_shipped_target_reads_its_column() {
        let all = [
            ("ios", "ios"),
            ("watchos", "ios"),
            ("macos", "mac"),
            ("web", "neutral"),
            ("linux", "neutral"),
            ("android", "material"),
            ("terminal", "terminal"),
        ];
        for (target, column) in all {
            assert_eq!(column_for(target), Some(column), "{target}");
        }
        assert_eq!(column_for("game"), None);
    }
}
