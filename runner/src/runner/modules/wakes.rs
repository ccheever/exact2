//! Wakes (LLP 1133.017 D5, D6): the platform runs the app for a reason a
//! module declared, while the person may not be using it. Only the host
//! raises a wake's events (`"raised": "wake"`), at the platform's call
//! ([`Runner::raise_wake`]); each event is an invocation of origin
//! [`CallOrigin::Wake`], delivered once, in order, one at a time, each as
//! its own commit. Events the host raises for one platform object are one
//! wake; a wake raised while another runs waits for it to end.
//!
//! - **Owned work.** An invocation owns the requests in flight, the armed
//!   `then`s, the held module calls and the queued sends stamped with its
//!   id (LLP 1133.023 D7, `admit.rs`). Sends waiting for the data source
//!   to be ready (a cold boot's) are counted too, since none can be told
//!   apart. A wake and a tool call (`calls.rs`) run one at a time.
//! - **Done.** An invocation completes when it owns nothing; the wake ends
//!   when every invocation has ended and the storage queue has drained
//!   (LLP 1097 D10), the whole process's.
//! - **Superseded.** A commit not its own that lets go of a request the
//!   running invocation owns ends it `superseded`, once that commit stands.
//! - **Expired** at the deadline, the first delivery plus the largest
//!   `budget` of the events raised, or when the host says the platform
//!   expired it ([`Runner::expire_wake`]): its requests answer as a network
//!   failure would, and its `then`s, held calls and queued sends are
//!   dropped. Invocations not yet delivered end with nothing delivered.
//!   An expired wake ends then, without waiting for storage to drain: the
//!   platform's time is up.
//! - **Timers** do not fire while a wake runs in a session with no view or
//!   a hidden one ([`WakeHow::hidden`]), and its page reads hidden
//!   (`exactPage().visibilityState`), as the platform's background is; a
//!   page the wake hid is shown again when it ends.
//! - **Refused** in a commit of the wake's origin: the host commands that
//!   open system UI or play sound, journaled `<name>: refused: during a wake`.
//!   A module name declared `"origin": "app"` and a permission request are
//!   refused by admission (`admit.rs`).
//! - **Told by an implementation.** A bound implementation the platform ran
//!   (a `BGTask` handed to it, a scheduler's block) raises a wake through its
//!   module's `changed`, as it emits an event ([`TOLD`]); each id it names is
//!   one wake, and at the wake's end the implementation is told
//!   `{"extension", "woke": id, "outcome"}`, the outcome its invocation's
//!   (`completed`, `expired`, `superseded`, `refused`), or `unheard` when no
//!   handler hears the event.

use super::super::{CommitReceipt, DataSource, Runner, RunnerError};
use super::{CallOrigin, Json, Route};
use crate::request::{FailureKind, Outcome, Reply};
use serde_json::json;
use std::collections::VecDeque;

/// The host commands a commit of a wake's origin may not run: each opens
/// system UI, asks a person, or plays sound with no audio session.
const REFUSED: &[&str] = &[
    "copyText",
    "openURL",
    "playSound",
    "playSounds",
    "requestFullscreen",
    "requestMIDIAccess",
    "saveFile",
    "share",
    "showDirectoryPicker",
    "showOpenFilePicker",
    "showPicker",
    "showSaveFilePicker",
];

/// The prefix of a `changed` topic that is a wake a bound implementation
/// raises, `{"extension", "event", "args", "id"}` after it (module doc).
pub(in crate::runner) const TOLD: &str = "exact-wake:";

/// How the host raises a wake's event.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WakeHow {
    /// The platform object the wake is for (a `BGTask`, a worker): the
    /// events of one id are one wake.
    pub id: u64,
    /// The host booted the session for the wake.
    pub cold: bool,
    /// No view of the session is visible, so timers stop while it runs.
    pub hidden: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Queued,
    Running,
    Completed,
    Expired,
    Superseded,
    Refused,
}

impl State {
    fn word(self) -> &'static str {
        match self {
            State::Queued => "queued",
            State::Running => "running",
            State::Completed => "completed",
            State::Expired => "expired",
            State::Superseded => "superseded",
            State::Refused => "refused",
        }
    }

    fn ended(self) -> bool {
        !matches!(self, State::Queued | State::Running)
    }
}

