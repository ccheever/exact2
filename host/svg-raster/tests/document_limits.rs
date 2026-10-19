//! A small document that usvg or tiny-skia would expand without bound is
//! refused before the expansion is allocated: fast, and within a few MiB of
//! this thread's heap. A per-thread allocation count measures it, so tests
//! running beside it do not.
use exact_svg_raster::document::{render, size, Error};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::time::{Duration, Instant};

struct Counting;

thread_local! {
    static LIVE: Cell<usize> = const { Cell::new(0) };
    static PEAK: Cell<usize> = const { Cell::new(0) };
}

fn track(grow: usize, shrink: usize) {
    let _ = LIVE.try_with(|live| {
        let now = live.get().saturating_add(grow).saturating_sub(shrink);
        live.set(now);
        let _ = PEAK.try_with(|peak| peak.set(peak.get().max(now)));
    });
}

// SAFETY: every call forwards to the system allocator unchanged.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        track(layout.size(), 0);
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        track(0, layout.size());
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new: usize) -> *mut u8 {
        track(new, layout.size());
        unsafe { System.realloc(ptr, layout, new) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

/// The call's time and the most heap it held above what was live before it.
fn measure<T>(call: impl FnOnce() -> T) -> (T, Duration, usize) {
    let base = LIVE.with(Cell::get);
    PEAK.with(|peak| peak.set(base));
    let start = Instant::now();
    let result = call();
    let elapsed = start.elapsed();
    (result, elapsed, PEAK.with(Cell::get) - base)
}

const FAST: Duration = Duration::from_millis(250);
const SMALL: usize = 4 * 1024 * 1024;

fn refused_fast_and_small(source: &str, natural: (f32, f32)) {
    let mut pixels = vec![0; 64 * 64 * 4];
    let (result, elapsed, heap) = measure(|| {
        render(
            source.as_bytes(),
            &mut pixels,
            64,
            64,
            256,
            natural,
            (64.0, 64.0),
        )
    });
    assert_eq!(result, Err(Error::Limit));
    assert!(elapsed < FAST, "render refused in {elapsed:?}");
    assert!(heap < SMALL, "render held {heap} bytes");
    assert!(pixels.iter().all(|byte| *byte == 0));
}

/// 16 levels of groups that each use the previous one twice, over a path
/// with 2,000 vertices: 14 KB that usvg expands to 131 million vertices.
fn doubling(levels: usize, root: &str) -> String {
    let mut d = String::from("M0 0");
    for i in 0..1999 {
        d += &format!(" L{} {}", i % 64, i * 7 % 64);
    }
    let mut defs = format!("<g id='g0'><path d='{d}' fill='none' stroke='black'/></g>");
    for i in 1..=levels {
        defs += &format!(
            "<g id='g{i}'><use href='#g{0}'/><use href='#g{0}'/></g>",
            i - 1
        );
    }
    format!("<svg xmlns='http://www.w3.org/2000/svg' {root}><defs>{defs}</defs><use href='#g{levels}'/></svg>")
}

#[test]
fn use_expansion_is_refused_before_conversion() {
    let source = doubling(16, "width='64' height='64'");
    assert!(source.len() < 15 * 1024);
    refused_fast_and_small(&source, (64.0, 64.0));
    // A viewBox-only root takes usvg's size resolver: metadata inspection,
    // before any reservation, must refuse too.
    let source = doubling(16, "viewBox='0 0 64 64'");
    let (result, elapsed, heap) = measure(|| size(source.as_bytes()));
    assert_eq!(result, Err(Error::Limit));
    assert!(elapsed < FAST, "size refused in {elapsed:?}");
    assert!(heap < SMALL, "size held {heap} bytes");
    // Seven levels (256 copies' worth of vertices) are an ordinary document.
    let source = doubling(7, "width='64' height='64'");
    let mut pixels = vec![0; 64 * 64 * 4];
    render(
        source.as_bytes(),
        &mut pixels,
        64,
        64,
        256,
        (64.0, 64.0),
        (64.0, 64.0),
    )
    .unwrap();
    assert!(pixels.chunks_exact(4).any(|pixel| pixel[3] > 0));
}

#[test]
fn marker_instances_are_refused_before_conversion() {
    // 12,000 vertices, each given a marker of 50 circles: 82 KB.
    let mut d = String::from("M0 0");
    for i in 0..11_999 {
        d += &format!(" L{} {}", i % 64, i * 13 % 64);
    }
    let circles: String = (0..50)
        .map(|i| format!("<circle cx='{}' cy='2' r='1'/>", i % 4))
        .collect();
    let source = format!("<svg xmlns='http://www.w3.org/2000/svg' width='64' height='64'><defs><marker id='m' markerWidth='4' markerHeight='4'>{circles}</marker></defs><path d='{d}' fill='none' stroke='black' marker-mid='url(#m)'/></svg>");
    assert!(source.len() > 80_000);
    refused_fast_and_small(&source, (64.0, 64.0));
}

#[test]
fn stroke_dashes_are_bounded_per_document() {
    let path = "<path d='M0 0H64V64H0Z' stroke='black' stroke-dasharray='0.0003' fill='none'/>";
    let source = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='64' height='64'>{}</svg>",
        path.repeat(300)
    );
    refused_fast_and_small(&source, (64.0, 64.0));
    // One copy is past tiny-skia's own million per path, where it would
    // otherwise stroke the path solid and still report success.
    let source = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='64' height='64'>{}</svg>",
        path.replace("0.0003", "0.0001")
    );
    refused_fast_and_small(&source, (64.0, 64.0));
    // Dashes in a pattern tile, a mask and text count as well.
    for content in [
        "<defs><pattern id='p' patternUnits='userSpaceOnUse' width='64' height='64'><path d='M0 0H64V64H0Z' stroke='black' stroke-dasharray='0.001' fill='none'/></pattern></defs><rect width='64' height='64' fill='url(#p)'/>",
        "<defs><mask id='m'><path d='M0 0H64V64H0Z' stroke='white' stroke-dasharray='0.001' fill='none'/></mask></defs><rect width='64' height='64' mask='url(#m)'/>",
    ] {
        let source = format!("<svg xmlns='http://www.w3.org/2000/svg' width='64' height='64'>{content}</svg>");
        refused_fast_and_small(&source, (64.0, 64.0));
    }
    // An ordinary dashed border renders, dashed.
    let source = "<svg xmlns='http://www.w3.org/2000/svg' width='64' height='64'><path d='M0 0.5H64' stroke='black' stroke-dasharray='4' fill='none'/></svg>";
    let mut pixels = vec![0; 64 * 64 * 4];
    render(
        source.as_bytes(),
        &mut pixels,
        64,
        64,
        256,
        (64.0, 64.0),
        (64.0, 64.0),
    )
    .unwrap();
    assert!(pixels[2 * 4 + 3] > 0, "a dash");
    assert_eq!(pixels[6 * 4 + 3], 0, "a gap");
}
