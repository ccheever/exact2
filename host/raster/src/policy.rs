//! Bounded native image admission and delivery. No decoder or worker threads.
use crate::account::{BudgetAccount, Wake};
use crate::*;
use std::any::Any;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc, Mutex, OnceLock, Weak,
};
use std::time::{Duration, Instant};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);
fn identity() -> u64 {
    NEXT_ID
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
        .expect("native raster identity space exhausted")
}

struct Image {
    // Payload first: its native backing drops before our allocation reference.
    payload: Box<dyn Any + Send + Sync>,
    output: AllocationCharge,
    copy: Option<AllocationCharge>,
    key: RasterKey,
    metadata: Metadata,
    gate: Weak<GateInner>,
    session: u64,
}
impl Image {
    fn cached(&self, value: bool) {
        self.output.cached(value);
        if let Some(copy) = &self.copy {
            copy.cached(value);
        }
    }
    fn pin(&self) {
        self.output.pin();
        if let Some(copy) = &self.copy {
            copy.pin();
        }
    }
    fn unpin(&self) {
        self.output.unpin();
        if let Some(copy) = &self.copy {
            copy.unpin();
        }
    }
}

/// A native payload with charged backing. Clones share storage, not a new copy.
/// Providers retain `allocation_charge()`, never this payload-bearing lease.
pub struct RasterLease {
    image: Arc<Image>,
}
impl RasterLease {
    fn new(image: Arc<Image>) -> Self {
        image.pin();
        Self { image }
    }
    pub fn key(&self) -> RasterKey {
        self.image.key
    }
    pub fn metadata(&self) -> Metadata {
        self.image.metadata
    }
    pub fn payload<T: Any>(&self) -> Option<&T> {
        self.image.payload.downcast_ref()
    }
    pub fn allocation_charge(&self) -> AllocationCharge {
        self.image.output.clone()
    }
    pub fn copy_charge(&self) -> Option<AllocationCharge> {
        self.image.copy.clone()
    }
}
impl Clone for RasterLease {
    fn clone(&self) -> Self {
        Self::new(self.image.clone())
    }
}
impl Drop for RasterLease {
    fn drop(&mut self) {
        // A detached displayed backing remains in the dedup index while pinned.
        // Final unpin must also bound cold metadata without waiting for new work.
        if let Some(gate) = self.image.gate.upgrade() {
            let mut garbage = Vec::new();
            let mut state = gate.state.lock().unwrap();
            self.image.unpin();
            if self.image.output.pins() == 0 {
                if let Some(session) = state.sessions.get_mut(&self.image.session) {
                    enforce_cold_limit(session, &mut garbage);
                }
            }
            drop(state);
            drop(garbage);
        } else {
            self.image.unpin();
        }
    }
}
impl std::fmt::Debug for RasterLease {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RasterLease")
            .field("key", &self.key())
            .finish()
    }
}

