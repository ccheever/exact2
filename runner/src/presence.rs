//! Presence secrets (LLP 1133.010 D4–D6): a `secret.keep` name whose grant
//! line carries a policy is kept where only the person's check releases it.
//!
//! The runner's [`crate::Store`] holds such a name's occupancy, never its
//! bytes. The host's backend keeps the value and makes every move between
//! its plain store and its locked one. [`Backend`] is that backend's logic,
//! the same on every host: one queue per name, so a move is never
//! interleaved with a write or a read; each move in D4's order, its intent
//! recorded before anything moves; and a boot reconciliation that resolves
//! a half-done move toward locked. A host supplies the [`Vault`]: its plain
//! store, its record of each name's intended state, and its locked items
//! (the data-protection keychain with access control on Apple, a Keystore
//! key on Android, the agent's own store under the agent).
//!
//! Each step is a durable call that returns when it has landed, so the order
//! is the order on disk. Under the agent, driver faults stop a move after a
//! step, as a crash would (`exact-presence:<op>.<step>`), or hold it there
//! (`exact-presence:hold.<op>.<step>`) until the fault is passed.

pub use crate::runner::{PresenceAnswer, PresenceAsk};
pub use exact_grants::{Policy, Presence};
use std::collections::{BTreeMap, VecDeque};

/// What the runner knows of a presence name (D5): never its bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Occupancy {
    /// Nothing is kept.
    Absent,
    /// Kept in the plain store: an `optional` name the person left unlocked.
    Plain,
    /// Awaiting reconciliation or a plain copy's move to locked (D4): its
    /// bytes are kept out of the runner as a locked value's are.
    Pending,
    /// Kept under the name's policy.
    Locked,
}

impl Occupancy {
    /// The word the store's snapshot and the agent's `state` use.
    pub fn word(self) -> &'static str {
        match self {
            Occupancy::Absent => "absent",
            Occupancy::Plain => "plain",
            Occupancy::Pending => "pending",
            Occupancy::Locked => "locked",
        }
    }

    /// The occupancy a snapshot entry names.
    pub fn from_word(word: &[u8]) -> Option<Occupancy> {
        [
            Occupancy::Absent,
            Occupancy::Plain,
            Occupancy::Pending,
            Occupancy::Locked,
        ]
        .into_iter()
        .find(|o| o.word().as_bytes() == word)
    }

    /// Locked or awaiting the move to it: `store.locked` is true, and the
    /// value never enters the runner.
    pub fn locked(self) -> bool {
        matches!(self, Occupancy::Locked | Occupancy::Pending)
    }
}

/// The host's record of a name's intended state (D4 step 1), kept in its
/// plain store under `exact.presence.<name>`: `locked <the line's words>`,
/// or `plain`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Intent {
    /// Locked under the policy these words name.
    Locked(String),
    /// Plain, as the person chose.
    Plain,
}

impl Intent {
    fn encode(&self) -> Vec<u8> {
        match self {
            Intent::Locked(words) => format!("locked {words}").into_bytes(),
            Intent::Plain => b"plain".to_vec(),
        }
    }

    fn decode(bytes: &[u8]) -> Option<Intent> {
        let text = std::str::from_utf8(bytes).ok()?;
        match text.split_once(' ') {
            Some(("locked", words)) => Some(Intent::Locked(words.to_owned())),
            None if text == "plain" => Some(Intent::Plain),
            _ => None,
        }
    }
}

/// A host's durable stores, as the backend moves values between them. Each
/// call returns once what it did has landed (D4: "waits for that write to
/// be durable"). None shows the person anything.
pub trait Vault: Send {
    /// The plain copy of `name`.
    fn plain(&self, name: &str) -> Result<Option<Vec<u8>>, String>;
    /// Keep (`Some`) or delete (`None`) the plain copy of `name`.
    fn put_plain(&mut self, name: &str, value: Option<&[u8]>) -> Result<(), String>;
    /// The record of `name`'s intended state.
    fn record(&self, name: &str) -> Result<Option<Vec<u8>>, String>;
    /// Keep or delete the record of `name`.
    fn put_record(&mut self, name: &str, record: Option<&[u8]>) -> Result<(), String>;
    /// The generations of `name`'s locked items, read without asking anyone
    /// (on Apple indexed candidates, probed without permitting authentication).
    /// An incomplete probe returns an error, never an unconfirmed generation.
    fn generations(&self, name: &str) -> Result<Vec<u64>, String>;
    /// Add a locked item for `name` at `generation`, under `presence`. It
    /// asks no one: adding an item with access control, or encrypting to a
    /// Keystore key's public half, needs no person (D5).
    fn add_locked(
        &mut self,
        name: &str,
        generation: u64,
        presence: &Presence,
        value: &[u8],
    ) -> Result<(), String>;
    /// Delete one locked item, unread.
    fn delete_locked(&mut self, name: &str, generation: u64) -> Result<(), String>;
    /// Intentionally forget every locked item, including unconfirmed
    /// candidates. Unlike reconciliation, this does not select a replacement.
    fn delete_all_locked(&mut self, name: &str) -> Result<(), String> {
        for generation in self.generations(name)? {
            self.delete_locked(name, generation)?;
        }
        Ok(())
    }
    /// Read a locked item after the person's check passed; `None` when the
    /// platform has dropped it (the passcode removed, enrolment changed).
    fn read_locked(&mut self, name: &str, generation: u64) -> Result<Option<Vec<u8>>, String>;
}