struct Invocation {
    /// Its invocation id (`admit.rs`).
    id: u64,
    /// Its event, by `module_events` row.
    row: usize,
    args: Vec<Json>,
    /// Its first argument, where that is a string: a registration's tag.
    tag: String,
    state: State,
    delivered: bool,
    /// The substitute's registration it runs for, by `ref`.
    pub(super) entry: Option<u64>,
}

struct Wake {
    how: WakeHow,
    /// The first delivery, on the session's clock.
    started: Option<f64>,
    budget: f64,
    invocations: Vec<Invocation>,
    expired: bool,
    /// The wake hid a page that was shown.
    hid: bool,
    /// The implementation that raised it and its id for it, told the end.
    told: Option<(String, String)>,
}

#[derive(Default)]
pub(in crate::runner) struct Wakes {
    current: Option<Wake>,
    waiting: VecDeque<Wake>,
    /// Wakes ended and not yet taken by the host.
    ended: Vec<Json>,
    /// The last wake that ended, for `state.wake`.
    last: Option<Json>,
}

impl<D: DataSource> Runner<D> {
    /// The host raises wake event `name` of `extension` with `args`, a JSON
    /// array, for the platform object `how.id`. `Err` for an event the plan
    /// declares but not as a wake's, or arguments that are not an array. An
    /// event no handler hears still makes a wake, which ends at once, so the
    /// host can tell the platform.
    pub fn raise_wake(
        &mut self,
        extension: &str,
        name: &str,
        args: &str,
        how: WakeHow,
    ) -> Result<(), String> {
        let key = format!("{extension}.{name}");
        let args: Vec<Json> = match serde_json::from_str(args) {
            Ok(Json::Array(args)) => args,
            _ => return Err(format!("{key}: the arguments are a JSON array")),
        };
        let row = (self.plan.module_events.iter()).position(|e| {
            self.plan.str(e.extension) == extension && self.plan.str(e.declared) == name
        });
        if let Some(row) = row {
            if self.plan.str(self.plan.module_events[row].raised) != "wake" {
                return Err(format!("{key}: not an event a wake raises"));
            }
        }
        self.wake_invocation(row, args, how, None);
        if row.is_none() {
            self.log(format!("{key}: no handler hears it"));
        }
        Ok(())
    }

    /// Queue an invocation of event `row` (none: unheard) in the wake for
    /// `how.id`, for the registration `entry` where the substitute keeps one.
    pub(super) fn wake_invocation(
        &mut self,
        row: Option<usize>,
        args: Vec<Json>,
        how: WakeHow,
        entry: Option<u64>,
    ) {
        let id = self.next_invocation();
        let w = &mut self.extensions.wakes;
        let wake = match (w.current.as_mut()).filter(|c| c.how.id == how.id) {
            Some(current) => current,
            None => match w.waiting.iter().position(|q| q.how.id == how.id) {
                Some(i) => &mut w.waiting[i],
                None => {
                    w.waiting.push_back(Wake {
                        how,
                        started: None,
                        budget: 0.0,
                        invocations: Vec::new(),
                        expired: false,
                        hid: false,
                        told: None,
                    });
                    w.waiting.back_mut().expect("pushed")
                }
            },
        };
        let Some(row) = row else {
            self.native_slot().changed("");
            return;
        };
        let event = &self.plan.module_events[row];
        let (budget, hold) = (event.budget as f64, event.hold.max(1) as usize);
        let name = self.plan.str(event.declared).to_owned();
        let tag = args.first().and_then(Json::as_str).unwrap_or_default();
        let line = format!("wake {name} {tag}: raised");
        wake.budget = wake.budget.max(budget);
        wake.invocations.push(Invocation {
            id,
            row,
            tag: tag.to_owned(),
            args,
            state: State::Queued,
            delivered: false,
            entry,
        });
        // Each event's own bound: past it, its own oldest not yet delivered
        // ends with nothing delivered.
        let mut dropped = Vec::new();
        let queued = |wake: &Wake| {
            (wake.invocations.iter())
                .filter(|i| i.row == row && i.state == State::Queued)
                .count()
        };
        while queued(wake) > hold {
            let oldest = (wake.invocations.iter_mut())
                .find(|i| i.row == row && i.state == State::Queued)
                .expect("counted above");
            oldest.state = State::Expired;
            dropped.push(oldest.tag.clone());
        }
        self.log(line);
        for tag in dropped {
            self.log(format!("wake {name} {tag}: dropped"));
        }
        self.native_slot().changed("");
    }

