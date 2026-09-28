//! CSS's font-relative lengths, `rem` and `em` (CSS Values 4 §6.1).
//!
//! A row authored as `1.5rem` or `0.8em` keeps what was written here, beside
//! the row, and the row itself always holds the pixels it resolves to — what
//! CSS calls the computed value. Every reader (layout, the measurer, every
//! host's painter, the web's CSS text) reads pixels as before; only the
//! kernel knows a row is relative, and it resolves every such row at the end
//! of each commit ([`crate::txn`]): `rem` against the root font size the host
//! sets ([`crate::Kernel::set_root_font_size`]), `em` against the element's
//! own computed `font-size` — and, for `font-size` itself, against the
//! parent's, as CSS says. `px` never scales.
//! @ref LLP 1069.000 D3

use crate::error::StyleValueError;
use crate::generated::{StyleCodec, StyleId, StyleProps};
use crate::style::StyleValue;

/// CSS's initial root font size: `medium`, 16 px.
pub const MEDIUM: f32 = 16.0;

/// Which font size a relative length multiplies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Unit {
    /// The root element's font size.
    Rem,
    /// The element's font size (for `font-size`, the parent's).
    Em,
}

/// The rows of one style authored in `rem`/`em`, by row, with the factor
/// written. Empty — no allocation — for every style that uses neither.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Relative(Vec<(StyleId, Unit, f32)>);

impl Relative {
    /// Whether no row is relative.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The row's relative length, if it has one.
    pub fn get(&self, id: StyleId) -> Option<(Unit, f32)> {
        self.0
            .iter()
            .find(|(row, ..)| *row == id)
            .map(|(_, unit, n)| (*unit, *n))
    }

    /// Every relative row, in row order.
    pub fn iter(&self) -> impl Iterator<Item = (StyleId, Unit, f32)> + '_ {
        self.0.iter().copied()
    }

    /// Set or remove the row's relative length.
    pub(crate) fn put(&mut self, id: StyleId, value: Option<(Unit, f32)>) {
        let at = self.0.iter().position(|(row, ..)| *row >= id);
        let here = at.is_some_and(|i| self.0[i].0 == id);
        match (value, at) {
            (Some((unit, n)), Some(i)) if here => self.0[i] = (id, unit, n),
            (Some((unit, n)), Some(i)) => self.0.insert(i, (id, unit, n)),
            (Some((unit, n)), None) => self.0.push((id, unit, n)),
            (None, Some(i)) if here => {
                self.0.remove(i);
            }
            (None, _) => {}
        }
    }

    /// Take `from`'s relative lengths for the rows in `mask`, as the rows
    /// themselves are copied.
    pub(crate) fn copy(&mut self, from: &Relative, mask: crate::StyleMask) {
        if self.0.is_empty() && from.0.is_empty() {
            return;
        }
        self.0.retain(|(row, ..)| !mask.has(*row));
        for (row, unit, n) in &from.0 {
            if mask.has(*row) {
                self.put(*row, Some((*unit, *n)));
            }
        }
    }

    /// The bytes that tell two styles' relative rows apart, for sharing.
    pub(crate) fn encode(&self, out: &mut Vec<u8>) {
        for (row, unit, n) in &self.0 {
            out.extend_from_slice(&row.bit().to_le_bytes());
            out.push(*unit as u8);
            out.extend_from_slice(&n.to_bits().to_le_bytes());
        }
    }
}

/// Whether CSS admits a length on this row, and so `rem` and `em`: every
/// dimension row, `line-height`, and the pixel rows below. The rows CSS
/// refuses a negative length on say so.
pub fn admits_relative(id: StyleId) -> bool {
    matches!(id.codec(), StyleCodec::Dimension | StyleCodec::LineHeight)
        || id == StyleId::LetterSpacing
        || nonnegative(id)
}

fn nonnegative(id: StyleId) -> bool {
    use StyleId::*;
    matches!(
        id,
        FontSize
            | LineHeight
            | RowGap
            | ColumnGap
            | BorderWidthTop
            | BorderWidthRight
            | BorderWidthBottom
            | BorderWidthLeft
            | BorderRadiusTopLeft
            | BorderRadiusTopRight
            | BorderRadiusBottomRight
            | BorderRadiusBottomLeft
            | ShapeMargin
    )
}