/// One operation on a presence name, run in order on the name's queue.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Op {
    /// A committed `store.set` (`Some`) or `store.forget` (`None`).
    Write(Option<Vec<u8>>),
    /// `store.protect`: move to locked.
    Protect,
    /// `store.unprotect`: the person's check, then move to plain.
    Unprotect,
    /// `store.unlock`: the person's check, then the value.
    Unlock,
    /// A pending name's move to locked (D4), queued at boot.
    Move,
}

/// How the person's check ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Checked {
    /// Passed: the backend reads the item.
    Passed,
    /// Passed, and the host read the item with the evaluated context itself
    /// (Apple's sheet and read are one call): the value, or `None` when the
    /// platform has dropped it.
    Read(Option<Vec<u8>>),
    /// Failed with D1's code (`USER_CANCEL`, `LOCKOUT`, …) and a message.
    Failed(String, String),
    /// The platform has dropped the item (the agent's `invalidated`).
    Invalidated,
}

/// How a call ended, for its caller.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Done {
    /// `unlock`'s answer: the value, or `null` when nothing is kept.
    Value(Option<Vec<u8>>),
    /// `protect` or `unprotect` landed.
    Ok,
    /// Refused or failed: a code and a message.
    Failed(String, String),
}

/// What a pump asks of the runner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// Call `id` waits on the person's check of `name`.
    Check {
        /// The call.
        id: u64,
        /// The name.
        name: String,
        /// Its policy.
        presence: Presence,
        /// `unlock` or `unprotect`.
        op: &'static str,
    },
    /// Call `id` ended.
    Done {
        /// The call.
        id: u64,
        /// How.
        done: Done,
    },
}

/// What a step may meet under the agent: a fault that stops the move
/// there, as a crash would, or one that holds it until it is passed.
pub struct Faults<'a> {
    /// Whether a fault stops the move after `point`, counting it.
    pub fail: &'a dyn Fn(&str) -> bool,
    /// Whether a fault holds the move after `point`.
    pub hold: &'a dyn Fn(&str) -> bool,
}

impl Faults<'_> {
    /// No faults: outside the agent.
    pub const NONE: Faults<'static> = Faults {
        fail: &|_| false,
        hold: &|_| false,
    };
}

/// What a pump needs to know of the device and the session.
pub struct Env<'a> {
    /// A locked item can be kept now: the host keeps locked secrets, and a
    /// device credential is set (D5).
    pub can_lock: bool,
    /// The module has loaded, so a pending move may run (D4, D8).
    pub moves: bool,
    /// The session clock, ms: a refused write is tried again a second later.
    pub now_ms: f64,
    /// The driver's faults.
    pub faults: Faults<'a>,
}

const RETRY_MS: f64 = 1000.0;

struct Job {
    /// The caller's id; `None` for a write, which no one waits on.
    id: Option<u64>,
    op: Op,
    /// The steps done.
    step: u8,
    /// The check's outcome, once it came; `Some(None)` while asked.
    checked: Option<Option<Checked>>,
    /// A failed durable deletion waits before trying the same queued job.
    retry_at: Option<f64>,
}

struct Entry {
    presence: Option<Presence>,
    occupancy: Occupancy,
    /// The newest locked item's generation; 0 when none.
    generation: u64,
    queue: VecDeque<Job>,
    /// A refused write, tried again until it lands or a newer one replaces it.
    retry: Option<(Vec<u8>, f64)>,
    /// A driver fault stopped a move: the name takes nothing until a relaunch
    /// reconciles it, as after a crash.
    stopped: bool,
    /// A failed boot probe leaves the name pending until reconciliation
    /// succeeds. Retry on the pump's clock, or immediately for a new request.
    reconcile_at: Option<f64>,
}

