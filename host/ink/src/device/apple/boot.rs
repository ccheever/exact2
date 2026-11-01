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
    _callback: block2::RcBlock<dyn Fn(*mut CFRunLoopObserver, CFRunLoopActivity)>,
    _main: MainThreadMarker,
}

impl BootTicks {
    pub fn new(tick: impl Fn() -> bool + 'static) -> Result<Self> {
        let main = MainThreadMarker::new()
            .ok_or_else(|| Error::Invalid("boot ticks require the main thread".into()))?;
        let depth = std::cell::Cell::new(0usize);
        let callback = block2::RcBlock::new(
            move |observer: *mut CFRunLoopObserver, activity: CFRunLoopActivity| {
                if outer_boundary(&depth, activity) && !tick() {
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
            | CFRunLoopActivity::BeforeWaiting
            | CFRunLoopActivity::Exit;
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
            _callback: callback,
            _main: main,
        })
    }
}

impl Drop for BootTicks {
    fn drop(&mut self) {
        self.observer.invalidate();
    }
}

fn outer_boundary(depth: &std::cell::Cell<usize>, activity: CFRunLoopActivity) -> bool {
    if activity == CFRunLoopActivity::Entry {
        depth.set(depth.get() + 1);
    } else if activity == CFRunLoopActivity::Exit {
        depth.set(depth.get().saturating_sub(1));
        return false;
    }
    depth.get() == 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_nested_core_foundation_loop_does_not_consume_a_tick() {
        use objc2_core_foundation::{
            kCFRunLoopDefaultMode, CFRunLoopSource, CFRunLoopSourceContext,
        };
        use std::{cell::Cell, rc::Rc};
        struct State {
            nested: Cell<bool>,
            nested_entries: Cell<usize>,
            nested_ticks: Cell<usize>,
            outer_ticks: Cell<usize>,
        }
        unsafe extern "C-unwind" fn enter_nested(info: *mut core::ffi::c_void) {
            // The source is removed before its stack-owned context is dropped.
            let state = unsafe { &*info.cast::<State>() };
            state.nested.set(true);
            CFRunLoop::run();
            state.nested.set(false);
            CFRunLoop::current().unwrap().stop();
        }
        let state = Rc::new(State {
            nested: Cell::new(false),
            nested_entries: Cell::new(0),
            nested_ticks: Cell::new(0),
            outer_ticks: Cell::new(0),
        });
        let captured = state.clone();
        let depth = Cell::new(0);
        let callback = block2::RcBlock::new(move |_: *mut CFRunLoopObserver, activity| {
            let outer = outer_boundary(&depth, activity);
            if captured.nested.get() {
                if activity == CFRunLoopActivity::Entry {
                    captured
                        .nested_entries
                        .set(captured.nested_entries.get() + 1);
                }
                if outer {
                    captured.nested_ticks.set(captured.nested_ticks.get() + 1);
                }
                if activity == CFRunLoopActivity::BeforeSources {
                    CFRunLoop::current().unwrap().stop();
                }
            } else if outer {
                captured.outer_ticks.set(captured.outer_ticks.get() + 1);
            }
        });
        let activities = CFRunLoopActivity::Entry
            | CFRunLoopActivity::BeforeSources
            | CFRunLoopActivity::BeforeWaiting
            | CFRunLoopActivity::AfterWaiting
            | CFRunLoopActivity::Exit;
        let observer = unsafe {
            CFRunLoopObserver::with_handler(None, activities.0, true, -1, Some(&callback))
        }
        .unwrap();
        let mut context = CFRunLoopSourceContext {
            version: 0,
            info: Rc::as_ptr(&state).cast_mut().cast(),
            retain: None,
            release: None,
            copyDescription: None,
            equal: None,
            hash: None,
            schedule: None,
            cancel: None,
            perform: Some(enter_nested),
        };
        let source = unsafe { CFRunLoopSource::new(None, 0, &mut context) }.unwrap();
        let run_loop = CFRunLoop::current().unwrap();
        let mode = unsafe { kCFRunLoopDefaultMode };
        run_loop.add_observer(Some(&observer), mode);
        run_loop.add_source(Some(&source), mode);
        source.signal();
        CFRunLoop::run();
        source.invalidate();
        observer.invalidate();
        assert_eq!(state.nested_entries.get(), 1);
        assert_eq!(state.nested_ticks.get(), 0);
        assert!(state.outer_ticks.get() > 0);
    }

    #[test]
    fn nested_modal_and_tracking_loops_leave_the_tick_pending() {
        let depth = std::cell::Cell::new(0);
        assert!(outer_boundary(&depth, CFRunLoopActivity::Entry));
        for _ in 0..2 {
            assert!(!outer_boundary(&depth, CFRunLoopActivity::Entry));
            for activity in [
                CFRunLoopActivity::AfterWaiting,
                CFRunLoopActivity::BeforeSources,
                CFRunLoopActivity::BeforeWaiting,
            ] {
                assert!(!outer_boundary(&depth, activity));
            }
            assert!(!outer_boundary(&depth, CFRunLoopActivity::Exit));
        }
        assert!(outer_boundary(&depth, CFRunLoopActivity::BeforeSources));
        assert!(!outer_boundary(&depth, CFRunLoopActivity::Exit));
        assert_eq!(depth.get(), 0);
    }
}