/// `<number>rem` or `<number>em`, by CSS's number grammar, or `None` when
/// the text is neither.
pub fn parse(text: &str) -> Option<(Unit, f32)> {
    let t = text.trim_matches(['\t', '\n', '\u{c}', '\r', ' ']);
    let lower = t.get(t.len().saturating_sub(3)..)?.to_ascii_lowercase();
    let (unit, number) = if lower == "rem" {
        (Unit::Rem, &t[..t.len() - 3])
    } else if lower.ends_with("em") {
        (Unit::Em, &t[..t.len() - 2])
    } else {
        return None;
    };
    // A number with no sign or digit before the unit (`rem`, `-em`) is none.
    let n = super::parse_pixel_length(&format!("{number}px"))?;
    (!number.is_empty() && n.is_finite()).then_some((unit, n))
}

/// The row's relative length when `value` is one, refused when the row
/// admits no length or no negative one.
pub(crate) fn of(id: StyleId, value: &StyleValue) -> Result<Option<(Unit, f32)>, StyleValueError> {
    let StyleValue::Text(text) = value else {
        return Ok(None);
    };
    let Some((unit, n)) = parse(text) else {
        return Ok(None);
    };
    if !admits_relative(id) {
        return Err(StyleValueError::WrongKind {
            style: id,
            expected: "no length: `rem` and `em` are lengths",
        });
    }
    if n < 0.0 && nonnegative(id) {
        return Err(StyleValueError::WrongKind {
            style: id,
            expected: "a nonnegative length",
        });
    }
    Ok(Some((unit, n)))
}

/// What the row holds until the kernel resolves it: the length at CSS's
/// initial font size, in the form the row's codec reads as pixels.
pub(crate) fn provisional(id: StyleId, (_, n): (Unit, f32)) -> StyleValue {
    pixels(id, n * MEDIUM)
}

fn pixels(id: StyleId, px: f32) -> StyleValue {
    if id.codec() == StyleCodec::LineHeight {
        StyleValue::Text(format!("{}px", exact_num::Shortest(px as f64)))
    } else {
        StyleValue::Number(px as f64)
    }
}

impl StyleProps {
    /// Write the pixels a relative row resolves to, keeping it relative.
    /// `false` when the row already held them.
    pub(crate) fn set_resolved(&mut self, id: StyleId, px: f32) -> bool {
        let mut patch = StyleProps::default();
        // A resolved length the row refuses (an overflow to infinity) leaves
        // the row as it was.
        if patch.set_dynamic(id, &pixels(id, px)).is_err()
            || (self.mask.has(id) && self.get(id) == patch.get(id))
        {
            return false;
        }
        let keep = self.relative.get(id);
        self.copy_rows(&patch, crate::StyleMask::of(id));
        self.relative.put(id, keep);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_css_numbers_with_either_unit() {
        assert_eq!(parse("1.5rem"), Some((Unit::Rem, 1.5)));
        assert_eq!(parse(" 0.875REM "), Some((Unit::Rem, 0.875)));
        assert_eq!(parse("2em"), Some((Unit::Em, 2.0)));
        assert_eq!(parse("-.5em"), Some((Unit::Em, -0.5)));
        assert_eq!(parse("1e1em"), Some((Unit::Em, 10.0)));
        for refused in ["rem", "em", "-em", "1 rem", "1px", "1", "1.rem", "1remx"] {
            assert_eq!(parse(refused), None, "{refused}");
        }
    }

    #[test]
    fn a_patch_keeps_what_was_written_and_a_later_pixel_value_forgets_it() {
        let mut s = StyleProps::default();
        s.set_dynamic(StyleId::FontSize, &StyleValue::Text("1.5rem".into()))
            .unwrap();
        assert_eq!(s.font_size, 24.0);
        assert_eq!(s.relative.get(StyleId::FontSize), Some((Unit::Rem, 1.5)));
        s.set_dynamic(StyleId::FontSize, &StyleValue::Number(24.0))
            .unwrap();
        assert!(s.relative.is_empty());
        assert!(s
            .set_dynamic(StyleId::FontSize, &StyleValue::Text("-1em".into()))
            .is_err());
        assert!(s
            .set_dynamic(StyleId::Opacity, &StyleValue::Text("1em".into()))
            .is_err());
        assert!(s
            .set_dynamic(StyleId::MarginTop, &StyleValue::Text("-1em".into()))
            .is_ok());
    }
}
