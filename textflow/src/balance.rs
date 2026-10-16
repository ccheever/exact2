//! CSS `text-wrap-style: balance` (CSS Text 4 §6.1.2), bounded as Chromium
//! bounds it. @ref LLP 1001 §6 "Balanced wrapping"
//!
//! Balancing is a search over the host's own greedy breaker: the narrowest
//! width that keeps the line count the box's width gives. Each host runs it
//! over its paragraph's lines up to the first forced break (Chrome balances
//! only those, and breaks the rest greedily at the box's width), measuring
//! with the shaping it paints.

/// The most lines a paragraph balances; one longer breaks greedily, as in
/// Chromium (Firefox balances up to ten).
pub const BALANCE_MAX_LINES: usize = 6;

/// The width a balanced paragraph breaks at: the narrowest, to 1/64 px (a
/// browser layout unit), at which `lines(width)` (the greedy line count of
/// the lines being balanced) is still what it is at `width`. `None` leaves
/// the paragraph greedy: no finite positive width, one line, or more than
/// [`BALANCE_MAX_LINES`]. The box keeps `width`; only the breaks move, so
/// alignment and justification still fill `width`.
pub fn balanced_width(width: f32, mut lines: impl FnMut(f32) -> usize) -> Option<f32> {
    if !(width.is_finite() && width > 0.0) {
        return None;
    }
    let count = lines(width);
    if !(2..=BALANCE_MAX_LINES).contains(&count) {
        return None;
    }
    // In 1/64 px units: `lines(lo)` breaks into more lines, `lines(hi)` no more.
    let (mut lo, mut hi) = (0u32, (width * 64.0).floor().min(u32::MAX as f32) as u32);
    let at = |units: u32| units as f32 / 64.0;
    if lines(at(hi)) > count {
        return Some(width);
    }
    while hi - lo > 1 {
        let mid = lo + (hi - lo) / 2;
        if lines(at(mid)) > count {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    Some(at(hi))
}
