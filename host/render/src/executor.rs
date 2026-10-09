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
/// A new executor waits for a native-worker slot under test: the crate's own
/// tests, and the integration binary, whose every test's servers and renders
/// share one process's bound (`test-wait`, which only the dev-dependency on
/// this crate turns on). A server refuses (`Busy`) at the bound instead.
const WAIT_FOR_SLOT: bool = cfg!(any(test, feature = "test-wait"));

/// The native core with a condition variable for its wake.
pub struct Executor {
    core: core::Core,
    wake: Arc<(Mutex<bool>, Condvar)>,
}

impl Executor {
    /// Start the core's owners under `grants` (the render's, never more
    /// than the app's).
    pub fn start(grants: &str) -> Self {
        // @ref LLP 1048.000 D10 — a server, not an app on a device: its
        // fetches go over rustls on macOS too (Charlie, 2026-09-29), not
        // NSURLSession, whose per-call cost was ~30% of a RealWorld page's CPU.
        // `EXACT_RENDER_TRANSPORT=platform` keeps the platform's (a check
        // against the platform, or a way back).
        let platform = std::env::var("EXACT_RENDER_TRANSPORT").as_deref() == Ok("platform");
        let transport: core::StreamHost = if platform {
            Arc::new(ibex2::host::Host::new)
        } else {
            Arc::new(|| {
                ibex2::host::Host::with_transport(Box::new(
                    ibex2::transport::RustlsHttpTransport::new(),
                ))
            })
        };
        let host = transport();
        let bindings = ibex2::grant::GrantSet::parse(&exact_runner::io_grants(grants))
            .ok()
            .map(|g| host.endow(g));
        let wake = Arc::new((Mutex::new(false), Condvar::new()));
        let signal = wake.clone();
        let core = core::Core::start_on(
            bindings,
            grants,
            Box::new(move || {
                // Called under the core's lock: only record and signal.
                let (woken, ready) = &*signal;
                *woken.lock().unwrap() = true;
                ready.notify_all();
            }),
            transport,
        );
        Self { core, wake }
    }

    /// Where an app's files are, for a request whose body is one of them
    /// (LLP 1108 D6 R2). A render serves no app files and never sets them,
    /// so such a request is refused; an embedder that has them may.
    pub fn set_app_roots(&self, roots: [std::path::PathBuf; 3]) {
        self.core.set_app_roots(roots);
    }

    /// Admit work, or refuse it (a limit, the core retired).
    pub fn run(&self, request: RequestOut, work: Option<Work>) -> Result<(), &'static str> {
        self.core.run_owned(request, work)
    }

    /// At most one completion, oldest ordered first; more rewake.
    pub fn drain(&self) -> Vec<(u64, Outcome, Option<u64>)> {
        self.core.begin_pump();
        self.core.drain()
    }

    /// Lift the ordered lane's fence after a refusal, admitting the ordered
    /// requests held behind it; returns those refused at a limit.
    pub fn resume_ordered(&self) -> Vec<(u64, &'static str)> {
        self.core.resume_ordered()
    }

    /// No admitted ordered work is in flight or undrained.
    pub fn ordered_idle(&self) -> bool {
        self.core.ordered_idle()
    }

    /// Settle a re-ask (`Dispatch::Again`) in its ordered place, with no
    /// work (LLP 1041 §8.4, amended 2026-10-09); kept pending while the
    /// window is full. An error is terminal, for the host to record.
    pub fn again(&self, request: &RequestOut) -> Result<(), &'static str> {
        self.core.again(request.ticket)
    }
    /// Let go of the work for tickets the runner no longer holds.
    pub fn forget(&self, held: impl Fn(u64) -> bool) {
        self.core.forget(held);
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
