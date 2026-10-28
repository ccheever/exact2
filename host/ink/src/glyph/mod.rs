//! Hinted glyphs retain the shaper's face and quarter-pixel placement. @ref LLP 1149 D3.1.
mod swash;
pub use self::swash::{subpixel, Font, Glyph, Glyphs};