impl Entry {
    fn write_retry_due(&self) -> Option<f64> {
        // A check owns its generation until its read finishes; a retry
        // cannot replace that item while the platform is authenticating it.
        if self
            .queue
            .front()
            .is_some_and(|j| j.step > 0 || j.checked.is_some())
        {
            return None;
        }
        self.retry.as_ref().map(|(_, at)| *at)
    }
}

/// The backend's logic over a host's [`Vault`].
pub struct Backend {
    vault: Box<dyn Vault>,
    names: BTreeMap<String, Entry>,
    log: Vec<String>,
}

impl Backend {
    /// The backend for the app's `secret.keep` names (`lines`: each name
    /// with its policy, `None` for a plain line), reconciled as a boot finds
    /// them (D4): toward locked, never reading a locked item.
    pub fn new(vault: Box<dyn Vault>, lines: Vec<(String, Option<Presence>)>) -> Backend {
        let mut backend = Backend {
            vault,
            names: BTreeMap::new(),
            log: Vec::new(),
        };
        for (name, presence) in lines {
            backend.names.insert(
                name.clone(),
                Entry {
                    presence,
                    occupancy: Occupancy::Pending,
                    generation: 0,
                    queue: VecDeque::new(),
                    retry: None,
                    stopped: false,
                    reconcile_at: Some(0.0),
                },
            );
            if let Err(e) = backend.reconcile(&name) {
                backend.say(format!("presence {name}: reconciliation waits: {e}"));
            }
        }
        backend
    }

    fn say(&mut self, line: String) {
        self.log.push(line);
    }

    fn reconcile(&mut self, name: &str) -> Result<(), String> {
        let presence = self.names[name].presence.clone();
        let say = |e: String| format!("presence {name}: {e}");
        let record = self.vault.record(name)?.and_then(|r| Intent::decode(&r));
        let mut generations = self.vault.generations(name)?;
        let plain = self.vault.plain(name)?.is_some();
        generations.sort_unstable();
        let words = presence.as_ref().map(Presence::line_words);
        // A line that lost its policy, or changed it: the old items are
        // forgotten unread, and the record with them (D4).
        let changed = match (&words, &record) {
            (None, _) => !generations.is_empty() || record.is_some(),
            (Some(words), Some(Intent::Locked(old))) => words != old,
            _ => false,
        };
        if changed {
            for g in std::mem::take(&mut generations) {
                if let Err(e) = self.vault.delete_locked(name, g) {
                    self.log.push(say(format!("an old locked item stays: {e}")));
                }
            }
            let _ = self.vault.put_record(name, None);
            self.log.push(say(
                "its policy changed: the locked value is forgotten".into()
            ));
        }
        let Some(presence) = presence else {
            self.names.remove(name);
            return Ok(());
        };
        // The newest generation wins; a crash between a write's add and its
        // delete leaves the older one (D6).
        let newest = generations.last().copied();
        for &g in generations.iter().filter(|&&g| Some(g) != newest) {
            let _ = self.vault.delete_locked(name, g);
        }
        let record = if changed { None } else { record };
        let mut entry = Entry {
            presence: Some(presence.clone()),
            occupancy: Occupancy::Absent,
            generation: newest.unwrap_or(0),
            queue: VecDeque::new(),
            retry: None,
            stopped: false,
            reconcile_at: None,
        };
        if newest.is_some() {
            // A plain copy never outlives a locked one, whatever the record
            // says; an interrupted `unprotect` leaves the lock on.
            if plain {
                if let Err(e) = self.vault.put_plain(name, None) {
                    self.log.push(say(format!(
                        "the plain copy beside the locked one stays: {e}"
                    )));
                }
            }
            if !matches!(record, Some(Intent::Locked(_))) {
                let _ = self
                    .vault
                    .put_record(name, Some(&Intent::Locked(presence.line_words()).encode()));
            }
            entry.occupancy = Occupancy::Locked;
        } else if plain {
            if matches!(record, Some(Intent::Locked(_))) || !presence.optional {
                entry.occupancy = Occupancy::Pending;
                entry.queue.push_back(Job {
                    id: None,
                    op: Op::Move,
                    step: 0,
                    checked: None,
                    retry_at: None,
                });
            } else {
                entry.occupancy = Occupancy::Plain;
            }
        } else if record.is_some() {
            let _ = self.vault.put_record(name, None);
        }
        // Boot's pending move precedes operations queued while probes failed.
        entry.queue.append(&mut self.entry(name).queue);
        self.names.insert(name.to_owned(), entry);
        Ok(())
    }

    /// The occupancy of `name`, or `None` when it carries no policy.
    pub fn occupancy(&self, name: &str) -> Option<Occupancy> {
        let entry = self.names.get(name)?;
        entry.presence.as_ref()?;
        Some(entry.occupancy)
    }

