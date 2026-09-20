#![cfg(test)]
//! Allocation measurement support; unsafe stays inside storage even in test binaries.
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};
thread_local! { static COUNT: Cell<Option<(usize,usize)>> = const { Cell::new(None) }; }
struct Counter;
fn account(bytes: usize) {
    let _ = COUNT.try_with(|c| {
        if let Some((n, b)) = c.get() {
            c.set(Some((n + 1, b + bytes)));
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
