//! @ref LLP 1019 D9 — `font-variation-settings` as a style row:
//! `[["wght",650],…]`, the axes in order (FontAxes.swift reads it).
use exact_kernel::style::FontVariations;

/// Writes the row's value; `false` (nothing written) for `normal`.
pub(super) fn font_axes_json(out: &mut String, v: &FontVariations) -> bool {
    if v.is_empty() {
        return false;
    }
    out.push('[');
    for (i, (tag, value)) in v.axes().iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str("[\"");
        let tag = tag.to_be_bytes().map(char::from);
        out.extend(tag.iter().filter(|c| !matches!(c, '"' | '\\')));
        out.push_str("\",");
        super::push_num(out, *value);
        out.push(']');
    }
    out.push(']');
    true
}
