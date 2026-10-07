//! The phone's model: one pairing, the server answering now, the desktop's
//! last layout, and the open session's conversation.
//!
//! I/O runs in lanes. Each lane is one resource in the contract keyed by a
//! turn number; the model bumps a turn when it wants that lane's request,
//! `lib.rs` answers the turn with the request, and the reply comes back to
//! `*_done`. A lane never bumps while its request is in flight, so a reply
//! always belongs to the turn that asked.
//!
//! Failover (the laptop is closed): the Mac's `fleet serve` runs the same
//! server, with the same token, on every enrolled machine. Three failed
//! polls send the phone through the peers' `/api/health`, in order, and the
//! first that answers serves the sessions it can see; the desktop's layout
//! is the one kept from the Mac's last answer. Once a minute the phone asks
//! after the Mac and goes home when it answers.

use crate::api::Connection;
use crate::fleet::{Fleet, Peer, SendRoute, Window};
use crate::telemetry::{Attr, Telemetry};
use ocho_data::transcript::{Entry, Transcript, TranscriptUpdate};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};

/// A failed poll's kind, for telemetry.
fn failure_kind((status, why): &(u16, String)) -> &'static str {
    let lower = why.to_lowercase();
    match status {
        502 if lower.contains("not connected") || lower.contains("machine disconnected") => {
            "not_connected"
        }
        401 | 403 => "refused",
        0 if lower.contains("timed out") => "timeout",
        0 if lower.contains("offline")
            || lower.contains("network")
            || lower.contains("internet") =>
        {
            "offline"
        }
        0 => "network",
        _ => "http",
    }
}

/// A failed poll, said so a person knows what to check.
fn poll_failure((status, why): &(u16, String)) -> String {
    let lower = why.to_lowercase();
    match status {
        502 if lower.contains("not connected") || lower.contains("machine disconnected") => {
            "It isn't connected to the relay. Is it asleep, or is Ocho closed there?".into()
        }
        401 | 403 => "It refused this pairing. Pair again from Pair Phone.".into(),
        0 if lower.contains("timed out") => "The relay didn't answer in time.".into(),
        0 if lower.contains("offline")
            || lower.contains("network")
            || lower.contains("internet") =>
        {
            "This phone is offline.".into()
        }
        0 => why.lines().next().unwrap_or("").trim().to_string(),
        status => format!(
            "The relay answered {status}: {}",
            why.lines().next().unwrap_or("").trim()
        ),
    }
}

/// Store names (each has a `secret.keep` grant).
pub const KEY_CONNECTION: &str = "ocho.connection";
/// This install's random telemetry id.
pub const KEY_INSTALL: &str = "ocho.install";
/// The desktop layout and peers, kept for when the Mac is away.
pub const KEY_DESKTOP: &str = "ocho.desktop";

const FAILURES_BEFORE_FAILOVER: u32 = 3;
/// How long the Mac must be unreachable before a peer answers instead: an
/// update or relaunch of the desktop restarts `fleet serve`, which is off the
/// relay for a few seconds, and that is not the laptop closing.
const FAILOVER_AFTER_MS: f64 = 20_000.0;
/// How often, while a peer answers, the phone asks whether the Mac is back
/// (its `/api/health`, a few dozen bytes).
const HOME_CHECK_MS: f64 = 15_000.0;
/// The first look home after a switch to a peer: the Mac may only have been
/// restarting Ocho (a few seconds off the relay), so it comes back quickly.
const FIRST_HOME_CHECK_MS: f64 = 3_000.0;
const FAILOVER_RETRY_MS: f64 = 30_000.0;
const TRANSCRIPT_LIVE_MS: f64 = 2_000.0;
/// The same, from a server that answers only what changed (a few hundred
/// bytes, not the whole conversation): a working session reads closer to
/// live.
const TRANSCRIPT_DELTA_LIVE_MS: f64 = 1_000.0;
const TRANSCRIPT_IDLE_MS: f64 = 5_000.0;
const AFTER_SEND_MS: f64 = 800.0;
/// How long the last answer's sessions stay live through failed polls: a
/// bad connection drops a poll or two, which is not every session ending.
const POLL_GRACE_MS: f64 = 45_000.0;
/// The least time between polls. Working sessions bump the fleet's version
/// every second, so a long poll returns at once and each answer is the whole
/// fleet (about 180 KB compressed): polling back to back downloads that
/// every second, on whatever connection the phone has.
const POLL_GAP_MS: f64 = 2_500.0;

/// One request at a time, keyed by its turn.
#[derive(Clone, Debug, Default)]
pub struct Lane {
    /// The turn the contract asks with.
    pub turn: u64,
    /// A request is out.
    pub inflight: bool,
    /// Not before this (ms).
    pub next_at: f64,
    /// Failures in a row.
    pub failures: u32,
    /// When the request went out (ms).
    started_at: f64,
}

impl Lane {
    fn bump(&mut self) -> bool {
        if self.inflight {
            return false;
        }
        self.turn += 1;
        true
    }

    fn due(&self, now: f64) -> bool {
        !self.inflight && now >= self.next_at
    }

    fn start(&mut self, now: f64) {
        self.inflight = true;
        self.started_at = now;
    }

    /// Out longer than `limit`: its reply is not coming (a request iOS
    /// dropped when the app was in the background never calls back).
    fn overdue(&self, now: f64, limit: f64) -> bool {
        self.inflight && now - self.started_at > limit
    }
}

/// A session: machine and id.
pub type Key = (String, String);