    /// Deliver what is due of the wakes: the next invocation once the one
    /// running owns nothing, and each wake's end once its invocations have
    /// ended and storage has drained.
    pub(in crate::runner) fn step_wakes(&mut self) -> (Vec<CommitReceipt>, Option<RunnerError>) {
        let mut receipts = Vec::new();
        loop {
            if self.extensions.wakes.current.is_none() && self.call_running() {
                break;
            }
            let w = &mut self.extensions.wakes;
            if w.current.is_none() {
                let Some(mut next) = w.waiting.pop_front() else {
                    break;
                };
                next.hid = next.how.hidden && !self.page.hidden;
                let hide = next.hid;
                self.extensions.wakes.current = Some(next);
                if hide {
                    match self.wake_page(true) {
                        Ok(receipt) => receipts.extend(receipt),
                        Err(e) => return (receipts, Some(e)),
                    }
                }
            }
            let w = &mut self.extensions.wakes;
            let wake = w.current.as_ref().expect("set above");
            let running = (wake.invocations.iter()).position(|i| i.state == State::Running);
            let next = (wake.invocations.iter()).position(|i| i.state == State::Queued);
            let expired = wake.expired;
            if let Some(i) = running {
                if self.wake_owns() {
                    break;
                }
                self.end_invocation(i, State::Completed);
                continue;
            }
            if let Some(i) = next {
                match self.deliver_invocation(i) {
                    Ok(Some(receipt)) => receipts.push(receipt),
                    Ok(None) => {}
                    Err(e) => return (receipts, Some(e)),
                }
                continue;
            }
            if self.background_operations() > 0 && !expired {
                break;
            }
            match self.end_wake() {
                Ok(receipt) => receipts.extend(receipt),
                Err(e) => return (receipts, Some(e)),
            }
        }
        let (called, error) = self.step_calls();
        receipts.extend(called);
        (receipts, error)
    }

    /// The page hidden for a wake, or shown again after it.
    fn wake_page(&mut self, hidden: bool) -> Result<Option<CommitReceipt>, RunnerError> {
        let mut page = self.page();
        page.hidden = hidden;
        self.set_page(page)
    }

    /// Run invocation `i` of the current wake: its event's action, under
    /// the wake's origin, with as many of its arguments as the action takes.
    fn deliver_invocation(&mut self, i: usize) -> Result<Option<CommitReceipt>, RunnerError> {
        let now = self.now_ms;
        let wake = self.extensions.wakes.current.as_mut().expect("a wake");
        wake.started.get_or_insert(now);
        let inv = &mut wake.invocations[i];
        inv.state = State::Running;
        inv.delivered = true;
        let (id, row, raised) = (inv.id, inv.row, inv.args.clone());
        let event = self.plan.module_events[row].clone();
        let action = self.plan.action(event.action).clone();
        let mut args = Vec::new();
        for (k, p) in action.params.iter().enumerate() {
            let ty = self.plan.param(p).ty;
            let json = raised.get(k).cloned().unwrap_or(Json::Null);
            match self.event_arg(ty, &json) {
                Ok(v) => args.push(v),
                Err(why) => {
                    let key = format!(
                        "{}.{}",
                        self.plan.str(event.extension),
                        self.plan.str(event.declared)
                    );
                    self.log(format!("{key}: refused: {why}"));
                    self.end_invocation(i, State::Refused);
                    return Ok(None);
                }
            }
        }
        self.wake_entry_delivered(i);
        let name = self.plan.str(action.name).to_owned();
        let (event_name, tag) = self.invocation_words(i);
        self.log(format!("wake {event_name} {tag}: heard by {name}"));
        let result = self.as_invocation(CallOrigin::Wake, id, |r| {
            r.run_action(event.action, args, &[])
        });
        match result {
            Ok(receipt) => Ok(Some(receipt)),
            Err(e) => {
                self.end_invocation(i, State::Refused);
                Err(e)
            }
        }
    }

