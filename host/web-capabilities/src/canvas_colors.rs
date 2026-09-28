//! Canvas 2D's wide colour forms on the web (LLP 1056 §8.2): `lab()`,
//! `lch()`, `oklab()`, `oklch()` and `color()` in a Rust data crate's
//! drawing. Not a plan's use: the generated entry links them when the data
//! crate's source names one of those functions, so an app that draws only
//! sRGB colours carries none of their conversions (LLP 1047 D2).

use exact_web::Linked;

/// Link the wide colour forms into `linked`.
pub const fn link(mut linked: Linked) -> Linked {
    linked.canvas_colors = Some(exact_runner::exact_canvas::color::link_wide);
    linked
}