    /// The newest locked item's generation of `name`; 0 when none.
    pub fn generation(&self, name: &str) -> u64 {
        self.names.get(name).map_or(0, |e| e.generation)
    }

    /// Every presence name and its occupancy.
    pub fn occupancies(&self) -> Vec<(String, Occupancy)> {
        (self.names.iter())
            .filter(|(_, e)| e.presence.is_some())
            .map(|(n, e)| (n.clone(), e.occupancy))
            .collect()
    }

    /// The plain copy of a plain-state name, for the store to hold: the
    /// person chose no lock, so its bytes are an ordinary secret's.
    pub fn plain_value(&self, name: &str) -> Option<Vec<u8>> {
        let entry = self.names.get(name)?;
        (entry.occupancy == Occupancy::Plain)
            .then(|| self.vault.plain(name).ok().flatten())
            .flatten()
    }

    /// What the store's boot snapshot carries for these names: each one's
    /// occupancy, and whether this host keeps locked secrets.
    pub fn snapshot_entries(&self, capable: bool) -> Vec<(String, Vec<u8>)> {
        let mut out: Vec<(String, Vec<u8>)> = self
            .occupancies()
            .into_iter()
            .map(|(n, o)| {
                (
                    format!("{}{n}", crate::Store::PRESENCE),
                    o.word().as_bytes().to_vec(),
                )
            })
            .collect();
        if capable {
            out.push((crate::Store::PRESENCE.to_owned(), b"1".to_vec()));
        }
        out
    }

    /// Queue `op` on `name`'s queue; `id` names a call that waits for it.
    /// A write replaces a refused write still waiting to be tried again.
    pub fn enqueue(&mut self, name: &str, op: Op, id: Option<u64>) -> bool {
        let Some(entry) = self.names.get_mut(name).filter(|e| e.presence.is_some()) else {
            return false;
        };
        if matches!(op, Op::Write(_)) {
            entry.retry = None;
        }
        if entry.reconcile_at.is_some() {
            entry.reconcile_at = Some(0.0);
        }
        entry.queue.push_back(Job {
            id,
            op,
            step: 0,
            checked: None,
            retry_at: None,
        });
        true
    }

    /// The person's check for call `id` ended.
    pub fn checked(&mut self, id: u64, outcome: Checked) {
        for entry in self.names.values_mut() {
            if let Some(job) = entry.queue.front_mut().filter(|j| j.id == Some(id)) {
                if job.checked == Some(None) {
                    job.checked = Some(Some(outcome));
                }
                return;
            }
        }
    }

    /// Call `id` was let go: it leaves its queue, and a check's late answer
    /// is dropped. A move already begun finishes; only its answer is lost.
    pub fn cancel(&mut self, id: u64) {
        for entry in self.names.values_mut() {
            for (i, job) in entry.queue.iter_mut().enumerate() {
                if job.id != Some(id) {
                    continue;
                }
                if i == 0 && job.step > 0 {
                    job.id = None;
                } else {
                    entry.queue.remove(i);
                }
                return;
            }
        }
    }

    /// Whether a call waits on the person's check.
    pub fn checking(&self) -> bool {
        (self.names.values()).any(|e| e.queue.front().is_some_and(|j| j.checked == Some(None)))
    }

    /// The backend's journal lines since the last take: names, never values.
    pub fn take_log(&mut self) -> Vec<String> {
        std::mem::take(&mut self.log)
    }

    /// The next retry the host must wake for. A check or a partly finished
    /// move suspends a refused write's retry until it releases the name.
    pub(crate) fn retry_due_ms(&self) -> Option<f64> {
        self.names
            .values()
            .filter(|e| !e.stopped)
            .filter_map(|e| {
                e.reconcile_at.or_else(|| {
                    e.queue
                        .front()
                        .and_then(|j| j.retry_at)
                        .into_iter()
                        .chain(e.write_retry_due())
                        .reduce(f64::min)
                })
            })
            .reduce(f64::min)
    }

    /// Run what can run now, in each name's order.
    pub fn pump(&mut self, env: &Env) -> Vec<Event> {
        let mut events = Vec::new();
        let names: Vec<String> = self.names.keys().cloned().collect();
        for name in names {
            if let Some(at) = self.names[&name].reconcile_at {
                if env.now_ms < at {
                    continue;
                }
                if self.reconcile(&name).is_err() {
                    self.entry(&name).reconcile_at = Some(env.now_ms + RETRY_MS);
                    continue;
                }
            }
            self.retry(&name, env);
            while self.step(&name, env, &mut events) {}
        }
        events
    }