    fn invocation_words(&self, i: usize) -> (String, String) {
        let wake = self.extensions.wakes.current.as_ref().expect("a wake");
        let inv = &wake.invocations[i];
        let event = &self.plan.module_events[inv.row];
        (self.plan.str(event.declared).to_owned(), inv.tag.clone())
    }

    /// Invocation `i` of the current wake ends `state`.
    fn end_invocation(&mut self, i: usize, state: State) {
        let (event, tag) = self.invocation_words(i);
        let wake = self.extensions.wakes.current.as_mut().expect("a wake");
        let inv = &mut wake.invocations[i];
        inv.state = state;
        let (entry, delivered) = (inv.entry, inv.delivered);
        self.log(format!("wake {event} {tag}: {}", state.word()));
        if let Some(entry) = entry {
            self.wake_entry_ended(entry, state.word(), delivered);
        }
    }

    /// Whether the running invocation still owns work (module doc).
    fn wake_owns(&self) -> bool {
        self.running_wake()
            .is_some_and(|id| self.invocation_owns(id))
            || !self.unsent.is_empty()
    }

    /// The id of the current wake's running invocation.
    fn running_wake(&self) -> Option<u64> {
        let wake = self.extensions.wakes.current.as_ref()?;
        (wake.invocations.iter())
            .find(|i| i.state == State::Running)
            .map(|i| i.id)
    }

    /// Whether a wake runs.
    pub(in crate::runner) fn wake_running(&self) -> bool {
        self.extensions.wakes.current.is_some()
    }

    /// The current wake ends: the host takes it, and `state.wake` shows it.
    fn end_wake(&mut self) -> Result<Option<CommitReceipt>, RunnerError> {
        let Some(wake) = self.extensions.wakes.current.take() else {
            return Ok(None);
        };
        let outcome = if wake.expired || wake.invocations.iter().any(|i| i.state == State::Expired)
        {
            "expired"
        } else {
            "completed"
        };
        let described = self.describe_wake(&wake, Some(outcome));
        self.log(format!("wake: {outcome}"));
        let w = &mut self.extensions.wakes;
        if wake.told.is_none() {
            w.ended.push(described.clone());
        }
        w.last = Some(described);
        if let Some((extension, id)) = &wake.told {
            let state = |s: State| wake.invocations.iter().any(|i| i.state == s);
            let told = if wake.invocations.is_empty() {
                "unheard"
            } else if wake.expired || state(State::Expired) {
                "expired"
            } else if state(State::Superseded) {
                "superseded"
            } else if state(State::Refused) {
                "refused"
            } else {
                "completed"
            };
            self.tell_woke(extension, id, told);
        }
        self.native_slot().changed("");
        if wake.hid {
            return self.wake_page(false);
        }
        Ok(None)
    }

    fn describe_wake(&self, wake: &Wake, outcome: Option<&str>) -> Json {
        let epoch = self.time.epoch_at_zero;
        let invocations: Vec<Json> = (wake.invocations.iter())
            .map(|i| {
                let event = &self.plan.module_events[i.row];
                json!({
                    "extension": self.plan.str(event.extension),
                    "event": self.plan.str(event.declared),
                    "tag": i.tag,
                    "state": i.state.word(),
                })
            })
            .collect();
        let kind = (wake.invocations.first())
            .map(|i| self.plan.str(self.plan.module_events[i.row].declared));
        json!({
            "id": wake.how.id,
            "kind": kind,
            "cold": wake.how.cold,
            "started": wake.started.map(|s| epoch + s),
            "deadline": wake.started.map(|s| epoch + s + wake.budget),
            "invocations": invocations,
            "outcome": outcome,
        })
    }

