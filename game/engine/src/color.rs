//! Colour as an author writes it, stored as the renderer reads it. CSS text and
//! sRGB floats are encoded for display; the renderer blends and lights in linear
//! light, so both are decoded once, here, through the repository's CSS colour
//! crate (`exact-color`, LLP 1100), the parser Contract's `color` uses.
use crate::{Data, DataError, Reader, Writer};

/// A colour in linear light: `N` channels, red, green, blue and (when `N` is 4)
/// alpha. Write it as CSS (`Color::hex("#527b65")`, `Color::hex("forestgreen")`)
/// or as sRGB floats (`Color::srgb(0.32, 0.48, 0.4)`); `Color([r, g, b])` and
/// `[r, g, b].into()` take linear values as they are. It saves, hashes and reads
/// back as its channel array.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color<const N: usize = 4>(pub [f32; N]);

impl<const N: usize> Color<N> {
    /// A CSS colour: hex (`#527b65`, `#5b6`, `#527b6580`), a named colour
    /// (`forestgreen`), or any other form Contract's `color` takes. Panics,
    /// naming the text, when it is not one.
    pub fn hex(css: &str) -> Self {
        Self::css(css).unwrap_or_else(|e| panic!("{e}"))
    }
    /// sRGB-encoded channels in 0–1, as a colour picker shows them; alpha 1.
    pub fn srgb(r: f32, g: f32, b: f32) -> Self {
        Self::take([r, g, b].map(|c| decode(c.into())), 1.0)
    }
    fn css(css: &str) -> Result<Self, String> {
        let (linear, alpha) = match exact_color::parse(css) {
            Some(exact_color::Parsed::Legacy(c)) => (
                [c.r, c.g, c.b].map(|v| decode(f64::from(v) / 255.0)),
                c.a as f32,
            ),
            Some(exact_color::Parsed::Wide(w)) => {
                (w.linear_srgb().map(|v| v as f32), w.alpha as f32)
            }
            _ => return Err(format!("{css:?} is not a CSS colour; write `#527b65`, `#527b6580` or a named colour such as `forestgreen`")),
        };
        Ok(Self::take(linear, alpha))
    }
    fn take(rgb: [f32; 3], alpha: f32) -> Self {
        assert!(N == 3 || N == 4, "a Color has 3 or 4 channels");
        Self(std::array::from_fn(|i| if i < 3 { rgb[i] } else { alpha }))
    }
}

fn decode(c: f64) -> f32 {
    exact_color::srgb_to_linear(c) as f32
}

impl<const N: usize> Default for Color<N>
where
    [f32; N]: Default,
{
    /// White, opaque.
    fn default() -> Self {
        Self([1.0; N])
    }
}
impl<const N: usize> From<[f32; N]> for Color<N> {
    fn from(linear: [f32; N]) -> Self {
        Self(linear)
    }
}
impl<const N: usize> From<Color<N>> for [f32; N] {
    fn from(c: Color<N>) -> Self {
        c.0
    }
}
impl<const N: usize> Data for Color<N>
where
    [f32; N]: Default,
{
    fn write(&self, w: &mut dyn Writer) {
        self.0.write(w);
    }
    fn read(&mut self, r: &mut dyn Reader) -> Result<(), DataError> {
        self.0.read(r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn css_and_srgb_decode_to_linear_light() {
        // #527b65: 0x52/255 = 0.3216 sRGB, 0.0844 linear.
        let meadow = Color::<3>::hex("#527b65").0;
        assert!((meadow[0] - 0.0844).abs() < 1e-4, "{meadow:?}");
        assert_eq!(Color::<3>::hex("#527B65"), Color::<3>::hex("#527b65"));
        let picked = Color::<3>::srgb(82. / 255., 123. / 255., 101. / 255.).0;
        assert!(picked.iter().zip(meadow).all(|(a, b)| (a - b).abs() < 1e-6));
        assert_eq!(Color::<4>::hex("white").0, [1.0; 4]);
        assert_eq!(Color::<4>::hex("#ff000080").0[3], 128. / 255.);
        assert_eq!(Color::<3>::hex("black").0, [0.0; 3]);
        // Linear values stay as written.
        assert_eq!(Color::from([0.5, 0.25, 0.125]).0, [0.5, 0.25, 0.125]);
        let refused = std::panic::catch_unwind(|| Color::<3>::hex("#52g"));
        assert!(refused.is_err());
    }

    #[test]
    fn a_color_saves_and_hashes_as_its_channel_array() {
        let c = Color::<3>::hex("#527b65");
        assert_eq!(crate::hash::of(&c), crate::hash::of(&c.0));
        assert_eq!(
            crate::json::to_string(&c).unwrap(),
            crate::json::to_string(&c.0).unwrap()
        );
        let back: Color<3> = crate::json::from_str(&crate::json::to_string(&c).unwrap()).unwrap();
        assert_eq!(back, c);
    }
}
