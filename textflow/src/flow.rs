//! @ref LLP 1043.000 §3 D6 — one band, multiple fragments, logical reading order.
use crate::{
    bands::{append_intervals, Slot},
    finite, Cursor, FlowShape, Prepared,
};

/// One consumed source range positioned in a line band; bidi is the painter's job.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fragment {
    /// Inclusive UTF-8 source offset.
    pub start: usize,
    /// Exclusive source offset, including collapsed spaces and hard-break controls.
    pub end: usize,
    /// Left origin of the fragment.
    pub x: f32,
    /// Top of the line band.
    pub y: f32,
    /// Painted advance, excluding hanging whitespace.
    pub width: f32,
    /// This break paints a discretionary hyphen.
    pub hyphenated: bool,
    /// Zero-based band index; skipped obstructed bands count too.
    pub line: u32,
}
/// CSS paragraph base direction; source ranges always remain logical.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Direction {
    /// Consume same-band intervals left to right.
    #[default]
    Ltr,
    /// Consume same-band intervals right to left.
    Rtl,
}
/// Geometry and stopping limits for a paragraph flow.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlowOptions {
    /// Paragraph direction controls interval traversal.
    pub direction: Direction,
    /// Width of the paragraph; negative/NaN becomes zero.
    pub width: f32,
    /// Positive finite line-band height; invalid values return an empty result.
    pub line_height: f32,
    /// Minimum usable free interval; negative/nonfinite becomes zero.
    pub min_fragment: f32,
    /// Maximum number of bands, including skipped bands; zero means no user cap.
    pub max_lines: u32,
}
/// Outcome of a bounded paragraph walk.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FlowResult {
    /// Bottom of the visited bands.
    pub height: f32,
    /// All noncollapsible source content was consumed.
    pub complete: bool,
    /// Incomplete specifically because the caller's line clamp was reached.
    pub clamped: bool,
}
impl Slot for Fragment {
    fn interval(a: f32, b: f32) -> Self {
        Self {
            start: 0,
            end: 0,
            x: a,
            y: 0.0,
            width: b - a,
            line: 0,
            hyphenated: false,
        }
    }
    fn edges(self) -> (f32, f32) {
        (self.x, self.x + self.width)
    }
}

/// Clear `out`, flow text around shapes, and report height and completion.
///
/// Ranges own every consumed source byte exactly once, including hanging spaces,
/// leading spaces and break controls; painters collapse these as `Prepared` does.
/// Entirely space-only input emits nothing. A hard break ends the whole band.
/// An obstructed interval must fit the next fragment; a clear band may overflow.
/// The sliver floor is clamped to the paragraph width; shapes always subtract.
/// Zero width produces no fragments (incomplete for nonempty text).
///
/// The unused tail of `out` is band scratch and retains its peak capacity.
/// Concave polygons can emit several intervals per shape. CSS polygons (at most
/// 64 vertices) use stack geometry scratch; larger programmatic polygons also
/// allocate O(V²) temporary topology storage per query.
/// Ordinary flow takes O(G + B*(E + I log I)), with G cached segments/graphemes,
/// B bands, E geometry-query work (including polygon topology slabs) and I emitted
/// exclusion intervals per band. The general worst case includes rejected line
/// probes: O(B*(G*(I+1) + E + I log I)). An unfit soft
/// hyphen may require inspecting later opportunities before rejecting a slot.
/// A single fixed-complexity shape and ordinary words take O(G+B); arbitrarily
/// ordered exclusion intervals cannot promise O(B*I) merging.
///
/// A guard stops after max(1024, 8*G) probes, independently of vertical jumps.
/// Flat fully blocked bands jump to their next shape boundary; curved/variable
/// boundaries retain band-sized probes. Partial output must be explicitly refused
/// by callers unless `clamped` is true. Unrepresentable heights also stop.
pub fn flow(
    prepared: &Prepared,
    shapes: &[FlowShape],
    options: &FlowOptions,
    out: &mut Vec<Fragment>,
) -> FlowResult {
    out.clear();
    if !options.line_height.is_finite() || options.line_height <= 0.0 {
        return FlowResult {
            complete: !prepared.has_remaining(Cursor::default()),
            ..FlowResult::default()
        };
    }
    out.reserve(
        prepared
            .work_units()
            .saturating_add(shapes.len())
            .saturating_add(1),
    );
    let width = if options.width.is_finite() {
        options.width.max(0.0)
    } else {
        0.0
    };
    let min = if options.min_fragment.is_finite() {
        options.min_fragment.max(0.0).min(width)
    } else {
        0.0
    };
    if width == 0.0 {
        return FlowResult {
            complete: !prepared.has_remaining(Cursor::default()),
            ..FlowResult::default()
        };
    }
    let guard = 1024_usize
        .max(prepared.work_units().saturating_mul(8))
        .min(u32::MAX as usize) as u32;
    let mut cursor = Cursor::default();
    let mut height = 0.0;
    let mut band = 0u32;
    for _ in 0..guard {
        if options.max_lines != 0 && band >= options.max_lines {
            break;
        }
        // Also avoids visiting empty bands for exhausted or whitespace-only text.
        if !prepared.has_remaining(cursor) {
            break;
        }
        let top = finite(band as f64 * options.line_height as f64);
        let bottom = finite((band as f64 + 1.0) * options.line_height as f64);
        if bottom <= top {
            break;
        }
        let start = out.len();
        append_intervals(shapes, top, bottom, width, min, out);
        // @ref LLP 1043.000 §3 D6 — bidi fragments follow the paragraph direction.
        if options.direction == Direction::Rtl {
            out[start..].reverse();
        }
        let end = out.len();
        if end == start {
            // Only jump over bands proven unchanged: flat rectangle interiors or
            // one silhouette row. A curved edge can reopen an interval earlier.
            let boundary = shapes
                .iter()
                .filter(|s| s.extent(top as f64, bottom as f64).is_some())
                .map(|s| match s {
                    FlowShape::RoundRect {
                        y, height, radius, ..
                    } if top >= y + radius && bottom <= y + height - radius => y + height - radius,
                    FlowShape::Spans { y, row_height, .. } if *row_height > 0.0 => {
                        y + (((top - y) / row_height).floor() + 1.0) * row_height
                    }
                    _ => bottom,
                })
                .fold(f32::INFINITY, f32::min);
            let next = if boundary.is_finite() {
                ((boundary as f64 / options.line_height as f64).ceil() as u32)
                    .max(band.saturating_add(1))
            } else {
                band.saturating_add(1)
            };
            band = if options.max_lines > 0 {
                next.min(options.max_lines)
            } else {
                next
            };
            height = finite(band as f64 * options.line_height as f64);
            continue;
        }
        let clear = end == start + 1 && out[start].x == 0.0 && out[start].width == width;
        let mut write = start;
        for read in start..end {
            let slot = out[read];
            let Some(line) = prepared.next_line(cursor, slot.width) else {
                break;
            };
            if !clear && line.width > slot.width {
                continue;
            }
            out[write] = Fragment {
                start: line.start.byte,
                end: line.end.byte,
                x: slot.x,
                y: top,
                width: line.width,
                line: band,
                hyphenated: line.hyphenated,
            };
            write += 1;
            cursor = line.end;
            if line.hard_break {
                break;
            }
        }
        out.truncate(write);
        height = bottom;
        band = band.saturating_add(1);
    }
    let complete = !prepared.has_remaining(cursor);
    FlowResult {
        height,
        complete,
        clamped: !complete && options.max_lines > 0 && band >= options.max_lines,
    }
}
