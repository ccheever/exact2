//! The page's live clock while it owns time (the timers RFC, D1): told to
//! the running host and kept for every boot, a dev restart's included.

use super::Bridge;
use exact_runner::DataSource;

impl<D: DataSource> Bridge<D> {
    /// Requests the agent waits on, including turns parked inside a source.
    /// Declared holds and streams after their first message keep the runner's exemptions.
    pub fn requests_pending(&self) -> u32 {
        u32::from(self.host.as_ref().is_some_and(|h| h.runner().has_pending()))
    }

    /// The page's clock owns time (`Some`), or the agent's seekable one does
    /// (`None`): the running runner and every one booted later take it.
    pub fn set_live_clock(&mut self, now: Option<fn() -> f64>) {
        self.live_clock = now;
        self.give_clock();
    }

    /// What a boot is told at launch: the date and place last told, and the
    /// page's clock now, where a fresh runner's clock starts (a checkpoint
    /// keeps its own).
    pub(super) fn launch(&self) -> exact_runner::time::Launch {
        exact_runner::time::Launch {
            clock: self.live_clock.map(|now| now()),
            ..self.launch_facts.clone()
        }
    }

    pub(super) fn give_clock(&mut self) {
        let sample = self
            .live_clock
            .map(|now| Box::new(now) as Box<dyn FnMut() -> f64>);
        if let Some(h) = self.host.as_mut() {
            h.runner_mut().set_live_clock(sample);
        }
    }
}

/// `exact_live_clock(on)`, in the artifact `host!` makes: nonzero, the
/// page's clock owns time; zero, the agent's seekable one does (the timers
/// RFC, D1). The page's clock is the `exact_clock.now` import, which this
/// crate, forbidding `unsafe`, cannot declare; the artifact does, as it
/// declares its exports.
#[macro_export]
#[doc(hidden)]
macro_rules! live_clock_export {
    ($bridge:ident) => {
        /// Whether a clock must wait for a runner request, even without host work.
        #[no_mangle]
        pub extern "C" fn exact_requests_pending() -> u32 {
            $bridge.with(|b| b.borrow().requests_pending())
        }

        /// Nonzero: the page's clock owns time; zero: the agent's does.
        #[no_mangle]
        pub extern "C" fn exact_live_clock(on: u32) {
            // `performance.now()` from the origin the glue gives every
            // other time from, in the runner's milliseconds.
            #[cfg(target_arch = "wasm32")]
            fn page_now() -> f64 {
                #[link(wasm_import_module = "exact_clock")]
                extern "C" {
                    fn now() -> f64;
                }
                // SAFETY: a pure read of the page's clock.
                unsafe { now() }
            }
            // Off the page there is no page clock: a sample that is no time.
            #[cfg(not(target_arch = "wasm32"))]
            fn page_now() -> f64 {
                f64::NAN
            }
            let now = (on != 0).then_some(page_now as fn() -> f64);
            $bridge.with(|b| b.borrow_mut().set_live_clock(now))
        }
    };
}

#[cfg(test)]
#[path = "clock_tests.rs"]
mod clock_tests;