/// A conversation as last read.
#[derive(Clone, Debug, Default)]
pub struct Conversation {
    /// The entries.
    pub transcript: Transcript,
    /// Read at least once.
    pub loaded: bool,
    /// Why the last read failed.
    pub error: String,
    /// Entries drawn, from the end: `view::PAGE_ENTRIES` until "Show earlier".
    pub shown: usize,
    /// The revision of the transcript held: the next read asks only for
    /// what changed since.
    pub revision: String,
    /// The server's fingerprint of the entries held but the last.
    pub base: String,
}

/// A message on its way.
#[derive(Clone, Debug)]
pub struct Outgoing {
    /// Where.
    pub key: Key,
    /// What.
    pub text: String,
    /// How.
    pub route: SendRoute,
    /// The idempotency key `/message` and the queue take.
    pub request_id: String,
    /// The Codex thread a queued message goes to.
    pub thread: String,
    /// Where a queued message is in attach, send, detach.
    pub step: Step,
    /// Interrupt the running turn for it (`interrupt-send`) instead of waiting.
    pub interrupt: bool,
    /// A queued message's route (`codex-client`, `session-client`).
    pub leaf: &'static str,
}

/// A queued Codex message's requests, in order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Step {
    /// Subscribe this phone to the thread.
    #[default]
    Attach,
    /// Hand Fleet the message; it waits for the running turn.
    Send,
    /// Unsubscribe, so the desktop's attached badge clears.
    Detach,
}

/// Sent and not yet in the transcript: drawn as the user's bubble.
#[derive(Clone, Debug)]
pub struct Pending {
    /// Where.
    pub key: Key,
    /// What.
    pub text: String,
    /// The transcript's length when sent.
    pub after: usize,
    /// Its idempotency key: what "Send now" asks for again.
    pub request_id: String,
    /// Through Codex's queue: it waits behind a running turn and can be sent
    /// now instead, interrupting it.
    pub queued: bool,
    /// "Send now" was pressed: the turn is being interrupted for it.
    pub interrupting: bool,
}

/// What survives a relaunch besides the pairing.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct Kept {
    windows: Vec<Window>,
    peers: Vec<Peer>,
}

/// The whole phone.
#[derive(Default)]
pub struct Model {
    /// The pairing.
    pub conn: Option<Connection>,
    /// The store was read.
    pub loaded: bool,
    /// The clock, ms, as the last event carried it.
    pub now: f64,
    /// The view's version.
    pub version: u64,

    /// The machine answering now: home, or a peer.
    pub via: String,
    /// The last answer from `via`.
    pub fleet: Option<Fleet>,
    /// The last poll succeeded.
    pub fresh: bool,
    /// When the last poll succeeded (ms).
    answered_at: f64,
    /// When this run first knew the time (ms), and polls asked until the
    /// first answer: how long "Connecting…" showed.
    launched_at: f64,
    polls_before_answer: u32,
    connected: bool,
    /// The view has shown the last answer as too old to trust.
    expired: bool,
    /// The server refused the token.
    pub refused: bool,
    /// Why the last poll failed, in words a person can act on.
    pub poll_error: String,
    since: u64,
    instance: String,
    /// The phone view's tag, from the last answer.
    tag: String,
    /// The fleet long poll.
    pub poll: Lane,
    poll_via: String,

    /// Peers, from the Mac's last answer.
    pub peers: Vec<Peer>,
    /// The health probe.
    pub probe: Lane,
    probe_queue: Vec<String>,
    probe_target: String,
    next_home_check: f64,
    failover_retry_at: f64,

    /// The desktop's windows: live from home, else kept.
    pub windows: Vec<Window>,
    /// Which window the phone shows.
    pub window: usize,

    /// The open session.
    pub open: Option<Key>,
    /// Conversations read this run.
    pub conversations: HashMap<Key, Conversation>,
    /// The transcript poll.
    pub transcript: Lane,
    transcript_key: Option<Key>,
    /// Bumped to jump the conversation to its end.
    pub scroll_revision: u64,
    /// The native composer's height (points), as it last asked.
    pub composer_height: f64,

    /// The message lane.
    pub send: Lane,
    outbox: VecDeque<Outgoing>,
    sending: Option<Outgoing>,
    /// Sent, not yet read back.
    pub pending: Vec<Pending>,
    /// The last send's failure: where, what, why.
    pub failed: Option<(Key, String, String)>,
    sent_count: u64,

    /// Pull to refresh.
    pub refresh: Lane,

    /// The page was seen visible and online at the last ask.
    awake: Option<bool>,

    /// Health and funnel events, and the lane that sends them.
    pub telemetry: Telemetry,
    /// The telemetry lane.
    pub report: Lane,
    /// When the current run of failed polls began (ms), for recovery time.
    failing_since: f64,
    /// A stall (no answer for a while) was reported and not yet over.
    stall_reported: bool,

    /// The haptic lane: the app's native module plays it.
    pub buzz: Lane,
    /// The haptic to play next (the latest wins: two in one turn are one).
    haptic: Option<&'static str>,
    /// The open session's state at the last answer, for its haptics.
    open_state: Option<(Key, String)>,

    /// Marking desktop tabs read (the Mac's `POST /api/desktop/read`).
    pub reads: Lane,
    /// Tab keys read here and not yet sent to the Mac.
    read_queue: Vec<String>,
    /// Tab keys read here that the Mac's sidebar may still show unread: until
    /// its next answer after the mark lands. `true` once the mark is sent.
    read_here: HashMap<String, bool>,
    /// Keys out in the read lane's request.
    reads_out: Vec<String>,
    /// Whether each session was working at its last observation.
    turn_running: HashMap<Key, bool>,
    /// Tabs whose turn finished while the Mac was away: this phone marks
    /// them until the Mac's own record is back.
    unread_here: HashMap<String, f64>,
    /// The refresh is out.
    pub refreshing: bool,