struct Control {
    cancelled: AtomicBool,
}
enum Phase {
    Queued { budget: bool },
    Decoding(Arc<Control>),
    Ready { image: Arc<Image>, delivery: bool },
    Failed(Refusal),
}
struct Entry {
    id: u64,
    demand: Demand,
    requests: BTreeSet<RequestId>,
    phase: Phase,
    touched: u64,
}
impl Entry {
    /// What removing this entry's image returns to the budget.
    fn bytes(&self) -> u64 {
        match &self.phase {
            Phase::Ready { image, .. } => {
                image.output.bytes() + image.copy.as_ref().map_or(0, |c| c.bytes())
            }
            _ => 0,
        }
    }
}
struct Binding {
    demand: Demand,
}
struct SessionState {
    account: Arc<BudgetAccount>,
    entries: BTreeMap<RasterKey, Entry>,
    requests: BTreeMap<RequestId, Binding>,
    views: BTreeMap<ViewKey, RequestId>,
    active: BTreeMap<u64, Arc<Control>>,
    cells: usize,
    paused: bool,
    closed: bool,
    dedup_hits: u64,
    cancelled: u64,
    evicted: u64,
}
impl SessionState {
    fn pending(&self) -> usize {
        self.entries
            .values()
            .filter(|e| {
                !matches!(
                    e.phase,
                    Phase::Ready {
                        delivery: false,
                        ..
                    }
                )
            })
            .count()
            + self
                .active
                .keys()
                .filter(|id| !self.entries.values().any(|e| e.id == **id))
                .count()
    }
    fn cold_keys(&self) -> Vec<RasterKey> {
        let mut cold: Vec<_> = self
            .entries
            .iter()
            .filter(|(_, e)| {
                e.requests.is_empty()
                    && matches!(
                        &e.phase,
                        Phase::Ready {
                            image,
                            delivery: false,
                        } if image.output.pins() == 0
                    )
            })
            .map(|(key, entry)| (entry.touched, *key))
            .collect();
        cold.sort_unstable();
        cold.into_iter().map(|(_, key)| key).collect()
    }
    fn pinned_unsubscribed(&self) -> usize {
        self.entries
            .values()
            .filter(|e| {
                e.requests.is_empty()
                    && matches!(&e.phase, Phase::Ready { image, .. } if image.output.pins() > 0)
            })
            .count()
    }
}
#[derive(Default)]
struct State {
    sessions: BTreeMap<u64, SessionState>,
    running: usize,
    last_session: u64,
}
struct GateInner {
    state: Mutex<State>,
    wake: Arc<Wake>,
}

