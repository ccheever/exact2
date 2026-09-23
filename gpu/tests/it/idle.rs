// Count only allocations on this test's thread, including reallocations.
struct Counting;
thread_local! { static ALLOCATED: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) }; }
unsafe impl std::alloc::GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
        ALLOCATED.with(|n| {
            if let Some(bytes) = n.get() {
                n.set(Some(bytes + layout.size()));
            }
        });
        unsafe { std::alloc::System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: std::alloc::Layout) {
        unsafe { std::alloc::System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: std::alloc::Layout, size: usize) -> *mut u8 {
        ALLOCATED.with(|n| {
            if let Some(bytes) = n.get() {
                n.set(Some(bytes + size));
            }
        });
        unsafe { std::alloc::System.realloc(ptr, layout, size) }
    }
}
#[global_allocator]
static ALLOCATOR: Counting = Counting;
fn allocated(f: impl FnOnce()) -> usize {
    ALLOCATED.with(|n| n.set(Some(0)));
    f();
    ALLOCATED.with(|n| n.replace(None).unwrap())
}

#[test]
fn idle_message_abi_allocates_nothing() {
    // No module/device is required for the empty drain. Live instances use the
    // same empty Vec path, covered by module.rs after draining a real message.
    exact_gpu::native::messages(0);
    assert_eq!(
        allocated(|| {
            for _ in 0..1000 {
                drop(exact_gpu::native::messages(0));
            }
        }),
        0
    );
}
