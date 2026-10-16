//! CSS's one-to-four-value box shorthands: `padding`, `margin`, `inset`,
//! `border-width`, `border-style`, `border-color`, `scroll-padding`,
//! `scroll-margin` (CSS Box 4 §6, CSS Backgrounds 3 §4, CSS Position 3 §3.1,
//! CSS Scroll Snap 1 §6) and `border-radius` (CSS Backgrounds 3 §5.1). The
//! compiler splits a literal into its four rows, as the browser expands the
//! shorthand, and refuses a computed list (LLP 1001 §1): no row splits one.

use crate::generated::StyleId;

/// The shorthands' rows, in top, right, bottom, left order, and
/// `border-radius`'s corners in top-left, top-right, bottom-right,
/// bottom-left order, which CSS fills from fewer values the same way.
pub const FAMILIES: [[StyleId; 4]; 9] = [
    [
        StyleId::PaddingTop,
        StyleId::PaddingRight,
        StyleId::PaddingBottom,
        StyleId::PaddingLeft,
    ],
    [
        StyleId::MarginTop,
        StyleId::MarginRight,
        StyleId::MarginBottom,
        StyleId::MarginLeft,
    ],
    [
        StyleId::BorderWidthTop,
        StyleId::BorderWidthRight,
        StyleId::BorderWidthBottom,
        StyleId::BorderWidthLeft,
    ],
    [
        StyleId::BorderStyleTop,
        StyleId::BorderStyleRight,
        StyleId::BorderStyleBottom,
        StyleId::BorderStyleLeft,
    ],
    [
        StyleId::BorderColorTop,
        StyleId::BorderColorRight,
        StyleId::BorderColorBottom,
        StyleId::BorderColorLeft,
    ],
    [StyleId::Top, StyleId::Right, StyleId::Bottom, StyleId::Left],
    [
        StyleId::ScrollPaddingTop,
        StyleId::ScrollPaddingRight,
        StyleId::ScrollPaddingBottom,
        StyleId::ScrollPaddingLeft,
    ],
    [
        StyleId::ScrollMarginTop,
        StyleId::ScrollMarginRight,
        StyleId::ScrollMarginBottom,
        StyleId::ScrollMarginLeft,
    ],
    [
        StyleId::BorderRadiusTopLeft,
        StyleId::BorderRadiusTopRight,
        StyleId::BorderRadiusBottomRight,
        StyleId::BorderRadiusBottomLeft,
    ],
];

/// A shorthand's values, split where CSS splits them: at white space (CSS
/// Syntax 3 §4.2: space, tab and the three newlines, never U+0085 or a
/// no-break space) outside parentheses, so `light-dark(#fff, #000)` and
/// `calc(50% - 4px)` are one value.
pub fn values(text: &str) -> Vec<&str> {
    let (mut values, mut depth, mut start) = (Vec::new(), 0usize, None);
    for (i, c) in text.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            ' ' | '\t' | '\n' | '\r' | '\u{c}' if depth == 0 => {
                if let Some(s) = start.take() {
                    values.push(&text[s..i]);
                }
                continue;
            }
            _ => {}
        }
        start.get_or_insert(i);
    }
    if let Some(s) = start {
        values.push(&text[s..]);
    }
    values
}

/// Which of `count` values (one to four) row `index` takes. CSS: top;
/// right = top; bottom = top; left = right.
pub fn pick(count: usize, index: usize) -> usize {
    match (count, index) {
        (0 | 1, _) => 0,
        (2, i) => i % 2,
        (3, 3) => 1,
        (_, i) => i,
    }
}

/// Whether `text` has a `/` outside parentheses: `border-radius`'s
/// elliptical form (`10px / 20px`, `10px/20px`), which no row holds. One
/// inside a function (`calc(10px / 2)`) is the function's.
pub fn elliptical(text: &str) -> bool {
    let mut depth = 0usize;
    text.chars().any(|c| {
        match c {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            _ => {}
        }
        c == '/' && depth == 0
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_split_at_css_white_space_outside_parentheses() {
        assert_eq!(values(" 1px  2px\t3px "), ["1px", "2px", "3px"]);
        assert_eq!(values("8px calc(50% - 4px)"), ["8px", "calc(50% - 4px)"]);
        // CSS white space only: U+0085 and a no-break space are in a value.
        assert_eq!(values("1px\u{85}2px\u{a0}3px").len(), 1);
        assert_eq!(values("1px\r\n\u{c}2px"), ["1px", "2px"]);
        assert_eq!([0, 1, 2, 3].map(|i| pick(3, i)), [0, 1, 2, 1]);
        assert!(elliptical("10px / 20px") && elliptical("10px/20px"));
        assert!(!elliptical("calc(20px / 2) 0"));
    }
}