    /// The pairing field.
    pub pair_draft: String,
    /// Why pairing failed.
    pub pair_error: String,

    /// Store writes to make: name, value (`None` forgets).
    pub writes: Vec<(&'static str, Option<String>)>,
}

impl Model {
    /// Read the pairing and the kept layout, once.
    pub fn load(&mut self, connection: Option<&str>, desktop: Option<&str>, install: Option<&str>) {
        if self.loaded {
            return;
        }
        self.loaded = true;
        match install.filter(|id| id.len() == 32) {
            Some(id) => self.telemetry.install = id.to_string(),
            None => {
                let id =
                    crate::telemetry::new_install_id(self.now, std::ptr::from_ref(self) as u64);
                self.writes.push((KEY_INSTALL, Some(id.clone())));
                self.telemetry.install = id;
            }
        }
        self.track(
            "app.launch",
            false,
            vec![("paired", connection.is_some().into())],
        );
        if let Some(kept) = desktop.and_then(|d| serde_json::from_str::<Kept>(d).ok()) {
            self.windows = kept.windows;
            self.peers = kept.peers;
        }
        if let Some(conn) = connection.and_then(|c| serde_json::from_str::<Connection>(c).ok()) {
            self.adopt(conn);
        }
        self.version += 1;
    }

    fn adopt(&mut self, conn: Connection) {
        self.via = conn.machine.clone();
        self.conn = Some(conn);
        self.since = 0;
        self.instance.clear();
        self.tag.clear();
        self.poll = Lane {
            turn: self.poll.turn,
            ..Lane::default()
        };
        self.poll.bump();
    }

    /// The Mac.
    pub fn home(&self) -> &str {
        self.conn.as_ref().map(|c| c.machine.as_str()).unwrap_or("")
    }

    /// Where `machine` answers on the relay: its key's route when the
    /// pairing (the Mac) or the Mac's peer list gave one, else its id.
    pub fn route_of(&self, machine: &str) -> String {
        let route = match &self.conn {
            Some(c) if c.machine == machine => c.route.clone(),
            _ => self
                .peers
                .iter()
                .find(|p| p.id == machine)
                .map(|p| p.route.clone())
                .unwrap_or_default(),
        };
        if route.is_empty() {
            machine.to_string()
        } else {
            route
        }
    }

    /// The Mac's name.
    pub fn home_name(&self) -> String {
        self.conn
            .as_ref()
            .map(|c| c.name.clone())
            .unwrap_or_default()
    }

    /// A peer's name, else the fleet's.
    pub fn name_of(&self, machine: &str) -> String {
        if machine == self.home() {
            return self.home_name();
        }
        if let Some(p) = self.peers.iter().find(|p| p.id == machine) {
            return p.name.clone();
        }
        match &self.fleet {
            Some(f) => f.machine_name(machine),
            None => machine.chars().take(8).collect(),
        }
    }

    // ---- Events ----

    /// The time (ms). The first poll goes out at launch, before any event has
    /// said what time it is: it started now, not at the epoch, or the
    /// watchdog would give it up on the first tick.
    pub fn clock(&mut self, now: f64) {
        if self.now == 0.0 {
            for lane in [
                &mut self.poll,
                &mut self.transcript,
                &mut self.send,
                &mut self.probe,
                &mut self.refresh,
                &mut self.reads,
                &mut self.report,
                &mut self.buzz,
            ] {
                if lane.inflight && lane.started_at == 0.0 {
                    lane.started_at = now;
                }
            }
        }
        if self.launched_at == 0.0 {
            self.launched_at = now;
        }
        self.now = now;
    }

    /// One second: start whatever lane is due.
    pub fn tick(&mut self, now: f64) {
        self.clock(now);
        if self.conn.is_none() {
            return;
        }
        self.give_up_on_overdue(now);
        if self.telemetry.due(now) && !self.report.inflight {
            self.report.bump();
        }
        if self.fresh && !self.stall_reported && self.since_answer() >= 15_000.0 {
            self.stall_reported = true;
            let quiet = (self.since_answer() / 1000.0) as i64;
            let inflight = self.poll.inflight;
            self.track(
                "poll.stalled",
                true,
                vec![("quiet_s", quiet.into()), ("inflight", inflight.into())],
            );
        }
        if !self.fresh && !self.expired && !self.trusted() {
            // The grace ran out: sessions grey now, not at the next event.
            self.expired = true;
            self.version += 1;
        }
        if self.poll.due(now) {
            self.poll.bump();
        }
        if !self.probe.inflight && now >= self.probe.next_at {
            if !self.probe_queue.is_empty() {
                self.probe.bump();
            } else if self.via != self.home() && now >= self.next_home_check {
                self.next_home_check = now + HOME_CHECK_MS;
                self.probe_queue = vec![self.home().to_string()];
                self.probe.bump();
            } else if !self.fresh && self.failover_due() && now >= self.failover_retry_at {
                self.begin_failover();
            }
        }
        if self.open.is_some() && self.transcript.due(now) {
            self.transcript.bump();
        }
        if !self.outbox.is_empty() && !self.send.inflight {
            self.send.bump();
        }
        if self.haptic.is_some() && !self.buzz.inflight {
            self.buzz.bump();
        }
        if !self.read_queue.is_empty() && self.via == self.home() && self.reads.due(now) {
            self.reads.bump();
        }
    }

