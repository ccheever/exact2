//! Raster C exports, instantiated in the same app archive as `host!`.

/// Export the session-independent raster handle seam. All handles preserve u64.
#[macro_export]
macro_rules! raster_exports {
    () => {
        /// Create a session-lifetime raster account of `budget` decoded bytes.
        #[no_mangle]
        pub extern "C" fn exact_raster_session_create(budget: u64) -> u64 {
            $crate::raster::session_create(budget)
        }
        /// Follow a session's display capacity without invalidating live images.
        #[no_mangle]
        pub extern "C" fn exact_raster_session_budget(id: u64, budget: u64) {
            $crate::raster::session_budget(id, budget)
        }
        /// Reset, pause, resume, shut down, or trim a raster account.
        #[no_mangle]
        pub extern "C" fn exact_raster_session_control(id: u64, op: u32) {
            $crate::raster::session_control(id, op)
        }
        /// Request one independently cancellable image interest.
        #[no_mangle]
        pub extern "C" fn exact_raster_request(
            id: u64,
            demand: $crate::raster::RasterDemand,
        ) -> u64 {
            $crate::raster::request(id, demand)
        }
        /// Cancel one interest without retiring other observers.
        #[no_mangle]
        pub extern "C" fn exact_raster_cancel(id: u64, request: u64) {
            $crate::raster::cancel(id, request)
        }
        /// Read current request state and refusal reason.
        #[no_mangle]
        pub extern "C" fn exact_raster_status(id: u64, request: u64) -> u32 {
            $crate::raster::status(id, request)
        }
        /// Admit a decode on a worker with a bounded condition wait.
        #[no_mangle]
        pub extern "C" fn exact_raster_next_decode(timeout_ms: u32) -> $crate::raster::RasterWork {
            $crate::raster::next_decode(timeout_ms)
        }
        /// Test a worker permit before allocating.
        #[no_mangle]
        pub extern "C" fn exact_raster_is_cancelled(id: u64) -> u32 {
            $crate::raster::is_cancelled(id)
        }
        /// Transfer a native payload after decoder scratch has gone.
        #[no_mangle]
        pub extern "C" fn exact_raster_complete(
            id: u64,
            payload: u64,
            release: Option<extern "C" fn(u64)>,
            bytes: u64,
        ) -> u32 {
            $crate::raster::complete(id, payload, release, bytes)
        }
        /// Retire a failed worker after its scratch has gone.
        #[no_mangle]
        pub extern "C" fn exact_raster_fail(id: u64) {
            $crate::raster::fail(id)
        }
        /// Release a pixel provider charge on any thread.
        #[no_mangle]
        pub extern "C" fn exact_raster_charge_release(id: u64) {
            $crate::raster::charge_release(id)
        }
        /// Take a ready result and pin its backing for the native view.
        #[no_mangle]
        pub extern "C" fn exact_raster_take_ready(
            id: u64,
            request: u64,
        ) -> $crate::raster::RasterReady {
            $crate::raster::take_ready(id, request)
        }
        /// Release a native view lease on any thread.
        #[no_mangle]
        pub extern "C" fn exact_raster_lease_release(id: u64) {
            $crate::raster::lease_release(id)
        }
        /// Read session storage and process decode counters.
        #[no_mangle]
        pub extern "C" fn exact_raster_stats(id: u64) -> $crate::raster::RasterStats {
            $crate::raster::stats(id)
        }
    };
}
