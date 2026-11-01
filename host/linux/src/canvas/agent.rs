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
use std::sync::atomic::{AtomicBool, Ordering};

/// A driver has asked: the host's turns run at the runner's clock.
static DRIVEN: AtomicBool = AtomicBool::new(false);
/// A driver's launch: the runner's clock from the boot, before any request.
static SEEK: AtomicBool = AtomicBool::new(false);

/// Streams painted since the reader last took one, oldest first. A process
/// has one host, and only its thread paints and takes.
static STREAMS: Mutex<VecDeque<Vec<u32>>> = Mutex::new(VecDeque::new());

fn streams() -> MutexGuard<'static, VecDeque<Vec<u32>>> {
    STREAMS.lock().unwrap_or_else(|e| e.into_inner())
}

/// Whether a driver drives the reader (its first request has come).
pub(crate) fn driven() -> bool {
    DRIVEN.load(Ordering::Relaxed)
}

/// Who owns time from the boot. A driver's launch (`EXACT_AGENT=1`, the
/// reader's `--agent`) is the runner's seek clock from its boot, as the
/// stdio agent's: its clock starts at zero and its date at the drive's
/// epoch, and no frame before the driver's first request moves it (a live
/// clock there fired every boot timer due in the time since `started`, a
/// plan fetched from a URL included). Any other launch: the display's wall
/// from `started` (the timers RFC, D1).
pub(crate) fn own_time<D: DataSource + Default>(
    p: &mut crate::presenter::Presenter<D>,
    started: std::time::Instant,
) {
    if std::env::var("EXACT_AGENT").as_deref() == Ok("1") {
        SEEK.store(true, Ordering::Relaxed);
    } else {
        p.set_live_clock(Some(crate::presenter::wall_since(started)));
    }
}

/// The clock the host's turns run at while a driver holds it, or from the
/// boot of a driver's launch: the runner's. `None` otherwise: the wall's.
/// While it is the runner's, a display turn presents no frame: frame tasks
/// run at the driver's clock steps, as virtual frames, not once per display
/// refresh at an unmoved time.
pub(super) fn clock<D: DataSource>(p: &crate::presenter::Presenter<D>) -> Option<f64> {
    (driven() || SEEK.load(Ordering::Relaxed)).then(|| p.host().now())
}

/// A recording finished over `older`, which the reader never took. With a
/// driver it is queued for the reader; otherwise it is dropped. Unread
/// recordings carry their definitions in [`super::Recorder::finish_stream`].
pub(super) fn kept(older: Option<Vec<u32>>) {
    super::stream::keep(older.filter(|_| driven()));
}

/// The oldest stream the reader has not taken, before the host's own turn.
pub fn next_stream() -> Option<Vec<u32>> {
    super::stream::next_stream()
}

/// A turn's latest stream follows any older stream it could not split.
pub fn after_turn(latest: Option<Vec<u32>>) -> Option<Vec<u32>> {
    super::stream::after_turn(latest)
}

/// Whether a stream waits for the reader: it is owed another turn.
pub fn waiting() -> bool {
    super::stream::waiting()
}

impl<D: DataSource + Default> CanvasHost<D> {
    /// One request of the agent API, answered: a JSON line in, a JSON line
    /// out. `{"op":"ready"}` is the line the stdio carrier writes first.
    pub fn agent(&mut self, line: &str) -> String {
        if std::env::var("EXACT_AGENT").as_deref() != Ok("1") {
            return error("this launch is not a driver's: EXACT_AGENT is not 1");
        }
        // The driver's clock owns time from its first request (the timers RFC, D1)
        if !DRIVEN.swap(true, Ordering::Relaxed) {
            self.p.set_live_clock(None);
        }
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