    /// The page's visibility or connection changed. Waking (back to the
    /// foreground, back online) gives up on what was out, which iOS dropped
    /// while the app was suspended, and asks again now, not at the watchdog.
    pub fn page(&mut self, visible: bool, online: bool) {
        let awake = visible && online;
        let was = self.awake.replace(awake);
        if awake && was == Some(false) {
            self.telemetry
                .track(self.now, "app.foreground", false, vec![]);
            for lane in [
                &mut self.poll,
                &mut self.transcript,
                &mut self.probe,
                &mut self.refresh,
                &mut self.reads,
            ] {
                lane.inflight = false;
                lane.next_at = 0.0;
            }
            self.transcript_key = None;
            self.reads.inflight = false;
            if !self.reads_out.is_empty() {
                let out = std::mem::take(&mut self.reads_out);
                self.read_queue.extend(out);
            }
            self.poll.failures = 0;
            self.version += 1;
        } else if !awake {
            // The poll in flight is forgotten when the page is re-asked; the
            // next waking asks again.
            self.poll.inflight = false;
        }
    }

    /// The poll was asked again, so the runtime dropped the one in flight.
    pub fn poll_replaced(&mut self) {
        self.poll.inflight = false;
    }

    /// Requests out too long fail, so their lanes ask again. A reply that
    /// comes after all is for an old turn, and `lib.rs` drops it.
    fn give_up_on_overdue(&mut self, now: f64) {
        // The long poll holds 25 s, plus the server's settle and the relay;
        // the first (`since` 0) is answered at once, so waiting that long
        // for it is only "Connecting…" on the screen.
        let limit = if self.since == 0 { 10_000.0 } else { 40_000.0 };
        if self.poll.overdue(now, limit) {
            self.poll.inflight = false;
            self.poll_failed();
        }
        if self.transcript.overdue(now, 30_000.0) {
            self.transcript_done(Err("No answer from the Mac.".into()));
        }
        // An interrupting send waits on the server (up to 35 s) for the turn
        // to stop before it answers.
        if self.send.overdue(now, 45_000.0) {
            self.send_done(Err(
                "No answer from the Mac; the message may not have arrived.".into(),
            ));
        }
        if self.probe.overdue(now, 15_000.0) {
            self.probe_done(false);
        }
        if self.refresh.overdue(now, 30_000.0) {
            self.refresh_done();
        }
        if self.reads.overdue(now, 30_000.0) {
            self.reads_done(Err(0));
        }
        if self.report.overdue(now, 30_000.0) {
            self.report_done(false);
        }
        if self.buzz.overdue(now, 5_000.0) {
            self.haptic_done();
        }
    }

    /// The turn `source`'s lane is on: a reply asked with another is stale.
    pub fn lane_turn(&self, source: &str) -> Option<u64> {
        Some(match source {
            "poll" => self.poll.turn,
            "transcript" => self.transcript.turn,
            "send" => self.send.turn,
            "probe" => self.probe.turn,
            "resync" => self.refresh.turn,
            "markRead" => self.reads.turn,
            "haptic" => self.buzz.turn,
            "report" => self.report.turn,
            _ => return None,
        })
    }

    /// The pairing field changed.
    pub fn pair_typed(&mut self, text: &str) {
        self.pair_draft = text.to_string();
        self.pair_error.clear();
        self.version += 1;
    }

    /// Pair with what was pasted.
    pub fn pair(&mut self) {
        self.pair_from("paste");
    }

    fn pair_from(&mut self, via: &'static str) {
        let scanned = via == "scan";
        match Connection::parse(&self.pair_draft) {
            Ok(conn) => {
                let keyed = !conn.route.is_empty();
                self.track(
                    "pair.result",
                    false,
                    vec![
                        ("ok", true.into()),
                        ("via", via.into()),
                        ("keyed", keyed.into()),
                    ],
                );
                self.writes.push((
                    KEY_CONNECTION,
                    Some(serde_json::to_string(&conn).unwrap_or_default()),
                ));
                self.pair_draft.clear();
                self.pair_error.clear();
                self.refused = false;
                self.adopt(conn);
                // The scanner already played success as it read the code.
                if !scanned {
                    self.feel("success");
                }
            }
            Err(e) => {
                let kind = if e.contains("reaches") {
                    "other_relay"
                } else if e.contains("no machine or token") {
                    "incomplete"
                } else if e.contains("route") {
                    "bad_route"
                } else if e.contains("Paste") {
                    "empty"
                } else {
                    "not_pairing"
                };
                self.track(
                    "pair.result",
                    true,
                    vec![
                        ("ok", false.into()),
                        ("via", via.into()),
                        ("error", kind.into()),
                    ],
                );
                self.pair_error = e;
                self.feel("error");
            }
        }
        self.version += 1;
    }

    /// A pairing link opened the app (`ocho://pair/…`, from the relay's
    /// pairing page after the iPhone Camera read the QR).
    pub fn pair_link(&mut self, location: &str) {
        self.pair_draft = location.to_string();
        self.pair_from("link");
        if self.conn.is_none() {
            self.pair_draft.clear();
            self.pair_error = "That link isn't a complete Ocho pairing. Scan the code under Pair Phone on your Mac again.".into();
        }
    }

    /// A QR code the camera read: Pair Phone's, or something else.
    pub fn pair_scanned(&mut self, text: &str) {
        self.pair_draft = text.to_string();
        self.pair_from("scan");
        if self.conn.is_none() {
            self.pair_draft.clear();
            self.pair_error = "That QR code isn't an Ocho pairing code. Scan the one under Pair Phone on your Mac.".into();
        }
    }

