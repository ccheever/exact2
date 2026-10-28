//! Hinted glyphs retain the shaper's face and quarter-pixel placement. @ref LLP 1149 D3.1.
mod swash;
pub use self::swash::{subpixel, Font, Glyph, Glyphs};

/// Physical coordinates before quarter-pixel x binning and whole-pixel y truncation.
pub fn position(x: f32, y: f32, baseline: f32, origin: (f32, f32), scale: f32) -> (f32, f32) {
    (
        x.mul_add(scale, origin.0 * scale),
        y.mul_add(scale, (origin.1 + baseline) * scale),
    )
}

#[cfg(test)]
#[test]
fn physical_y_preserves_the_oracles_association_at_an_integer_boundary() {
    let (_, y) = position(0., 4.169_204_7, 12.390_034, (0., 17.040_76), 1.25);
    assert_eq!(y.trunc() as i32, 41);
}