    /// The wake's deadline on the session's clock, while one runs unexpired.
    pub(in crate::runner) fn wake_deadline(&self) -> Option<f64> {
        let wake = self.extensions.wakes.current.as_ref()?;
        (!wake.expired)
            .then_some(wake.started? + wake.budget)
            .filter(|_| wake.invocations.iter().any(|i| !i.state.ended()))
    }

    /// When the host should next advance for the wakes: now while one can
    /// move, else at the deadline.
    pub(in crate::runner) fn wake_due(&self) -> Option<f64> {
        let w = &self.extensions.wakes;
        let Some(wake) = w.current.as_ref() else {
            return (!w.waiting.is_empty()).then_some(self.now_ms);
        };
        let running = wake.invocations.iter().any(|i| i.state == State::Running);
        let queued = wake.invocations.iter().any(|i| i.state == State::Queued);
        let moves = if running {
            !self.wake_owns()
        } else {
            queued || wake.expired || self.background_operations() == 0
        };
        if moves {
            Some(self.now_ms)
        } else {
            self.wake_deadline()
        }
    }

    /// Whether a wake runs in a session no one sees: its timers stop.
    pub(in crate::runner) fn wake_stops_timers(&self) -> bool {
        (self.extensions.wakes.current.as_ref()).is_some_and(|w| w.how.hidden)
            || self.call_stops_timers()
    }

    /// The platform expired the current wake, or its deadline passed
    /// (module doc). The commits its requests' failures made.
    pub fn expire_wake(&mut self) -> (Vec<CommitReceipt>, Option<RunnerError>) {
        let mut receipts = Vec::new();
        let mut error = None;
        match self.extensions.wakes.current.as_mut() {
            Some(wake) if !wake.expired => wake.expired = true,
            _ => return (receipts, None),
        }
        let id = self.running_wake().unwrap_or(0);
        for ticket in self.invocation_tickets(id) {
            let failed = Outcome::Failed {
                kind: FailureKind::Network,
                message: "the wake expired".into(),
            };
            match self.fulfill(ticket, failed) {
                Ok(Some(receipt)) => receipts.push(receipt),
                Ok(None) => {}
                Err(e) => error = error.or(Some(e)),
            }
        }
        let held: Vec<(u64, u64)> = (self.extensions.held.iter())
            .filter(|h| h.id == id && id != 0)
            .map(|h| (h.token, h.hold))
            .collect();
        for (token, hold) in held {
            self.retire_hold(hold);
            self.release(
                token,
                json!({ "error": "AbortError", "message": "the wake expired" }),
            );
        }
        self.drop_invocation_thens(id);
        self.drop_queued_of(id);
        let open: Vec<usize> = (self.extensions.wakes.current.as_ref())
            .map(|w| w.invocations.iter().enumerate())
            .into_iter()
            .flatten()
            .filter(|(_, i)| !i.state.ended())
            .map(|(k, _)| k)
            .collect();
        for i in open {
            self.end_invocation(i, State::Expired);
        }
        let (more, e) = self.step_wakes();
        receipts.extend(more);
        (receipts, error.or(e))
    }

    /// Invocation `id`'s work was let go of by a commit that stood: the
    /// running invocation it is ends `superseded` (D5).
    pub(in crate::runner) fn wake_superseded(&mut self, id: u64) {
        let running = (self.extensions.wakes.current.as_ref()).and_then(|w| {
            (w.invocations.iter()).position(|i| i.state == State::Running && i.id == id)
        });
        if let Some(i) = running {
            self.end_invocation(i, State::Superseded);
        }
    }

    /// Take off this commit's host commands from `first` that a wake may
    /// not run, when the commit is the wake's.
    pub(in crate::runner) fn refuse_in_wake(&mut self, first: usize) {
        if self.call_origin() != CallOrigin::Wake {
            return;
        }
        let mut i = first;
        while i < self.commands.len() {
            if REFUSED.contains(&self.commands[i].name.as_str()) {
                let name = self.commands.remove(i).name;
                self.log(format!("{name}: refused: during a wake"));
            } else {
                i += 1;
            }
        }
    }