    /// Forget the pairing and everything kept with it.
    pub fn unpair(&mut self) {
        self.writes.push((KEY_CONNECTION, None));
        self.writes.push((KEY_DESKTOP, None));
        let writes = std::mem::take(&mut self.writes);
        let (version, turns) = (self.version, self.turns());
        *self = Model {
            loaded: true,
            now: self.now,
            writes,
            ..Model::default()
        };
        // Turns only move forward: a lane keyed by an old turn must not
        // collide with a new one.
        (
            self.poll.turn,
            self.transcript.turn,
            self.send.turn,
            self.probe.turn,
            self.refresh.turn,
            self.reads.turn,
            self.buzz.turn,
            self.report.turn,
        ) = turns;
        self.version = version + 1;
    }

    fn turns(&self) -> (u64, u64, u64, u64, u64, u64, u64, u64) {
        (
            self.poll.turn,
            self.transcript.turn,
            self.send.turn,
            self.probe.turn,
            self.refresh.turn,
            self.reads.turn,
            self.buzz.turn,
            self.report.turn,
        )
    }

    /// Record a health or funnel event (see `telemetry.rs` for what never
    /// goes in one).
    /// An event, with the machine answering now (its tag, and whether it is
    /// the Mac this phone paired with or a peer).
    fn track(&mut self, name: &'static str, warn: bool, mut attrs: Vec<(&'static str, Attr)>) {
        if !self.via.is_empty() {
            let role = if self.via == self.home() {
                "home"
            } else {
                "peer"
            };
            attrs.push(("machine", crate::telemetry::machine_tag(&self.via).into()));
            attrs.push(("role", role.into()));
        }
        self.telemetry.track(self.now, name, warn, attrs);
    }

    /// The telemetry batch to send, if one is due.
    pub fn report_request(&mut self) -> Option<String> {
        let body = self.telemetry.take(self.now, &crate::telemetry::build())?;
        self.report.start(self.now);
        Some(body)
    }

    /// The batch landed, or it waits for the next.
    pub fn report_done(&mut self, ok: bool) {
        self.report.inflight = false;
        self.telemetry.done(ok);
        if !ok {
            self.report.failures += 1;
        } else {
            self.report.failures = 0;
        }
    }

    /// Play `kind` (`success`, `warning`, `error`, `light`, …) now.
    fn feel(&mut self, kind: &'static str) {
        self.haptic = Some(kind);
        self.buzz.bump();
    }

    /// The haptic to play, if one waits: the native module's request body.
    pub fn haptic_request(&mut self) -> Option<String> {
        let kind = self.haptic.take()?;
        self.buzz.start(self.now);
        Some(serde_json::json!({ "op": "haptic", "kind": kind }).to_string())
    }

    /// The haptic played (or the module could not: nothing to retry).
    pub fn haptic_done(&mut self) {
        self.buzz.inflight = false;
    }

    /// Show another desktop window.
    pub fn pick_window(&mut self, index: usize) {
        self.window = index;
        self.version += 1;
    }

    /// Open a session's conversation.
    pub fn open(&mut self, machine: &str, session: &str) {
        let key = (machine.to_string(), session.to_string());
        self.mark_read(&key);
        // Its state now: the next answer's change is what is felt.
        self.open_state = self
            .live_session(&key)
            .map(|s| (key.clone(), s.state.clone()));
        if self.open.as_ref() != Some(&key) {
            self.open = Some(key);
            self.transcript.next_at = 0.0;
            self.transcript.failures = 0;
            self.transcript.bump();
            self.scroll_revision += 1;
        }
        self.version += 1;
    }

    /// Back to the list.
    pub fn close(&mut self) {
        self.open = None;
        self.version += 1;
    }

    /// The router moved (a back swipe, a restored location).
    pub fn navigated(&mut self, location: &str) {
        let parts: Vec<&str> = location.trim_matches('/').split('/').collect();
        match parts.as_slice() {
            ["s", machine, session] => {
                self.open(&crate::api::decode(machine), &crate::api::decode(session))
            }
            _ => self.close(),
        }
    }

    /// Pull to refresh: ask the server to observe now.
    pub fn pull(&mut self) {
        if self.conn.is_none() {
            return;
        }
        // Whatever the lanes believe, a pull starts a fresh read: a poll
        // that never came back is given up, and the next goes now.
        self.poll.inflight = false;
        self.poll.next_at = 0.0;
        self.poll.bump();
        if self.refresh.bump() {
            self.feel("light");
            self.refreshing = true;
        }
        self.version += 1;
    }

    /// How long since the server answered (ms); 0 before it ever has.
    pub fn since_answer(&self) -> f64 {
        if self.fleet.is_none() {
            0.0
        } else {
            (self.now - self.answered_at).max(0.0)
        }
    }

    // ---- Requests: what each lane's turn asks ----

    /// The poll's URL and bearer.
    pub fn poll_request(&mut self) -> Option<(String, String)> {
        let conn = self.conn.as_ref()?;
        if !self.connected {
            self.polls_before_answer += 1;
        }
        let mut url = conn.fleet_url(
            &self.route_of(&self.via),
            self.since,
            &self.instance,
            &self.tag,
        );
        // Exact keeps a request in flight when it is asked for an equal one
        // (LLP 1054.000.000 D3), so a retry of a poll that hung, a relay
        // that never answered, would wait on that same request until its 60 s
        // timeout. A retry differs; the server ignores the parameter.
        if self.poll.failures > 0 {
            url.push_str(&format!("&try={}", self.poll.failures));
        }
        let bearer = conn.bearer();
        self.poll.start(self.now);
        self.poll_via = self.via.clone();
        Some((url, bearer))
    }