/// One process-wide admission gate: two running decodes, two delivery cells per
/// session. Full delivery mailboxes never occupy another session's running slot.
#[derive(Clone)]
pub struct Gate {
    inner: Arc<GateInner>,
}
impl std::fmt::Debug for Gate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Gate")
            .field("stats", &self.stats())
            .finish()
    }
}
impl Default for Gate {
    fn default() -> Self {
        Self::new()
    }
}
impl Gate {
    /// Native adapters in one process share this gate.
    pub fn process() -> Self {
        static PROCESS: OnceLock<Gate> = OnceLock::new();
        PROCESS.get_or_init(Self::new).clone()
    }
    /// Isolated gate for tests or an isolated native-image executor.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(GateInner {
                state: Mutex::new(State::default()),
                wake: Arc::new(Wake::default()),
            }),
        }
    }
    pub fn session(&self) -> RasterSession {
        self.session_with_budget(SESSION_BYTES)
    }
    /// A session that may hold `budget` decoded bytes (at least
    /// `SESSION_BYTES`): what views pin, plus a cache of what they left.
    /// One decode's peak stays within `SESSION_BYTES` either way.
    pub fn session_with_budget(&self, budget: u64) -> RasterSession {
        let id = identity();
        let account = BudgetAccount::new(&self.inner.wake, budget.max(SESSION_BYTES));
        self.inner.state.lock().unwrap().sessions.insert(
            id,
            SessionState {
                account: account.clone(),
                entries: BTreeMap::new(),
                requests: BTreeMap::new(),
                views: BTreeMap::new(),
                active: BTreeMap::new(),
                cells: 0,
                paused: false,
                closed: false,
                dedup_hits: 0,
                cancelled: 0,
                evicted: 0,
            },
        );
        RasterSession {
            owner: Arc::new(SessionOwner {
                gate: self.clone(),
                id,
                account,
            }),
        }
    }
    /// Round-robin sessions, visible before overscan within each. Reservations
    /// and a session delivery cell are acquired atomically before returning.
    pub fn next_decode(&self) -> Option<DecodePermit> {
        loop {
            let mut garbage = Vec::new();
            let mut state = self.inner.state.lock().unwrap();
            if state.running == RUNNING_DECODES {
                return None;
            }
            let mut sessions: Vec<_> = state.sessions.keys().copied().collect();
            sessions.sort_by_key(|id| (*id <= state.last_session, *id));
            let mut selected = None;
            'sessions: for id in sessions {
                let session = state.sessions.get_mut(&id).unwrap();
                if session.closed || session.paused || session.cells == DELIVERY_CELLS {
                    continue;
                }
                let mut candidates: Vec<_> = session
                    .entries
                    .iter()
                    .filter(|(_, e)| matches!(e.phase, Phase::Queued { .. }))
                    .map(|(key, e)| {
                        let priority = e
                            .requests
                            .iter()
                            .filter_map(|r| session.requests.get(r))
                            .map(|b| b.demand.priority)
                            .min()
                            .unwrap_or(Priority::Overscan);
                        (priority, e.id, *key)
                    })
                    .collect();
                candidates.sort_unstable();
                for (_, _, key) in candidates {
                    let cost = session.entries[&key].demand.cost;
                    let available = session.account.available();
                    if cost.peak().unwrap() > available {
                        // The least recently used cold entries, only until
                        // their bytes cover the shortfall: the rest stay
                        // cached for the rows that show them again.
                        let mut short = cost.peak().unwrap() - available;
                        for cold in session.cold_keys() {
                            if short == 0 {
                                break;
                            }
                            short = short.saturating_sub(session.entries[&cold].bytes());
                            remove_entry(session, cold, &mut garbage);
                            session.evicted += 1;
                        }
                        session.entries.get_mut(&key).unwrap().phase =
                            Phase::Queued { budget: true };
                        // Reclaim outside the lock, then retry priority order
                        // before admitting a smaller lower-priority request.
                        // Retained native owners stay charged; with no cold
                        // entries left, the next pass skips rather than spins.
                        if !garbage.is_empty() {
                            break 'sessions;
                        }
                        continue;
                    }
                    let output = session.account.reserve(cost.output_bytes).unwrap();
                    let copy = (cost.copy_bytes > 0)
                        .then(|| session.account.reserve(cost.copy_bytes).unwrap());
                    let scratch = (cost.scratch_bytes > 0)
                        .then(|| session.account.reserve(cost.scratch_bytes).unwrap());
                    let control = Arc::new(Control {
                        cancelled: AtomicBool::new(false),
                    });
                    let entry = session.entries.get_mut(&key).unwrap();
                    entry.phase = Phase::Decoding(control.clone());
                    session.active.insert(entry.id, control.clone());
                    session.cells += 1;
                    selected = Some(DecodePermit {
                        gate: self.clone(),
                        session: id,
                        job: entry.id,
                        demand: entry.demand,
                        control,
                        output: Some(output),
                        copy,
                        scratch,
                        active: true,
                    });
                    break;
                }
                if selected.is_some() {
                    state.last_session = id;
                    state.running += 1;
                    break;
                }
            }
            let reclaimed_candidates = !garbage.is_empty();
            drop(state);
            drop(garbage); // Native payload Drop may call this gate/session again.
            if selected.is_some() {
                return selected;
            }
            if !reclaimed_candidates {
                return None;
            }
        }
    }
    /// The adapter's existing fixed workers may wait here. Never hold the wake
    /// mutex while entering gate/account code; this also avoids lost wakeups.
    pub fn wait_decode(&self, timeout: Duration) -> Option<DecodePermit> {
        let start = Instant::now();
        loop {
            let observed = *self.inner.wake.sequence.lock().unwrap();
            if let Some(permit) = self.next_decode() {
                return Some(permit);
            }
            let remaining = timeout.saturating_sub(start.elapsed());
            if remaining.is_zero() {
                return None;
            }
            let sequence = self.inner.wake.sequence.lock().unwrap();
            if *sequence != observed {
                continue;
            }
            let (sequence, timed) = self
                .inner
                .wake
                .changed
                .wait_timeout(sequence, remaining)
                .unwrap();
            drop(sequence);
            if timed.timed_out() {
                return self.next_decode();
            }
        }
    }
    pub fn stats(&self) -> GateStats {
        let state = self.inner.state.lock().unwrap();
        GateStats {
            running: state.running,
            ready: state
                .sessions
                .values()
                .map(|s| {
                    s.entries
                        .values()
                        .filter(|e| matches!(e.phase, Phase::Ready { delivery: true, .. }))
                        .count()
                })
                .sum(),
            sessions: state.sessions.values().filter(|s| !s.closed).count(),
        }
    }
}

