//! CSS shape values, exclusion bands, and arithmetic text flow.
//!
//! @ref LLP 1043.000 §3 D1, D5, D6 — the host shapes and paints our byte ranges.
//! Preparation owns advances and offsets, never the source string or a font.
//! CSS-bounded layout calls are allocation-free once caller output has capacity.
//! Direct programmatic polygons above 64 vertices use temporary geometry scratch.
//! Geometry and line widths use the same units as the caller's advances.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod bands;
mod chrome;
pub use chrome::chrome_break;
mod collapse;
mod flow;
mod geometry;
mod shape;
mod walker;

pub use bands::intervals;
pub use collapse::{collapse, Collapsed, Edit};
pub use flow::{flow, Direction, FlowOptions, FlowResult, Fragment};
pub use geometry::{meets, FlowShape};
pub use shape::ShapeOutside;
pub use walker::{
    grapheme_ranges, utf16_words, Cursor, LineRange, Measure, Options, OverflowWrap, Prepared,
    WhiteSpace,
};

// Saturate arithmetic at the public f32 boundary; never emit NaN or infinity.
fn finite(value: f64) -> f32 {
    if value.is_nan() {
        0.0
    } else {
        value.clamp(-(f32::MAX as f64), f32::MAX as f64) as f32
    }
}

/// Minimum fragment width in strut ems, clamped to the paragraph width.
/// @ref LLP 1043.000 §3 D5 — four ems avoid stranded single short words.
pub const MIN_FRAGMENT_EM: f32 = 4.0;

// Compact heapsort: O(n log n), in-place, with no input-size fallback. Flow's
// small Copy records do not need std's general sort specialization machinery.
// @ref LLP 1043.000 §3 D7 — preserve the frame work bound while reducing wasm.
fn sort_by<T: Copy>(items: &mut [T], cmp: impl Fn(&T, &T) -> std::cmp::Ordering) {
    let sift = |items: &mut [T], mut root: usize, end: usize| {
        while root < end / 2 {
            let mut child = root * 2 + 1;
            if child + 1 < end && cmp(&items[child], &items[child + 1]).is_lt() {
                child += 1;
            }
            if !cmp(&items[root], &items[child]).is_lt() {
                break;
            }
            items.swap(root, child);
            root = child;
        }
    };
    let len = items.len();
    for root in (0..len / 2).rev() {
        sift(items, root, len);
    }
    for end in (1..len).rev() {
        items.swap(0, end);
        sift(items, 0, end);
    }
}
