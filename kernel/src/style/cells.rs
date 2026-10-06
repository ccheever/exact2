//! A terminal's cell units, `ch` and `lh` (CSS Values 4 §6.1), at a fixed
//! cell: one column is 8 px and one row 16 px.
//!
//! In a monospace face at a fixed line height, `1ch × 1lh` is one terminal
//! cell, so a terminal entry writes its lengths in them (LLP 1101 D3). The
//! layout space keeps a 1:2 cell, whatever the physical terminal's, so a
//! length in cells lands exactly on the grid and aspect ratios stay square.
//! The terminal host snaps everything else to it.
//! @ref LLP 1101 D3

use crate::error::StyleValueError;
use crate::generated::{StyleCodec, StyleId};
use crate::style::relative::admits_relative;
use crate::style::StyleValue;

/// One column, in layout pixels.
pub const COLUMN: f32 = 8.0;
/// One row, in layout pixels.
pub const ROW: f32 = 16.0;

/// Each side's border as it occupies space in a terminal: a drawn side is
/// one cell — a row on top and bottom, a column on the sides — whatever
/// width was written (LLP 1101 §4, a declared deviation). Applied only
/// under a kernel's terminal border rule ([`crate::Env::cell_borders`]).
pub fn border(widths: [f32; 4]) -> [f32; 4] {
    let cell = [ROW, COLUMN, ROW, COLUMN];
    std::array::from_fn(|i| if widths[i] > 0.0 { cell[i] } else { 0.0 })
}

/// `<number>ch` or `<number>lh` in pixels, or `None` when the text is neither.
pub fn parse(text: &str) -> Option<f32> {
    let t = text.trim_matches(['\t', '\n', '\u{c}', '\r', ' ']);
    let lower = t.get(t.len().saturating_sub(2)..)?.to_ascii_lowercase();
    let scale = match lower.as_str() {
        "ch" => COLUMN,
        "lh" => ROW,
        _ => return None,
    };
    let number = &t[..t.len() - 2];
    let n = super::parse_pixel_length(&format!("{number}px"))?;
    (!number.is_empty() && n.is_finite()).then_some(n * scale)
}

/// The row's value in pixels when `value` is in cells, refused where the row
/// admits no length.
pub(crate) fn of(id: StyleId, value: &StyleValue) -> Result<Option<StyleValue>, StyleValueError> {
    let StyleValue::Text(text) = value else {
        return Ok(None);
    };
    let Some(px) = parse(text) else {
        return Ok(None);
    };
    if !admits_relative(id) {
        return Err(StyleValueError::WrongKind {
            style: id,
            expected: "no length: `ch` and `lh` are lengths",
        });
    }
    Ok(Some(if id.codec() == StyleCodec::LineHeight {
        StyleValue::Text(format!("{}px", exact_num::Shortest(px as f64)))
    } else {
        StyleValue::Number(px as f64)
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generated::StyleProps;

    #[test]
    fn cells_are_eight_by_sixteen() {
        assert_eq!(parse("2ch"), Some(16.0));
        assert_eq!(parse(" 1.5lh "), Some(24.0));
        assert_eq!(parse("ch"), None);
        assert_eq!(parse("2em"), None);
        let mut s = StyleProps::default();
        s.set_dynamic(StyleId::ColumnGap, &StyleValue::Text("1ch".into()))
            .unwrap();
        assert_eq!(s.column_gap, 8.0);
        s.set_dynamic(StyleId::PaddingTop, &StyleValue::Text("1lh".into()))
            .unwrap();
        assert_eq!(s.padding_top, crate::Dimension::Points(16.0));
    }
}
