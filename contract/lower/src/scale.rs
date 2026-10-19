//! The base scale in lowering (LLP 1121 stage 11 B, theme2's condition (c)):
//! the compiler knows the nine base names (`exact_kernel::style::scale`), so
//! a view reading one compiles with no design defining it, as a role's name
//! does, and the kernel reads its length in the process's column. The plan
//! keeps nothing for one: the kernel holds every column.

use exact_kernel::style::{metrics::Column, scale};
use exact_tamber as tamber;

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

/// `tamber-base-kind`: a base name the design declares as anything but a
/// length token (a colour, a number, an icon), each `(line, col,
/// message)`. A base name is a length, app-wide, as the scale's own; an icon
/// under one would hide it while a row reads the base (round 1).
pub(crate) fn misnamed(folding: &tamber::Folding) -> Vec<(u32, u32, String)> {
    let mut out = Vec::new();
    for (t, declared) in folding.tokens.iter().zip(&folding.design.tokens) {
        let name = &declared.name;
        if scale::is_base(&name.value) && t.kind != tamber::Kind::Length {
            out.push((name.line, name.col, format!("`{0}` is a base length (the scale's `{0}`): an app sets it to a length (`{0} = 12px`), not a {1}", name.value, t.kind.name())));
        }
    }
    // An icon (a dial's name is an identifier, so no dial is one).
    for name in folding.design.icons.iter().map(|t| &t.name) {
        if scale::is_base(&name.value) {
            out.push((name.line, name.col, format!("`{}` is a base length (the scale's): an app sets it as a length token, not an icon", name.value)));
        }
    }
    out
}