    /// A refused write, tried again once its second has passed and nothing
    /// is half-done ahead of it.
    fn retry(&mut self, name: &str, env: &Env) {
        let Some(entry) = self.names.get_mut(name) else {
            return;
        };
        let Some(at) = entry.write_retry_due() else {
            return;
        };
        if env.now_ms < at {
            return;
        }
        let (value, _) = entry.retry.take().expect("a due retry");
        entry.queue.push_front(Job {
            id: None,
            op: Op::Write(Some(value)),
            step: 0,
            checked: None,
            retry_at: None,
        });
        self.say(format!("presence {name}: the refused write is tried again"));
    }

    /// One step of `name`'s head job: whether another may follow now.
    fn step(&mut self, name: &str, env: &Env, events: &mut Vec<Event>) -> bool {
        let Some(entry) = self.names.get_mut(name) else {
            return false;
        };
        let Some(job) = entry.queue.front() else {
            return false;
        };
        let presence = entry.presence.clone().expect("a presence name's queue");
        if entry.stopped {
            let job = entry.queue.pop_front().expect("the head");
            if let Some(id) = job.id {
                events.push(Event::Done {
                    id,
                    done: Done::Failed(
                        "InvalidStateError".into(),
                        format!("{name}: a driver fault stopped a move; relaunch to reconcile it"),
                    ),
                });
            }
            return true;
        }
        if job.retry_at.is_some_and(|at| env.now_ms < at) {
            return false;
        }
        let op = job.op.clone();
        self.head(name).retry_at = None;
        let outcome = match op {
            Op::Write(value) => self.write(name, &presence, value, env),
            Op::Protect => self.protect(name, &presence, env),
            Op::Unprotect => self.unprotect(name, &presence, env, events),
            Op::Unlock => self.unlock(name, &presence, env, events),
            Op::Move => self.moved(name, &presence, env),
        };
        let Some(done) = outcome else {
            return false;
        };
        let entry = self.names.get_mut(name).expect("the entry");
        let job = entry.queue.pop_front().expect("the head");
        if let (Some(id), Some(done)) = (job.id, done) {
            events.push(Event::Done { id, done });
        }
        true
    }

    fn head(&mut self, name: &str) -> &mut Job {
        (self.names.get_mut(name))
            .and_then(|e| e.queue.front_mut())
            .expect("a head job")
    }

    fn entry(&mut self, name: &str) -> &mut Entry {
        self.names.get_mut(name).expect("a presence name")
    }

    /// After step `step` of the head job: `None` to go on; `Some(stop)`
    /// when a fault holds it (`stop` false) or stopped it (`true`).
    fn fault(&mut self, name: &str, point: &str, env: &Env) -> Option<bool> {
        if (env.faults.hold)(&format!("exact-presence:hold.{point}")) {
            return Some(false);
        }
        if (env.faults.fail)(&format!("exact-presence:{point}")) {
            self.say(format!(
                "presence {name}: {point}: stopped by a driver fault"
            ));
            self.entry(name).stopped = true;
            return Some(true);
        }
        None
    }

    /// A stop's answer: the head job ends, failed.
    fn stopped(name: &str, point: &str) -> Option<Option<Done>> {
        Some(Some(Done::Failed(
            "InvalidStateError".into(),
            format!("{name}: {point}: stopped by a driver fault"),
        )))
    }

    fn write(
        &mut self,
        name: &str,
        presence: &Presence,
        value: Option<Vec<u8>>,
        env: &Env,
    ) -> Option<Option<Done>> {
        let Some(value) = value else {
            if let Err(e) = self
                .vault
                .delete_all_locked(name)
                .and_then(|()| self.vault.put_plain(name, None))
                .and_then(|()| self.vault.put_record(name, None))
            {
                self.say(format!("presence {name}: forgetting waits: {e}"));
                self.head(name).retry_at = Some(env.now_ms + RETRY_MS);
                return None;
            }
            let entry = self.entry(name);
            entry.generation = 0;
            entry.occupancy = Occupancy::Absent;
            entry.retry = None;
            return Some(None);
        };
        let occupancy = self.entry(name).occupancy;
        let plain =
            occupancy == Occupancy::Plain || (occupancy == Occupancy::Absent && presence.optional);
        if plain {
            match self.vault.put_plain(name, Some(&value)) {
                Ok(()) => self.entry(name).occupancy = Occupancy::Plain,
                Err(e) => self.refused(name, value, &e, env),
            }
            return Some(None);
        }
        if !env.can_lock {
            self.say(format!(
                "presence {name}: not kept: this device keeps no locked secret now"
            ));
            return Some(None);
        }
        // The new value is added under the next generation before the old
        // one goes, so a failure or a crash leaves a readable value (D6).
        let generation = self.entry(name).generation + 1;
        let added = if (env.faults.fail)("exact-presence:set") {
            Err("refused by a driver fault".to_owned())
        } else {
            self.vault.add_locked(name, generation, presence, &value)
        };
        match added {
            Ok(()) => {
                let old = self.entry(name).generation;
                if old > 0 {
                    if let Err(e) = self.vault.delete_locked(name, old) {
                        self.say(format!("presence {name}: an older locked item stays: {e}"));
                    }
                }
                if occupancy == Occupancy::Absent {
                    let _ = self
                        .vault
                        .put_record(name, Some(&Intent::Locked(presence.line_words()).encode()));
                }
                let entry = self.entry(name);
                entry.generation = generation;
                entry.occupancy = Occupancy::Locked;
            }
            Err(e) => self.refused(name, value, &e, env),
        }
        Some(None)
    }