    /// A finished poll: the body on a 200, else why.
    pub fn poll_done(&mut self, result: Result<serde_json::Value, (u16, String)>) {
        self.poll.inflight = false;
        if self.poll_via != self.via {
            // The phone moved servers while this was out.
            self.poll.next_at = self.now;
            return;
        }
        let why = match &result {
            Ok(_) => String::new(),
            Err(failure) => poll_failure(failure),
        };
        let kind = match &result {
            Ok(_) => "parse",
            Err(failure) => failure_kind(failure),
        };
        let status = match &result {
            Ok(_) => 200,
            Err((status, _)) => i64::from(*status),
        };
        match result.ok().as_ref().and_then(Fleet::read) {
            Some(fleet) => {
                if self.poll.failures > 0 {
                    let after = ((self.now - self.failing_since) / 1000.0) as i64;
                    let failures = i64::from(self.poll.failures);
                    self.track(
                        "connection.recovered",
                        false,
                        vec![("after_s", after.into()), ("failures", failures.into())],
                    );
                }
                self.stall_reported = false;
                self.poll.failures = 0;
                self.poll.next_at = self.now + POLL_GAP_MS;
                self.refused = false;
                self.poll_error.clear();
                self.apply(fleet);
            }
            None => {
                if self.poll.failures == 0 {
                    self.failing_since = self.now;
                    self.track(
                        "poll.failure",
                        true,
                        vec![("kind", kind.into()), ("status", status.into())],
                    );
                }
                self.poll_error = if why.is_empty() {
                    "Its server answered with something this app doesn't understand.".into()
                } else {
                    why
                };
                self.poll_failed();
                // The relay itself says the machine is gone (asleep, quit, or
                // its connection just dropped): no reason to wait out the
                // retries for a peer; look for one now. A timeout or a
                // dropped request could be the network, so it still waits.
                if kind == "not_connected" && self.via_is_definitely_gone() {
                    self.begin_failover();
                }
            }
        }
    }

    /// A poll that failed: back off; after three, look for a peer.
    pub fn poll_refused(&mut self) {
        self.refused = true;
    }

    fn poll_failed(&mut self) {
        self.poll.failures += 1;
        let backoff = (1u64 << self.poll.failures.min(5)) as f64 * 1000.0;
        self.poll.next_at = self.now + backoff.min(30_000.0);
        if self.fresh || self.poll.failures == FAILURES_BEFORE_FAILOVER {
            self.version += 1;
        }
        self.fresh = false;
        if self.failover_due() {
            self.begin_failover();
        }
    }

    /// A definite "not connected" is worth acting on only when somewhere
    /// else could answer: a peer, or home while a peer is answering.
    fn via_is_definitely_gone(&self) -> bool {
        self.via != self.home() || self.peers.iter().any(|p| p.id != self.via)
    }

    /// Enough failures, for long enough, to look for a peer.
    fn failover_due(&self) -> bool {
        self.poll.failures >= FAILURES_BEFORE_FAILOVER
            && self.now - self.answered_at >= FAILOVER_AFTER_MS
    }

    fn apply(&mut self, fleet: Fleet) {
        self.since = fleet.version;
        self.instance = fleet.instance.clone();
        self.tag = fleet.tag.clone();
        if self.via == self.home() {
            let mut keep = false;
            if !fleet.windows.is_empty() && fleet.windows != self.windows {
                self.windows = fleet.windows.clone();
                keep = true;
            }
            if !fleet.peers.is_empty() && fleet.peers != self.peers {
                self.peers = fleet.peers.clone();
                keep = true;
            }
            if keep {
                let kept = Kept {
                    windows: self.windows.clone(),
                    peers: self.peers.clone(),
                };
                self.writes
                    .push((KEY_DESKTOP, serde_json::to_string(&kept).ok()));
            }
        }
        self.observe_turns(&fleet);
        if self.via == self.home() {
            // The Mac's sidebar is the record again: what it shows is what
            // is unread, and marks it has applied need no holding.
            self.unread_here.clear();
            self.read_here.retain(|_, sent| !*sent);
        }
        self.fleet = Some(fleet);
        self.fresh = true;
        self.answered_at = self.now;
        if !self.connected {
            self.connected = true;
            let ms = (self.now - self.launched_at).max(0.0) as i64;
            let polls = i64::from(self.polls_before_answer);
            let via_home = self.via == self.home();
            self.track(
                "connect.first",
                ms > 5_000,
                vec![
                    ("ms", ms.into()),
                    ("polls", polls.into()),
                    ("home", via_home.into()),
                ],
            );
        }
        self.expired = false;
        // The open session's turn ends (success) or stops for you (warning).
        let open_now = self.open.clone().and_then(|key| {
            let state = self.live_session(&key)?.state.clone();
            Some((key, state))
        });
        if let (Some((was_key, was)), Some((key, now))) = (&self.open_state, &open_now) {
            let working = |s: &str| matches!(s, "running" | "starting");
            let blocked = |s: &str| matches!(s, "blocked" | "awaiting approval");
            if was_key == key && was != now {
                if blocked(now) {
                    self.feel("warning");
                } else if working(was) && now == "idle" {
                    self.feel("success");
                }
            }
        }
        self.open_state = open_now;
        // Reading the open session as its turn ends: it is read.
        if let Some(key) = self.open.clone() {
            let unread = self.tab_keys(&key).iter().any(|k| self.unread(k));
            if unread {
                self.mark_read(&key);
            }
        }
        self.version += 1;
    }

