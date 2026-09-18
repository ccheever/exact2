//! Measure requested allocation capacities in the decoder's calling thread.
//! The test allocator is absent from production; encoded fixture bytes exist
//! before tracking begins. Realloc counts old+new until the old block retires.
#![allow(unsafe_code)]
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

thread_local! {
    static TRACK: Cell<bool> = const { Cell::new(false) };
    static LIVE: Cell<usize> = const { Cell::new(0) };
    static PEAK: Cell<usize> = const { Cell::new(0) };
}
struct Observed;
#[global_allocator]
static ALLOCATOR: Observed = Observed;
fn add(n: usize) {
    if TRACK.try_with(Cell::get).unwrap_or(false) {
        LIVE.with(|v| {
            v.set(v.get() + n);
            PEAK.with(|p| p.set(p.get().max(v.get())));
        });
    }
}
fn sub(n: usize) {
    if TRACK.try_with(Cell::get).unwrap_or(false) {
        LIVE.with(|v| v.set(v.get().saturating_sub(n)));
    }
}
// SAFETY: every operation delegates the exact pointer/layout contract to
// System; observations never read or modify the allocation itself.
unsafe impl GlobalAlloc for Observed {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            add(layout.size());
        }
        ptr
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc_zeroed(layout) };
        if !ptr.is_null() {
            add(layout.size());
        }
        ptr
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) };
        sub(layout.size());
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        add(size);
        let new = unsafe { System.realloc(ptr, layout, size) };
        if new.is_null() {
            sub(size);
        } else {
            sub(layout.size());
        }
        new
    }
}

#[test]
fn measured_decoder_capacity_stays_under_reserved_peak_for_tall_wide_and_adam7_pngs() {
    use super::{
        decode_tests::{adam7, png},
        png_decode::{decode_rows, inspect, DecodePlan},
    };
    use std::io::Cursor;
    for (name, bytes, target) in [
        ("large", png(4000, 2000), (100, 50)),
        ("wide", png(80_000, 1), (100, 1)),
        ("tall", png(1, 8192), (1, 100)),
        ("adam7", adam7(19, 11), (7, 5)),
    ] {
        LIVE.with(|n| n.set(0));
        PEAK.with(|n| n.set(0));
        TRACK.with(|t| t.set(true));
        let header = inspect(&mut Cursor::new(&bytes), bytes.len() as u64).unwrap();
        TRACK.with(|t| t.set(false));
        assert_eq!(
            PEAK.with(Cell::get),
            0,
            "metadata inspection allocates nothing"
        );
        let plan = DecodePlan::new(header, target).unwrap();
        LIVE.with(|n| n.set(0));
        PEAK.with(|n| n.set(0));
        TRACK.with(|t| t.set(true));
        let result = decode_rows(Cursor::new(&bytes), &plan, || false);
        let succeeded = result.is_ok();
        drop(result);
        TRACK.with(|t| t.set(false));
        let peak = PEAK.with(Cell::get);
        let remaining = LIVE.with(Cell::get);
        eprintln!(
            "{name}: measured capacity peak={peak}; reserved={}; remaining={remaining}",
            plan.peak_bytes()
        );
        assert!(succeeded, "{name}: decoder must actually produce pixels");
        assert_eq!(remaining, 0);
        assert!(
            peak as u64 <= plan.peak_bytes(),
            "{name}: {peak} > {}",
            plan.peak_bytes()
        );
    }
}
