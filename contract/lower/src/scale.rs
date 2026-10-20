//! The base scale in lowering (LLP 1121 stage 11 B, theme2's condition (c)):
//! the compiler knows the nine base names (`exact_kernel::style::scale`), so
//! a view reading one compiles with no design defining it, as a role's name
//! does, and the kernel reads its length in the process's column. The plan
//! keeps nothing for one: the kernel holds every column.

use exact_kernel::style::{metrics::Column, scale};

thread_local! {
    /// Every token name the design declares, of any kind, while its styles
    /// are lowered: a base name in it is the design's, never the scale's.
    static DECLARED: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// The design's declared token names, for [`values`] (`theme::declare_theme`
/// and the lint's scope set them).
pub(crate) fn declare(names: Vec<String>) {
    DECLARED.with(|d| *d.borrow_mut() = names);
}

/// The names [`declare`] last set (the design server's write check puts
/// them back after its own).
pub(crate) fn declared() -> Vec<String> {
    DECLARED.with(|d| d.borrow().clone())
}

/// The base scale's values for `text` when it is a base name the design
/// does not define: each px column's length, distinct, as a token's values
/// over the tuples are (what a row checks). A base varies by column, never
/// by an app axis, so it is no `tamber-inherited-axis`. `None` for any
/// other text.
pub(crate) fn values(text: &str) -> Option<Vec<String>> {
    let text = text.trim();
    let row = scale::row(text)?;
    if DECLARED.with(|d| d.borrow().iter().any(|n| n == text)) {
        return None;
    }
    let mut values: Vec<String> = Vec::new();
    for (_, column) in scale::COLUMNS
        .iter()
        .filter(|(_, c)| *c != Column::Terminal)
    {
        let v = scale::css(scale::cell(row, *column).value);
        if !values.contains(&v) {
            values.push(v);
        }
    }
    Some(values)
}

/// A length token's value over base names the design leaves unset (LLP
/// 1121 D11, stage 12 B2): its px in each column, by the column's name, as
/// that column's host folds it (`scale::evaluate`, the terminal's cells as
/// px, which its spaces then round as every space there does). `None` for
/// any other text.
pub(crate) fn columns(value: &str) -> Option<Vec<(&'static str, f32)>> {
    let value = value.trim();
    if !value.contains('(') || scale::named_in(value).is_empty() {
        return None;
    }
    scale::COLUMNS
        .iter()
        .map(|(name, column)| Some((*name, scale::evaluate(value, *column)?)))
        .collect()
}

/// Why an expression [`unfolded`] names doesn't fold.
pub(crate) const UNFOLDED: &str =
    "past the kernel's `calc()` limits (32 values, 32 levels) or past the lengths a host holds";

/// The first column a length's expression over base names doesn't fold
/// in (past the kernel's `calc()` limits, `MAX_VALUES` terms): what the
/// build refuses, so no host meets a value it can't read. `None` for one
/// that folds in every column, or any other text.
pub(crate) fn unfolded(value: &str) -> Option<&'static str> {
    let value = value.trim();
    if !value.contains('(') || scale::named_in(value).is_empty() {
        return None;
    }
    scale::COLUMNS
        .iter()
        .find(|(_, column)| scale::evaluate(value, *column).is_none())
        .map(|(name, _)| *name)
}

/// [`columns`], and a bare base name the design leaves unset's too: what a
/// ranged param checks (stage 12 B2, theme2's "ranges included").
pub(crate) fn ranged(value: &str) -> Option<Vec<(&'static str, f32)>> {
    let value = value.trim();
    if !scale::is_base(value) {
        return columns(value);
    }
    let calc = format!("calc({value})");
    scale::COLUMNS
        .iter()
        .map(|(name, column)| Some((*name, scale::evaluate(&calc, *column)?)))
        .collect()
}

/// px as a length's text: `-2px`.
pub(crate) fn px(n: f32) -> String {
    format!("{}px", exact_num::Shortest32(n))
}

/// A token's values, each over base names as its columns' px (what a row
/// checks: every column's, as every tuple's), distinct.
pub(crate) fn expanded(values: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for v in values {
        let each = match columns(v) {
            Some(columns) => columns.into_iter().map(|(_, n)| px(n)).collect(),
            None => vec![v.clone()],
        };
        for v in each {
            if !out.contains(&v) {
                out.push(v);
            }
        }
    }
    out
}

/// Where among `values` (a token's, over its tuples) a length is `px`:
/// the columns an expression over base names folds to it in, said with
/// the expression (`on mac (`calc(space.md - 12px)`)`); `None` when no
/// expression does.
pub(crate) fn place(values: &[String], px_text: &str) -> Option<String> {
    values.iter().find_map(|v| {
        let on: Vec<&str> = columns(v)?
            .into_iter()
            .filter(|(_, n)| px(*n) == px_text)
            .map(|(name, _)| name)
            .collect();
        (!on.is_empty()).then(|| format!("on {} (`{}`)", on.join(", "), v.trim()))
    })
}

/// The base names' lengths outside Exact (the DTCG export, `tamber gen`'s
/// CSS), where no host's column applies: the neutral column's, the web's
/// look (round 2, Astra 2), px at the default root, from the kernel's table.
pub(crate) fn sizes() -> impl Iterator<Item = (String, String)> {
    scale::names().map(|n| {
        let px = match scale::of(n, Column::Neutral) {
            Some(scale::ScaleValue::Px(px)) => px,
            _ => scale::RADIUS_FULL,
        };
        (n.to_string(), exact_num::Shortest(px).to_string())
    })
}
