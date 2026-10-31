//! Consume a pending boot tick before AppKit's display-cycle observers.
//! @ref LLP 1149.001 §3 M10; LLP 1149 D7, D11.
use super::{trace_mark, Error, Result};
use objc2::MainThreadMarker;
use objc2_core_foundation::{
    kCFRunLoopCommonModes, CFRetained, CFRunLoop, CFRunLoopActivity, CFRunLoopObserver,
};

/// A main-thread callback which retires when it returns false.
pub struct BootTicks {
    observer: CFRetained<CFRunLoopObserver>,
    _main: MainThreadMarker,
}

impl BootTicks {
    pub fn new(tick: impl Fn() -> bool + 'static) -> Result<Self> {
        let main = MainThreadMarker::new()
            .ok_or_else(|| Error::Invalid("boot ticks require the main thread".into()))?;
        let callback = block2::RcBlock::new(
            move |observer: *mut CFRunLoopObserver, _: CFRunLoopActivity| {
                // AppKit may enter a nested loop while the carrier is borrowed.
                // The caller must leave that tick pending instead of reentering it.
                if !tick() {
                    trace_mark(c"boot ticks retired", || 0.);
                    // SAFETY: Core Foundation supplies this live observer to its callback.
                    unsafe { &*observer }.invalidate();
                }
            },
        );
        // Entry precedes AppKit's first display cycle. Subsequent sources still
        // wake normally; the early observer consumes their tick before winit's
        // last-priority BeforeWaiting observer drains its user-event queue.
        let activities = CFRunLoopActivity::Entry
            | CFRunLoopActivity::AfterWaiting
            | CFRunLoopActivity::BeforeSources
            | CFRunLoopActivity::BeforeWaiting;
        // SAFETY: CF copies the block; its non-Send captures are invoked and
        // released on main. Drop invalidates the observer before releasing it.
        let observer = unsafe {
            CFRunLoopObserver::with_handler(None, activities.0, true, -1, Some(&callback))
        }
        .ok_or_else(|| Error::Device("create boot tick observer".into()))?;
        CFRunLoop::main()
            .ok_or_else(|| Error::Device("find main run loop".into()))?
            .add_observer(Some(&observer), unsafe { kCFRunLoopCommonModes });
        Ok(Self {
            observer,
            _main: main,
        })
    }
}

impl Drop for BootTicks {
    fn drop(&mut self) {
        self.observer.invalidate();
    }
}
