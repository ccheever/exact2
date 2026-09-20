#![cfg(test)]
//! Allocation measurement support; unsafe stays inside storage even in test binaries.
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::{Cell, RefCell},
};
thread_local! { static COUNT: Cell<Option<(usize,usize)>> = const { Cell::new(None) }; }
thread_local! {
    static TRACE: RefCell<Option<(usize, [usize; 256])>> = const { RefCell::new(None) };
}
struct Counter;
fn account(bytes: usize) {
    let _ = COUNT.try_with(|c| {
        if let Some((n, b)) = c.get() {
            c.set(Some((n + 1, b + bytes)));
            let _ = TRACE.try_with(|trace| {
                if let Some((len, sizes)) = trace.borrow_mut().as_mut() {
                    if *len < sizes.len() {
                        sizes[*len] = bytes;
                        *len += 1;
                    }
                }
            });
        }
    });
}
// SAFETY: every allocation is forwarded unchanged to System with its original layout.
unsafe impl GlobalAlloc for Counter {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        account(layout.size());
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        account(layout.size());
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        account(new_size);
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}
#[global_allocator]
static ALLOCATOR: Counter = Counter;
pub fn measure<T>(f: impl FnOnce() -> T) -> (T, (usize, usize)) {
    struct Stop;
    impl Drop for Stop {
        fn drop(&mut self) {
            COUNT.set(None);
        }
    }
    assert!(COUNT.get().is_none());
    COUNT.set(Some((0, 0)));
    let stop = Stop;
    let result = f();
    let count = COUNT.get().unwrap();
    drop(stop);
    (result, count)
}

pub fn histogram<T>(f: impl FnOnce() -> T) -> (T, (usize, usize), Vec<(usize, usize)>) {
    TRACE.with(|t| *t.borrow_mut() = Some((0, [0; 256])));
    let (result, counts) = measure(f);
    let (len, sizes) = TRACE.with(|t| t.borrow_mut().take().unwrap());
    assert_eq!(len, counts.0, "trace capacity");
    let mut bins = std::collections::BTreeMap::new();
    for &size in &sizes[..len] {
        *bins.entry(size).or_insert(0) += 1;
    }
    (result, counts, bins.into_iter().collect())
}
