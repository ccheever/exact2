//! The wide colour forms on the web (LLP 1056 §8.2): `lab()`, `lch()`,
//! `oklab()`, `oklch()` and `color()`, in a colour row or a Rust data crate's
//! drawing. The generated entry links them when the plan's strings or the
//! data crate's source name one of those functions, so an app that uses only
//! sRGB colours carries none of their conversions (LLP 1047 D2).

use exact_web::Linked;

/// Link the wide colour forms into `linked`.
pub const fn link(mut linked: Linked) -> Linked {
    linked.wide_colors = Some(exact_kernel::style::link_wide_colors);
    linked
}
