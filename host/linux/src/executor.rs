//! Bounded native I/O, shared with Apple. A nonblocking socketpair makes
//! completions and admission refusals visible to the Linux display loop.
use crate::wake::Stream as UnixStream;
use exact_runner::{Outcome, RequestOut, Work};
use std::io::{Read, Write};
#[cfg(unix)]
use std::os::unix::io::{AsRawFd, RawFd};
#[path = "../../apple/src/executor_core.rs"]
mod core;
/// A new executor waits for a native-worker slot under test: the crate's own
/// tests, and the integration binaries, whose every test's presenters share
/// one process's bound (`test-wait`, which only the dev-dependency on this
/// crate turns on). A presenter's executor is refused at the bound instead.
const WAIT_FOR_SLOT: bool = cfg!(any(test, feature = "test-wait"));

/// Ordered native work plus explicitly independent HTTP, with a poll wake.
pub struct Executor {
    core: core::Core,
    wake: UnixStream,
    note: Option<String>,
}
/// The Linux host's transports: rustls over ureq and the portable WebSocket.
fn portable_host() -> ibex2::host::Host {
    ibex2::host::Host::with_transport(Box::new(ibex2::transport::RustlsHttpTransport::new()))
        .with_socket_transport(Box::new(
            ibex2::transport::websocket::TcpSocketTransport::new(),
        ))
}

impl Executor {
    /// Start native transport owners from the app's grants.
    pub fn start(grants: &str) -> Self {
        let (wake, signal) = UnixStream::pair().expect("executor socketpair");
        wake.set_nonblocking(true).expect("nonblocking wake");
        signal.set_nonblocking(true).expect("nonblocking signal");
        // A Mac build takes the platform's transports, but under conformance's network
        // (`EXACT_NET_PROXY`, host/web-js/conformance/net.mjs) speaks as the Linux one
        // does: rustls and portable sockets take the run's proxy and trust from the
        // environment, and NSURLSession takes neither
        let portable =
            cfg!(not(target_vendor = "apple")) || std::env::var_os("EXACT_NET_PROXY").is_some();
        let (host, note) = if portable {
            let roots = ibex2::transport::RustlsHttpTransport::new().roots();
            (portable_host(), Some(format!("trust roots: {roots}")))
        } else {
            (ibex2::host::Host::new(), None)
        };
        // Grants that do not parse hold nothing: the journal says so once,
        // and every request's refusal names the line (the core's).
        let (bindings, note) = match ibex2::grant::GrantSet::parse(&exact_runner::io_grants(grants))
        {
            Ok(g) => (Some(host.endow(g)), note),
            Err(e) => {
                let why = format!("the app's grants did not parse: {e}; every request is refused");
                (
                    None,
                    Some(note.map_or(why.clone(), |n| format!("{n}; {why}"))),
                )
            }
        };
        // The other owners, scoped grants and streams take the same transports as the first
        let core = core::Core::start_on(
            bindings,
            grants,
            Box::new(move || {
                // EAGAIN means a wake is already pending. Never block the worker
                // (or retirement) because the presenter hasn't drained a pipe.
                let _ = (&signal).write(&[1]);
            }),
            if portable {
                std::sync::Arc::new(portable_host)
            } else {
                std::sync::Arc::new(ibex2::host::Host::new)
            },
        );
        Self { core, wake, note }
    }
    /// Where the app's files are, for a request whose body is one of them
    /// (LLP 1108 D6 R2): set before the first request.
    pub fn set_app_roots(&self, roots: [std::path::PathBuf; 3]) {
        self.core.set_app_roots(roots);
    }
    /// A note for the journal: the transport's trust roots, and grants
    /// that do not parse.
    pub fn note(&self) -> Option<&str> {
        self.note.as_deref()
    }
    /// FD readable when the presenter should pump.
    #[cfg(unix)]
    pub fn fd(&self) -> RawFd {
        self.wake.as_raw_fd()
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
    /// Settle a re-ask (`Dispatch::Again`) in its ordered place, with no
    /// work (LLP 1041 §8.4, amended 2026-10-09); kept pending while the
    /// window is full. An error is terminal, for the host to record.
    pub fn again(&self, request: &RequestOut) -> Result<(), &'static str> {
        self.core.again(request.ticket)
    }
    /// Admit work, or return a refusal without an overflow queue.
    pub fn run(&self, request: RequestOut, work: Option<Work>) -> Result<(), &'static str> {
        self.core.run_owned(request, work)
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
    pub fn drain(&self) -> Vec<(u64, Outcome, Option<u64>)> {
        self.core.drain()
    }
    /// Wake for a refusal stored on a runner ticket.
    pub fn notify(&self) {
        self.core.notify();
    }
    /// A wake another thread may keep: a native module's announcements.
    pub fn waker(&self) -> std::sync::Arc<dyn Fn() + Send + Sync> {
        self.core.waker()
    }
    /// Send a mutation's or mapper call's socket batch whose commit stood
    /// (LLP 1016.000 D7).
    pub fn socket(&self, batch: &exact_runner::SocketSend) -> Result<Vec<(u64, u64, u64)>, String> {
        self.core.socket(batch)
    }
    /// Each open socket's buffered bytes, by its stream ticket, live.
    pub fn socket_reader(&self) -> exact_runner::SocketReader {
        self.core.socket_reader()
    }
}
