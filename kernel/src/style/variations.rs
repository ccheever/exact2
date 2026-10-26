//! CSS `font-variation-settings` (CSS Fonts 4 §7.2). @ref LLP 1019 D9.

/// `normal`, or up to [`Self::MAX`] axes by four-letter tag, each set to a
/// number in the axis's own units. A tag named twice keeps its last value,
/// in the place of its first. It applies after `font-weight`,
/// `font-stretch` and optical sizing, and wins for the same axis.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct FontVariations {
    len: u8,
    axes: [(u32, f32); FontVariations::MAX],
}

impl FontVariations {
    /// The most axes one value names: a variable font has a dozen or so,
    /// and a run carries these by value (`TextStyle`, the hosts' runs).
    pub const MAX: usize = 8;

    /// The first [`Self::MAX`] of `axes`, each tag once (the caller's).
    pub fn from_axes(axes: &[(u32, f32)]) -> Self {
        let mut out = Self::default();
        for (slot, axis) in out.axes.iter_mut().zip(axes) {
            *slot = *axis;
            out.len += 1;
        }
        out
    }

    /// The axes as (tag, value), the tag big-endian (`'wght'` is
    /// `0x77676874`).
    pub fn axes(&self) -> &[(u32, f32)] {
        &self.axes[..self.len as usize]
    }

    /// Whether it is `normal`: no axis named.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The row's parse: `normal`, or a comma-separated list of a quoted
    /// four-character tag (U+0020–U+007E) and a finite number. More than
    /// [`Self::MAX`] distinct tags is refused, as anything else invalid.
    pub fn parse(css: &str) -> Option<Self> {
        let css = css.trim();
        let mut out = Self::default();
        if css.eq_ignore_ascii_case("normal") {
            return Some(out);
        }
        for item in css.split(',') {
            let item = item.trim();
            let quote = item.chars().next()?;
            if quote != '"' && quote != '\'' {
                return None;
            }
            let rest = &item[1..];
            let close = rest.find(quote)?;
            let (tag, number) = (&rest[..close], rest[close + 1..].trim());
            if tag.len() != 4 || !tag.bytes().all(|b| (0x20..=0x7e).contains(&b)) {
                return None;
            }
            let value = exact_num::parse_f32(number)
                .ok()
                .filter(|v| v.is_finite())?;
            let tag = u32::from_be_bytes(tag.as_bytes().try_into().ok()?);
            if let Some(axis) = out.axes[..out.len as usize]
                .iter_mut()
                .find(|(t, _)| *t == tag)
            {
                axis.1 = value;
            } else {
                if out.len as usize == Self::MAX {
                    return None;
                }
                out.axes[out.len as usize] = (tag, value);
                out.len += 1;
            }
        }
        Some(out)
    }

    /// As CSS writes it, what [`Self::parse`] reads back.
    pub fn css(&self) -> String {
        if self.is_empty() {
            return "normal".into();
        }
        self.axes()
            .iter()
            .map(|(tag, value)| {
                let tag = tag.to_be_bytes().map(char::from).iter().collect::<String>();
                format!("\"{tag}\" {}", exact_num::Shortest(f64::from(*value)))
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// The value set for `tag`, if any.
    pub fn get(&self, tag: &[u8; 4]) -> Option<f32> {
        let tag = u32::from_be_bytes(*tag);
        self.axes().iter().find(|(t, _)| *t == tag).map(|(_, v)| *v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_css_and_writes_it_back() {
        let v = FontVariations::parse(r#""wght" 650, 'wdth' 87.5,"GRAD" -50"#).unwrap();
        assert_eq!(v.get(b"wght"), Some(650.));
        assert_eq!(v.get(b"wdth"), Some(87.5));
        assert_eq!(v.get(b"GRAD"), Some(-50.));
        assert_eq!(v.css(), r#""wght" 650, "wdth" 87.5, "GRAD" -50"#);
        assert_eq!(FontVariations::parse(&v.css()), Some(v));
        assert!(FontVariations::parse("normal").unwrap().is_empty());
        assert_eq!(FontVariations::parse("normal").unwrap().css(), "normal");
    }

    #[test]
    fn a_repeated_tag_keeps_its_last_value_in_its_first_place() {
        let v = FontVariations::parse(r#""wght" 300, "wdth" 90, "wght" 700"#).unwrap();
        assert_eq!(
            v.axes(),
            &[
                (u32::from_be_bytes(*b"wght"), 700.),
                (u32::from_be_bytes(*b"wdth"), 90.)
            ]
        );
    }

    #[test]
    fn refuses_what_css_refuses_and_more_than_eight_axes() {
        for bad in [
            r#"wght 650"#,
            r#""wgh" 650"#,
            r#""wghts" 650"#,
            r#""wght""#,
            r#""wght" x"#,
            r#""wght" 650,"#,
            "",
        ] {
            assert_eq!(FontVariations::parse(bad), None, "{bad}");
        }
        let nine = (0..9)
            .map(|i| format!("\"ax{i:02}\" 1"))
            .collect::<Vec<_>>()
            .join(", ");
        assert_eq!(FontVariations::parse(&nine), None);
        let eight = (0..8)
            .map(|i| format!("\"ax{i:02}\" 1"))
            .collect::<Vec<_>>()
            .join(", ");
        assert_eq!(FontVariations::parse(&eight).unwrap().axes().len(), 8);
    }
}