struct SessionOwner {
    gate: Gate,
    id: u64,
    account: Arc<BudgetAccount>,
}
impl Drop for SessionOwner {
    fn drop(&mut self) {
        clear(&self.gate, self.id, Clear::Shutdown);
    }
}
/// Cloneable session owner; last-owner drop shuts down. Allocation charges do
/// not retain this owner. Keep it above replaceable app runtimes.
#[derive(Clone)]
pub struct RasterSession {
    owner: Arc<SessionOwner>,
}
impl std::fmt::Debug for RasterSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RasterSession")
            .field("id", &self.id())
            .finish()
    }
}
impl RasterSession {
    pub fn id(&self) -> u64 {
        self.owner.id
    }
    /// Same-view replacement validates all candidate metadata/capacity first.
    /// Temporary budget pressure retains the queued request and wakes on free.
    pub fn request(&self, demand: Demand) -> Result<RequestId, Refusal> {
        validate(demand)?;
        let mut garbage = Vec::new();
        let mut state = self.owner.gate.inner.state.lock().unwrap();
        let session = state
            .sessions
            .get_mut(&self.id())
            .ok_or(Refusal::Shutdown)?;
        if session.closed {
            return Err(Refusal::Shutdown);
        }
        if session.paused {
            return Err(Refusal::Paused);
        }
        let previous = session.views.get(&demand.view).copied();
        if let Some(entry) = session.entries.get(&demand.key) {
            if entry.demand.metadata != demand.metadata || entry.demand.cost != demand.cost {
                return Err(Refusal::ConflictingMetadata);
            }
        }
        if let Some(id) = previous {
            if session.requests[&id].demand.key == demand.key {
                session.requests.get_mut(&id).unwrap().demand = demand;
                drop(state);
                self.owner.gate.inner.wake.notify();
                return Ok(id);
            }
        }
        // A replacement can leave its previous displayed backing pinned. Count
        // that dedup metadata as well, so repeated replacements cannot grow it.
        let detached_previous = previous.is_some_and(|id| {
            let entry = &session.entries[&session.requests[&id].demand.key];
            entry.requests.len() == 1
                && matches!(&entry.phase, Phase::Ready { image, .. } if image.output.pins() > 1)
        });
        let reattached = session.entries.get(&demand.key).is_some_and(|e| {
            e.requests.is_empty()
                && matches!(&e.phase, Phase::Ready { image, .. } if image.output.pins() > 0)
        });
        let subscriptions = session.requests.len()
            + session.pinned_unsubscribed()
            + 1
            + usize::from(detached_previous)
            - usize::from(previous.is_some())
            - usize::from(reattached);
        if subscriptions > SUBSCRIPTIONS {
            return Err(Refusal::SubscriberLimit);
        }
        if !session.entries.contains_key(&demand.key) {
            let removable = previous.is_some_and(|id| {
                let entry = &session.entries[&session.requests[&id].demand.key];
                entry.requests.len() == 1
                    && matches!(
                        entry.phase,
                        Phase::Queued { .. }
                            | Phase::Failed(_)
                            | Phase::Ready { delivery: true, .. }
                    )
            });
            if session.pending() - usize::from(removable) >= PENDING_JOBS {
                return Err(Refusal::QueueFull);
            }
        }
        let id = RequestId(identity());
        if let Some(previous) = previous {
            cancel_request(session, previous, &mut garbage);
        }
        if let Some(entry) = session.entries.get_mut(&demand.key) {
            if entry.requests.is_empty() {
                if let Phase::Ready { image, .. } = &entry.phase {
                    image.pin();
                }
            }
            entry.requests.insert(id);
            entry.touched = identity();
            session.dedup_hits += 1;
        } else {
            session.entries.insert(
                demand.key,
                Entry {
                    id: identity(),
                    demand,
                    requests: BTreeSet::from([id]),
                    phase: Phase::Queued { budget: false },
                    touched: identity(),
                },
            );
        }
        session.requests.insert(id, Binding { demand });
        session.views.insert(demand.view, id);
        enforce_cold_limit(session, &mut garbage);
        drop(state);
        drop(garbage);
        self.owner.gate.inner.wake.notify();
        Ok(id)
    }
    pub fn status(&self, request: RequestId) -> Option<RequestStatus> {
        let state = self.owner.gate.inner.state.lock().unwrap();
        let session = state.sessions.get(&self.id())?;
        let entry = session
            .entries
            .get(&session.requests.get(&request)?.demand.key)?;
        Some(match entry.phase {
            Phase::Queued { budget: false } => RequestStatus::Queued,
            Phase::Queued { budget: true } => RequestStatus::WaitingBudget,
            Phase::Decoding(_) => RequestStatus::Decoding,
            Phase::Ready { .. } => RequestStatus::Ready,
            Phase::Failed(reason) => RequestStatus::Failed(reason),
        })
    }
    /// First consumption frees the session delivery cell. Further deduplicated
    /// observers get the same backing, without freeing any additional cell.
    pub fn take_ready(&self, request: RequestId) -> Option<RasterLease> {
        let mut state = self.owner.gate.inner.state.lock().unwrap();
        let session = state.sessions.get_mut(&self.id())?;
        let key = session.requests.get(&request)?.demand.key;
        let entry = session.entries.get_mut(&key)?;
        let Phase::Ready { image, delivery } = &mut entry.phase else {
            return None;
        };
        let released = *delivery;
        if released {
            *delivery = false;
            session.cells -= 1;
        }
        // Pin before unlocking: cancellation must not classify this backing as
        // cold or admit another metadata owner between cloning and pinning.
        let lease = RasterLease::new(image.clone());
        entry.touched = identity();
        drop(state);
        if released {
            self.owner.gate.inner.wake.notify();
        }
        Some(lease)
    }
    pub fn cancel(&self, request: RequestId) -> bool {
        let mut garbage = Vec::new();
        let mut state = self.owner.gate.inner.state.lock().unwrap();
        let Some(session) = state.sessions.get_mut(&self.id()) else {
            return false;
        };
        let result = cancel_request(session, request, &mut garbage);
        enforce_cold_limit(session, &mut garbage);
        drop(state);
        drop(garbage);
        if result {
            self.owner.gate.inner.wake.notify();
        }
        result
    }
    pub fn retire_view(&self, view: ViewKey) -> bool {
        let request = self
            .owner
            .gate
            .inner
            .state
            .lock()
            .unwrap()
            .sessions
            .get(&self.id())
            .and_then(|s| s.views.get(&view).copied());
        request.is_some_and(|id| self.cancel(id))
    }
    pub fn retire_generation(&self, generation: u64) {
        let mut garbage = Vec::new();
        let mut state = self.owner.gate.inner.state.lock().unwrap();
        if let Some(session) = state.sessions.get_mut(&self.id()) {
            let ids: Vec<_> = session
                .requests
                .iter()
                .filter(|(_, b)| b.demand.key.generation == generation)
                .map(|(id, _)| *id)
                .collect();
            for id in ids {
                cancel_request(session, id, &mut garbage);
            }
            let keys: Vec<_> = session
                .entries
                .keys()
                .filter(|k| k.generation == generation)
                .copied()
                .collect();
            for key in keys {
                remove_entry(session, key, &mut garbage);
            }
        }
        drop(state);
        drop(garbage);
        self.owner.gate.inner.wake.notify();
    }
    /// Cancel demands/cache, preserving the debit for old workers/providers.
    pub fn reset(&self) {
        clear(&self.owner.gate, self.id(), Clear::Reset);
    }
    /// Cancel demands/results; native resume reconciles current live image demand.
    pub fn pause(&self) {
        clear(&self.owner.gate, self.id(), Clear::Pause);
    }
    pub fn resume(&self) {
        if let Some(s) = self
            .owner
            .gate
            .inner
            .state
            .lock()
            .unwrap()
            .sessions
            .get_mut(&self.id())
        {
            if !s.closed {
                s.paused = false;
            }
        }
        self.owner.gate.inner.wake.notify();
    }
    pub fn shutdown(&self) {
        clear(&self.owner.gate, self.id(), Clear::Shutdown);
    }
    /// Follow the owning viewport's capacity without retiring displayed images
    /// or in-flight decoder charges. A shrink drops cold images; surviving
    /// charges can exceed the new budget until their owners release them.
    pub fn set_budget(&self, budget: u64) {
        // Admission checks capacity and reserves all decode allocations while
        // holding this gate. A shrink must not split that transaction.
        let shrunk = {
            let _state = self.owner.gate.inner.state.lock().unwrap();
            self.owner.account.set_budget(budget.max(SESSION_BYTES))
        };
        if shrunk {
            self.trim();
        }
    }
    /// Work arrived outside the gate (a source to read): wake a worker.
    pub fn wake(&self) {
        self.owner.gate.inner.wake.notify();
    }
    /// Memory pressure: drop unpinned cold owners, retaining displayed dedup.
    /// Surviving backing owners always retain their independent byte charge.
    pub fn trim(&self) {
        let mut garbage = Vec::new();
        let mut state = self.owner.gate.inner.state.lock().unwrap();
        if let Some(s) = state.sessions.get_mut(&self.id()) {
            for key in s.cold_keys() {
                remove_entry(s, key, &mut garbage);
                s.evicted += 1;
            }
        }
        drop(state);
        drop(garbage);
        self.owner.gate.inner.wake.notify();
    }
    /// Reserve a distinct allocation before an upload/conversion CPU copy.
    pub fn reserve_allocation(&self, bytes: u64) -> Result<AllocationReservation, Refusal> {
        let state = self.owner.gate.inner.state.lock().unwrap();
        let s = state.sessions.get(&self.id()).ok_or(Refusal::Shutdown)?;
        if s.closed {
            return Err(Refusal::Shutdown);
        }
        if s.paused {
            return Err(Refusal::Paused);
        }
        self.owner.account.reserve(bytes)
    }
    pub fn stats(&self) -> Stats {
        let state = self.owner.gate.inner.state.lock().unwrap();
        let mut stats = *self.owner.account.usage.lock().unwrap();
        if let Some(s) = state.sessions.get(&self.id()) {
            stats.queued = s
                .entries
                .values()
                .filter(|e| matches!(e.phase, Phase::Queued { .. }))
                .count();
            stats.running = s.active.len();
            stats.ready = s
                .entries
                .values()
                .filter(|e| matches!(e.phase, Phase::Ready { delivery: true, .. }))
                .count();
            stats.delivery_cells = s.cells;
            stats.pending_jobs = s.pending();
            stats.subscribers = s.requests.len();
            stats.cold_entries = s.cold_keys().len();
            stats.dedup_hits = s.dedup_hits;
            stats.cancelled = s.cancelled;
            stats.evicted = s.evicted;
        }
        stats
    }
}