    fn begin_failover(&mut self) {
        if self.probe.inflight || !self.probe_queue.is_empty() {
            return;
        }
        let home = self.home().to_string();
        let mut queue: Vec<String> = Vec::new();
        if self.via != home {
            queue.push(home);
        }
        for p in &self.peers {
            if p.id != self.via && !queue.contains(&p.id) {
                queue.push(p.id.clone());
            }
        }
        self.failover_retry_at = self.now + FAILOVER_RETRY_MS;
        self.probe_queue = queue;
        self.probe.next_at = 0.0;
        self.probe.bump();
    }

    /// The probe's URL, if one is queued.
    pub fn probe_request(&mut self) -> Option<String> {
        let conn = self.conn.as_ref()?;
        if self.probe_queue.is_empty() {
            return None;
        }
        self.probe_target = self.probe_queue.remove(0);
        self.probe.start(self.now);
        Some(conn.url(&self.route_of(&self.probe_target), "/health"))
    }

    /// A finished probe: answered or not.
    pub fn probe_done(&mut self, ok: bool) {
        self.probe.inflight = false;
        if !ok {
            return;
        }
        let target = std::mem::take(&mut self.probe_target);
        self.probe_queue.clear();
        if target == self.via && self.fresh {
            return;
        }
        let to = if target == self.home() {
            "home"
        } else {
            "peer"
        };
        let target_tag = crate::telemetry::machine_tag(&target);
        self.track(
            "failover",
            true,
            vec![("to", to.into()), ("target", target_tag.into())],
        );
        self.via = target;
        self.since = 0;
        self.instance.clear();
        self.tag.clear();
        self.poll.failures = 0;
        self.poll.next_at = 0.0;
        self.next_home_check = self.now
            + if to == "peer" {
                FIRST_HOME_CHECK_MS
            } else {
                HOME_CHECK_MS
            };
        self.transcript.next_at = 0.0;
        self.version += 1;
    }

    /// The open session's transcript URL and bearer.
    pub fn transcript_request(&mut self) -> Option<(String, String)> {
        let conn = self.conn.as_ref()?;
        let key = self.open.clone()?;
        let mut url = conn.session_url(&self.route_of(&self.via), &key.0, &key.1, "transcript");
        // What this phone holds, so the server answers "unchanged" or only
        // the entries that moved (a long conversation is hundreds of KB).
        if let Some(c) = self
            .conversations
            .get(&key)
            .filter(|c| c.loaded && c.error.is_empty())
        {
            if !c.revision.is_empty() {
                url.push_str(&format!("?revision={}", crate::api::encode(&c.revision)));
                if !c.base.is_empty() {
                    url.push_str(&format!(
                        "&have={}&base={}",
                        c.transcript.entries.len(),
                        crate::api::encode(&c.base)
                    ));
                }
            }
        }
        let bearer = conn.bearer();
        self.transcript.start(self.now);
        self.transcript_key = Some(key);
        Some((url, bearer))
    }

    /// A finished transcript read.
    pub fn transcript_done(&mut self, result: Result<String, String>) {
        self.transcript.inflight = false;
        let Some(key) = self.transcript_key.take() else {
            return;
        };
        let live = self
            .live_session(&key)
            .is_some_and(|s| s.working() || s.blocked());
        let conversation = self.conversations.entry(key.clone()).or_default();
        let deltas = !conversation.base.is_empty();
        self.transcript.next_at = self.now
            + match (live, deltas) {
                (true, true) => TRANSCRIPT_DELTA_LIVE_MS,
                (true, false) => TRANSCRIPT_LIVE_MS,
                _ => TRANSCRIPT_IDLE_MS,
            };
        match result.and_then(|body| TranscriptUpdate::decode(&body).map_err(|e| e.to_string())) {
            Ok(update) => {
                let mut changed = !conversation.loaded || !conversation.error.is_empty();
                conversation.loaded = true;
                conversation.error.clear();
                conversation.revision = update.revision;
                conversation.base = update.base;
                let entries = &mut conversation.transcript.entries;
                match update.from {
                    _ if update.unchanged => {}
                    // Only what moved: the entries from `from` on.
                    Some(from) if from <= entries.len() => {
                        if entries[from..] != update.transcript.entries[..] {
                            entries.truncate(from);
                            entries.extend(update.transcript.entries);
                            changed = true;
                        }
                    }
                    _ => {
                        if *entries != update.transcript.entries {
                            *entries = update.transcript.entries;
                            changed = true;
                        }
                    }
                }
                let entries = &conversation.transcript.entries;
                let before = self.pending.len();
                self.pending.retain(|p| {
                    p.key != key
                        || !entries
                            .iter()
                            .skip(p.after)
                            .any(|e| e.kind == "user" && send::echoes(&e.text, &p.text))
                });
                if changed || before != self.pending.len() {
                    self.version += 1;
                }
            }
            Err(e) => {
                self.transcript.failures += 1;
                if self.transcript.failures == 1 {
                    let kind = if e.contains("timed out") {
                        "timeout"
                    } else if e.contains("not connected") {
                        "not_connected"
                    } else {
                        "other"
                    };
                    self.telemetry.track(
                        self.now,
                        "transcript.failure",
                        true,
                        vec![("kind", kind.into())],
                    );
                }
                if conversation.error.is_empty() || !conversation.loaded {
                    conversation.loaded = true;
                    self.version += 1;
                }
                conversation.error = e;
            }
        }
    }

    /// Every desktop tab showing `key`'s session.
    fn tab_keys(&self, key: &Key) -> Vec<String> {
        self.windows
            .iter()
            .flat_map(|w| &w.tabs)
            .filter(|t| t.machine == key.0 && t.session == key.1)
            .map(|t| t.key.clone())
            .collect()
    }

