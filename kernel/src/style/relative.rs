//! CSS's font-relative lengths, `rem` and `em` (CSS Values 4 §6.1), and a
//! platform text style as a `font-size` (`-exact-title1`, LLP 1115 D3).
//!
//! A row authored as `1.5rem` or `0.8em` keeps what was written here, beside
//! the row, and the row itself always holds the pixels it resolves to — what
//! CSS calls the computed value. Every reader (layout, the measurer, every
//! host's painter, the live web host's CSS text) reads pixels as before; only
//! the web's JS target, which keeps no kernel at run time, writes the units
//! for the browser to resolve (`exact_web::css::css_text_relative`). The
//! kernel resolves every such row at the end of each commit
//! ([`crate::txn`]): `rem` against the root font size the host sets
//! ([`crate::Kernel::set_root_font_size`]), `em` against the element's own
//! computed `font-size` — and, for `font-size` itself, against the parent's,
//! as CSS says. `px` never scales. A text style is the platform's size for it
//! at the root font size (the platform's body size): the schema's ramp,
//! [`text_style_size`].
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
    /// A platform text style's size at the root font size, by its id in
    /// [`crate::TEXT_STYLES`]; only a `font-size` is one.
    TextStyle(u8),
}

impl Unit {
    /// The pixels one of this unit is, given the root's and the
    /// reference (the parent's or the element's) font size.
    pub fn basis(self, root: f32, reference: f32) -> f32 {
        match self {
            Unit::Rem => root,
            Unit::Em => reference,
            Unit::TextStyle(id) => text_style_size(id, root),
        }
    }
}

/// A text style's id by its written name: `-exact-<name>` or its WebKit
/// alias, ASCII case-insensitively, as CSS matches keywords.
/// @ref LLP 1115 D3
pub fn text_style(name: &str) -> Option<u8> {
    let name = name.trim_matches(['\t', '\n', '\u{c}', '\r', ' ']);
    let exact = name
        .get(..7)
        .filter(|p| p.eq_ignore_ascii_case("-exact-"))
        .map(|_| &name[7..]);
    crate::TEXT_STYLES
        .iter()
        .position(|s| {
            exact.is_some_and(|n| n.eq_ignore_ascii_case(s.name))
                || (!s.alias.is_empty() && name.eq_ignore_ascii_case(s.alias))
        })
        .map(|i| i as u8)
}