/// Move-only reservation and running slot. Drop is decode failure/cancellation.
/// Its output charge can outlive it; scratch must actually drop before it does.
pub struct DecodePermit {
    gate: Gate,
    session: u64,
    job: u64,
    demand: Demand,
    control: Arc<Control>,
    output: Option<AllocationReservation>,
    copy: Option<AllocationReservation>,
    scratch: Option<AllocationReservation>,
    active: bool,
}
impl std::fmt::Debug for DecodePermit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DecodePermit")
            .field("session", &self.session)
            .field("key", &self.demand.key)
            .finish()
    }
}
impl DecodePermit {
    pub fn session_id(&self) -> u64 {
        self.session
    }
    pub fn key(&self) -> RasterKey {
        self.demand.key
    }
    pub fn metadata(&self) -> Metadata {
        self.demand.metadata
    }
    pub fn cost(&self) -> DecodeCost {
        self.demand.cost
    }
    pub fn is_cancelled(&self) -> bool {
        self.control.cancelled.load(Ordering::Acquire)
    }
    pub fn allocation_charge(&self) -> AllocationCharge {
        self.output.as_ref().unwrap().charge()
    }
    pub fn copy_charge(&self) -> Option<AllocationCharge> {
        self.copy.as_ref().map(AllocationReservation::charge)
    }
    /// Native scratch is already destroyed. Completion never blocks on delivery:
    /// its session cell was reserved at admission. False means stale/cancelled.
    pub fn complete<T: Any + Send + Sync>(
        mut self,
        payload: T,
        bytes: ResidentBytes,
    ) -> Result<bool, Refusal> {
        if bytes.output == 0
            || bytes.output > self.demand.cost.output_bytes
            || bytes.copy > self.demand.cost.copy_bytes
        {
            return Err(Refusal::ActualExceedsReservation);
        }
        drop(self.scratch.take());
        let output = self.output.take().unwrap().commit(bytes.output)?;
        let copy = self.copy.take().map(|r| r.commit(bytes.copy)).transpose()?;
        let image = Arc::new(Image {
            payload: Box::new(payload),
            output,
            copy,
            key: self.demand.key,
            metadata: self.demand.metadata,
            gate: Arc::downgrade(&self.gate.inner),
            session: self.session,
        });
        let mut state = self.gate.inner.state.lock().unwrap();
        let session = state
            .sessions
            .get_mut(&self.session)
            .expect("active permit retains session bookkeeping");
        session.active.remove(&self.job);
        let entry = session.entries.get_mut(&self.demand.key);
        let publish = !session.closed
            && !session.paused
            && !self.is_cancelled()
            && entry
                .as_ref()
                .is_some_and(|e| e.id == self.job && !e.requests.is_empty());
        if publish {
            image.cached(true);
            image.pin(); // Live subscriber interest, separate from external leases.
            entry.unwrap().phase = Phase::Ready {
                image: image.clone(),
                delivery: true,
            };
        } else {
            session.cells -= 1;
        }
        let remove_session = session.closed && session.active.is_empty();
        state.running -= 1;
        self.active = false;
        if remove_session {
            state.sessions.remove(&self.session);
        }
        drop(state);
        drop(image);
        self.gate.inner.wake.notify();
        Ok(publish)
    }
    pub fn fail(self) {
        drop(self);
    }
}
impl Drop for DecodePermit {
    fn drop(&mut self) {
        // Charge clones retained by actual backings remain charged independently.
        drop(self.scratch.take());
        drop(self.output.take());
        drop(self.copy.take());
        if !self.active {
            return;
        }
        let mut state = self.gate.inner.state.lock().unwrap();
        let s = state
            .sessions
            .get_mut(&self.session)
            .expect("active permit retains session bookkeeping");
        s.active.remove(&self.job);
        s.cells -= 1;
        if let Some(e) = s.entries.get_mut(&self.demand.key) {
            if e.id == self.job {
                e.phase = Phase::Failed(Refusal::DecodeFailed);
            }
        }
        let remove = s.closed && s.active.is_empty();
        state.running -= 1;
        if remove {
            state.sessions.remove(&self.session);
        }
        drop(state);
        self.gate.inner.wake.notify();
    }
}