    /// Mark `key`'s tabs read here, and on the Mac when it answers.
    fn mark_read(&mut self, key: &Key) {
        for tab in self.tab_keys(key) {
            let shown = self.unread(&tab);
            self.unread_here.remove(&tab);
            if !shown
                && !self
                    .windows
                    .iter()
                    .flat_map(|w| &w.tabs)
                    .any(|t| t.key == tab && t.unread_at > 0)
            {
                continue;
            }
            self.read_here.insert(tab.clone(), false);
            if !self.read_queue.contains(&tab) {
                self.read_queue.push(tab);
            }
            self.version += 1;
        }
        if !self.read_queue.is_empty() && self.via == self.home() {
            self.reads.next_at = 0.0;
            self.reads.bump();
        }
    }

    /// Whether the tab `key` shows unread: the Mac says so and it was not
    /// read here since, or its turn finished here while the Mac was away.
    pub fn unread(&self, key: &str) -> bool {
        if self.unread_here.contains_key(key) {
            return true;
        }
        !self.read_here.contains_key(key)
            && self
                .windows
                .iter()
                .flat_map(|w| &w.tabs)
                .any(|t| t.key == key && t.unread_at > 0)
    }

    /// Note each tabbed session's turns. While the Mac is away (a peer
    /// answers) one that finishes is unread here; at home the Mac keeps the
    /// record. A session's first observation is its baseline.
    fn observe_turns(&mut self, fleet: &Fleet) {
        let away = self.via != self.home();
        let tabs: Vec<(String, Key)> = self
            .windows
            .iter()
            .flat_map(|w| &w.tabs)
            .filter(|t| !t.machine.is_empty() && !t.session.is_empty())
            .map(|t| (t.key.clone(), (t.machine.clone(), t.session.clone())))
            .collect();
        for (tab, key) in tabs {
            let Some(machine) = fleet.machines.get(&key.0).filter(|m| !m.stale) else {
                continue;
            };
            let Some(session) = machine.sessions.get(&key.1) else {
                continue;
            };
            let working = session.working() || session.blocked();
            let was = self.turn_running.insert(key.clone(), working);
            if away
                && was == Some(true)
                && session.state == "idle"
                && self.open.as_ref() != Some(&key)
            {
                self.unread_here.insert(tab, self.now);
                self.version += 1;
            }
        }
    }

    /// The read marks' URL, bearer and body: to the Mac only.
    pub fn reads_request(&mut self) -> Option<(String, String, String)> {
        let conn = self.conn.as_ref()?;
        if self.read_queue.is_empty() || self.via != conn.machine {
            return None;
        }
        self.reads_out = std::mem::take(&mut self.read_queue);
        self.reads.start(self.now);
        let body = serde_json::json!({ "keys": self.reads_out }).to_string();
        Some((
            conn.url(&self.route_of(&conn.machine), "/desktop/read"),
            conn.bearer(),
            body,
        ))
    }

    /// The marks landed, or they go back in the queue for the next try. An
    /// older server without the route (404) has no marks to keep.
    pub fn reads_done(&mut self, result: Result<(), u16>) {
        self.reads.inflight = false;
        let out = std::mem::take(&mut self.reads_out);
        match result {
            Ok(()) | Err(404) => {
                self.reads.failures = 0;
                for key in out {
                    if let Some(sent) = self.read_here.get_mut(&key) {
                        *sent = true;
                    }
                }
            }
            Err(_) => {
                self.reads.failures += 1;
                self.reads.next_at = self.now + 2_000.0 * f64::from(self.reads.failures.min(5));
                for key in out {
                    if !self.read_queue.contains(&key) {
                        self.read_queue.push(key);
                    }
                }
            }
        }
    }

    /// The refresh's URL and bearer.
    pub fn refresh_request(&mut self) -> Option<(String, String)> {
        let conn = self.conn.as_ref()?;
        self.refresh.start(self.now);
        Some((
            conn.url(&self.route_of(&self.via), "/refresh"),
            conn.bearer(),
        ))
    }

    /// The refresh came back: poll at once.
    pub fn refresh_done(&mut self) {
        self.refresh.inflight = false;
        self.refreshing = false;
        if self.poll.next_at > self.now {
            self.poll.next_at = self.now;
        }
        self.version += 1;
    }

    // ---- Reading ----

    /// A session that answers now: observed by the server answering, on a
    /// machine whose last observation worked, and not ended.
    pub fn live_session(&self, key: &Key) -> Option<&crate::fleet::Session> {
        if !self.trusted() {
            return None;
        }
        let fleet = self.fleet.as_ref()?;
        let machine = fleet.machines.get(&key.0)?;
        if machine.stale {
            return None;
        }
        machine.sessions.get(&key.1).filter(|s| !s.ended())
    }

    /// The last answer still stands: it succeeded, or failed polls since
    /// are within [`POLL_GRACE_MS`] of it.
    fn trusted(&self) -> bool {
        self.fresh || (self.fleet.is_some() && self.now - self.answered_at < POLL_GRACE_MS)
    }

    /// The session as last observed, live or not.
    pub fn known_session(&self, key: &Key) -> Option<&crate::fleet::Session> {
        self.fleet.as_ref()?.session(&key.0, &key.1)
    }

    /// The pending bubbles for `key`.
    pub fn pending_for<'a>(&'a self, key: &'a Key) -> impl Iterator<Item = &'a Pending> + 'a {
        self.pending.iter().filter(move |p| &p.key == key)
    }

    /// The transcript entries for `key`, oldest first.
    pub fn entries(&self, key: &Key) -> &[Entry] {
        self.conversations
            .get(key)
            .map(|c| c.transcript.entries.as_slice())
            .unwrap_or(&[])
    }
}

mod send;

#[cfg(test)]
mod tests;
