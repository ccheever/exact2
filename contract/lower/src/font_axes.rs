//! A face's `wght` and `wdth` ranges from its file's `fvar` table, for an
//! `@font-face` descriptor the declaration leaves `auto` (CSS Fonts 4 §4.5,
//! §4.6). @ref LLP 1019 D2.
//!
//! OpenType's table directory and `fvar` layout:
//! <https://learn.microsoft.com/en-us/typography/opentype/spec/otff>,
//! <https://learn.microsoft.com/en-us/typography/opentype/spec/fvar>.

/// A variable face's axis ranges in CSS units: weights, and widths in percent.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct Axes {
    pub wght: Option<[f32; 2]>,
    pub wdth: Option<[f32; 2]>,
}

/// The `wght` and `wdth` ranges of an sfnt's `fvar`, or none for a static
/// face; a file this cannot read has none either.
pub(crate) fn read(bytes: &[u8]) -> Axes {
    fn u16_at(b: &[u8], at: usize) -> Option<u16> {
        Some(u16::from_be_bytes(b.get(at..at + 2)?.try_into().ok()?))
    }
    fn u32_at(b: &[u8], at: usize) -> Option<u32> {
        Some(u32::from_be_bytes(b.get(at..at + 4)?.try_into().ok()?))
    }
    // 16.16 fixed point.
    fn fixed_at(b: &[u8], at: usize) -> Option<f32> {
        Some(i32::from_be_bytes(b.get(at..at + 4)?.try_into().ok()?) as f32 / 65536.0)
    }
    let fvar = || -> Option<&[u8]> {
        let tables = u16_at(bytes, 4)? as usize;
        (0..tables).find_map(|i| {
            let record = 12 + i * 16;
            (bytes.get(record..record + 4)? == b"fvar").then(|| {
                let offset = u32_at(bytes, record + 8)? as usize;
                let length = u32_at(bytes, record + 12)? as usize;
                bytes.get(offset..offset.checked_add(length)?)
            })?
        })
    };
    let mut axes = Axes::default();
    let Some(fvar) = fvar() else { return axes };
    let (Some(first), Some(count), Some(size)) =
        (u16_at(fvar, 4), u16_at(fvar, 8), u16_at(fvar, 10))
    else {
        return axes;
    };
    for i in 0..count as usize {
        let at = first as usize + i * size as usize;
        let (Some(tag), Some(min), Some(max)) = (
            fvar.get(at..at + 4),
            fixed_at(fvar, at + 4),
            fixed_at(fvar, at + 12),
        ) else {
            break;
        };
        if !(min.is_finite() && max.is_finite() && min <= max) {
            continue;
        }
        match tag {
            b"wght" => axes.wght = Some([min, max]),
            b"wdth" => axes.wdth = Some([min, max]),
            _ => {}
        }
    }
    axes
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An sfnt holding one `fvar` with the given axes.
    fn font(axes: &[(&[u8; 4], f32, f32, f32)]) -> Vec<u8> {
        let fixed = |v: f32| ((v * 65536.0) as i32).to_be_bytes();
        let mut fvar = Vec::new();
        fvar.extend([0, 1, 0, 0, 0, 16, 0, 2]);
        fvar.extend((axes.len() as u16).to_be_bytes());
        fvar.extend([0, 20, 0, 0, 0, 0]);
        for (tag, min, default, max) in axes {
            fvar.extend(*tag);
            fvar.extend(fixed(*min));
            fvar.extend(fixed(*default));
            fvar.extend(fixed(*max));
            fvar.extend([0, 0, 1, 0]);
        }
        let mut out = vec![0, 1, 0, 0, 0, 1, 0, 16, 0, 0, 0, 0];
        out.extend(b"fvar");
        out.extend([0; 4]);
        out.extend(28u32.to_be_bytes());
        out.extend((fvar.len() as u32).to_be_bytes());
        out.extend(fvar);
        out
    }

    #[test]
    fn a_variable_face_reports_its_weight_and_width_ranges() {
        let bytes = font(&[
            (b"opsz", 8.0, 14.0, 144.0),
            (b"wght", 100.0, 400.0, 1000.0),
            (b"wdth", 25.0, 100.0, 151.0),
        ]);
        assert_eq!(
            read(&bytes),
            Axes {
                wght: Some([100.0, 1000.0]),
                wdth: Some([25.0, 151.0]),
            }
        );
    }

    #[test]
    fn a_static_or_unreadable_face_has_no_ranges() {
        assert_eq!(read(&[0, 1, 0, 0, 0, 0]), Axes::default());
        assert_eq!(read(b"not a font"), Axes::default());
        let mut truncated = font(&[(b"wght", 100.0, 400.0, 900.0)]);
        truncated.truncate(40);
        assert_eq!(read(&truncated), Axes::default());
    }
}
