//! The page's live clock while it owns time (the timers RFC, D1): told to
//! the running host and kept for every boot, a dev restart's included; and
//! what the agent's clock waits on or hands the worlds between its frames.

use super::Bridge;
use exact_runner::DataSource;

impl<D: DataSource> Bridge<D> {
    /// Move the clock: `mode` 0 fires every timer due (the wall clock), 1
    /// stops after a timer that sends (the agent's jump), 2 lands only the
    /// `then`s already armed, the clock unmoved (an agent's input's end).
    pub fn advance(&mut self, now_ms: f64, mode: u32) -> u32 {
        if mode >= 4 {
            return self.timer_waits(now_ms, mode);
        }
        let out = match self.host.as_mut() {
            // The `then`s an agent's input settled (Runner::land_then).
            Some(h) if mode == 2 => {
                let a = h.runner_mut().land_then();
                h.advanced(a)
            }
            Some(h) if mode == 1 => h.advance_until_request(now_ms),
            Some(h) => h.advance(now_ms),
            None => "{\"ops\":[],\"timers\":false,\"error\":\"not booted\"}".to_string(),
        };
        self.emit(out)
    }

    pub(super) fn timer_waits(&mut self, cutoff: f64, mode: u32) -> u32 {
        let out = self
            .host
            .as_mut()
            .map(|h| {
                if mode == 4 {
                    h.runner_mut().hold_waits(exact_runner::WaitKinds::ALL);
                    return h.advance(cutoff);
                }
                if mode == 8 {
                    let a = h.runner_mut().complete_frame_tasks(cutoff);
                    return h.advanced(a);
                }
                let a = h.runner_mut().release_waits(
                    exact_runner::WaitKinds::from_bits((mode >> 8) as u8),
                    cutoff,
                );
                let continuing = a.continuing;
                let mut batch = h.advanced(a.advanced);
                if continuing {
                    batch.pop();
                    batch.push_str(",\"waitContinue\":true}");
                }
                batch
            })
            .unwrap_or_else(|| "{\"ops\":[]}".into());
        self.emit(out)
    }

    /// Requests the agent waits on, including turns parked inside a source.
    /// Declared holds and streams after their first message keep the runner's exemptions.
    pub fn requests_pending(&self) -> u32 {
        u32::from(self.host.as_ref().is_some_and(|h| h.runner().has_pending()))
    }

    /// Whether the substitute feeds a canvas a sensor's samples (LLP
    /// 1133.013 D6): the agent's clock then steps the worlds a frame at a time.
    pub fn sensor_inputs(&self) -> u32 {
        u32::from((self.host.as_ref()).is_some_and(|h| h.runner().surface_sensor_inputs()))
    }

    /// The samples the substitute's canvas inputs owe their surfaces up to
    /// `to_ms`, as `[{"surface", "event"}]`, each event the input JSON a
    /// canvas takes.
    pub fn sensor_events(&mut self, to_ms: f64) -> u32 {
        let events = (self.host.as_mut())
            .map(|h| h.runner_mut().surface_sensor_events(to_ms))
            .unwrap_or_default();
        let out: Vec<serde_json::Value> = (events.into_iter())
            .map(|(surface, event)| serde_json::json!({ "surface": surface, "event": event }))
            .collect();
        self.emit(serde_json::Value::Array(out).to_string())
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
            h.runner_mut().hold_waits(exact_runner::WaitKinds::ALL);
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

        /// Nonzero while the substitute feeds a canvas a sensor's samples.
        #[no_mangle]
        pub extern "C" fn exact_sensor_inputs() -> u32 {
            $bridge.with(|b| b.borrow().sensor_inputs())
        }

        /// The samples owed the canvases up to `to_ms`; returns the length.
        #[no_mangle]
        pub extern "C" fn exact_sensor_events(to_ms: f64) -> u32 {
            $bridge.with(|b| b.borrow_mut().sensor_events(to_ms))
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
