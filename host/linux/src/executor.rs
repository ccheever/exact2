//! Bounded native I/O, shared with Apple. A nonblocking socketpair makes
//! completions and admission refusals visible to the Linux display loop.
use exact_runner::{Outcome, RequestOut, Work};
use std::io::{Read, Write};
use std::os::unix::{
    io::{AsRawFd, RawFd},
    net::UnixStream,
};
#[path = "../../apple/src/executor_core.rs"]
mod core;

/// Ordered native work plus explicitly independent HTTP, with a poll wake.
pub struct Executor {
    core: core::Core,
    wake: UnixStream,
    note: Option<String>,
}
impl Executor {
    /// Start native transport owners from the app's grants.
    pub fn start(grants: &str) -> Self {
        let (wake, signal) = UnixStream::pair().expect("executor socketpair");
        wake.set_nonblocking(true).expect("nonblocking wake");
        signal.set_nonblocking(true).expect("nonblocking signal");
        #[cfg(not(target_vendor = "apple"))]
        let (host, note) = {
            let transport = ibex2::transport::RustlsHttpTransport::new();
            let note = Some(format!("trust roots: {}", transport.roots()));
            (ibex2::host::Host::with_transport(Box::new(transport)), note)
        };
        #[cfg(target_vendor = "apple")]
        let (host, note) = (ibex2::host::Host::new(), None);
        let bindings = ibex2::grant::GrantSet::parse(grants)
            .ok()
            .map(|g| host.endow(g));
        let core = core::Core::start(
            bindings,
            grants,
            Box::new(move || {
                // EAGAIN means a wake is already pending. Never block the worker
                // (or retirement) because the presenter hasn't drained a pipe.
                let _ = (&signal).write(&[1]);
            }),
        );
        Self { core, wake, note }
    }
    /// Transport trust-store note for the journal.
    pub fn note(&self) -> Option<&str> {
        self.note.as_deref()
    }
    /// FD readable when the presenter should pump.
    pub fn fd(&self) -> RawFd {
        self.wake.as_raw_fd()
    }
    /// Earlier admitted ordered outcomes have all reached the UI pump.
    pub fn ordered_idle(&self) -> bool {
        self.core.ordered_idle()
    }
    /// Called only after the runner has no retained ordered admission refusals.
    pub fn resume_ordered(&self) {
        self.core.resume_ordered();
    }
    /// Admit work, or return a refusal without an overflow queue.
    pub fn run(&self, request: RequestOut, work: Option<Work>) -> Result<(), &'static str> {
        self.core.run(request, work)
    }
    /// Acknowledge a coalesced wake, including turns used by refusal settlement.
    pub fn begin_pump(&self) {
        let mut bytes = [0; 256];
        while let Ok(n) = (&self.wake).read(&mut bytes) {
            if n == 0 {
                break;
            }
        }
        self.core.begin_pump();
    }
    /// One completion per turn, rearming the FD if more remain.
    pub fn drain(&self) -> Vec<(u64, Outcome)> {
        self.core.drain()
    }
    /// Wake for a refusal stored on a runner ticket.
    pub fn notify(&self) {
        self.core.notify();
    }
}