    fn refused(&mut self, name: &str, value: Vec<u8>, why: &str, env: &Env) {
        self.say(format!(
            "presence {name}: the write was refused ({why}); it is tried again"
        ));
        self.entry(name).retry = Some((value, env.now_ms + RETRY_MS));
    }

    fn protect(&mut self, name: &str, presence: &Presence, env: &Env) -> Option<Option<Done>> {
        let occupancy = self.entry(name).occupancy;
        if !presence.optional || occupancy.locked() {
            return Some(Some(Done::Ok));
        }
        let step = self.head(name).step;
        if step == 0 {
            if occupancy == Occupancy::Absent {
                return Some(Some(Done::Failed(
                    "InvalidStateError".into(),
                    format!("nothing is kept under {name}"),
                )));
            }
            if !env.can_lock {
                return Some(Some(Done::Failed(
                    "NotSupportedError".into(),
                    "this device keeps no locked secret: no device credential is set, or this build cannot keep one".into(),
                )));
            }
            if let Err(e) = self
                .vault
                .put_record(name, Some(&Intent::Locked(presence.line_words()).encode()))
            {
                return Some(Some(Done::Failed("UnknownError".into(), e)));
            }
            self.head(name).step = 1;
        }
        if self.head(name).step == 1 {
            match self.fault(name, "protect.record", env) {
                Some(true) => return Self::stopped(name, "protect.record"),
                Some(false) => return None,
                None => {}
            }
            let Some(value) = self.vault.plain(name).ok().flatten() else {
                let _ = self.vault.put_record(name, None);
                self.entry(name).occupancy = Occupancy::Absent;
                return Some(Some(Done::Failed(
                    "InvalidStateError".into(),
                    format!("nothing is kept under {name}"),
                )));
            };
            let generation = self.entry(name).generation + 1;
            if let Err(e) = self.vault.add_locked(name, generation, presence, &value) {
                // Nothing moved: the record goes back to plain.
                let _ = self.vault.put_record(name, Some(&Intent::Plain.encode()));
                return Some(Some(Done::Failed("NotSupportedError".into(), e)));
            }
            self.entry(name).generation = generation;
            self.head(name).step = 2;
        }
        match self.fault(name, "protect.write", env) {
            Some(true) => return Self::stopped(name, "protect.write"),
            Some(false) => return None,
            None => {}
        }
        if let Err(e) = self.vault.put_plain(name, None) {
            self.say(format!(
                "presence {name}: the plain copy stays until the next launch: {e}"
            ));
        }
        self.entry(name).occupancy = Occupancy::Locked;
        self.say(format!("presence {name}: protected"));
        Some(Some(Done::Ok))
    }

    /// The head job's check: `Some(outcome)` once it came; `None` while it
    /// is asked, after asking it once.
    fn check(
        &mut self,
        name: &str,
        presence: &Presence,
        op: &'static str,
        events: &mut Vec<Event>,
    ) -> Option<Checked> {
        let job = self.head(name);
        match job.checked.clone() {
            Some(Some(outcome)) => Some(outcome),
            Some(None) => None,
            None => {
                job.checked = Some(None);
                match job.id {
                    Some(id) => events.push(Event::Check {
                        id,
                        name: name.to_owned(),
                        presence: presence.clone(),
                        op,
                    }),
                    // A call let go before its check: nothing is asked.
                    None => {
                        job.checked = Some(Some(Checked::Failed(
                            "SYSTEM_CANCEL".into(),
                            "the call was let go".into(),
                        )))
                    }
                }
                None
            }
        }
    }

