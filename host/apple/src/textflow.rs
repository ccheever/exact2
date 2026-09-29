//! @ref LLP 1043.000 §3 D5–D7 — CoreText advances into the shared walker.
//! Handles are thread-confined, never reused; stale/null handles are harmless.
//! Pointer buffers are borrowed only during a call. Non-null pointers must name
//! valid, aligned buffers of the stated length, as with the measure callback.
#![allow(unsafe_code)]
#![allow(clippy::not_unsafe_ptr_arg_deref)]
use exact_textflow::{FlowOptions, FlowShape, Fragment, Options, OverflowWrap, Prepared};
use std::{
    collections::HashMap,
    sync::atomic::{AtomicU64, Ordering},
};

/// The ABI's CSS `white-space` (the schema's wire order): 0 normal,
/// 1 pre-wrap, 2 nowrap, 3 pre-line, 4 pre.
pub(crate) fn white_space_mode(code: u32) -> exact_textflow::WhiteSpace {
    match code {
        1 => exact_textflow::WhiteSpace::PreWrap,
        2 => exact_textflow::WhiteSpace::Nowrap,
        3 => exact_textflow::WhiteSpace::PreLine,
        4 => exact_textflow::WhiteSpace::Pre,
        _ => exact_textflow::WhiteSpace::Normal,
    }
}

