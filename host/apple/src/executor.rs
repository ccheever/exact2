//! Bounded native I/O (LLP 1016 / 1041). Unannotated requests and native
//! continuations share one FIFO. Explicit independent HTTP uses six workers
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
    /// Start one ordered owner and six independent HTTP owners. Bindings are
    /// never shared between owners. Retired but unfinished workers remain
    /// charged against a process limit; exhaustion refuses new work.
    pub fn start(
        bindings: Option<ibex2::host::Bindings>,
        grants: &str,
        wake: Option<(WakeFn, *mut c_void)>,
    ) -> Self {
        exact_runner::local_network::launch_once(local_network_prompts);
        let wake = wake.map(|(f, ctx)| (f, ctx as usize));
        Self {
            core: core::Core::start_switched(
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
    /// Settle a re-ask (`Dispatch::Again`) in its ordered place, with no
    /// work (LLP 1041 §8.4, amended 2026-10-09); kept pending while the
    /// window is full. An error is terminal, for the host to record.
    pub fn again(&self, request: &RequestOut) -> Result<(), &'static str> {
        self.core.again(request.ticket)
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
    /// Send a mutation's or mapper call's socket batch whose commit stood
    /// (LLP 1016.000 D7):
    /// all of it, in order, or none. Each socket's frames sent and bytes
    /// buffered, for [`exact_runner::Runner::socket_published`].
    pub fn socket(&self, batch: &exact_runner::SocketSend) -> Result<Vec<(u64, u64, u64)>, String> {
        self.core.socket(batch)
    }
    /// Each open socket's buffered bytes, by its stream ticket, live.
    pub fn socket_reader(&self) -> exact_runner::SocketReader {
        self.core.socket_reader()
    }
    /// Each long poll's backoff wait, by its stream ticket, live (D8).
    pub fn retry_reader(&self) -> exact_runner::RetryReader {
        self.core.retry_reader()
    }
    /// Each listener's address and connections, by its stream ticket, live.
    pub fn listen_reader(&self) -> exact_runner::listen::ListenReader {
        self.core.listen_reader()
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
    /// [`Executor::again`].
    fn again(&self, request: &RequestOut) -> Result<(), &'static str>;
    /// [`Executor::begin_pump`].
    fn begin_pump(&self);
    /// [`Executor::drain`].
    fn drain(&self) -> Vec<(u64, Outcome, Option<u64>)>;
    /// [`Executor::notify`].
    fn notify(&self);
    /// [`Executor::waker`].
    fn waker(&self) -> std::sync::Arc<dyn Fn() + Send + Sync>;
    /// [`Executor::socket`].
    fn socket(&self, batch: &exact_runner::SocketSend) -> Result<Vec<(u64, u64, u64)>, String>;
    /// [`Executor::socket_reader`].
    fn socket_reader(&self) -> exact_runner::SocketReader;
    /// [`Executor::retry_reader`].
    fn retry_reader(&self) -> exact_runner::RetryReader;
    /// [`Executor::listen_reader`]; none by default.
    fn listen_reader(&self) -> exact_runner::listen::ListenReader {
        std::sync::Arc::new(|_| None)
    }
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
    fn again(&self, request: &RequestOut) -> Result<(), &'static str> {
        Executor::again(self, request)
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
    fn socket(&self, batch: &exact_runner::SocketSend) -> Result<Vec<(u64, u64, u64)>, String> {
        Executor::socket(self, batch)
    }
    fn socket_reader(&self) -> exact_runner::SocketReader {
        Executor::socket_reader(self)
    }
    fn retry_reader(&self) -> exact_runner::RetryReader {
        Executor::retry_reader(self)
    }
    fn listen_reader(&self) -> exact_runner::listen::ListenReader {
        Executor::listen_reader(self)
    }
}

/// Whether this process's target has local network privacy, said once per
/// launch (LLP 1133.036 D6, D8): iOS, and macOS from 15 (TN3179). The iOS
/// Simulator has none ("The simulator doesn't support local network
/// privacy"), nor tvOS, so there the fact is `granted`. Under the agent the
/// substitute's state stands for the fact on every target.
fn local_network_prompts() -> bool {
    let simulator = cfg!(all(target_os = "ios", target_abi = "sim"));
    cfg!(target_os = "ios") && !simulator || cfg!(target_os = "macos") && macos_major() >= 15
}

/// The running macOS's major version, from `kern.osproductversion`; 0 if
/// it cannot be read.
#[allow(unsafe_code)] // sysctlbyname(3) into a buffer this function owns.
fn macos_major() -> u32 {
    extern "C" {
        fn sysctlbyname(
            name: *const std::os::raw::c_char,
            old: *mut c_void,
            len: *mut usize,
            new: *mut c_void,
            new_len: usize,
        ) -> std::os::raw::c_int;
    }
    let mut buffer = [0u8; 32];
    let mut len = buffer.len();
    // SAFETY: the name is NUL-terminated and `len` is the buffer's size.
    let read = unsafe {
        sysctlbyname(
            c"kern.osproductversion".as_ptr(),
            buffer.as_mut_ptr().cast(),
            &mut len,
            std::ptr::null_mut(),
            0,
        )
    };
    let text =
        (read == 0).then(|| String::from_utf8_lossy(&buffer[..len.saturating_sub(1)]).into_owned());
    text.and_then(|t| t.split('.').next()?.parse().ok())
        .unwrap_or(0)
}

/// Evidence of the host's local-network fact (LLP 1133.036 D8), which
/// `@exact/local-network`'s browse or resolve reports through the host's
/// `exact.localNetwork` notification: 0 `granted`, 2 `denied`. Answers 1
/// if the fact moved.
pub fn report_local_network(state: u32) -> u32 {
    use exact_runner::local_network::{report, State};
    let state = match state {
        0 => State::Granted,
        2 => State::Denied,
        _ => return 0,
    };
    report(state) as u32
}

/// Export [`report_local_network`] from the application's static archive.
#[macro_export]
macro_rules! local_network_exports {
    () => {
        /// The package's evidence of the local-network fact (LLP 1133.036 D8).
        #[no_mangle]
        pub extern "C" fn exact_local_network_report(state: u32) -> u32 {
            $crate::executor::report_local_network(state)
        }
    };
}

/// Start the executor as the bridge holds it: [`crate::link::IoLinks`]'s.
pub fn start_io(
    bindings: Option<ibex2::host::Bindings>,
    grants: &str,
    wake: Option<(WakeFn, *mut c_void)>,
) -> Box<dyn Io> {
    Box::new(Executor::start(bindings, grants, wake))
}
