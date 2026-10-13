//! The looks Exact draws itself, as data (LLP 1115.002 D6, D8.4): the
//! schema's `metrics`, each a name and its value a platform. A sheet the
//! compiler writes (the grouped list's, `contract/lower/src/grouped.rs`)
//! names a metric, `-exact-<name>`, where it used to hold a number, and the
//! kernel reads the name as its value in this process's column when the row
//! is set: a number, a keyword, a text style or a colour role by its own
//! name, or `unset`, which leaves the row unset. So every reader and the
//! wire see the value, as they see the pixels of a `rem`, and a platform
//! does not change while a process runs, so nothing is resolved again.
//!
//! The column is the host's to say, once, before it boots a plan
//! ([`set_column`]), as the colour roles' is (`roles::set_column`). Until a
//! host says, it is the web's: what a browser shows, and what a host that
//! paints its own controls shows (LLP 1115.000 D17).
//!
//! No author writes a metric's name: lowering refuses it in an authored
//! value, and the runner refuses it on a row its author wrote, whatever
//! built the value (`exact_runner::bridge::set_binding_style`).
//! @ref LLP 1115.002 D6, D8.4

use crate::generated::{Metric, MetricValue, StyleCodec, StyleId, METRICS};
use crate::style::StyleValue;
use std::sync::atomic::{AtomicU8, Ordering};

/// The metric table's columns: one a platform whose look Exact draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Column {
    /// The web's, and a host's that paints its own controls (D17).
    Web,
    /// iOS's (and tvOS's and visionOS's, which draw as it does).
    Ios,
    /// The Mac's.
    Mac,
}

/// The column this process shows. One platform a process, so one switch.
static COLUMN: AtomicU8 = AtomicU8::new(0);

/// Say which column this process shows: a host's, once, before it boots a
/// plan. A row set before it keeps the value it was read as.
pub fn set_column(column: Column) {
    let index = match column {
        Column::Web => 0,
        Column::Ios => 1,
        Column::Mac => 2,
    };
    COLUMN.store(index, Ordering::Release);
}

/// The column this process shows ([`set_column`]).
pub fn column() -> Column {
    match COLUMN.load(Ordering::Acquire) {
        1 => Column::Ios,
        2 => Column::Mac,
        _ => Column::Web,
    }
}

/// The metric `text` names, written `-exact-<name>` exactly: a compiler
/// wrote it, so it has no other spelling.
pub fn named(text: &str) -> Option<&'static Metric> {
    let name = text.strip_prefix("-exact-")?;
    METRICS.iter().find(|m| m.name == name)
}

/// A metric's value in a column.
pub fn value(metric: &Metric, column: Column) -> MetricValue {
    match column {
        Column::Web => metric.web,
        Column::Ios => metric.ios,
        Column::Mac => metric.macos,
    }
}

/// The CSS text `text` stands for when it names a metric whose value in
/// this process's column is text (a keyword, a role, `unset`).
pub fn text(text: &str) -> Option<&'static str> {
    match value(named(text)?, column()) {
        MetricValue::Text(t) => Some(t),
        MetricValue::Number(_) => None,
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
    Some(as_written(id, self::value(metric, column())))
}

/// A metric's value as the value a producer hands a row (the runner's
/// bridge, the compiler): `auto` on a dimension row and a lone percentage
/// are their own kinds; `unset` is no row.
pub fn as_written(id: StyleId, value: MetricValue) -> Option<StyleValue<'static>> {
    Some(match value {
        MetricValue::Number(n) => StyleValue::Number(n),
        MetricValue::Text("unset") => return None,
        MetricValue::Text("auto") if id.codec() == StyleCodec::Dimension => StyleValue::Auto,
        MetricValue::Text(t) => match t.strip_suffix('%').and_then(|p| p.parse::<f64>().ok()) {
            Some(percent) => StyleValue::Percent(percent),
            None => StyleValue::Text(t.into()),
        },
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
            ("grouped-row-min-height", Number(52.0), Number(16.0)),
            ("grouped-row-padding-block", Text("unset"), Number(10.0)),
            ("grouped-card-radius", Number(26.0), Number(10.0)),
            ("grouped-card-inset", Number(16.0), Number(20.0)),
            ("grouped-section-max-width", Text("unset"), Number(744.0)),
            ("grouped-section-top", Number(17.33), Number(10.0)),
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
                Text("light-dark(#f9f9f9, #272727)"),
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
        // Until the web's look is its own, its column is iOS's whole: the
        // surfaces too, which stay the sheet's pairs there. As roles the
        // ground would be a device's palette on Android, which reads this
        // column, under cards that stayed iOS's.
        for m in METRICS {
            assert_eq!(m.web, m.ios, "{}", m.name);
        }
    }

    /// Every value of every column is one its kind of row takes: a metric
    /// that a row refused would leave a hole in one platform's sheet only.
    #[test]
    fn every_value_is_one_a_row_takes() {
        // The rows the grouped sheet writes each kind of metric on.
        let rows = |name: &str| -> &'static [StyleId] {
            use StyleId::*;
            match name {
                n if n.ends_with("-fill") || n.contains("-ground") => &[BackgroundColor],
                "grouped-separator" => &[BorderColorBottom, BorderColorTop],
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
                n if n.ends_with("-width") || n.ends_with("-size") && !n.contains("switch") => {
                    &[Width]
                }
                "grouped-switch-size" => &[ControlSize],
                "grouped-field-align" => &[TextAlign],
                "grouped-button-appearance" => &[Appearance],
                "grouped-button-title-grow" => &[FlexGrow],
                "grouped-button-align" => &[AlignSelf],
                n if n.contains("padding") => &[PaddingTop, PaddingLeft],
                _ => &[MarginTop, MarginLeft],
            }
        };
        for m in METRICS {
            for (column, cell) in [("iOS", m.ios), ("the Mac", m.macos), ("the web", m.web)] {
                for &row in rows(m.name) {
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
