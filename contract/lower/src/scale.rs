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