/// A counted pair used for polygon vertices or silhouette intervals.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Pair {
    /// X coordinate or start.
    pub x: f32,
    /// Y coordinate or end.
    pub y: f32,
}
/// Flat geometry: 0 circle, 1 ellipse, 2 rounded rectangle, 3 polygon, 4 spans.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Shape {
    /// Shape tag.
    pub kind: u32,
    /// Center for circle/ellipse, origin for rectangle/spans.
    pub x: f32,
    /// Center or origin.
    pub y: f32,
    /// Circle radius, ellipse rx, rectangle width, or spans row height.
    pub a: f32,
    /// Ellipse ry or rectangle height.
    pub b: f32,
    /// Rectangle radius.
    pub radius: f32,
    /// Polygon vertices or spans rows.
    pub pairs: *const Pair,
    /// Pair count.
    pub count: usize,
}
/// A positioned fragment, with both source encodings and a trimmed paint range.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct CFragment {
    /// Consumed UTF-8 start.
    pub start: usize,
    /// Consumed UTF-8 end.
    pub end: usize,
    /// Consumed UTF-16 start.
    pub utf16_start: usize,
    /// Consumed UTF-16 end.
    pub utf16_end: usize,
    /// Trimmed UTF-16 paint start.
    pub paint_start: usize,
    /// Trimmed UTF-16 paint end.
    pub paint_end: usize,
    /// Fragment origin.
    pub x: f32,
    /// Band top.
    pub y: f32,
    /// Painted advance.
    pub width: f32,
    /// Available interval width for alignment/ellipsis.
    pub available: f32,
    /// Selected discretionary hyphen; the painter must supply its ink.
    pub hyphenated: u8,
    /// Band index, including skipped bands.
    pub line: u32,
}
/// Full fragment count and height, even when the supplied output is smaller.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Result {
    /// Required output count; only min(count, capacity) entries were written.
    pub count: usize,
    /// Visited band height.
    pub height: f32,
    /// Owned Rust array/string capacity for residency accounting.
    pub bytes: usize,
    /// All paragraph content was consumed.
    pub complete: u8,
    /// Caller line-clamp caused the partial result.
    pub clamped: u8,
}
struct Source {
    text: String,
    utf16: Vec<usize>,
    prepared: Prepared,
    fragments: Vec<Fragment>,
    intervals: Vec<(f32, f32)>,
}
// Process-wide, not per thread (LLP 1072 §2.3): a source prepared while the
// owner thread measures is flowed and freed by whichever thread paints it.
static SOURCES: std::sync::Mutex<Option<HashMap<u64, Source>>> = std::sync::Mutex::new(None);
fn with_sources<R>(f: impl FnOnce(&mut HashMap<u64, Source>) -> R) -> R {
    let mut guard = SOURCES
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    f(guard.get_or_insert_with(HashMap::new))
}
static NEXT: AtomicU64 = AtomicU64::new(1);
fn slice<'a, T>(p: *const T, n: usize) -> Option<&'a [T]> {
    if n == 0 {
        Some(&[])
    } else if p.is_null() || n > isize::MAX as usize / std::mem::size_of::<T>() {
        None
    } else {
        Some(unsafe { std::slice::from_raw_parts(p, n) })
    }
}
/// Prepare once from UTF-8 and one advance per UTF-16 unit. Invalid input returns 0.
/// Cluster advances belong to their lowest string index; continuation units are 0.
/// Words are ascending UTF-16 line-break boundaries between Thai, Lao, Khmer or
/// Myanmar letters (`CFStringTokenizer`'s); the walker has no dictionary.
#[allow(clippy::too_many_arguments)]
pub fn prepare(
    text: *const u8,
    len: usize,
    advances: *const f32,
    count: usize,
    words: *const u32,
    word_count: usize,
    overflow_wrap: u32,
    white_space: u32,
    hyphen_advance: f32,
) -> u64 {
    let Some(text) = slice(text, len).and_then(|b| std::str::from_utf8(b).ok()) else {
        return 0;
    };
    let Some(advances) = slice(advances, count) else {
        return 0;
    };
    let Some(words) = slice(words, word_count) else {
        return 0;
    };
    let words = exact_textflow::utf16_words(text, words.iter().map(|&w| w as usize));
    if text.encode_utf16().count() != count {
        return 0;
    }
    let mut utf16 = vec![0; len + 1];
    let mut offset = 0;
    for (byte, ch) in text.char_indices() {
        utf16[byte..byte + ch.len_utf8()].fill(offset);
        offset += ch.len_utf16();
    }
    utf16[len] = offset;
    let mut prefix = Vec::with_capacity(count + 1);
    prefix.push(0.0f64);
    for advance in advances {
        prefix.push(
            prefix.last().unwrap()
                + if advance.is_finite() {
                    advance.max(0.0) as f64
                } else {
                    0.0
                },
        );
    }
    let prepared = Prepared::with_words(
        text,
        Options {
            white_space: white_space_mode(white_space),
            overflow_wrap: match overflow_wrap {
                1 => OverflowWrap::BreakWord,
                2 => OverflowWrap::Anywhere,
                _ => OverflowWrap::Normal,
            },
            hyphen_advance,
        },
        &words,
        &mut |range: std::ops::Range<usize>| {
            (prefix[utf16[range.end]] - prefix[utf16[range.start]]) as f32
        },
    );
    let Ok(id) = NEXT.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
    else {
        return 0;
    };
    with_sources(|sources| {
        sources.insert(
            id,
            Source {
                text: text.into(),
                utf16,
                prepared,
                fragments: Vec::new(),
                intervals: Vec::new(),
            },
        )
    });
    id
}
/// Borrow a kernel shape for the duration of a synchronous measure callback.
pub fn borrowed(shape: &FlowShape) -> Shape {
    let mut c = Shape {
        kind: 0,
        x: 0.,
        y: 0.,
        a: 0.,
        b: 0.,
        radius: 0.,
        pairs: std::ptr::null(),
        count: 0,
    };
    match shape {
        FlowShape::Circle { cx, cy, r } => {
            c.x = *cx;
            c.y = *cy;
            c.a = *r;
        }
        FlowShape::Ellipse { cx, cy, rx, ry } => {
            c.kind = 1;
            c.x = *cx;
            c.y = *cy;
            c.a = *rx;
            c.b = *ry;
        }
        FlowShape::RoundRect {
            x,
            y,
            width,
            height,
            radius,
        } => {
            c.kind = 2;
            c.x = *x;
            c.y = *y;
            c.a = *width;
            c.b = *height;
            c.radius = *radius;
        }
        // Tuple layout is not a C contract: callbacks use `Shapes` below to own pairs.
        FlowShape::Polygon(_) | FlowShape::EvenOddPolygon(_) | FlowShape::Spans { .. } => {
            unreachable!("use Shapes for counted pairs")
        }
    }
    c
}
/// Owns the counted C pair buffers alongside flat shapes; safe through callbacks.
pub struct Shapes {
    /// Flat array, valid while this value lives.
    pub flat: Vec<Shape>,
    _pairs: Vec<Vec<Pair>>,
}
impl Shapes {
    /// Convert resolved kernel geometry without relying on Rust tuple layout.
    pub fn new(shapes: &[FlowShape]) -> Self {
        let mut flat = Vec::with_capacity(shapes.len());
        let mut pairs = Vec::new();
        for shape in shapes {
            let (kind, x, y, a, rows) = match shape {
                FlowShape::Polygon(rows) => (3, 0., 0., 0., rows),
                FlowShape::EvenOddPolygon(rows) => (5, 0., 0., 0., rows),
                FlowShape::Spans {
                    x,
                    y,
                    row_height,
                    rows,
                } => (4, *x, *y, *row_height, rows),
                _ => {
                    flat.push(borrowed(shape));
                    continue;
                }
            };
            let row: Vec<_> = rows.iter().map(|&(x, y)| Pair { x, y }).collect();
            flat.push(Shape {
                kind,
                x,
                y,
                a,
                b: 0.,
                radius: 0.,
                pairs: row.as_ptr(),
                count: row.len(),
            });
            pairs.push(row);
        }
        Self {
            flat,
            _pairs: pairs,
        }
    }
}
fn decode(c: &Shape) -> Option<FlowShape> {
    Some(match c.kind {
        0 => FlowShape::Circle {
            cx: c.x,
            cy: c.y,
            r: c.a,
        },
        1 => FlowShape::Ellipse {
            cx: c.x,
            cy: c.y,
            rx: c.a,
            ry: c.b,
        },
        2 => FlowShape::RoundRect {
            x: c.x,
            y: c.y,
            width: c.a,
            height: c.b,
            radius: c.radius,
        },
        3 | 5 => {
            let points = slice(c.pairs, c.count)?
                .iter()
                .map(|p| (p.x, p.y))
                .collect();
            if c.kind == 5 {
                FlowShape::EvenOddPolygon(points)
            } else {
                FlowShape::Polygon(points)
            }
        }
        4 => FlowShape::Spans {
            x: c.x,
            y: c.y,
            row_height: c.a,
            rows: slice(c.pairs, c.count)?
                .iter()
                .map(|p| (p.x, p.y))
                .collect(),
        },
        _ => return None,
    })
}

