//! Bounded native I/O (LLP 1016 / 1041). Unannotated requests and native
//! continuations share one FIFO. Explicit independent HTTP uses two workers
//! with separate transports; their held sockets cannot occupy the ordered lane.
use exact_runner::{Outcome, RequestOut, Work};
use std::ffi::c_void;
#[path = "executor_core.rs"]
mod core;

/// Schedule a pump on the presenter's thread. Must enqueue asynchronously;
/// called under the retirement guard, never synchronously reenter Exact.
pub type WakeFn = extern "C" fn(ctx: *mut c_void);

/// Native executor with admission reservations through undrained outcomes.
pub struct Executor {
    core: core::Core,
}
impl Executor {
    /// Start one ordered owner and two independent HTTP owners. Bindings are
    /// never shared between owners. Retired but unfinished workers remain
    /// charged against a process limit; exhaustion refuses new work.
    pub fn start(
        bindings: Option<ibex2::host::Bindings>,
        grants: &str,
        wake: Option<(WakeFn, *mut c_void)>,
    ) -> Self {
        let wake = wake.map(|(f, ctx)| (f, ctx as usize));
        Self {
            core: core::Core::start(
                bindings,
                grants,
                Box::new(move || {
                    if let Some((f, ctx)) = wake {
                        f(ctx as *mut c_void);
                    }
                }),
            ),
        }
    }
    /// Earlier admitted ordered outcomes have all reached the UI pump.
    pub fn ordered_idle(&self) -> bool {
        self.core.ordered_idle()
    }
    /// Called only after the runner has no retained ordered admission refusals.
    pub fn resume_ordered(&self) {
        self.core.resume_ordered();
    }
    /// Let go of the work for tickets the runner no longer holds.
    pub fn forget(&self, held: impl Fn(u64) -> bool) {
        self.core.forget(held);
    }
    /// Admit or return a terminal refusal without allocating a failure queue.
    /// The caller records refusals on existing runner tickets and wakes a pump.
    pub fn run(&self, request: RequestOut, work: Option<Work>) -> Result<(), &'static str> {
        self.core.run_owned(request, work)
    }
    /// Acknowledge the coalesced wake before choosing a completion or refusal.
    pub fn begin_pump(&self) {
        self.core.begin_pump();
    }
    /// Take at most one result; remaining outcomes schedule another pump.
    pub fn drain(&self) -> Vec<(u64, Outcome)> {
        self.core.drain()
    }
    /// Wake the presenter for a refusal retained on a runner ticket.
    pub fn notify(&self) {
        self.core.notify();
    }
}
