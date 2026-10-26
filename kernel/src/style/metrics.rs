//! The looks Exact draws itself, as data (LLP 1115.002 D6, D8.4; LLP
//! 1115.000 D17): the schema's `metrics`, each a name, a kind and its value
//! in each column.
//!
//! A `style` row is one a sheet the compiler writes (the grouped list's,
//! `contract/lower/src/grouped.rs`) names, `-exact-<name>`, where it used to
//! hold a number. The kernel reads the name as its value in this process's
//! column when the row is set: a number, a keyword, a text style or a colour
//! role by its own name, or `unset`, which leaves the row unset. So every
//! reader and the wire see the value, as they see the pixels of a `rem`, and
//! a column does not change while a process runs, so nothing is resolved
//! again. No author writes such a name: lowering refuses it in an authored
//! value, and the runner refuses it on a row its author wrote, whatever
//! built the value (`exact_runner::bridge::set_binding_style`).
//!
//! A `length` or `number` row is one code reads ([`length`], [`number`]): the
//! size of a control no host measured (`ControlKind::default_size`), and the
//! shapes a host paints where it has no platform control to show.
//!
//! The column is the host's to say, once, before it boots a plan. An Apple
//! host says its platform ([`set_column`]). A process that shows the neutral
//! look says so with the role table's switch (`roles::set_column`), the one
//! switch for that look's colours and its metrics: every web build (the wasm
//! host and the JS target), so the browser reads the neutral column, and a
//! host that paints its own controls (D17). Android's reader reads its own
//! Material column (LLP 1115.001.001), with `web` only as that column's
//! fallback for a row it lacks. The `web` column is read only by a process
//! that says no column (headless and test runs; D17 as corrected).
//! The terminal's column (LLP 1121 stage 11 B) is the `web` column's in
//! every metric, as it read before it had one; it differs in the base scale
//! (`style::scale`), whose spacings it reads in cells.
//! @ref LLP 1115.002 D6, D8.4
//! @ref LLP 1115.000 D17

use crate::generated::{Metric, MetricKind, MetricValue, StyleCodec, StyleId, METRICS};
use crate::style::{roles, StyleValue};
use std::sync::atomic::{AtomicU8, Ordering};

/// The metric table's columns: a platform or a look whose shapes Exact draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Column {
    /// The `web` column (LLP 1116 D1's first look): read only by a process
    /// that says no column (headless and test runs), and as Material's
    /// fallback for a row it lacks. The browser reads [`Column::Neutral`]
    /// (every web build throws the neutral switch); the base scale has no
    /// web column and reads the neutral cell here (`style::scale`).
    Web,
    /// iOS's (and tvOS's, visionOS's and watchOS's, which draw as it does).
    Ios,
    /// The Mac's.
    Mac,
    /// The neutral look's (LLP 1115.000 D4 part B, D17): the browser's (every
    /// web build), and a host's that paints its own controls.
    Neutral,
    /// Android's Material look (LLP 1115.001.001 D2.4), which Android's
    /// reader reads: Compose Material 3's values at the pinned version, and
    /// the `web` column's where a row has none of its own.
    Material,
    /// The terminal's (LLP 1121 stage 11 B), which its host says: the `web`
    /// column's in every metric (as it read before it had a column), and
    /// its own in the base scale (cells; no corners).
    Terminal,
}

/// The column this process said. One platform a process, so one switch.
static COLUMN: AtomicU8 = AtomicU8::new(0);

/// Say which column this process shows: an Apple host's platform, once,
/// before it boots a plan. A row set before it keeps the value it was read
/// as.
pub fn set_column(column: Column) {
    let index = match column {
        Column::Web => 0,
        Column::Ios => 1,
        Column::Mac => 2,
        Column::Neutral => 3,
        Column::Material => 4,
        Column::Terminal => 5,
    };
    COLUMN.store(index, Ordering::Release);
}

/// The column this process shows: the one it said ([`set_column`]), and
/// where it said none, the neutral look's when its colours are that look's
/// (`roles::set_column`).
pub fn column() -> Column {
    match COLUMN.load(Ordering::Acquire) {
        1 => Column::Ios,
        2 => Column::Mac,
        3 => Column::Neutral,
        4 => Column::Material,
        5 => Column::Terminal,
        _ if roles::column() == roles::Column::Neutral => Column::Neutral,
        _ => Column::Web,
    }
}

