//! Bounded native I/O (LLP 1016 / 1041). Unannotated requests and native
//! continuations share one FIFO. Explicit independent HTTP uses two workers
//! with separate transports; their held sockets cannot occupy the ordered lane.
use exact_runner::{Outcome, RequestOut, Work};
use std::ffi::c_void;
#[path = "executor_core.rs"]
mod core;
/// A new executor waits for a native-worker slot only under test (the core's `reserve`).
const WAIT_FOR_SLOT: bool = cfg!(test);

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
    /// Where the app's files are, for a request whose body is one of them
    /// (LLP 1108 D6 R2): set before the first request.
    pub fn set_app_roots(&self, roots: [std::path::PathBuf; 3]) {
        self.core.set_app_roots(roots);
    }
    /// Earlier admitted ordered outcomes have all reached the UI pump.
    pub fn ordered_idle(&self) -> bool {
        self.core.ordered_idle()
    }
    /// Called only after the runner has no retained ordered admission refusals.
    /// Admits the ordered requests held behind them; returns any of those
    /// refused at a limit, for the host to record on their tickets.
    pub fn resume_ordered(&self) -> Vec<(u64, &'static str)> {
        self.core.resume_ordered()
    }
    /// Let go of the work for tickets the runner no longer holds.
    pub fn forget(&self, held: impl Fn(u64) -> bool) {
        self.core.forget(held);
    }
    /// Admit, hold behind an earlier ordered refusal until it settles, or
    /// return a terminal refusal without allocating a failure queue.
    /// The caller records refusals on existing runner tickets and wakes a pump.
    pub fn run(&self, request: RequestOut, work: Option<Work>) -> Result<(), &'static str> {
        self.core.run_owned(request, work)
    }
    /// Acknowledge the coalesced wake before choosing a completion or refusal.
    pub fn begin_pump(&self) {
        self.core.begin_pump();
    }
    /// Take at most one result; remaining outcomes schedule another pump.
    pub fn drain(&self) -> Vec<(u64, Outcome, Option<u64>)> {
        self.core.drain()
    }
    /// Wake the presenter for a refusal retained on a runner ticket.
    pub fn notify(&self) {
        self.core.notify();
    }
    /// A wake another thread may keep: a native module's announcements.
    pub fn waker(&self) -> std::sync::Arc<dyn Fn() + Send + Sync> {
        self.core.waker()
    }
}

/// What the bridge asks of the I/O executor (LLP 1047.001): the executor's
/// own methods, behind a trait so that an archive that links no I/O names
/// none of them, nor the transports and stores they reach.
pub trait Io {
    /// [`Executor::set_app_roots`].
    fn set_app_roots(&self, roots: [std::path::PathBuf; 3]);
    /// [`Executor::ordered_idle`].
    fn ordered_idle(&self) -> bool;
    /// [`Executor::resume_ordered`].
    fn resume_ordered(&self) -> Vec<(u64, &'static str)>;
    /// [`Executor::forget`].
    fn forget(&self, held: &dyn Fn(u64) -> bool);
    /// [`Executor::run`].
    fn run(&self, request: RequestOut, work: Option<Work>) -> Result<(), &'static str>;
    /// [`Executor::begin_pump`].
    fn begin_pump(&self);
    /// [`Executor::drain`].
    fn drain(&self) -> Vec<(u64, Outcome, Option<u64>)>;
    /// [`Executor::notify`].
    fn notify(&self);
    /// [`Executor::waker`].
    fn waker(&self) -> std::sync::Arc<dyn Fn() + Send + Sync>;
}

impl Io for Executor {
    fn set_app_roots(&self, roots: [std::path::PathBuf; 3]) {
        Executor::set_app_roots(self, roots)
    }
    fn ordered_idle(&self) -> bool {
        Executor::ordered_idle(self)
    }
    fn resume_ordered(&self) -> Vec<(u64, &'static str)> {
        Executor::resume_ordered(self)
    }
    fn forget(&self, held: &dyn Fn(u64) -> bool) {
        Executor::forget(self, held)
    }
    fn run(&self, request: RequestOut, work: Option<Work>) -> Result<(), &'static str> {
        Executor::run(self, request, work)
    }
    fn begin_pump(&self) {
        Executor::begin_pump(self)
    }
    fn drain(&self) -> Vec<(u64, Outcome, Option<u64>)> {
        Executor::drain(self)
    }
    fn notify(&self) {
        Executor::notify(self)
    }
    fn waker(&self) -> std::sync::Arc<dyn Fn() + Send + Sync> {
        Executor::waker(self)
    }
}

/// Start the executor as the bridge holds it: [`crate::link::IoLinks`]'s.
pub fn start_io(
    bindings: Option<ibex2::host::Bindings>,
    grants: &str,
    wake: Option<(WakeFn, *mut c_void)>,
) -> Box<dyn Io> {
    Box::new(Executor::start(bindings, grants, wake))
}