/// The size of text style `id` where the root font size is `root`: the
/// schema's ramp read at `root` (LLP 1115 D3). Between two of its body
/// sizes it interpolates; below the first or above the last, the nearest
/// row scales with the root.
pub fn text_style_size(id: u8, root: f32) -> f32 {
    let bodies = crate::TEXT_STYLE_BODIES;
    let Some(style) = crate::TEXT_STYLES.get(usize::from(id)) else {
        return root;
    };
    let sizes = style.sizes;
    let last = bodies.len() - 1;
    if root <= bodies[0] {
        return sizes[0] * root / bodies[0];
    }
    if root >= bodies[last] {
        return sizes[last] * root / bodies[last];
    }
    let i = bodies.iter().position(|&b| b >= root).unwrap_or(last);
    let (b0, b1) = (bodies[i - 1], bodies[i]);
    let t = (root - b0) / (b1 - b0);
    sizes[i - 1] + (sizes[i] - sizes[i - 1]) * t
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
            match unit {
                Unit::Rem => out.push(0),
                Unit::Em => out.push(1),
                Unit::TextStyle(id) => out.extend_from_slice(&[2, *id]),
            }
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
        || id == StyleId::TextIndent
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
    if let Some(id) = text_style(text) {
        return Some((Unit::TextStyle(id), 1.0));
    }
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
    if matches!(unit, Unit::TextStyle(_)) && id != StyleId::FontSize {
        return Err(StyleValueError::WrongKind {
            style: id,
            expected: "a length: a text style is a `font-size`",
        });
    }
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

/// A `<number>px` text on a pixel row that reads a bare number as pixels
/// (`font-size="14px"`, `letter-spacing="0.5px"`): the number, as CSS takes
/// both spellings (LLP 1102 §3.10). A dimension or line-height row reads its
/// own text; any other row is left to its conversion, which refuses it. A
/// negative length on a row CSS refuses one on is refused, as `rem` is.
pub(crate) fn pixels_text(
    id: StyleId,
    value: &StyleValue,
) -> Result<Option<StyleValue>, StyleValueError> {
    let StyleValue::Text(text) = value else {
        return Ok(None);
    };
    // SVG's stroke lengths read a bare number as pixels too, and CSS takes `2px` there.
    let stroke = matches!(id, StyleId::StrokeWidth | StyleId::StrokeDashoffset);
    if matches!(id.codec(), StyleCodec::Dimension | StyleCodec::LineHeight)
        || !(admits_relative(id) || stroke)
    {
        return Ok(None);
    }
    let t = text.trim_matches(['\t', '\n', '\u{c}', '\r', ' ']);
    let px = t.len() > 2
        && t.get(t.len() - 2..)
            .is_some_and(|u| u.eq_ignore_ascii_case("px"));
    let Some(n) = super::parse_pixel_length(t).filter(|_| px) else {
        return Ok(None);
    };
    if n < 0.0 && (nonnegative(id) || id == StyleId::StrokeWidth) {
        return Err(StyleValueError::WrongKind {
            style: id,
            expected: "a nonnegative length",
        });
    }
    Ok(Some(StyleValue::Number(n as f64)))
}

/// What the row holds until the kernel resolves it: the length at CSS's
/// initial font size, in the form the row's codec reads as pixels.
pub(crate) fn provisional(id: StyleId, (unit, n): (Unit, f32)) -> StyleValue {
    pixels(id, n * unit.basis(MEDIUM, MEDIUM))
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
    fn a_text_style_is_a_font_size_read_from_the_ramp_at_the_root() {
        let title1 = text_style("-exact-title1").unwrap();
        assert_eq!(text_style(" -EXACT-Title1 "), Some(title1));
        assert_eq!(text_style("-apple-system-title1"), Some(title1));
        assert_eq!(
            text_style("title1"),
            None,
            "an Exact role is written -exact-"
        );
        assert_eq!(text_style("-exact-title9"), None);
        assert_eq!(parse("-exact-title1"), Some((Unit::TextStyle(title1), 1.0)));
        // iOS at the default Dynamic Type (17), the Mac (13), the web (16).
        assert_eq!(text_style_size(title1, 17.0), 28.0);
        assert_eq!(text_style_size(title1, 13.0), 22.0);
        assert_eq!(text_style_size(title1, 16.0), 27.0);
        // AX5's body is 53, its title1 58: Apple's ramp, not a ratio.
        assert_eq!(text_style_size(title1, 53.0), 58.0);
        assert_eq!(text_style_size(title1, 18.0), 29.0, "between 17 and 19");
        assert_eq!(
            text_style_size(title1, 106.0),
            116.0,
            "past the ramp, scaled"
        );
        assert_eq!(text_style_size(title1, 6.5), 11.0, "below it, scaled");
        let body = text_style("-exact-body").unwrap();
        for root in [12.0, 13.0, 17.0, 18.0, 33.0, 60.0] {
            assert_eq!(text_style_size(body, root), root, "body is the root size");
        }
        let mut s = StyleProps::default();
        s.set_dynamic(StyleId::FontSize, &StyleValue::Text("-exact-title1".into()))
            .unwrap();
        assert_eq!(s.font_size, 27.0, "provisionally at CSS's medium");
        assert_eq!(
            s.relative.get(StyleId::FontSize),
            Some((Unit::TextStyle(title1), 1.0))
        );
        for row in [StyleId::MarginTop, StyleId::LineHeight, StyleId::FontWeight] {
            assert!(
                s.set_dynamic(row, &StyleValue::Text("-exact-title1".into()))
                    .is_err(),
                "{row:?}"
            );
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

    #[test]
    fn a_px_text_on_a_pixel_row_is_its_number() {
        let mut s = StyleProps::default();
        s.set_dynamic(StyleId::FontSize, &StyleValue::Text("14px".into()))
            .unwrap();
        assert_eq!(s.font_size, 14.0);
        assert!(s.relative.is_empty());
        s.set_dynamic(StyleId::LetterSpacing, &StyleValue::Text(" -0.5PX ".into()))
            .unwrap();
        assert_eq!(s.letter_spacing, -0.5);
        // SVG's stroke lengths too; a stroke width is never negative.
        s.set_dynamic(StyleId::StrokeWidth, &StyleValue::Text("2px".into()))
            .unwrap();
        assert_eq!(s.stroke_width, 2.0);
        s.set_dynamic(StyleId::StrokeDashoffset, &StyleValue::Text("-3px".into()))
            .unwrap();
        assert_eq!(s.stroke_dashoffset, -3.0);
        assert!(s
            .set_dynamic(StyleId::StrokeWidth, &StyleValue::Text("-1px".into()))
            .is_err());
        // CSS refuses a negative font size; a unitless text and a non-pixel row stay refused.
        for (row, text) in [
            (StyleId::FontSize, "-2px"),
            (StyleId::FontSize, "14"),
            (StyleId::FontSize, "px"),
            (StyleId::Opacity, "1px"),
        ] {
            assert!(
                s.set_dynamic(row, &StyleValue::Text(text.into())).is_err(),
                "{row:?} {text}"
            );
        }
    }

    #[test]
    fn none_on_a_maximum_is_its_unbounded_auto() {
        let mut s = StyleProps::default();
        s.set_dynamic(StyleId::MaxHeight, &StyleValue::Text(" NONE ".into()))
            .unwrap();
        assert_eq!(s.max_height, crate::Dimension::Auto);
        s.set_dynamic(StyleId::MaxWidth, &StyleValue::Text("none".into()))
            .unwrap();
        assert_eq!(s.max_width, crate::Dimension::Auto);
        // Only a maximum: `none` is no width.
        assert!(s
            .set_dynamic(StyleId::Width, &StyleValue::Text("none".into()))
            .is_err());
    }
}
