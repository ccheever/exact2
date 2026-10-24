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
    within_limit(path, outset, dev, REACH)
}

// Border regions and clips can cover the frame past the SVG guard's margin.
// Stay below i32's limit, leaving one f32 step for integer rounding.
pub(super) fn border(path: &Path, dev: Transform) -> bool {
    within_limit(path, 0.0, dev, (i32::MAX as f32).next_down())
        && path.bounds().transform(dev).is_none_or(|r| {
            // AA shifts by two before clipping. A negative top that wraps
            // positive starts scanning after the clipped edges and can panic.
            r.top() >= 0.0 || (r.top().floor() as i32).wrapping_shl(2) <= 0
        })
}

fn within_limit(path: &Path, outset: f32, dev: Transform, limit: f32) -> bool {
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
        r.left() >= -limit && r.top() >= -limit && r.right() <= limit && r.bottom() <= limit
    })
}

/// How far a stroke of `width` with `miter` reaches past its path: half
/// the width, by the miter at a sharp join, and by a square cap's diagonal.
pub(super) fn stroke(width: f32, miter: f32) -> f32 {
    width / 2.0 * miter.max(std::f32::consts::SQRT_2)
}
