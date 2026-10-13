//! @ref LLP 0068.000#3-ownership-callbacks-and-cancellation — refund only after upload bytes are freed

use ibex2::stdlib::fetch_async::Upload;
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use std::sync::Arc;

struct ObservedAllocator;

static WATCHED: AtomicPtr<u8> = AtomicPtr::new(std::ptr::null_mut());
static FREED: AtomicBool = AtomicBool::new(false);

unsafe impl GlobalAlloc for ObservedAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let watched = ptr == WATCHED.load(Ordering::SeqCst);
        unsafe { System.dealloc(ptr, layout) };
        if watched {
            FREED.store(true, Ordering::SeqCst);
        }
    }
}

#[global_allocator]
static ALLOCATOR: ObservedAllocator = ObservedAllocator;

#[test]
fn final_upload_notification_observes_the_freed_allocation() {
    let bytes = vec![7; 128 * 1024];
    WATCHED.store(bytes.as_ptr().cast_mut(), Ordering::SeqCst);
    FREED.store(false, Ordering::SeqCst);
    let notified = Arc::new(AtomicBool::new(false));
    let freed_before_notification = Arc::new(AtomicBool::new(false));
    let on_release = notified.clone();
    let observed = freed_before_notification.clone();
    let upload = Upload::new(
        bytes,
        Some(Box::new(move || {
            observed.store(FREED.load(Ordering::SeqCst), Ordering::SeqCst);
            on_release.store(true, Ordering::SeqCst);
        })),
    );
    let platform = upload.clone();
    drop(upload);
    assert!(!notified.load(Ordering::SeqCst));
    assert!(!FREED.load(Ordering::SeqCst));
    assert_eq!(platform.bytes().len(), 128 * 1024);
    drop(platform);
    WATCHED.store(std::ptr::null_mut(), Ordering::SeqCst);
    assert!(notified.load(Ordering::SeqCst));
    assert!(freed_before_notification.load(Ordering::SeqCst));
}