/// The metric `text` names, written `-exact-<name>` exactly: a compiler
/// wrote it, so it has no other spelling. Only a `style` row has a name a
/// sheet can write.
pub fn named(text: &str) -> Option<&'static Metric> {
    let name = text.strip_prefix("-exact-")?;
    METRICS
        .iter()
        .find(|m| m.kind == MetricKind::Style && m.name == name)
}

/// A metric's value in a column.
pub fn value(metric: &Metric, column: Column) -> MetricValue {
    match column {
        Column::Web => metric.web,
        Column::Ios => metric.ios,
        Column::Mac => metric.macos,
        Column::Neutral => metric.neutral,
        Column::Material => metric.material,
        Column::Terminal => metric.terminal,
    }
}

/// A `length` metric in this process's column, in pixels: `index` is its
/// place in the table (`generated::metric`), `em` the font size of the
/// control it is read for and `rem` the root's. `None` where the column has
/// no value: a look that leaves the size to its host.
pub fn length(index: usize, em: f32, rem: f32) -> Option<f32> {
    length_in(index, column(), em, rem)
}

/// [`length`] in a column.
pub fn length_in(index: usize, column: Column, em: f32, rem: f32) -> Option<f32> {
    let metric = &METRICS[index];
    debug_assert_eq!(metric.kind, MetricKind::Length, "{}", metric.name);
    match value(metric, column) {
        MetricValue::Number(px) => Some(px as f32),
        MetricValue::Em(n) => Some(n as f32 * em),
        MetricValue::Rem(n) => Some(n as f32 * rem),
        MetricValue::Text(_) => None,
    }
}

/// A `number` metric in this process's column: a ratio, a weight or an
/// opacity. `None` where the column has no value.
pub fn number(index: usize) -> Option<f32> {
    number_in(index, column())
}

/// [`number`] in a column.
pub fn number_in(index: usize, column: Column) -> Option<f32> {
    let metric = &METRICS[index];
    debug_assert_eq!(metric.kind, MetricKind::Number, "{}", metric.name);
    match value(metric, column) {
        MetricValue::Number(n) => Some(n as f32),
        _ => None,
    }
}

/// The CSS text `text` stands for when it names a metric whose value in
/// this process's column is text (a keyword, a role, `unset`).
pub fn text(text: &str) -> Option<&'static str> {
    match value(named(text)?, column()) {
        MetricValue::Text(t) => Some(t),
        _ => None,
    }
}

/// What row `id` is given when `value` names a metric: `None` when it names
/// none; `Some(None)` when the metric has no row in this process's column
/// (`unset`); else its value there, as a producer would have written it.
pub fn of(id: StyleId, value: &StyleValue) -> Option<Option<StyleValue<'static>>> {
    let StyleValue::Text(text) = value else {
        return None;
    };
    let metric = named(text)?;
    let column = column();
    let mut value = self::value(metric, column);
    if column == Column::Mac {
        // These measured AppKit surfaces differ from the roles' defaults;
        // a theme replaces them without changing an unthemed form.
        let role = match metric.name {
            "grouped-card-fill" => Some("-exact-secondary-grouped-background"),
            "grouped-separator" => Some("-exact-separator"),
            _ => None,
        };
        if let Some(role) =
            role.filter(|name| roles::role(name).and_then(roles::overridden).is_some())
        {
            value = MetricValue::Text(role);
        }
    }
    Some(as_written(id, value))
}