/// Reflow retained advances. Work/termination follow exact_textflow::flow's band
/// guard; past it output is partial, detectable from the final consumed end.
/// Null output supports count/height queries; malformed shapes return an empty result.
#[allow(clippy::too_many_arguments)]
pub fn flow(
    handle: u64,
    shapes: *const Shape,
    count: usize,
    width: f32,
    line_height: f32,
    font_size: f32,
    max_lines: u32,
    direction: u32,
    out: *mut CFragment,
    cap: usize,
) -> Result {
    let min_fragment = font_size * exact_textflow::MIN_FRAGMENT_EM;
    let Some(shapes) =
        slice(shapes, count).and_then(|s| s.iter().map(decode).collect::<Option<Vec<_>>>())
    else {
        return Result::default();
    };
    with_sources(|sources| {
        let Some(source) = sources.get_mut(&handle) else {
            return Result::default();
        };
        let result = exact_textflow::flow(
            &source.prepared,
            &shapes,
            &FlowOptions {
                direction: if direction == 1 {
                    exact_textflow::Direction::Rtl
                } else {
                    exact_textflow::Direction::Ltr
                },
                width,
                line_height,
                min_fragment,
                max_lines,
            },
            &mut source.fragments,
        );
        let mut band = u32::MAX;
        for (i, f) in source
            .fragments
            .iter()
            .enumerate()
            .take(if out.is_null() { 0 } else { cap })
        {
            if f.line != band {
                band = f.line;
                exact_textflow::intervals(
                    &shapes,
                    f.y,
                    f.y + line_height,
                    width,
                    min_fragment,
                    &mut source.intervals,
                );
            }
            let available = source
                .intervals
                .iter()
                .find(|(x, _)| *x == f.x)
                .map_or(width, |(a, b)| b - a);
            let paint = source.prepared.paint_range(&source.text, f.start..f.end);
            let (start, end) = (paint.start, paint.end);
            let item = CFragment {
                start: f.start,
                end: f.end,
                utf16_start: source.utf16[f.start],
                utf16_end: source.utf16[f.end],
                paint_start: source.utf16[start],
                paint_end: source.utf16[end],
                x: f.x,
                y: f.y,
                width: f.width,
                available,
                line: f.line,
                hyphenated: u8::from(f.hyphenated),
            };
            unsafe { out.add(i).write(item) };
        }
        Result {
            count: source.fragments.len(),
            height: result.height,
            complete: u8::from(result.complete),
            clamped: u8::from(result.clamped),
            bytes: source.text.capacity()
                + source.utf16.capacity() * std::mem::size_of::<usize>()
                + source.prepared.capacity_bytes()
                + source.fragments.capacity() * std::mem::size_of::<Fragment>()
                + source.intervals.capacity() * std::mem::size_of::<(f32, f32)>(),
        }
    })
}
/// Release a prepared source. Zero, stale, and repeated frees are no-ops.
pub fn free(handle: u64) {
    with_sources(|sources| sources.remove(&handle));
}

/// Export the text-shape-owned walker seam from the application's static archive.
#[macro_export]
macro_rules! textflow_exports {
    () => {
        /// Prepare retained Unicode break opportunities and advances.
        #[no_mangle]
        #[allow(clippy::too_many_arguments)]
        pub extern "C" fn exact_textflow_prepare(
            text: *const u8,
            len: usize,
            advances: *const f32,
            count: usize,
            words: *const u32,
            word_count: usize,
            overflow_wrap: u32,
            white_space: u32,
            hyphen_advance: f32,
        ) -> u64 {
            $crate::textflow::prepare(
                text,
                len,
                advances,
                count,
                words,
                word_count,
                overflow_wrap,
                white_space,
                hyphen_advance,
            )
        }
        /// Flow a prepared source, writing at most cap fragments.
        #[no_mangle]
        pub extern "C" fn exact_textflow_flow(
            handle: u64,
            shapes: *const $crate::textflow::Shape,
            count: usize,
            width: f32,
            line_height: f32,
            font_size: f32,
            max_lines: u32,
            direction: u32,
            out: *mut $crate::textflow::CFragment,
            cap: usize,
        ) -> $crate::textflow::Result {
            $crate::textflow::flow(
                handle,
                shapes,
                count,
                width,
                line_height,
                font_size,
                max_lines,
                direction,
                out,
                cap,
            )
        }
        /// Release one prepared source.
        #[no_mangle]
        pub extern "C" fn exact_textflow_free(handle: u64) {
            $crate::textflow::free(handle)
        }
    };
}

#[cfg(test)]
#[path = "textflow_tests.rs"]
mod tests;
