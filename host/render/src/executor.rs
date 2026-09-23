//! One render's executor: the native core the Apple and Linux hosts share
//! (grant-checked transport, ordered and independent lanes, continuations on
//! its workers), with a wake the rendering thread waits on instead of a UI
//! loop. Dropping it aborts whatever is still in flight.
use exact_runner::{Outcome, RequestOut, Work};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Instant;
#[path = "../../apple/src/executor_core.rs"]
#[allow(dead_code)] // `run` is the core's own tests' entry
mod core;

/// The native core with a condition variable for its wake.
pub struct Executor {
    core: core::Core,
    wake: Arc<(Mutex<bool>, Condvar)>,
}

impl Executor {
    /// Start the core's owners under `grants` (the render's, never more
    /// than the app's).
    pub fn start(grants: &str) -> Self {
        #[cfg(not(target_vendor = "apple"))]
        let host = ibex2::host::Host::with_transport(Box::new(
            ibex2::transport::RustlsHttpTransport::new(),
        ));
        #[cfg(target_vendor = "apple")]
        let host = ibex2::host::Host::new();
        let bindings = ibex2::grant::GrantSet::parse(&exact_runner::io_grants(grants))
            .ok()
            .map(|g| host.endow(g));
        let wake = Arc::new((Mutex::new(false), Condvar::new()));
        let signal = wake.clone();
        let core = core::Core::start(
            bindings,
            grants,
            Box::new(move || {
                // Called under the core's lock: only record and signal.
                let (woken, ready) = &*signal;
                *woken.lock().unwrap() = true;
                ready.notify_all();
            }),
        );
        Self { core, wake }
    }

    /// Admit work, or refuse it (a limit, the core retired).
    pub fn run(&self, request: RequestOut, work: Option<Work>) -> Result<(), &'static str> {
        self.core.run_owned(request, work)
    }

    /// At most one completion, oldest ordered first; more rewake.
    pub fn drain(&self) -> Vec<(u64, Outcome)> {
        self.core.begin_pump();
        self.core.drain()
    }

    /// Lift the ordered lane's fence after a refusal.
    pub fn resume_ordered(&self) {
        self.core.resume_ordered();
    }

    /// Wait until the core has news, or until `until`.
    pub fn wait(&self, until: Instant) {
        let (woken, ready) = &*self.wake;
        let mut guard = woken.lock().unwrap();
        while !*guard {
            let now = Instant::now();
            if now >= until {
                return;
            }
            guard = ready.wait_timeout(guard, until - now).unwrap().0;
        }
        *guard = false;
    }
}
