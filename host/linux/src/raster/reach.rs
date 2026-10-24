//! How far a CPU path may reach before tiny-skia is given it. tiny-skia
//! 0.12 panics filling, stroking or masking a path whose device bounds
//! pass about ±2³¹ pixels (a coordinate overflows an `i32` past its own
//! `is_too_big_for_math` check). SVG geometry and raw border or shadow
//! paths can reach that range. Such a path is not drawn.

use tiny_skia::{Path, Rect, Transform};

/// Half of where tiny-skia was found to panic, in device pixels.
const REACH: f32 = (1u32 << 30) as f32;

/// Whether `path`, grown by `outset` user units on every side (a stroke's
/// extent), stays within [`REACH`] once drawn through `dev`. A transform
/// that makes the bounds non-finite is tiny-skia's to refuse, which it does
/// without panicking.
pub(super) fn within(path: &Path, outset: f32, dev: Transform) -> bool {
    let b = path.bounds();
    let Some(grown) = Rect::from_ltrb(
        b.left() - outset,
        b.top() - outset,
        b.right() + outset,
        b.bottom() + outset,
    ) else {
        return false;
    };
    grown.transform(dev).is_none_or(|r| {
        r.left() >= -REACH && r.top() >= -REACH && r.right() <= REACH && r.bottom() <= REACH
    })
}

/// How far a stroke of `width` with `miter` reaches past its path: half
/// the width, by the miter at a sharp join, and by a square cap's diagonal.
pub(super) fn stroke(width: f32, miter: f32) -> f32 {
    width / 2.0 * miter.max(std::f32::consts::SQRT_2)
}
