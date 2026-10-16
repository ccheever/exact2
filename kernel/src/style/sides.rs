//! CSS's one-to-four-value box shorthands: `padding`, `margin`, `inset`,
//! `border-width`, `border-style`, `border-color`, `scroll-padding`,
//! `scroll-margin` (CSS Box 4 §6, CSS Backgrounds 3 §4, CSS Position 3 §3.1,
//! CSS Scroll Snap 1 §6) and `border-radius` (CSS Backgrounds 3 §5.1). The
//! compiler splits a literal into its four rows; a computed one reaches
//! each row whole, and the runner and the web's JS target take each row's
//! value from it here, as the browser expands the shorthand.

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

/// The shorthand `row` is one of the rows of, and its index there.
pub fn family(row: StyleId) -> Option<(&'static [StyleId; 4], usize)> {
    FAMILIES
        .iter()
        .find_map(|f| f.iter().position(|&r| r == row).map(|i| (f, i)))
}

/// A shorthand's values, split where CSS splits them: at white space
/// outside parentheses, so `light-dark(#fff, #000)` and `calc(50% - 4px)`
/// are one value.
pub fn values(text: &str) -> Vec<&str> {
    let (mut values, mut depth, mut start) = (Vec::new(), 0usize, None);
    for (i, c) in text.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            c if c.is_whitespace() && depth == 0 => {
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

/// The value row `index` of a shorthand takes from `text` when it lists two
/// to four values. `None` for one value, which is every row's whole, and
/// for more than four, which CSS refuses whole.
pub fn side(text: &str, index: usize) -> Option<&str> {
    let v = values(text);
    (2..=4).contains(&v.len()).then(|| v[pick(v.len(), index)])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_row_takes_its_side_as_css_expands_the_shorthand() {
        let sides = |t| [0, 1, 2, 3].map(|i| side(t, i));
        assert_eq!(sides("16px"), [None; 4]);
        assert_eq!(sides("16px 0"), ["16px", "0", "16px", "0"].map(Some));
        assert_eq!(
            sides(" 1px  2px 3px "),
            ["1px", "2px", "3px", "2px"].map(Some)
        );
        assert_eq!(
            sides("8px 8px 0 calc(50% - 4px)"),
            ["8px", "8px", "0", "calc(50% - 4px)"].map(Some)
        );
        assert_eq!(sides("1px 2px 3px 4px 5px"), [None; 4]);
        assert_eq!(
            family(StyleId::BorderRadiusBottomLeft),
            Some((&FAMILIES[8], 3))
        );
        assert_eq!(family(StyleId::Width), None);
    }
}
