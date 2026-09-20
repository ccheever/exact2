//! Advances by arithmetic: Pretext's `prepare` cache, read from the app's own
//! font files at build time (`build.rs`) instead of from canvas `measureText`.
use std::ops::Range;

/// One face's horizontal metrics in font units.
pub struct FaceData {
    /// Units per em.
    pub upem: f32,
    /// hhea ascender, font units.
    pub ascent: f32,
    /// hhea descender, font units (negative).
    pub descent: f32,
    /// Advance per ASCII code point; zero where the face has no glyph.
    pub ascii: [u16; 128],
    /// Advances for the non-ASCII characters the prose uses.
    pub extra: &'static [(char, u16)],
    /// GPOS `kern` pairs, keyed `(a << 21) | b`, sorted by key.
    pub kern: &'static [(u32, i16)],
}

include!(concat!(env!("OUT_DIR"), "/metrics.rs"));

impl FaceData {
    fn advance_units(&self, c: char) -> f32 {
        let code = c as u32;
        let units = if code < 128 {
            self.ascii[code as usize]
        } else {
            self.extra
                .iter()
                .find(|&&(k, _)| k == c)
                .map(|&(_, v)| v)
                .unwrap_or(self.ascii[b'x' as usize])
        };
        units as f32
    }

    fn kern_units(&self, a: char, b: char) -> f32 {
        let key = ((a as u32) << 21) | b as u32;
        match self.kern.binary_search_by_key(&key, |&(k, _)| k) {
            Ok(i) => self.kern[i].1 as f32,
            Err(_) => 0.0,
        }
    }

    /// The advance of `text` at `size` px: glyph advances plus kerning between
    /// neighbours, the way every host shapes a run of this face.
    pub fn width(&self, text: &str, size: f32) -> f32 {
        let mut units = 0.0f32;
        let mut previous: Option<char> = None;
        for c in text.chars() {
            let c = if c == '\n' || c == '\t' { ' ' } else { c };
            units += self.advance_units(c);
            if let Some(p) = previous {
                units += self.kern_units(p, c);
            }
            previous = Some(c);
        }
        units * size / self.upem
    }

    /// A `Measure` for the walker over `text` at `size`.
    pub fn measure<'a>(&'a self, text: &'a str, size: f32) -> impl FnMut(Range<usize>) -> f32 + 'a {
        move |range| self.width(&text[range], size).max(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn metrics_are_present_and_kerned() {
        assert!(REGULAR.upem > 0.0);
        assert!(REGULAR.width("n", 16.0) > 4.0);
        assert!(REGULAR.width("m", 16.0) > REGULAR.width("i", 16.0));
        assert!(
            !REGULAR.kern.is_empty(),
            "Exposure Sans declares a kern feature"
        );
        let sum = REGULAR.width("A", 16.0) + REGULAR.width("V", 16.0);
        assert!(
            REGULAR.width("AV", 16.0) < sum,
            "AV kerns tighter than A + V"
        );
        assert!(BOLD.width("Reflow", 16.0) > REGULAR.width("Reflow", 16.0));
    }
}
