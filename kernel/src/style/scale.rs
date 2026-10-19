//! The base scale (LLP 1121 stage 11 B; the schema's `scale`): nine names an
//! app's views read with no design defining them, `space.xs` to `space.xl`
//! and `radius.sm` to `radius.full`, each a length in this process's column.
//!
//! - The token lookup falls back to them (`theme::measure`): a design that
//!   defines one wins, app-wide, as any token does (ruling 4); one it leaves
//!   unset reads this table. The compiler knows the nine names, so a view
//!   reading one compiles with no design (`contract_lower::theme`), and the
//!   plan keeps nothing for them: the kernel already holds every column.
//! - Every cell names its source (a metrics row, a measurement or a
//!   guideline, `docs/scale-measured.md`), which the build resolves. This is
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

/// A cell's source as words: `metric grouped-row-gap`, `measured <id>`,
/// `guideline <id>`, `full`, `terminal cells`.
pub fn source(cell: ScaleCell) -> String {
    match cell.source {
        ScaleSource::Metric(m) => format!("metric {m}"),
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
                    ScaleSource::Metric(m) => {
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