    /// A wake a bound implementation raised (module doc): its id, with the
    /// extension's name, is the platform object; the page hidden or not as
    /// it is now. One raised for an extension the substitute answers, or for
    /// an event no wake raises, is refused, and the implementation told so.
    pub(in crate::runner) fn raise_told_wake(&mut self, body: &str) {
        let v: Json = serde_json::from_str(body).unwrap_or_default();
        let (Some(extension), Some(event), Some(id)) = (
            v["extension"].as_str(),
            v["event"].as_str(),
            v["id"].as_str(),
        ) else {
            self.log(format!("a wake raised is unreadable: {body}"));
            return;
        };
        let bound = (self.plan.modules.iter())
            .any(|m| self.plan.str(m.extension) == extension && self.route(m) != Route::Substitute);
        if !bound {
            self.log(format!(
                "{extension}.{event}: a wake raised, but the substitute answers"
            ));
            return;
        }
        // FNV-1a of the extension and its id, the top bit set: never an id a
        // host's own platform object takes (the agent's count from 1).
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in extension.bytes().chain([0]).chain(id.bytes()) {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3);
        }
        let how = WakeHow {
            id: hash | 1 << 63,
            cold: false,
            hidden: self.page.hidden,
        };
        if let Err(e) = self.raise_wake(extension, event, &v["args"].to_string(), how) {
            self.log(e);
            self.tell_woke(extension, id, "refused");
            return;
        }
        let w = &mut self.extensions.wakes;
        let told = Some((extension.to_owned(), id.to_owned()));
        if let Some(wake) = w
            .current
            .iter_mut()
            .chain(w.waiting.iter_mut())
            .find(|w| w.how.id == how.id)
        {
            wake.told = told;
        }
    }

    /// Tell the implementation of `extension` that its wake `id` ended.
    fn tell_woke(&self, extension: &str, id: &str, outcome: &str) {
        if let Some(handler) = self.native_slot().handler() {
            let body = json!({ "extension": extension, "woke": id, "outcome": outcome });
            handler(body.to_string().into_bytes(), Reply::new(|_| {}));
        }
    }

    /// The wakes that ended since the host last asked, each as JSON:
    /// `{"id", "kind", "cold", "started", "deadline", "invocations":
    /// [{"extension", "event", "tag", "state"}], "outcome"}`. The host
    /// tells each invocation's package its outcome and the platform the
    /// wake's.
    pub fn take_ended_wakes(&mut self) -> Vec<String> {
        (std::mem::take(&mut self.extensions.wakes.ended).into_iter())
            .map(|w| w.to_string())
            .collect()
    }

    /// `state.wake`: the wake running, else the last that ended; `None`
    /// before any.
    pub fn wake_state(&self) -> Option<Json> {
        let w = &self.extensions.wakes;
        match w.current.as_ref() {
            Some(wake) => Some(self.describe_wake(wake, None)),
            None => w.last.clone(),
        }
    }

    /// The registration `ref`s of the current wake's invocations that are
    /// running now.
    pub(super) fn running_entries(&self) -> Vec<u64> {
        (self
            .extensions
            .wakes
            .current
            .as_ref()
            .map(|w| w.invocations.iter()))
        .into_iter()
        .flatten()
        .filter(|i| i.state == State::Running)
        .filter_map(|i| i.entry)
        .collect()
    }

    /// Registration `from` was registered again as `to`: an invocation for
    /// it now runs for the new one.
    pub(super) fn wake_entry_moved(&mut self, from: u64, to: u64) {
        let w = &mut self.extensions.wakes;
        for wake in w.current.iter_mut().chain(w.waiting.iter_mut()) {
            for i in wake
                .invocations
                .iter_mut()
                .filter(|i| i.entry == Some(from))
            {
                i.entry = Some(to);
            }
        }
    }

    /// The registration invocation `i` of the current wake runs for.
    fn wake_entry_delivered(&mut self, i: usize) {
        let entry = (self.extensions.wakes.current.as_ref()).and_then(|w| w.invocations[i].entry);
        if let Some(entry) = entry {
            self.wake_entry_running(entry);
        }
    }
}
