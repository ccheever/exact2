//! The agent API in the reader's own process (LLP 1012; LLP 1107 D1's
//! `--reader`): `scripts/agent.mjs android --reader` drives the app the
//! reader shows, on a phone, by the Linux host's own protocol. A request is
//! one JSON line and its answer another, as on stdio ([`crate::agent`]);
//! here they cross by `Native.agent`, on the thread that holds the host,
//! from a socket the shell opens for adb (`host/android`, `AgentDrive`).
//!
//! Nothing is a second protocol: [`CanvasHost::agent`] hands the line to
//! [`crate::agent::handle`]. What this host adds is what that handler
//! assumes of a host with no display and this one has:
//!
//! - **Its paints reach the reader.** The handler paints as it goes (a
//!   `clock` may paint several times), each paint a stream that defines
//!   faces, pictures and rows the next ones draw with. A recording that
//!   finishes over one the reader has not taken hands the older one here
//!   ([`kept`]), and the reader takes them oldest first ([`next_stream`]).
//! - **What it painted was displayed.** A display admits a touch only on
//!   what its last displayed frame showed, and the handler's own paints
//!   are not displayed frames. After each request the host takes one
//!   display turn, so the next request finds what this one made.
//! - **The clock is the driver's.** From the first request the host's
//!   turns run at the runner's clock, which only a `clock` request moves
//!   ([`clock`]): no timer fires and no motion advances between requests.
//!   A reply that lands between them still commits when the reader's turn
//!   finds it, where the stdio carrier commits it with the next request.
//!
//! What the reader plays by itself is outside that clock: a layer it
//! animates (a drive launches with `lower=0`, so the engine samples
//! every one), a fling, the keyboard's slide. A `screenshot` is not
//! answered here: the picture is the device's, and the carrier takes it.
//!
//! Only a launch that is a driver's is answered: one whose environment
//! says `EXACT_AGENT=1`, as the stdio carrier's does. A production bake
//! drops that variable before anything reads it (LLP 1069.007 D2), so it
//! answers no request whatever its shell attaches.

use super::{CanvasHost, FINISHED};
use exact_runner::agent::{error, field_str};
use exact_runner::DataSource;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard};

/// A driver has asked: the host's turns run at the runner's clock.
static DRIVEN: AtomicBool = AtomicBool::new(false);

/// Streams painted since the reader last took one, oldest first. A process
/// has one host, and only its thread paints and takes.
static STREAMS: Mutex<VecDeque<Vec<u32>>> = Mutex::new(VecDeque::new());

fn streams() -> MutexGuard<'static, VecDeque<Vec<u32>>> {
    STREAMS.lock().unwrap_or_else(|e| e.into_inner())
}

fn driven() -> bool {
    DRIVEN.load(Ordering::Relaxed)
}

/// The clock the host's turns run at while a driver holds it: the
/// runner's. `None` with no driver: the wall's.
pub(super) fn clock<D: DataSource>(p: &crate::presenter::Presenter<D>) -> Option<f64> {
    driven().then(|| p.host().now())
}

/// A recording finished over `older`, which the reader never took. With a
/// driver it is kept for the reader; with none it goes, as it always did.
pub(super) fn kept(older: Option<Vec<u32>>) {
    if let Some(ops) = older.filter(|_| driven()) {
        streams().push_back(ops);
    }
}

/// The oldest stream the reader has not taken, before the host's own turn.
/// With no driver there is none, and nothing is locked to say so.
pub fn next_stream() -> Option<Vec<u32>> {
    driven().then(|| streams().pop_front()).flatten()
}

/// Whether a stream waits for the reader: it is owed another turn.
pub fn waiting() -> bool {
    driven() && !streams().is_empty()
}

impl<D: DataSource + Default> CanvasHost<D> {
    /// One request of the agent API, answered: a JSON line in, a JSON line
    /// out. `{"op":"ready"}` is the line the stdio carrier writes first.
    pub fn agent(&mut self, line: &str) -> String {
        if std::env::var("EXACT_AGENT").as_deref() != Ok("1") {
            return error("this launch is not a driver's: EXACT_AGENT is not 1");
        }
        DRIVEN.store(true, Ordering::Relaxed);
        match field_str(line, "op").as_deref() {
            Some("ready") => {
                return format!(
                    "{{\"ready\":true,\"boot\":null,\"views\":{},\"error\":null}}",
                    self.p.node_count()
                )
            }
            Some("screenshot") => {
                return error("the reader's picture is the device's: the carrier takes it")
            }
            _ => {}
        }
        let reply = crate::agent::handle(&mut self.p, line);
        // One display turn, a paint whatever changed: what the request made
        // is admitted to the next one, and its stream follows the handler's.
        self.p.repaint();
        self.force = true;
        let now = self.now();
        let turn = self.frame_held(now);
        // The handler's last stream, when the turn painted nothing over it.
        kept(FINISHED.with(|f| f.borrow_mut().take()));
        kept(turn);
        reply
    }
}