    /// The platform dropped the item: it is forgotten, and the name reads
    /// absent (D6).
    fn invalidated(&mut self, name: &str) -> Result<(), String> {
        self.vault.delete_all_locked(name)?;
        self.vault.put_record(name, None)?;
        let entry = self.entry(name);
        entry.generation = 0;
        entry.occupancy = Occupancy::Absent;
        self.say(format!(
            "presence {name}: the platform dropped the locked value"
        ));
        Ok(())
    }

    fn read(&mut self, name: &str, checked: Checked) -> Result<Option<Vec<u8>>, Done> {
        match checked {
            Checked::Failed(code, message) => Err(Done::Failed(code, message)),
            Checked::Invalidated => Ok(None),
            Checked::Read(value) => Ok(value),
            Checked::Passed => {
                let generation = self.entry(name).generation;
                self.vault
                    .read_locked(name, generation)
                    .map_err(|e| Done::Failed("UnknownError".into(), e))
            }
        }
    }

    fn unlock(
        &mut self,
        name: &str,
        presence: &Presence,
        _env: &Env,
        events: &mut Vec<Event>,
    ) -> Option<Option<Done>> {
        match self.entry(name).occupancy {
            Occupancy::Absent => return Some(Some(Done::Value(None))),
            Occupancy::Plain => {
                let value = self.vault.plain(name).ok().flatten();
                return Some(Some(Done::Value(value)));
            }
            Occupancy::Pending | Occupancy::Locked => {}
        }
        let checked = self.check(name, presence, "unlock", events)?;
        Some(Some(match self.read(name, checked) {
            Ok(Some(value)) => Done::Value(Some(value)),
            Ok(None) => match self.invalidated(name) {
                Ok(()) => Done::Value(None),
                Err(e) => Done::Failed("UnknownError".into(), e),
            },
            Err(done) => done,
        }))
    }

    fn unprotect(
        &mut self,
        name: &str,
        presence: &Presence,
        env: &Env,
        events: &mut Vec<Event>,
    ) -> Option<Option<Done>> {
        if !presence.optional {
            return Some(Some(Done::Failed(
                "InvalidStateError".into(),
                format!("{name} is locked by its grant line, not by the person"),
            )));
        }
        if !self.entry(name).occupancy.locked() {
            return Some(Some(Done::Ok));
        }
        if self.head(name).step == 0 {
            let checked = self.check(name, presence, "unprotect", events)?;
            let value = match self.read(name, checked) {
                Ok(Some(value)) => value,
                Ok(None) => {
                    return Some(Some(match self.invalidated(name) {
                        Ok(()) => Done::Ok,
                        Err(e) => Done::Failed("UnknownError".into(), e),
                    }));
                }
                Err(done) => return Some(Some(done)),
            };
            if let Err(e) = self.vault.put_record(name, Some(&Intent::Plain.encode())) {
                return Some(Some(Done::Failed("UnknownError".into(), e)));
            }
            self.head(name).step = 1;
            // The value read waits in the job, in the backend and never the
            // store, only until the next step lands.
            self.head(name).checked = Some(Some(Checked::Read(Some(value))));
        }
        if self.head(name).step == 1 {
            match self.fault(name, "unprotect.record", env) {
                Some(true) => return Self::stopped(name, "unprotect.record"),
                Some(false) => return None,
                None => {}
            }
            let Some(Some(Checked::Read(Some(value)))) = self.head(name).checked.take() else {
                return Some(Some(Done::Failed("UnknownError".into(), "lost".into())));
            };
            if let Err(e) = self.vault.put_plain(name, Some(&value)) {
                // The lock stays on; the record says so again.
                let _ = self
                    .vault
                    .put_record(name, Some(&Intent::Locked(presence.line_words()).encode()));
                return Some(Some(Done::Failed("UnknownError".into(), e)));
            }
            self.head(name).step = 2;
        }
        match self.fault(name, "unprotect.write", env) {
            Some(true) => return Self::stopped(name, "unprotect.write"),
            Some(false) => return None,
            None => {}
        }
        let generation = self.entry(name).generation;
        if let Err(e) = self.vault.delete_locked(name, generation) {
            self.say(format!(
                "presence {name}: the locked item stays until the next launch: {e}"
            ));
        }
        let entry = self.entry(name);
        entry.generation = 0;
        entry.occupancy = Occupancy::Plain;
        self.say(format!("presence {name}: unprotected"));
        Some(Some(Done::Ok))
    }

