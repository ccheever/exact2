use exact_textflow::{flow, FlowOptions, FlowShape, Options, Prepared};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    time::Instant,
};

struct CountAlloc;
static COUNTING: AtomicBool = AtomicBool::new(false);
static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
// This dedicated integration-test binary has one test, so other tests cannot
// contaminate its allocator counts. Production exact-textflow forbids unsafe.
unsafe impl GlobalAlloc for CountAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if COUNTING.load(Ordering::Relaxed) {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        }
        // SAFETY: pass the allocator contract through unchanged to System.
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: allocations and their original layouts come from System above.
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        if COUNTING.load(Ordering::Relaxed) {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        }
        // SAFETY: delegate the caller's valid allocation and layout to System.
        unsafe { System.realloc(ptr, layout, size) }
    }
}
#[global_allocator]
static ALLOCATOR: CountAlloc = CountAlloc;

#[test]
fn moving_circle_600_frames_no_allocations_after_first() {
    let text = "word ".repeat(1000);
    let prepared = Prepared::new(&text, Options::default(), &mut |r: std::ops::Range<
        usize,
    >| r.len() as f32 * 8.0);
    let options = FlowOptions {
        direction: exact_textflow::Direction::Ltr,
        width: 640.0,
        line_height: 22.0,
        min_fragment: 48.0,
        max_lines: 0,
    };
    let shape = |frame: usize| FlowShape::Circle {
        cx: 60.0 + (frame as f32 / 599.0) * 520.0,
        cy: 100.0 + 250.0 * ((frame as f32 / 599.0) * 9.0).sin().abs(),
        r: 98.0,
    };
    let mut out = Vec::new();
    flow(&prepared, &[shape(0)], &options, &mut out);
    assert_eq!(out.last().unwrap().end, text.len());
    let start = Instant::now();
    let mut fragments = 0;
    ALLOCATIONS.store(0, Ordering::Relaxed);
    COUNTING.store(true, Ordering::Relaxed);
    for frame in 0..600 {
        std::hint::black_box(flow(&prepared, &[shape(frame)], &options, &mut out));
        fragments += out.len();
        assert_eq!(out.last().unwrap().end, text.len());
    }
    COUNTING.store(false, Ordering::Relaxed);
    let micros = start.elapsed().as_secs_f64() * 1e6 / 600.0;
    let allocations = ALLOCATIONS.load(Ordering::Relaxed);
    println!("textflow: 1000 segments, 600 moving-circle frames: {micros:.3} µs/frame; {allocations} allocations; {:.1} fragments/frame",fragments as f64/600.0);
    assert_eq!(allocations, 0);
}