/// A `style` metric's value as the value a producer hands a row (the
/// runner's bridge, the compiler): `auto` on a dimension row and a lone
/// percentage are their own kinds; `unset` is no row.
pub fn as_written(id: StyleId, value: MetricValue) -> Option<StyleValue<'static>> {
    Some(match value {
        MetricValue::Number(n) => StyleValue::Number(n),
        MetricValue::Text("unset") => return None,
        MetricValue::Text("auto") if id.codec() == StyleCodec::Dimension => StyleValue::Auto,
        MetricValue::Text(t) => match t.strip_suffix('%').and_then(|p| p.parse::<f64>().ok()) {
            Some(percent) => StyleValue::Percent(percent),
            None => StyleValue::Text(t.into()),
        },
        // A `length` row's, and no sheet names a `length` row.
        MetricValue::Em(_) | MetricValue::Rem(_) => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generated::StyleProps;

    fn metric(name: &str) -> &'static Metric {
        named(&format!("-exact-{name}")).unwrap_or_else(|| panic!("{name}"))
    }

    #[test]
    fn a_metric_is_named_exactly_and_nothing_else_is_one() {
        assert_eq!(
            metric("grouped-row-min-height").name,
            "grouped-row-min-height"
        );
        for other in [
            "grouped-row-min-height",
            "-EXACT-grouped-row-min-height",
            " -exact-grouped-row-min-height",
            "-exact-grouped-row-min-height ",
            "-exact-hairline",
            "-exact-body",
            "-exact-label",
            "-exact-",
            "",
        ] {
            assert!(named(other).is_none(), "{other:?}");
        }
    }

    /// iOS's and the web's columns are the grouped sheet's numbers as they
    /// stood before they were data (LLP 1115.002 D6: neither moves), and the
    /// Mac's are the measured form's.
    #[test]
    fn the_columns_hold_each_platforms_grouped_form() {
        use MetricValue::{Number, Text};
        for (name, ios, mac) in [
            ("grouped-row-min-height", Number(52.0), Number(36.0)),
            ("grouped-separator-end", Number(0.0), Number(10.0)),
            ("grouped-row-padding-block", Text("unset"), Number(10.0)),
            ("grouped-card-radius", Number(26.0), Number(10.0)),
            ("grouped-card-inset", Number(16.0), Number(20.0)),
            ("grouped-section-max-width", Text("unset"), Number(744.0)),
            ("grouped-section-top", Number(35.0), Number(10.0)),
            ("grouped-section-top-first", Number(35.33), Number(20.0)),
            ("grouped-header-weight", Number(600.0), Number(700.0)),
            (
                "grouped-footer-font",
                Text("-exact-footnote"),
                Text("-exact-subheadline"),
            ),
            (
                "grouped-card-fill",
                Text("light-dark(#ffffff, #1c1c1e)"),
                Text("-exact-platform-color(macos exactGroupedCardColor, light-dark(#f9f9f9, #272727))"),
            ),
            (
                "grouped-destructive-color",
                Text("-exact-system-red"),
                Text("-exact-label"),
            ),
        ] {
            let m = metric(name);
            assert_eq!(value(m, Column::Ios), ios, "{name} on iOS");
            assert_eq!(value(m, Column::Web), ios, "{name} on the web");
            assert_eq!(value(m, Column::Mac), mac, "{name} on the Mac");
        }
        let MetricValue::Text(card) = value(metric("grouped-card-fill"), Column::Mac) else {
            panic!("the Mac card is a surface colour")
        };
        let card = crate::ColorValue::parse_light_dark(card).unwrap();
        assert_eq!(card.resolve(false), crate::style::Color(0xf9f9_f9ff));
        assert_eq!(card.resolve(true), crate::style::Color(0x2727_27ff));
        assert!(matches!(card, crate::ColorValue::Platform(_)));
        // The first web look keeps iOS's metrics except title-heading
        // emphasis, which native text styles supply themselves.
        for m in METRICS {
            let emphasis = match m.name {
                "heading-title-weight" => Some(700.0),
                "heading-small-title-weight" => Some(600.0),
                _ => None,
            };
            if let Some(weight) = emphasis {
                assert_eq!(
                    [m.ios, m.macos, m.material],
                    [Text("unset"); 3],
                    "{}",
                    m.name
                );
                assert_eq!(
                    [m.web, m.neutral, m.terminal],
                    [Number(weight); 3],
                    "{}",
                    m.name
                );
            } else {
                assert_eq!(m.web, m.ios, "{}", m.name);
            }
        }
        // The neutral look's grouped list is that one on the neutral
        // grounds, with a surface's corners (LLP 1115.000 D4 part B): what
        // the page's sheet draws (host/web/neutral.css).
        for m in METRICS.iter().filter(|m| m.kind == MetricKind::Style) {
            let neutral = match m.name {
                "grouped-ground" => Text("-exact-grouped-background"),
                "grouped-ground-plain" => Text("-exact-background"),
                "grouped-card-radius" => Number(12.0),
                "header-ground" => Text("-exact-background"),
                "header-color" => Text("-exact-label"),
                "header-edge-width" => Number(1.0),
                "header-edge-style" => Text("solid"),
                "header-edge-color" => Text("-exact-separator"),
                "header-min-height" => Number(32.0),
                "header-padding-block" => Number(6.0),
                "header-padding-inline" => Number(16.0),
                "header-position" => Text("sticky"),
                "header-top" => Number(0.0),
                _ => m.web,
            };
            assert_eq!(m.neutral, neutral, "{}", m.name);
            if m.name.starts_with("header-") {
                assert_eq!([m.web, m.ios, m.macos], [Text("unset"); 3], "{}", m.name);
            }
        }
    }

    /// A row code reads (LLP 1115.000 D17). The neutral look sizes a mark by
    /// its control's font size; every other column holds the sizes the
    /// kernel had before they were rows, so no host that is not neutral
    /// moves. A row of the neutral look alone has no value elsewhere.
    #[test]
    fn a_length_is_read_in_a_column_at_a_font_size() {
        use crate::generated::metric;
        for (index, old, neutral) in [
            (metric::MARK_SIZE, 13.0, 20.0),
            (metric::SWITCH_WIDTH, 38.0, 36.0),
            (metric::SWITCH_HEIGHT, 22.0, 20.0),
            (metric::RANGE_HEIGHT, 16.0, 20.0),
        ] {
            for column in [Column::Web, Column::Ios, Column::Mac] {
                assert_eq!(length_in(index, column, 16.0, 16.0), Some(old));
                assert_eq!(length_in(index, column, 32.0, 16.0), Some(old));
            }
            assert_eq!(length_in(index, Column::Neutral, 16.0, 16.0), Some(neutral));
            assert_eq!(
                length_in(index, Column::Neutral, 32.0, 16.0),
                Some(neutral * 2.0)
            );
        }
        assert_eq!(
            length_in(metric::CONTROL_HEIGHT, Column::Neutral, 16.0, 20.0),
            Some(36.0)
        );
        assert_eq!(
            length_in(metric::TABBAR_HEIGHT, Column::Neutral, 12.0, 20.0),
            Some(70.0),
            "a rem follows the root"
        );
        assert_eq!(
            length_in(metric::CONTROL_HEIGHT, Column::Web, 16.0, 16.0),
            None
        );
        // This process shows the web's first look, where the neutral look's
        // own rows have no value and the kernel's sizes are the old ones.
        assert_eq!(column(), Column::Web);
        assert_eq!(length(metric::MARK_SIZE, 16.0, 16.0), Some(13.0));
        assert_eq!(number(metric::CONTROL_LINE_ALPHA), None);
        assert_eq!(
            METRICS[metric::CONTROL_LINE_ALPHA].neutral,
            MetricValue::Number(0.45)
        );
        // No sheet names a row code reads.
        assert!(named("-exact-mark-size").is_none());
        assert!(named("-exact-control-height").is_none());
    }

    /// The rows the grouped sheet writes a `style` metric on, by its name, as
    /// the row test found them before theme2's unit flag: the oracle the
    /// flag is held to ([`the_length_marker_is_the_names_length_rows`]).
    fn written_on(name: &str) -> &'static [StyleId] {
        use StyleId::*;
        match name {
            n if n.ends_with("-fill") || n.contains("-ground") => &[BackgroundColor],
            "grouped-separator" => &[BorderColorBottom, BorderColorTop],
            "header-edge-width" => &[BorderWidthBottom],
            "header-edge-style" => &[BorderStyleBottom],
            "header-edge-color" => &[BorderColorBottom],
            "header-position" => &[PositionType],
            "header-top" => &[Top],
            n if n.ends_with("-color") => &[TextColor],
            n if n.ends_with("-tint") => &[TintColor],
            "grouped-section-width" => &[Width],
            "grouped-section-max-width" => &[MaxWidth],
            "grouped-footer-font" => &[FontSize],
            n if n.ends_with("-weight") => &[FontWeight],
            n if n.ends_with("-radius") => &[BorderRadiusTopLeft],
            "grouped-edge-width" => &[BorderWidthTop],
            n if n.ends_with("-gap") => &[RowGap, ColumnGap],
            n if n.ends_with("-min-height") => &[MinHeight],
            n if n.ends_with("-height") => &[Height],
            n if n.ends_with("-width") || n.ends_with("-size") && !n.contains("switch") => &[Width],
            "grouped-switch-size" => &[ControlSize],
            "grouped-field-align" | "grouped-title-button-align" => &[TextAlign],
            "grouped-button-appearance" => &[Appearance],
            "grouped-button-box-sizing" => &[BoxSizing],
            "grouped-button-title-grow" => &[FlexGrow],
            "grouped-button-align" => &[AlignSelf],
            "grouped-value-wrap" => &[FlexWrap],
            n if n.contains("padding") => &[PaddingTop, PaddingLeft],
            _ => &[MarginTop, MarginLeft],
        }
    }

    /// The length properties among [`written_on`]'s.
    const LENGTH_ROWS: [StyleId; 14] = {
        use StyleId::*;
        [
            Top,
            Width,
            MaxWidth,
            MinHeight,
            Height,
            RowGap,
            ColumnGap,
            BorderRadiusTopLeft,
            BorderWidthTop,
            BorderWidthBottom,
            PaddingTop,
            PaddingLeft,
            MarginTop,
            MarginLeft,
        ]
    };

    /// theme2's unit flag (LLP 1121 stage 11 B): the `style` rows the schema
    /// marks as lengths (`style length`, [`Metric::length`]) are exactly
    /// those the names said are written on a length property, so the marker
    /// moves nothing the kernel reads.
    #[test]
    fn the_length_marker_is_the_names_length_rows() {
        let mut marked = 0;
        for m in METRICS.iter().filter(|m| m.kind == MetricKind::Style) {
            let by_name = written_on(m.name).iter().all(|r| LENGTH_ROWS.contains(r));
            assert_eq!(m.length, by_name, "{}", m.name);
            marked += usize::from(m.length);
        }
        assert_eq!(marked, 61);
        // A `length` row is a length too; a `number` row is not.
        assert!(METRICS
            .iter()
            .all(|m| m.length == (m.kind == MetricKind::Length) || m.kind == MetricKind::Style));
    }

    /// Every value of every column is one its kind of row takes: a metric
    /// that a row refused would leave a hole in one platform's sheet only.
    /// Whether a row is a length is the schema's marker; which length (or
    /// other) property it is written on, its name's.
    #[test]
    fn every_value_is_one_a_row_takes() {
        for m in METRICS.iter().filter(|m| m.kind == MetricKind::Style) {
            let rows = written_on(m.name);
            assert_eq!(
                rows.iter().all(|r| LENGTH_ROWS.contains(r)),
                m.length,
                "{}: the marker and its row",
                m.name
            );
            for (column, cell) in [
                ("iOS", m.ios),
                ("the Mac", m.macos),
                ("the web", m.web),
                ("the neutral look", m.neutral),
            ] {
                for &row in rows {
                    let Some(written) = as_written(row, cell) else {
                        continue;
                    };
                    let mut style = StyleProps::default();
                    assert!(
                        style.set_dynamic(row, &written).is_ok(),
                        "{} on {column}: {row:?} refuses {cell:?}",
                        m.name
                    );
                    assert!(style.mask.has(row), "{} on {column}", m.name);
                }
            }
        }
    }

    /// The web's column is the default, and `set_dynamic` reads a name as
    /// its value there: the kernel's other tests run beside this one, so the
    /// switch is thrown in a binary of its own (`tests/metrics.rs`).
    #[test]
    fn a_row_given_a_metric_holds_its_value() {
        assert_eq!(column(), Column::Web);
        let name = |n: &str| StyleValue::Text(format!("-exact-{n}").into());
        let mut style = StyleProps::default();
        style
            .set_dynamic(StyleId::MinHeight, &name("grouped-row-min-height"))
            .unwrap();
        assert_eq!(style.min_height, crate::Dimension::Points(52.0));
        assert!(style.relative.is_empty(), "a value, not a unit");
        style
            .set_dynamic(StyleId::FontSize, &name("grouped-footer-font"))
            .unwrap();
        assert_eq!(
            style.relative.get(StyleId::FontSize),
            crate::style::relative::parse("-exact-footnote"),
            "a text style, which still follows the root font size"
        );
        // No row in this column: nothing is set, and it reads as `unset`.
        style
            .set_dynamic(StyleId::MaxWidth, &name("grouped-section-max-width"))
            .unwrap();
        assert!(!style.mask.has(StyleId::MaxWidth));
        assert!(name("grouped-section-max-width").unsets(StyleId::MaxWidth));
        assert!(!name("grouped-row-min-height").unsets(StyleId::MinHeight));
        assert_eq!(text("-exact-grouped-row-min-height"), None);
        assert_eq!(text("-exact-grouped-separator"), Some("-exact-separator"));
    }
}