enum Clear {
    Reset,
    Pause,
    Shutdown,
}
fn clear(gate: &Gate, id: u64, mode: Clear) {
    let mut garbage = Vec::new();
    let mut state = gate.inner.state.lock().unwrap();
    if let Some(s) = state.sessions.get_mut(&id) {
        if matches!(mode, Clear::Shutdown) {
            s.closed = true;
        }
        if matches!(mode, Clear::Pause) {
            s.paused = true;
        }
        let keys: Vec<_> = s.entries.keys().copied().collect();
        for key in keys {
            remove_entry(s, key, &mut garbage);
        }
        s.cancelled += s.requests.len() as u64;
        s.requests.clear();
        s.views.clear();
        if s.closed && s.active.is_empty() {
            state.sessions.remove(&id);
        }
    }
    drop(state);
    drop(garbage);
    gate.inner.wake.notify();
}
fn remove_entry(s: &mut SessionState, key: RasterKey, garbage: &mut Vec<Arc<Image>>) {
    if let Some(e) = s.entries.remove(&key) {
        match e.phase {
            Phase::Decoding(control) => {
                control.cancelled.store(true, Ordering::Release);
            }
            Phase::Ready { image, delivery } => {
                if delivery {
                    s.cells -= 1;
                }
                if !e.requests.is_empty() {
                    image.unpin();
                }
                image.cached(false);
                garbage.push(image);
            }
            _ => {}
        }
    }
}
fn cancel_request(s: &mut SessionState, request: RequestId, garbage: &mut Vec<Arc<Image>>) -> bool {
    let Some(binding) = s.requests.remove(&request) else {
        return false;
    };
    s.views.remove(&binding.demand.view);
    s.cancelled += 1;
    let key = binding.demand.key;
    let e = s.entries.get_mut(&key).unwrap();
    e.requests.remove(&request);
    if e.requests.is_empty() {
        if let Phase::Ready {
            image,
            delivery: false,
        } = &e.phase
        {
            image.unpin();
            e.touched = identity();
        } else {
            // remove_entry normally unpins live ready interest; this last
            // subscriber was removed immediately above, so account it here.
            if let Phase::Ready { image, .. } = &e.phase {
                image.unpin();
            }
            remove_entry(s, key, garbage);
        }
    }
    true
}
fn enforce_cold_limit(s: &mut SessionState, garbage: &mut Vec<Arc<Image>>) {
    let cold = s.cold_keys();
    let excess = cold.len().saturating_sub(COLD_ENTRIES);
    for key in cold.into_iter().take(excess) {
        remove_entry(s, key, garbage);
        s.evicted += 1;
    }
}
fn validate(d: Demand) -> Result<(), Refusal> {
    let natural = d.metadata.natural;
    if natural.width == 0
        || natural.height == 0
        || d.key.pixels.width == 0
        || d.key.pixels.height == 0
    {
        return Err(Refusal::InvalidDimensions);
    }
    if d.metadata.encoded_bytes > MAX_ENCODED_BYTES {
        return Err(Refusal::EncodedLimit);
    }
    if d.metadata.header_bytes > MAX_HEADER_BYTES
        || d.metadata.header_bytes > d.metadata.encoded_bytes
    {
        return Err(Refusal::HeaderLimit);
    }
    if u64::from(natural.width) * u64::from(natural.height) > MAX_SOURCE_PIXELS {
        return Err(Refusal::SourcePixels);
    }
    // Both native adapters normalize to RGBA8. Variant identifies their fixed
    // orientation/color policy, not an arbitrary output storage format.
    let minimum = u64::from(d.key.pixels.width)
        .checked_mul(4)
        .ok_or(Refusal::Overflow)?;
    let output = d
        .cost
        .stride
        .checked_mul(u64::from(d.cost.height))
        .ok_or(Refusal::Overflow)?;
    if d.cost.height != d.key.pixels.height
        || d.cost.stride < minimum
        || d.cost.output_bytes != output
    {
        return Err(Refusal::InvalidDimensions);
    }
    if d.cost.peak()? > SESSION_BYTES {
        return Err(Refusal::TooLarge);
    }
    Ok(())
}