    fn moved(&mut self, name: &str, presence: &Presence, env: &Env) -> Option<Option<Done>> {
        if !env.moves {
            return None;
        }
        if (env.faults.hold)("exact-presence:hold.move") {
            return None;
        }
        // A copy the line or the person said must be locked is never kept
        // ungated for want of a way to lock it (D4, §12 question 7).
        if !env.can_lock {
            let _ = self.vault.put_plain(name, None);
            let _ = self.vault.put_record(name, None);
            self.entry(name).occupancy = Occupancy::Absent;
            self.say(format!(
                "presence {name}: it cannot be locked on this device, so its plain copy is deleted"
            ));
            return Some(None);
        }
        let _ = self
            .vault
            .put_record(name, Some(&Intent::Locked(presence.line_words()).encode()));
        let Some(value) = self.vault.plain(name).ok().flatten() else {
            let _ = self.vault.put_record(name, None);
            self.entry(name).occupancy = Occupancy::Absent;
            return Some(None);
        };
        let generation = self.entry(name).generation + 1;
        if let Err(e) = self.vault.add_locked(name, generation, presence, &value) {
            // Try again at the next launch: the copy stays out of the runner.
            self.say(format!("presence {name}: the move to locked failed: {e}"));
            return Some(None);
        }
        let _ = self.vault.put_plain(name, None);
        let entry = self.entry(name);
        entry.generation = generation;
        entry.occupancy = Occupancy::Locked;
        self.say(format!("presence {name}: moved to locked"));
        Some(None)
    }
}

/// A vault in memory: a test's, or a host's with nowhere durable to keep
/// presence names. Clones share one store, as two launches share a disk.
#[derive(Clone, Default)]
pub struct MemoryVault(std::sync::Arc<std::sync::Mutex<Memory>>);

/// What a [`MemoryVault`] holds, and the order its calls landed in.
#[derive(Clone, Debug, Default)]
pub struct Memory {
    /// Plain copies, by name.
    pub plain: BTreeMap<String, Vec<u8>>,
    /// Records, by name.
    pub records: BTreeMap<String, Vec<u8>>,
    /// Locked items, by name and generation.
    pub locked: BTreeMap<(String, u64), Vec<u8>>,
    /// Each durable call, as `put_plain a`, `add_locked a#2`, …
    pub calls: Vec<String>,
}

impl MemoryVault {
    /// What it holds now.
    pub fn memory(&self) -> Memory {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    fn with<T>(&self, f: impl FnOnce(&mut Memory) -> T) -> T {
        f(&mut self.0.lock().unwrap_or_else(|e| e.into_inner()))
    }
}

impl Vault for MemoryVault {
    fn plain(&self, name: &str) -> Result<Option<Vec<u8>>, String> {
        Ok(self.with(|m| m.plain.get(name).cloned()))
    }
    fn put_plain(&mut self, name: &str, value: Option<&[u8]>) -> Result<(), String> {
        self.with(|m| {
            m.calls
                .push(format!("put_plain {name} {}", value.is_some()));
            match value {
                Some(v) => m.plain.insert(name.to_owned(), v.to_vec()),
                None => m.plain.remove(name),
            };
        });
        Ok(())
    }
    fn record(&self, name: &str) -> Result<Option<Vec<u8>>, String> {
        Ok(self.with(|m| m.records.get(name).cloned()))
    }
    fn put_record(&mut self, name: &str, record: Option<&[u8]>) -> Result<(), String> {
        self.with(|m| {
            let word = record.map_or("none".into(), |r| String::from_utf8_lossy(r).into_owned());
            m.calls.push(format!("put_record {name} {word}"));
            match record {
                Some(r) => m.records.insert(name.to_owned(), r.to_vec()),
                None => m.records.remove(name),
            };
        });
        Ok(())
    }
    fn generations(&self, name: &str) -> Result<Vec<u64>, String> {
        Ok(self.with(|m| {
            (m.locked.keys())
                .filter(|(n, _)| n == name)
                .map(|(_, g)| *g)
                .collect()
        }))
    }
    fn add_locked(
        &mut self,
        name: &str,
        generation: u64,
        _presence: &Presence,
        value: &[u8],
    ) -> Result<(), String> {
        self.with(|m| {
            m.calls.push(format!("add_locked {name}#{generation}"));
            m.locked
                .insert((name.to_owned(), generation), value.to_vec());
        });
        Ok(())
    }
    fn delete_locked(&mut self, name: &str, generation: u64) -> Result<(), String> {
        self.with(|m| {
            m.calls.push(format!("delete_locked {name}#{generation}"));
            m.locked.remove(&(name.to_owned(), generation));
        });
        Ok(())
    }
    fn read_locked(&mut self, name: &str, generation: u64) -> Result<Option<Vec<u8>>, String> {
        Ok(self.with(|m| m.locked.get(&(name.to_owned(), generation)).cloned()))
    }
}

#[cfg(test)]
#[path = "presence_tests.rs"]
pub(crate) mod tests;
