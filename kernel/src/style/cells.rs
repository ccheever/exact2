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

/// A space (a padding, margin or gap) of `px` as a terminal lays it out,
/// whole cells of `cell` (a [`ROW`] down, a [`COLUMN`] across): to the
/// nearest, a tie toward zero, so half a cell of space is none (LLP 1101 §4,
/// a declared deviation beside [`border`]; LLP 1121 stage 11 (c)). Applied
/// only under a kernel's terminal space rule ([`crate::Env::cell_spaces`]).
pub fn space(px: f32, cell: f32) -> f32 {
    let n = ((px / cell).abs() - 0.5).ceil().max(0.0);
    match n == 0.0 {
        true => 0.0,
        false => (n * cell).copysign(px),
    }
}

/// `<number>ch` or `<number>lh` in pixels, or `None` when the text is neither.
pub fn parse(text: &str) -> Option<f32> {
    let t = text.trim_matches(['\t', '\n', '\u{c}', '\r', ' ']);
    let suffix = t.get(t.len().saturating_sub(2)..)?;
    let scale = if suffix.eq_ignore_ascii_case("ch") {
        COLUMN
    } else if suffix.eq_ignore_ascii_case("lh") {
        ROW
    } else {
        return None;
    };
    let number = &t[..t.len() - 2];
    let n = super::parse_pixel_length(&format!("{number}px"))?;
    (!number.is_empty() && n.is_finite()).then_some(n * scale)
}

/// The row's value in pixels when `value` is in cells, refused where the row
/// admits no length.
pub(crate) fn of(
    id: StyleId,
    value: &StyleValue,
) -> Result<Option<StyleValue<'static>>, StyleValueError> {
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
        StyleValue::Text(format!("{}px", exact_num::Shortest(px as f64)).into())
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
        assert_eq!(parse("\t+.5Ch\n"), Some(4.0));
        assert_eq!(parse("1e1LH"), Some(160.0));
        assert_eq!(parse("ch"), None);
        assert_eq!(parse("2em"), None);
        for text in ["", "界ch", "é", "light-dark(#000, #fff)", "#ababab", "2 ch"] {
            assert_eq!(parse(text), None, "{text}");
        }
        let mut s = StyleProps::default();
        s.set_dynamic(StyleId::ColumnGap, &StyleValue::Text("1ch".into()))
            .unwrap();
        assert_eq!(s.column_gap, 8.0);
        s.set_dynamic(StyleId::PaddingTop, &StyleValue::Text("1lh".into()))
            .unwrap();
        assert_eq!(s.padding_top, crate::Dimension::Points(16.0));
    }

    #[test]
    fn a_space_is_whole_cells_on_its_axis_under_the_terminal_rule() {
        // Down, a row: space.xs, sm, md (0, 8, 8 px) none, lg and xl (16) one;
        // an app's 14, 12 and 10 px one each; a tie toward zero.
        for (px, rows) in [
            (0.0, 0.0),
            (4.0, 0.0),
            (8.0, 0.0),
            (10.0, 1.0),
            (12.0, 1.0),
            (14.0, 1.0),
            (16.0, 1.0),
            (24.0, 1.0),
            (25.0, 2.0),
        ] {
            assert_eq!(space(px, ROW), rows * ROW, "{px}");
        }
        // Across, a column: a ch is one; a negative margin by its size.
        assert_eq!(space(8.0, COLUMN), 8.0);
        assert_eq!(space(12.0, COLUMN), 8.0);
        assert_eq!(space(-24.0, ROW), -16.0);
        assert_eq!(space(-8.0, ROW).to_bits(), 0.0f32.to_bits());
        // The engine's style: padding, margin and gap in points, each on its
        // axis; a percentage as it is; nothing changes without the rule.
        let mut s = StyleProps::default();
        for (id, v) in [
            (StyleId::PaddingTop, "8px"),
            (StyleId::PaddingLeft, "8px"),
            (StyleId::MarginBottom, "14px"),
            (StyleId::MarginRight, "12px"),
            (StyleId::PaddingRight, "10%"),
            (StyleId::RowGap, "8px"),
            (StyleId::ColumnGap, "12px"),
            (StyleId::MarginTop, "-24px"),
            (StyleId::PaddingBottom, "calc(10% + 8px)"),
        ] {
            s.set_dynamic(id, &StyleValue::Text(v.into())).unwrap();
        }
        let env = crate::Env::default();
        let plain = s.to_taffy(crate::NodeType::View, &env);
        let celled = s.to_taffy(crate::NodeType::View, &env.with_cell_spaces(true));
        use taffy::style::{LengthPercentage as Lp, LengthPercentageAuto as Lpa};
        assert_eq!(plain.padding.top, Lp::length(8.0));
        assert_eq!(celled.padding.top, Lp::length(0.0));
        assert_eq!(celled.padding.left, Lp::length(8.0));
        assert_eq!(celled.padding.right, Lp::percent(0.1));
        assert_eq!(celled.margin.bottom, Lpa::length(16.0));
        assert_eq!(celled.margin.right, Lpa::length(8.0));
        assert_eq!(celled.gap.height, Lp::length(0.0));
        assert_eq!(celled.gap.width, Lp::length(8.0));
        assert_eq!(plain.gap.width, Lp::length(12.0));
        // A negative margin by its size; a calc() with a percentage as it is.
        assert_eq!(celled.margin.top, Lpa::length(-16.0));
        assert_eq!(celled.padding.bottom, plain.padding.bottom);
        assert_ne!(celled.padding.bottom, Lp::length(0.0));
    }
}
