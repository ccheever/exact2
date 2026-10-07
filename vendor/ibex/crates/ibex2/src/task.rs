//! Off-thread work, and the completion queue the engine drains.
//!
//! This is the Rust half of LLP 0058 OQ1 — how a Rust future completing off
//! the JavaScript thread resolves into the engine's job queue without
//! reordering relative to JavaScript-enqueued jobs.
//!
//! The shape, which is the part that matters:
//!
//! 1. A delegating op returns a promise immediately and does no work on the
//!    JavaScript thread.
//! 2. The work happens on some other thread. It never touches the engine —
//!    `jsi` values are not `Send`, and nothing here is allowed to hold one.
//! 3. Completions land in this queue, which is the only thing crossing threads.
//! 4. The embedder pumps on the JavaScript thread: take completions, resolve
//!    their promises, then drain microtasks.
//!
//! Step 4 is where the ordering guarantee lives, and it is stated in
//! `Pump::CONTRACT` rather than left implicit.
//!
//! @ref LLP 0058#3-the-part-that-cannot-move — the job queue this interleaves with
//! @ref LLP 0059.000#11-which-ops-are-synchronous — delegating ops are async, without exception

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Condvar, Mutex, Weak};

use crate::{
    boundary::{HostError, HostValue},
    host,
};

/// A completed unit of off-thread work, waiting to be delivered to the engine.
#[derive(Debug)]
pub struct Completion {
    pub task_id: u64,
    pub result: Result<HostValue, HostError>,
}

/// One admitted host task.
///
/// LLP 0058.000.000 §8: Rust owns **one** sequence-numbered FIFO of admitted
/// host tasks, *including timer deliveries and async settlements*. Two queues
/// drained by different rules cannot state a single order between a timer and a
/// settlement, which is what an application observes.
#[derive(Debug)]
pub enum HostTask {
    /// An off-thread operation settled.
    Settlement(Completion),
    /// A timer came due and was admitted.
    Timer { handle: u64 },
    /// A subscribed host source published one event.
    Event {
        subscription: u64,
        payload: HostValue,
    },
}

/// A task together with its position in this runtime's admission order.
#[derive(Debug)]
pub struct AdmittedTask {
    pub sequence: u64,
    pub task: HostTask,
}

/// Completions waiting for **one** runtime.
///
/// Per-runtime, not process-global. A shared queue lets two runtimes in the
/// same process take each other's completions, and their task ids collide
/// because each numbers its own tasks from 1. That is not a theoretical
/// concern — it showed up the moment two runtimes existed at once.
pub struct CompletionQueue {
    ready: Mutex<ReadyState>,
    /// Lets an embedder block until there is something to pump instead of
    /// spinning. A runtime that polls in a loop burns a core to do nothing,
    /// which is the default failure mode of this design.
    signal: Condvar,
    wake: Mutex<WakeState>,
    wake_drained: Condvar,
}

#[derive(Debug, Default)]
struct ReadyState {
    next_sequence: u64,
    tasks: VecDeque<AdmittedTask>,
    closed: bool,
}

#[derive(Default)]
struct WakeState {
    callback: Option<Arc<dyn Fn() + Send + Sync>>,
    wake_pending: bool,
    invoking: bool,
    closed: bool,
}

// @ref LLP 0068#3-no-engine-in-the-process — re-entrant shutdown recognizes the queue's sole wake invoker
thread_local! {
    /// Queue identities whose wake callbacks this thread is invoking. A wake
    /// may synchronously admit work on another queue, so this remains a stack.
    static CLAIMED_WAKES: RefCell<Vec<usize>> = const { RefCell::new(Vec::new()) };
}

struct ClaimedWake {
    queue: usize,
}

impl ClaimedWake {
    fn new(queue: &CompletionQueue) -> Self {
        let queue = std::ptr::from_ref(queue) as usize;
        CLAIMED_WAKES.with(|claimed| claimed.borrow_mut().push(queue));
        Self { queue }
    }
}

impl Drop for ClaimedWake {
    fn drop(&mut self) {
        CLAIMED_WAKES.with(|claimed| {
            let popped = claimed.borrow_mut().pop();
            debug_assert_eq!(popped, Some(self.queue));
        });
    }
}

impl Default for CompletionQueue {
    fn default() -> Self {
        Self {
            ready: Mutex::default(),
            signal: Condvar::new(),
            wake: Mutex::default(),
            wake_drained: Condvar::new(),
        }
    }
}

impl std::fmt::Debug for CompletionQueue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompletionQueue")
            .field("ready", &self.ready.lock().map(|ready| ready.tasks.len()))
            .finish_non_exhaustive()
    }
}

impl CompletionQueue {
    pub fn new() -> Self {
        Self::default()
    }

    /// Schedule the caller's executor when a task arrives. Notifications are
    /// edge-triggered and coalesced: at most one callback runs for this queue,
    /// and admissions during it request one more invocation after it returns.
    /// A coalescing publisher returns without invoking or waiting for it.
    ///
    /// The callback runs on a publishing thread, without queue locks. It must
    /// only schedule the owner thread and return: it must not enter JSI or wait
    /// for owner-thread work. A callback already in flight may finish after
    /// replacement. Releasing the last owner from the callback itself is safe.
    /// Releasing it from another thread while holding a lock that the in-flight
    /// callback needs is unsupported ordinary lock ordering: shutdown waits for
    /// that callback, so the caller must release the lock first.
    pub fn set_wake(&self, wake: Option<Arc<dyn Fn() + Send + Sync>>) {
        let mut state = self.wake.lock().expect("completion wake poisoned");
        if !state.closed {
            state.callback = wake;
        }
    }

    /// Permanently remove the owner callback and wait for the sole invocation,
    /// if any. The invoking thread returns immediately when closure is
    /// re-entrant. Returning is the barrier: no later completion can call into
    /// an executor whose owner has gone away.
    fn close_wake(&self) {
        {
            let mut ready = self.ready.lock().expect("completion queue poisoned");
            ready.closed = true;
        }
        self.signal.notify_all();
        let queue = std::ptr::from_ref(self) as usize;
        let invoking_here = CLAIMED_WAKES.with(|claimed| claimed.borrow().contains(&queue));
        let mut state = self.wake.lock().expect("completion wake poisoned");
        state.closed = true;
        state.callback = None;
        state.wake_pending = false;
        // A wake may release the last owner itself. Its own invocation cannot
        // return before this call, so it leaves the invocation flag for the
        // outer loop to retire. Every other thread waits for that one invoker.
        if invoking_here {
            return;
        }
        while state.invoking {
            state = self
                .wake_drained
                .wait(state)
                .expect("completion wake poisoned");
        }
    }

    /// Publish a settlement. Callable from any thread.
    pub fn complete(&self, task_id: u64, result: Result<HostValue, HostError>) {
        self.admit(HostTask::Settlement(Completion { task_id, result }));
    }

    /// Admit a task to the FIFO. Order of admission is the order of delivery.
    pub fn admit(&self, task: HostTask) {
        if !self.enqueue(task) {
            return;
        }
        self.notify_admission();
    }

    /// Insert without notifying the embedder. Event publication uses this
    /// while holding its subscription registry lock, then wakes only after
    /// releasing that lock.
    fn enqueue(&self, task: HostTask) -> bool {
        let mut ready = self.ready.lock().expect("completion queue poisoned");
        // Queue closure and insertion share this lock. A result offered
        // after closure is dropped here, including any owned payload.
        // @ref LLP 0058.000.000#9-teardown-and-lifecycle — late worker results cannot publish after Closing
        if ready.closed {
            return false;
        }
        let sequence = ready.next_sequence;
        ready.next_sequence = ready
            .next_sequence
            .checked_add(1)
            .expect("host-task admission sequence overflow");
        ready.tasks.push_back(AdmittedTask { sequence, task });
        true
    }

    fn notify_admission(&self) {
        self.signal.notify_all();
        let invoke = {
            let mut state = self.wake.lock().expect("completion wake poisoned");
            if state.closed {
                false
            } else {
                state.wake_pending = true;
                if state.invoking || state.callback.is_none() {
                    false
                } else {
                    state.invoking = true;
                    true
                }
            }
        };
        if invoke {
            self.invoke_wake();
        }
    }

    /// Run the queue's one coalescing wake loop. The caller has changed
    /// `invoking` from false to true under the wake lock.
    fn invoke_wake(&self) {
        let _claimed = ClaimedWake::new(self);
        loop {
            let wake = {
                let mut state = self.wake.lock().expect("completion wake poisoned");
                if state.closed || !state.wake_pending || state.callback.is_none() {
                    state.invoking = false;
                    self.wake_drained.notify_all();
                    return;
                }
                state.wake_pending = false;
                state.callback.clone().expect("callback checked above")
            };
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| wake()));
            if let Err(payload) = outcome {
                let mut state = self.wake.lock().expect("completion wake poisoned");
                state.invoking = false;
                self.wake_drained.notify_all();
                drop(state);
                std::panic::resume_unwind(payload);
            }
        }
    }

    /// Take the next admitted task, if any. Called on the JavaScript thread.
    ///
    /// FIFO across BOTH kinds: a timer admitted before a settlement runs before
    /// it. The order an application observes is the order things became ready,
    /// not an artifact of which queue the driver happened to look at first.
    pub fn take(&self) -> Option<HostTask> {
        self.take_sequenced().map(|admitted| admitted.task)
    }

    /// Take the next task together with its monotonically assigned admission
    /// sequence. Engines normally need only [`Self::take`]; ordering tests and
    /// diagnostic consumers use this form to prove all source kinds share the
    /// same counter.
    pub fn take_sequenced(&self) -> Option<AdmittedTask> {
        self.ready
            .lock()
            .expect("completion queue poisoned")
            .tasks
            .pop_front()
    }

    /// Remove events for a subscription which have not been reserved yet.
    fn cancel_events(&self, subscription: u64) {
        self.ready
            .lock()
            .expect("completion queue poisoned")
            .tasks
            .retain(|admitted| {
                !matches!(
                    admitted.task,
                    HostTask::Event {
                        subscription: current,
                        ..
                    } if current == subscription
                )
            });
    }

    /// Drop every unreserved event while preserving settlements and timers.
    fn cancel_all_events(&self) {
        self.ready
            .lock()
            .expect("completion queue poisoned")
            .tasks
            .retain(|admitted| !matches!(admitted.task, HostTask::Event { .. }));
    }

    pub fn len(&self) -> usize {
        self.ready
            .lock()
            .expect("completion queue poisoned")
            .tasks
            .len()
    }

    /// Block until at least one completion is ready, or the timeout elapses.
    ///
    /// This is what the Condvar is for. An embedder that polls in a loop burns
    /// a core to do nothing; one that blocks here wakes exactly when there is
    /// work. Returns whether anything is ready.
    pub fn wait(&self, timeout: std::time::Duration) -> bool {
        let ready = self.ready.lock().expect("completion queue poisoned");
        if !ready.tasks.is_empty() {
            return true;
        }
        let (ready, _) = self
            .signal
            .wait_timeout(ready, timeout)
            .expect("completion queue poisoned");
        !ready.tasks.is_empty()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Everything one runtime owns on the Rust side.
///
/// Per-runtime for the reason C5 exists, and it holds the response registry as
/// well as the completion queue: a `Response` cannot cross the boundary as a
/// value, because §1.1 forbids serializing at the boundary and a `Response` is
/// a status, a header list, and a body. It crosses as a **handle** — the other
/// half of "primitives and handles" — and these are the objects the handle
/// refers to.
pub struct RuntimeState {
    pub queue: CompletionQueue,
    #[cfg(all(feature = "intl", any(target_os = "linux", windows)))]
    pub(crate) intl: crate::stdlib::intl::Registry,
    #[cfg(all(feature = "intl", any(target_os = "linux", windows)))]
    pub(crate) intl_datetime: crate::stdlib::intl_datetime::Registry,
    pub(crate) sqlite: crate::sqlite_abi::Registry,
    app_directories: std::sync::OnceLock<crate::stdlib::app_fs::AppDirectories>,
    /// The embedder's table of documents the person chose (Exact patch 5).
    documents: std::sync::OnceLock<Arc<crate::stdlib::fs::Documents>>,
    responses: Mutex<std::collections::HashMap<u64, Arc<StoredResponse>>>,
    controls: Mutex<std::collections::HashMap<u64, crate::stdlib::abort::AbortController>>,
    subscriptions: Mutex<HashMap<u64, Arc<EventSubscriptionState>>>,
    websockets: Mutex<HashMap<u64, StoredWebSocket>>,
    shutdown: std::sync::atomic::AtomicBool,
    #[cfg(test)]
    shutdown_transition_hook: Mutex<Option<Arc<dyn Fn() + Send + Sync>>>,
    /// References held by workers keep storage alive but do not keep the
    /// runtime open. Only Contexts and owning engine handles increment this.
    owners: std::sync::atomic::AtomicUsize,
    /// Header lists JavaScript holds by handle, for the same reason responses
    /// are: a header list is not a primitive and §1.1 forbids serializing.
    headers: Mutex<std::collections::HashMap<u64, crate::stdlib::fetch::Headers>>,
    /// Secret keys held by JavaScript `CryptoKey` objects. Material stays in
    /// Rust and leaves only through an allowed `exportKey` operation.
    crypto_keys: Mutex<std::collections::HashMap<u64, crate::stdlib::subtle::CryptoKey>>,
    /// The timer wheel (LLP 0059.000 §3.2). Rust owns *when*; the engine keeps
    /// the closures and owns *what*.
    timers: Mutex<crate::stdlib::timers::Timers>,
    /// The runtime's monotonic origin, so `now()` is milliseconds since boot —
    /// the same base `performance.now` will read (§2).
    started: std::time::Instant,
    /// True while a drive cycle is running, so a nested request records a
    /// wakeup instead of starting a second host task.
    driving: std::sync::atomic::AtomicBool,
    /// Work started on another thread and not yet delivered.
    ///
    /// Without this, "is the loop idle?" is answered by looking at the
    /// completion queue — which is empty both when there is nothing to do and
    /// when everything is still in flight. A program whose last act is a fetch
    /// would exit before its response arrived.
    in_flight: std::sync::atomic::AtomicUsize,
    next_handle: std::sync::atomic::AtomicU64,
    /// The exact transport and stores selected by the host endowment. Keeping
    /// the complete value here makes later binding families use those same
    /// components instead of reconstructing platform defaults.
    endowment: host::Bindings,
    /// An owning engine keeps its own queue and adopts the host-selected
    /// mechanisms exactly once. OnceLock makes the borrowed reference returned
    /// by transport() stable for the runtime's lifetime.
    adopted_endowment: std::sync::OnceLock<host::Bindings>,
}

struct StoredResponse {
    response: Mutex<crate::stdlib::fetch::StreamingResponse>,
    control: crate::stdlib::abort::AbortController,
    control_handle: Option<u64>,
}

struct StoredWebSocket {
    connection: crate::stdlib::websocket::Connection,
    _subscription: crate::stdlib::events::Subscription,
    activity: Arc<WebSocketActivity>,
}

const MAX_SUBSCRIPTION_EVENTS: usize = 256;
const MAX_SUBSCRIPTION_BYTES: usize = 32 << 20;

struct EventSubscriptionState {
    quota: Mutex<EventSubscriptionQuota>,
    space: std::sync::Condvar,
}

struct EventSubscriptionQuota {
    live: bool,
    messages: usize,
    bytes: usize,
}

impl EventSubscriptionState {
    fn new() -> Self {
        Self {
            quota: Mutex::new(EventSubscriptionQuota {
                live: true,
                messages: 0,
                bytes: 0,
            }),
            space: std::sync::Condvar::new(),
        }
    }

    fn reserve(&self, bytes: usize) -> bool {
        if bytes > MAX_SUBSCRIPTION_BYTES {
            return false;
        }
        let mut quota = self.quota.lock().expect("event subscription poisoned");
        while quota.live
            && (quota.messages >= MAX_SUBSCRIPTION_EVENTS
                || quota.bytes.saturating_add(bytes) > MAX_SUBSCRIPTION_BYTES)
        {
            quota = self.space.wait(quota).expect("event subscription poisoned");
        }
        if !quota.live {
            return false;
        }
        quota.messages += 1;
        quota.bytes += bytes;
        true
    }

    fn release(&self, bytes: usize) {
        let mut quota = self.quota.lock().expect("event subscription poisoned");
        quota.messages = quota.messages.saturating_sub(1);
        quota.bytes = quota.bytes.saturating_sub(bytes);
        self.space.notify_one();
    }

    fn close(&self) {
        let mut quota = self.quota.lock().expect("event subscription poisoned");
        quota.live = false;
        quota.messages = 0;
        quota.bytes = 0;
        self.space.notify_all();
    }
}

struct WebSocketActivity {
    state: Weak<RuntimeState>,
    finished: std::sync::atomic::AtomicBool,
}

impl WebSocketActivity {
    fn finish(&self) {
        if !self
            .finished
            .swap(true, std::sync::atomic::Ordering::AcqRel)
        {
            if let Some(state) = self.state.upgrade() {
                state.task_finished();
            }
        }
    }
}

impl Drop for StoredWebSocket {
    fn drop(&mut self) {
        self.activity.finish();
    }
}

impl RuntimeState {
    pub fn new(transport: Box<dyn crate::stdlib::fetch::Transport>) -> Self {
        let bindings = host::Host::with_transport(transport).endow(crate::grant::GrantSet::none());
        Self::from_bindings(&bindings)
    }

    pub(crate) fn from_bindings(bindings: &host::Bindings) -> Self {
        let app_directories = std::sync::OnceLock::new();
        if let Some(directories) = bindings.app_directories() {
            app_directories
                .set((*directories).clone())
                .expect("new app-directories cell is empty");
        }
        let state = Self {
            queue: CompletionQueue::new(),
            #[cfg(all(feature = "intl", any(target_os = "linux", windows)))]
            intl: crate::stdlib::intl::Registry::new(),
            #[cfg(all(feature = "intl", any(target_os = "linux", windows)))]
            intl_datetime: crate::stdlib::intl_datetime::Registry::new(),
            sqlite: crate::sqlite_abi::Registry::default(),
            app_directories,
            documents: std::sync::OnceLock::new(),
            responses: Mutex::new(std::collections::HashMap::new()),
            controls: Mutex::new(std::collections::HashMap::new()),
            subscriptions: Mutex::new(HashMap::new()),
            websockets: Mutex::new(HashMap::new()),
            shutdown: std::sync::atomic::AtomicBool::new(false),
            #[cfg(test)]
            shutdown_transition_hook: Mutex::new(None),
            owners: std::sync::atomic::AtomicUsize::new(0),
            headers: Mutex::new(std::collections::HashMap::new()),
            crypto_keys: Mutex::new(std::collections::HashMap::new()),
            timers: Mutex::new(crate::stdlib::timers::Timers::new()),
            started: std::time::Instant::now(),
            driving: std::sync::atomic::AtomicBool::new(false),
            in_flight: std::sync::atomic::AtomicUsize::new(0),
            next_handle: std::sync::atomic::AtomicU64::new(1),
            endowment: bindings.clone(),
            adopted_endowment: std::sync::OnceLock::new(),
        };
        if let Some(provider) = bindings.sqlite_provider() {
            state
                .sqlite
                .set_provider(provider)
                .expect("new SQLite registry has no provider");
        }
        state
    }

    pub fn set_app_directories(
        &self,
        directories: crate::stdlib::app_fs::AppDirectories,
    ) -> Result<(), HostError> {
        self.app_directories.set(directories).map_err(|_| {
            HostError::InvalidArgument("App directories are already configured".into())
        })
    }
    pub fn app_directories(&self) -> Option<&crate::stdlib::app_fs::AppDirectories> {
        self.app_directories.get()
    }
    /// Install the table `doc:` paths resolve through (Exact patch 5).
    pub fn set_documents(
        &self,
        documents: Arc<crate::stdlib::fs::Documents>,
    ) -> Result<(), HostError> {
        self.documents
            .set(documents)
            .map_err(|_| HostError::InvalidArgument("Documents are already configured".into()))
    }
    pub fn documents(&self) -> Option<&crate::stdlib::fs::Documents> {
        self.documents.get().map(|d| &**d)
    }
    pub fn set_sqlite_provider(
        &self,
        provider: Arc<dyn crate::stdlib::sqlite::Provider>,
    ) -> Result<(), HostError> {
        self.sqlite.set_provider(provider)
    }

    pub fn transport(&self) -> &dyn crate::stdlib::fetch::Transport {
        self.adopted_endowment
            .get()
            .unwrap_or(&self.endowment)
            .fetch
            .transport()
    }

    fn websocket(&self) -> &host::WebSocket {
        &self
            .adopted_endowment
            .get()
            .unwrap_or(&self.endowment)
            .websocket
    }

    /// Start a WebSocket source whose callback identity already belongs to
    /// this runtime. The platform worker publishes compact byte payloads into
    /// the same FIFO as settlements and timers; JSI is touched only by pump.
    // @ref LLP 0057.000#l4--websocket--completed-2026-10-04 — WebSocket is the first user of L3's subscription FIFO
    pub(crate) fn open_websocket(
        self: &Arc<Self>,
        subscription: u64,
        grants: Arc<crate::grant::GrantSet>,
        url: String,
        protocols: Vec<String>,
        max_message: usize,
    ) -> Result<u64, HostError> {
        if self.is_shutdown()
            || !self
                .subscriptions
                .lock()
                .expect("event subscriptions poisoned")
                .contains_key(&subscription)
        {
            return Err(HostError::Failed("the runtime is shutting down".into()));
        }
        self.task_started();
        let activity = Arc::new(WebSocketActivity {
            state: Arc::downgrade(self),
            finished: std::sync::atomic::AtomicBool::new(false),
        });
        let publisher = Arc::downgrade(self);
        let worker_activity = Arc::clone(&activity);
        let publish = Arc::new(move |event: crate::stdlib::websocket::Event| {
            let terminal = matches!(event, crate::stdlib::websocket::Event::Close { .. });
            let payload = websocket_event_payload(event);
            let published = publisher
                .upgrade()
                .is_some_and(|state| state.publish_event(subscription, payload));
            if terminal || !published {
                worker_activity.finish();
            }
            published
        });
        let (connection, source_subscription) =
            self.websocket()
                .watch_with_grants(grants, url, protocols, max_message, publish);
        let handle = self
            .next_handle
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let mut sockets = self.websockets.lock().expect("WebSockets poisoned");
        if self.is_shutdown() {
            drop(sockets);
            source_subscription.unsubscribe();
            activity.finish();
            return Err(HostError::Failed("the runtime is shutting down".into()));
        }
        sockets.insert(
            handle,
            StoredWebSocket {
                connection,
                _subscription: source_subscription,
                activity,
            },
        );
        Ok(handle)
    }

    pub(crate) fn with_websocket<T>(
        &self,
        handle: u64,
        apply: impl FnOnce(&crate::stdlib::websocket::Connection) -> Result<T, HostError>,
    ) -> Result<T, HostError> {
        let sockets = self.websockets.lock().expect("WebSockets poisoned");
        let socket = sockets
            .get(&handle)
            .ok_or_else(|| HostError::Failed("unknown WebSocket handle".into()))?;
        apply(&socket.connection)
    }

    pub(crate) fn drop_websocket(&self, handle: u64) {
        self.websockets
            .lock()
            .expect("WebSockets poisoned")
            .remove(&handle);
    }

    /// Snapshot the source endowment together with configuration applied
    /// directly to this state after the Context was constructed.
    // @ref LLP 0068#3-no-engine-in-the-process — owning Hermes adopts the configured Context without sharing its runtime identity
    #[cfg(feature = "bindings")]
    pub(crate) fn bindings_snapshot(&self, bindings: &host::Bindings) -> host::Bindings {
        bindings.with_runtime_configuration(
            self.app_directories.get().cloned().map(Arc::new),
            self.sqlite.provider(),
        )
    }

    /// Copy the mechanisms selected by a Host into an owning runtime without
    /// replacing its queue, handle registries, loader, or integrity snapshot.
    /// Explicit configuration already installed directly on the runtime wins
    /// when the endowment omits that optional component.
    // @ref LLP 0057.000#50-three-doors-one-implementation — owning engines copy the endowed mechanisms without sharing runtime identity
    #[cfg(feature = "bindings")]
    pub(crate) fn adopt_bindings(&self, bindings: &host::Bindings) -> Result<(), HostError> {
        if self.adopted_endowment.get().is_some() {
            return Err(HostError::InvalidArgument(
                "Runtime bindings are already endowed".into(),
            ));
        }
        if self.app_directories.get().is_none() {
            if let Some(directories) = bindings.app_directories() {
                let _ = self.app_directories.set((*directories).clone());
            }
        }
        if !self.sqlite.has_provider() {
            if let Some(provider) = bindings.sqlite_provider() {
                self.sqlite.set_provider(provider)?;
            }
        }
        self.adopted_endowment
            .set(bindings.clone())
            .map_err(|_| HostError::InvalidArgument("Runtime bindings are already endowed".into()))
    }

    pub fn create_control(&self) -> u64 {
        let handle = self
            .next_handle
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let control = crate::stdlib::abort::AbortController::new();
        let mut controls = self.controls.lock().unwrap();
        if self.shutdown.load(std::sync::atomic::Ordering::Acquire) {
            control.abort();
        }
        controls.insert(handle, control);
        handle
    }

    pub fn control(&self, handle: u64) -> Option<crate::stdlib::abort::AbortController> {
        self.controls.lock().unwrap().get(&handle).cloned()
    }

    pub fn release_control(&self, handle: u64) {
        self.controls.lock().unwrap().remove(&handle);
    }

    pub fn store_response(
        &self,
        response: crate::stdlib::fetch::StreamingResponse,
        control: crate::stdlib::abort::AbortController,
        control_handle: Option<u64>,
    ) -> u64 {
        let handle = self
            .next_handle
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let mut responses = self.responses.lock().unwrap();
        if self.shutdown.load(std::sync::atomic::Ordering::Acquire) {
            control.abort();
            return handle;
        }
        responses.insert(
            handle,
            Arc::new(StoredResponse {
                response: Mutex::new(response),
                control,
                control_handle,
            }),
        );
        handle
    }

    pub fn with_response<T>(
        &self,
        handle: u64,
        f: impl FnOnce(&crate::stdlib::fetch::StreamingResponse) -> T,
    ) -> Option<T> {
        let record = self.responses.lock().unwrap().get(&handle).cloned()?;
        let response = record.response.lock().unwrap();
        Some(f(&response))
    }

    pub fn read_response(&self, handle: u64) -> Result<HostValue, HostError> {
        let record = self
            .responses
            .lock()
            .unwrap()
            .get(&handle)
            .cloned()
            .ok_or_else(|| HostError::Failed("TypeError: unknown response body".into()))?;
        let mut chunk = vec![0; 16 * 1024];
        let result = record.response.lock().unwrap().body.read(&mut chunk);
        match result {
            Ok(n) if n > 0 => {
                chunk.truncate(n);
                Ok(HostValue::Bytes(chunk))
            }
            terminal => {
                self.responses.lock().unwrap().remove(&handle);
                if let Some(handle) = record.control_handle {
                    self.release_control(handle);
                }
                terminal.map(|_| HostValue::Null)
            }
        }
    }

    pub fn cancel_response(&self, handle: u64) {
        let record = self.responses.lock().unwrap().remove(&handle);
        if let Some(record) = record {
            record.control.abort();
            if let Some(handle) = record.control_handle {
                self.release_control(handle);
            }
        }
    }

    pub fn shutdown(&self) {
        let mut subscriptions = self
            .subscriptions
            .lock()
            .expect("event subscriptions poisoned");
        if self
            .shutdown
            .swap(true, std::sync::atomic::Ordering::AcqRel)
        {
            return;
        }
        #[cfg(test)]
        if let Some(hook) = self
            .shutdown_transition_hook
            .lock()
            .expect("shutdown transition hook poisoned")
            .take()
        {
            hook();
        }
        // @ref LLP 0058.000.000#9-teardown-and-lifecycle — Closing and event reservation serialize on the subscription registry
        let subscription_states = std::mem::take(&mut *subscriptions);
        for subscription in subscription_states.into_values() {
            subscription.close();
        }
        self.queue.cancel_all_events();
        drop(subscriptions);
        self.queue.close_wake();
        self.sqlite.shutdown();
        let controls = std::mem::take(&mut *self.controls.lock().unwrap());
        for control in controls.into_values() {
            control.abort();
        }
        let responses = std::mem::take(&mut *self.responses.lock().unwrap());
        for response in responses.into_values() {
            response.control.abort();
        }
        // Closing subscription quotas above wakes any WebSocket publisher
        // backpressured on the host FIFO before dropping its source
        // subscription waits for that publication to leave.
        let websockets = std::mem::take(&mut *self.websockets.lock().expect("WebSockets poisoned"));
        drop(websockets);
        self.crypto_keys
            .lock()
            .expect("crypto key registry poisoned")
            .clear();
        self.headers
            .lock()
            .expect("header registry poisoned")
            .clear();
    }

    pub(crate) fn is_shutdown(&self) -> bool {
        self.shutdown.load(std::sync::atomic::Ordering::Acquire)
    }

    #[cfg(test)]
    fn set_shutdown_transition_hook(&self, hook: Arc<dyn Fn() + Send + Sync>) {
        *self
            .shutdown_transition_hook
            .lock()
            .expect("shutdown transition hook poisoned") = Some(hook);
    }

    fn acquire_owner(&self) {
        let previous = self
            .owners
            .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        assert!(previous != usize::MAX, "runtime owner count overflow");
    }

    fn release_owner(&self) {
        let previous = self
            .owners
            .fetch_sub(1, std::sync::atomic::Ordering::AcqRel);
        assert!(previous != 0, "runtime owner count underflow");
        if previous == 1 {
            self.shutdown();
        }
    }

    pub fn store_headers(&self, headers: crate::stdlib::fetch::Headers) -> u64 {
        let handle = self
            .next_handle
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.headers
            .lock()
            .expect("header registry poisoned")
            .insert(handle, headers);
        handle
    }

    pub fn with_headers<T>(
        &self,
        handle: u64,
        f: impl FnOnce(&crate::stdlib::fetch::Headers) -> T,
    ) -> Option<T> {
        self.headers
            .lock()
            .expect("header registry poisoned")
            .get(&handle)
            .map(f)
    }

    pub fn with_headers_mut<T>(
        &self,
        handle: u64,
        f: impl FnOnce(&mut crate::stdlib::fetch::Headers) -> T,
    ) -> Option<T> {
        self.headers
            .lock()
            .expect("header registry poisoned")
            .get_mut(&handle)
            .map(f)
    }

    pub fn drop_headers(&self, handle: u64) {
        self.headers
            .lock()
            .expect("header registry poisoned")
            .remove(&handle);
    }

    #[cfg(feature = "bindings")]
    #[doc(hidden)]
    pub fn live_headers(&self) -> usize {
        self.headers.lock().expect("header registry poisoned").len()
    }

    pub(crate) fn store_crypto_key(&self, key: crate::stdlib::subtle::CryptoKey) -> u64 {
        let handle = self
            .next_handle
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.crypto_keys
            .lock()
            .expect("crypto key registry poisoned")
            .insert(handle, key);
        handle
    }

    pub(crate) fn with_crypto_key<T>(
        &self,
        handle: u64,
        f: impl FnOnce(&crate::stdlib::subtle::CryptoKey) -> T,
    ) -> Option<T> {
        self.crypto_keys
            .lock()
            .expect("crypto key registry poisoned")
            .get(&handle)
            .map(f)
    }

    pub(crate) fn release_crypto_key(&self, handle: u64) {
        self.crypto_keys
            .lock()
            .expect("crypto key registry poisoned")
            .remove(&handle);
    }

    #[doc(hidden)]
    pub fn crypto_key_count(&self) -> usize {
        self.crypto_keys
            .lock()
            .expect("crypto key registry poisoned")
            .len()
    }

    pub fn task_started(&self) {
        self.in_flight
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn task_finished(&self) {
        self.in_flight
            .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    }

    /// Is the driver already running a cycle?
    ///
    /// A drive request made while not `Idle` records a wakeup rather than
    /// nesting a second host task inside project JavaScript
    /// (LLP 0058.000.000 §8).
    pub fn begin_drive(&self) -> bool {
        !self.driving.swap(true, std::sync::atomic::Ordering::SeqCst)
    }

    pub fn end_drive(&self) {
        self.driving
            .store(false, std::sync::atomic::Ordering::SeqCst);
    }

    /// Nothing queued, nothing in flight, no timer pending.
    pub fn is_idle(&self) -> bool {
        self.queue.is_empty()
            && self.in_flight.load(std::sync::atomic::Ordering::SeqCst) == 0
            && self.millis_until_next_timer().is_none()
    }

    /// Milliseconds since this runtime started.
    pub fn now(&self) -> f64 {
        self.started.elapsed().as_secs_f64() * 1000.0
    }

    pub fn set_timer(&self, delay_ms: f64, repeating: bool) -> u64 {
        let delay = std::time::Duration::from_secs_f64((delay_ms.max(0.0)) / 1000.0);
        self.timers
            .lock()
            .expect("timers poisoned")
            .set(self.now(), delay, repeating)
    }

    pub fn clear_timer(&self, handle: u64) {
        self.timers.lock().expect("timers poisoned").clear(handle);
    }

    /// Move every timer due now into the host-task FIFO, and report how many.
    ///
    /// Admission, not delivery: the driver still takes at most one task per
    /// cycle (LLP 0058.000.000 §8). Intervals reschedule inside `take_due` —
    /// before their callback runs — so clearing an interval from within its own
    /// callback removes the next occurrence rather than the one in flight.
    pub fn admit_due_timers(&self) -> usize {
        let now = self.now();
        let mut admitted = 0;
        loop {
            let due = self.timers.lock().expect("timers poisoned").take_due(now);
            match due {
                Some(handle) => {
                    self.queue.admit(HostTask::Timer { handle });
                    admitted += 1;
                }
                None => return admitted,
            }
        }
    }

    /// Milliseconds until the next timer, for an embedder that wants to sleep
    /// exactly long enough rather than poll.
    pub fn millis_until_next_timer(&self) -> Option<f64> {
        let now = self.now();
        self.timers
            .lock()
            .expect("timers poisoned")
            .next_deadline()
            .map(|deadline| (deadline - now).max(0.0))
    }

    pub fn live_responses(&self) -> usize {
        self.responses
            .lock()
            .expect("response registry poisoned")
            .len()
    }

    /// Register one callback slot for a caller-owned runtime. The JSI adapter
    /// owns the returned handle and its JavaScript callback root together.
    pub(crate) fn subscribe_event(
        self: &Arc<Self>,
    ) -> Option<(u64, crate::stdlib::events::Subscription)> {
        let id = self
            .next_handle
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let mut subscriptions = self
            .subscriptions
            .lock()
            .expect("event subscriptions poisoned");
        if self.is_shutdown() {
            return None;
        }
        subscriptions.insert(id, Arc::new(EventSubscriptionState::new()));
        drop(subscriptions);
        let state: Weak<Self> = Arc::downgrade(self);
        let subscription = crate::stdlib::events::Subscription::new(move || {
            if let Some(state) = state.upgrade() {
                state.unsubscribe_event(id);
            }
        });
        Some((id, subscription))
    }

    fn unsubscribe_event(&self, id: u64) {
        let subscription = self
            .subscriptions
            .lock()
            .expect("event subscriptions poisoned")
            .remove(&id);
        if let Some(subscription) = subscription {
            subscription.close();
        }
        // @ref LLP 0057.000#l3--events-abort-and-the-second-direction — unsubscribe cancels admitted but unreserved delivery
        self.queue.cancel_events(id);
    }

    /// Publish from a host source without touching JSI. Holding the registry
    /// lock through FIFO insertion closes the race with unsubscribe: either
    /// this task is inserted first and cancellation removes it, or publication
    /// sees the missing subscription and refuses it. Notification happens only
    /// after that lock is released because the embedder wake may re-enter any
    /// subscription operation.
    #[cfg_attr(not(test), allow(dead_code))]
    #[doc(hidden)]
    pub fn publish_event(&self, subscription: u64, payload: HostValue) -> bool {
        let bytes = host_value_size(&payload);
        let state = {
            let subscriptions = self
                .subscriptions
                .lock()
                .expect("event subscriptions poisoned");
            if self.is_shutdown() {
                return false;
            }
            let Some(state) = subscriptions.get(&subscription) else {
                return false;
            };
            Arc::clone(state)
        };
        if !state.reserve(bytes) {
            return false;
        }
        // @ref LLP 0058.000.000#8-tasks-microtasks-timers-and-callbacks — subscription liveness and FIFO insertion are atomic, but wake is outside the registry lock
        let admitted = {
            let subscriptions = self
                .subscriptions
                .lock()
                .expect("event subscriptions poisoned");
            if self.is_shutdown()
                || !subscriptions
                    .get(&subscription)
                    .is_some_and(|current| Arc::ptr_eq(current, &state))
            {
                state.release(bytes);
                return false;
            }
            self.queue.enqueue(HostTask::Event {
                subscription,
                payload,
            })
        };
        if !admitted {
            state.release(bytes);
        }
        if admitted {
            self.queue.notify_admission();
        }
        admitted
    }

    /// Reserve the next live task. Reservation and the transition to shutdown
    /// share the subscription lock: a task either commits while the runtime is
    /// running, or observes Closing and stays out. An unsubscribe may remove
    /// an event identity before its queue cancellation acquires the FIFO lock;
    /// that stale event is discarded here.
    pub(crate) fn take_task(&self) -> Option<HostTask> {
        loop {
            let subscriptions = self
                .subscriptions
                .lock()
                .expect("event subscriptions poisoned");
            if self.is_shutdown() {
                return None;
            }
            let task = self.queue.take()?;
            let HostTask::Event { subscription, .. } = &task else {
                return Some(task);
            };
            // @ref LLP 0058.000.000#8-tasks-microtasks-timers-and-callbacks — reservation revalidates Running and subscription liveness at one serialized commit point
            let state = subscriptions.get(subscription).cloned();
            if let Some(state) = state {
                if let HostTask::Event { payload, .. } = &task {
                    state.release(host_value_size(payload));
                }
                return Some(task);
            }
        }
    }
}

fn host_value_size(value: &HostValue) -> usize {
    match value {
        HostValue::Str(value) => value.len(),
        HostValue::Bytes(value) => value.len(),
        _ => std::mem::size_of::<HostValue>(),
    }
}

/// One compact, non-JSON payload for the JSI subscription callback.
/// Byte 0 is open/text/binary/error/close; close then carries clean and code.
fn websocket_event_payload(event: crate::stdlib::websocket::Event) -> HostValue {
    use crate::stdlib::websocket::{Event, Message};
    let mut bytes = Vec::new();
    match event {
        Event::Open { protocol } => {
            bytes.push(0);
            bytes.extend_from_slice(protocol.as_bytes());
        }
        Event::Message(Message::Text(text)) => {
            bytes.push(1);
            bytes.extend_from_slice(text.as_bytes());
        }
        Event::Message(Message::Binary(payload)) => {
            bytes.push(2);
            bytes.extend_from_slice(&payload);
        }
        Event::Error(message) => {
            bytes.push(3);
            bytes.extend_from_slice(message.as_bytes());
        }
        Event::Close {
            code,
            reason,
            was_clean,
        } => {
            bytes.push(4);
            bytes.push(u8::from(was_clean));
            bytes.extend_from_slice(&code.to_be_bytes());
            bytes.extend_from_slice(reason.as_bytes());
        }
    }
    HostValue::Bytes(bytes)
}

impl Drop for RuntimeState {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// A runtime owner. Worker Arcs deliberately do not contain one, so the last
/// embedder owner initiates shutdown even while blocked work retains storage.
// @ref LLP 0058.000.000#9-teardown-and-lifecycle — worker storage references do not postpone owner-initiated Closing
#[cfg(any(test, feature = "bindings"))]
pub(crate) struct OwnerLease {
    state: Arc<RuntimeState>,
}

#[cfg(any(test, feature = "bindings"))]
impl OwnerLease {
    pub(crate) fn new(state: Arc<RuntimeState>) -> Self {
        state.acquire_owner();
        Self { state }
    }
}

#[cfg(any(test, feature = "bindings"))]
impl Drop for OwnerLease {
    fn drop(&mut self) {
        self.state.release_owner();
    }
}

/// Create a queue and hand ownership to the caller as a raw pointer.
///
/// # Safety
/// The result must be released exactly once with `ibex2_queue_destroy`.
#[no_mangle]
pub extern "C" fn ibex2_queue_create() -> *const RuntimeState {
    let state = Arc::new(RuntimeState::new(crate::transport::default_transport()));
    state.acquire_owner();
    Arc::into_raw(state)
}

/// Retain an Arc-backed runtime state for an owning embedder.
///
/// # Safety
/// `queue` must be a live pointer returned by this crate.
#[no_mangle]
pub unsafe extern "C" fn ibex2_queue_retain(queue: *const RuntimeState) -> *const RuntimeState {
    if !queue.is_null() {
        (*queue).acquire_owner();
        Arc::increment_strong_count(queue);
    }
    queue
}

/// # Safety
/// `queue` must come from `ibex2_queue_create` and not have been destroyed.
#[no_mangle]
pub unsafe extern "C" fn ibex2_queue_destroy(queue: *const RuntimeState) {
    if !queue.is_null() {
        let state = Arc::from_raw(queue);
        state.release_owner();
        drop(state);
    }
}

/// Opaque owner for one event-subscription registration.
#[repr(C)]
pub struct Ibex2Subscription {
    subscription: crate::stdlib::events::Subscription,
}

/// Register a host-event callback slot for an adapter.
///
/// # Safety
/// `state` must be a live runtime state and `out_id` must be writable. The
/// returned pointer must be released exactly once with
/// [`ibex2_subscription_destroy`].
#[no_mangle]
pub unsafe extern "C" fn ibex2_subscription_create(
    state: *const RuntimeState,
    out_id: *mut u64,
) -> *mut Ibex2Subscription {
    if out_id.is_null() {
        return std::ptr::null_mut();
    }
    let Some(state) = clone_queue(state) else {
        return std::ptr::null_mut();
    };
    if state.is_shutdown() {
        return std::ptr::null_mut();
    }
    let Some((id, subscription)) = state.subscribe_event() else {
        return std::ptr::null_mut();
    };
    *out_id = id;
    Box::into_raw(Box::new(Ibex2Subscription { subscription }))
}

/// Unsubscribe and release a handle made by [`ibex2_subscription_create`].
///
/// # Safety
/// `subscription` must be null or an unfreed pointer returned by
/// [`ibex2_subscription_create`].
#[no_mangle]
pub unsafe extern "C" fn ibex2_subscription_destroy(subscription: *mut Ibex2Subscription) {
    if !subscription.is_null() {
        drop(Box::from_raw(subscription));
    }
}

struct WebSocketOwner {
    state: Weak<RuntimeState>,
    handle: u64,
}

unsafe fn websocket_input<'a>(data: *const u8, len: usize) -> Result<&'a str, HostError> {
    let bytes = if len == 0 {
        &[]
    } else if data.is_null() {
        return Err(HostError::InvalidArgument(
            "a WebSocket string span was null".into(),
        ));
    } else {
        std::slice::from_raw_parts(data, len)
    };
    std::str::from_utf8(bytes)
        .map_err(|_| HostError::InvalidArgument("a WebSocket string was not UTF-8".into()))
}

unsafe fn websocket_error(out: *mut *mut std::ffi::c_char, error: HostError) -> i32 {
    if !out.is_null() {
        let message = error.to_string().replace('\0', "�");
        *out = std::ffi::CString::new(message)
            .expect("NUL was replaced")
            .into_raw();
    }
    1
}

/// Whether this build linked an application WebSocket transport.
#[no_mangle]
pub extern "C" fn ibex2_websocket_supported() -> i32 {
    i32::from(cfg!(feature = "websocket"))
}

/// Start one JavaScript WebSocket source for an existing callback identity.
/// Protocols are newline-delimited validated HTTP tokens; a token cannot
/// itself contain a newline.
///
/// # Safety
/// `state` is a live runtime-state pointer and every input/out span is valid.
#[no_mangle]
pub unsafe extern "C" fn ibex2_websocket_open(
    state: *const RuntimeState,
    grants: *const crate::grant::GrantSet,
    subscription: u64,
    url: *const u8,
    url_len: usize,
    protocols: *const u8,
    protocols_len: usize,
    max_message: usize,
    out_error: *mut *mut std::ffi::c_char,
) -> u64 {
    if !out_error.is_null() {
        *out_error = std::ptr::null_mut();
    }
    let Some(state) = clone_queue(state) else {
        websocket_error(
            out_error,
            HostError::Failed("the runtime is detached".into()),
        );
        return 0;
    };
    let Some(grants) = crate::boundary_abi::clone_grants(grants) else {
        websocket_error(
            out_error,
            HostError::Failed("the WebSocket authority is detached".into()),
        );
        return 0;
    };
    let url = match websocket_input(url, url_len) {
        Ok(value) => value.to_string(),
        Err(error) => {
            websocket_error(out_error, error);
            return 0;
        }
    };
    let protocols = match websocket_input(protocols, protocols_len) {
        Ok("") => Vec::new(),
        Ok(value) => value.split('\n').map(str::to_string).collect(),
        Err(error) => {
            websocket_error(out_error, error);
            return 0;
        }
    };
    match state.open_websocket(subscription, grants, url, protocols, max_message) {
        Ok(handle) => handle,
        Err(error) => {
            websocket_error(out_error, error);
            0
        }
    }
}

/// Make a GC owner for a JavaScript WebSocket object.
///
/// # Safety
/// `state` is a live runtime-state pointer and `handle` came from open.
#[no_mangle]
pub unsafe extern "C" fn ibex2_websocket_owner_create(
    state: *const RuntimeState,
    handle: u64,
) -> *mut std::ffi::c_void {
    let Some(state) = clone_queue(state) else {
        return std::ptr::null_mut();
    };
    Box::into_raw(Box::new(WebSocketOwner {
        state: Arc::downgrade(&state),
        handle,
    }))
    .cast()
}

/// Release a WebSocket object's native connection after it is unreachable.
///
/// # Safety
/// `owner` is null or an unfreed pointer returned by owner_create.
#[no_mangle]
pub unsafe extern "C" fn ibex2_websocket_owner_destroy(owner: *mut std::ffi::c_void) {
    if owner.is_null() {
        return;
    }
    let owner = Box::from_raw(owner.cast::<WebSocketOwner>());
    if let Some(state) = owner.state.upgrade() {
        state.drop_websocket(owner.handle);
    }
}

/// Queue one text or binary message.
///
/// # Safety
/// `state` and `data` describe live spans for this call; `out_error` is writable.
#[no_mangle]
pub unsafe extern "C" fn ibex2_websocket_send(
    state: *const RuntimeState,
    handle: u64,
    binary: i32,
    data: *const u8,
    len: usize,
    out_error: *mut *mut std::ffi::c_char,
) -> i32 {
    if !out_error.is_null() {
        *out_error = std::ptr::null_mut();
    }
    let Some(state) = clone_queue(state) else {
        return websocket_error(
            out_error,
            HostError::Failed("the runtime is detached".into()),
        );
    };
    let bytes = if len == 0 {
        &[]
    } else if data.is_null() {
        return websocket_error(
            out_error,
            HostError::InvalidArgument("a WebSocket byte span was null".into()),
        );
    } else {
        std::slice::from_raw_parts(data, len)
    };
    let result = if binary != 0 {
        state.with_websocket(handle, |socket| socket.send_binary(bytes))
    } else {
        match std::str::from_utf8(bytes) {
            Ok(text) => state.with_websocket(handle, |socket| socket.send_text(text)),
            Err(_) => Err(HostError::InvalidArgument(
                "a WebSocket text message was not UTF-8".into(),
            )),
        }
    };
    match result {
        Ok(()) => 0,
        Err(error) => websocket_error(out_error, error),
    }
}

/// Begin the closing handshake. Code zero means the optional code was absent.
///
/// # Safety
/// Input spans are valid for this call and `out_error` is writable.
#[no_mangle]
pub unsafe extern "C" fn ibex2_websocket_close(
    state: *const RuntimeState,
    handle: u64,
    code: i32,
    reason: *const u8,
    reason_len: usize,
    out_error: *mut *mut std::ffi::c_char,
) -> i32 {
    if !out_error.is_null() {
        *out_error = std::ptr::null_mut();
    }
    let Some(state) = clone_queue(state) else {
        return websocket_error(
            out_error,
            HostError::Failed("the runtime is detached".into()),
        );
    };
    let reason = match websocket_input(reason, reason_len) {
        Ok(value) => value,
        Err(error) => return websocket_error(out_error, error),
    };
    let code = (code != 0).then_some(code as u16);
    match state.with_websocket(handle, |socket| socket.close(code, reason)) {
        Ok(()) => 0,
        Err(error) => websocket_error(out_error, error),
    }
}

/// Read `readyState`; an already-collected handle is CLOSED.
///
/// # Safety
/// `state` must remain a live runtime-state pointer for this call.
#[no_mangle]
pub unsafe extern "C" fn ibex2_websocket_ready_state(
    state: *const RuntimeState,
    handle: u64,
) -> i32 {
    let Some(state) = clone_queue(state) else {
        return 3;
    };
    state
        .with_websocket(handle, |socket| Ok(socket.ready_state() as i32))
        .unwrap_or(3)
}

/// Read the transport queue plus bytes discarded after closing.
///
/// # Safety
/// `state` must remain a live runtime-state pointer for this call.
#[no_mangle]
pub unsafe extern "C" fn ibex2_websocket_buffered_amount(
    state: *const RuntimeState,
    handle: u64,
) -> usize {
    let Some(state) = clone_queue(state) else {
        return 0;
    };
    state
        .with_websocket(handle, |socket| Ok(socket.buffered_amount()))
        .unwrap_or(0)
}

/// Test-source seam used by the engine fixtures; production event sources call
/// `RuntimeState::publish_event` from their Rust binding.
///
/// # Safety
/// `state` must remain a live runtime-state pointer for this call.
#[cfg(feature = "bindings")]
#[no_mangle]
pub unsafe extern "C" fn ibex2_test_publish_event(
    state: *const RuntimeState,
    subscription: u64,
) -> std::ffi::c_int {
    let Some(state) = clone_queue(state) else {
        return 0;
    };
    std::ffi::c_int::from(state.publish_event(subscription, HostValue::Undefined))
}

struct ResponseOwner {
    state: std::sync::Weak<RuntimeState>,
    handle: u64,
}

/// Make a GC owner that does not keep the runtime alive.
/// # Safety
/// `queue` must be a live pointer returned by `ibex2_queue_create`.
#[no_mangle]
pub unsafe extern "C" fn ibex2_response_owner_create(
    queue: *const RuntimeState,
    handle: f64,
) -> *mut std::ffi::c_void {
    let Some(state) = clone_queue(queue) else {
        return std::ptr::null_mut();
    };
    if handle.fract() != 0.0 || !(1.0..=9_007_199_254_740_991.0).contains(&handle) {
        return std::ptr::null_mut();
    }
    Box::into_raw(Box::new(ResponseOwner {
        state: Arc::downgrade(&state),
        handle: handle as u64,
    }))
    .cast()
}

/// Release a collected JS body's outstanding native request.
/// # Safety
/// `owner` must be null or an unfreed pointer returned by `ibex2_response_owner_create`.
#[no_mangle]
pub unsafe extern "C" fn ibex2_response_owner_destroy(owner: *mut std::ffi::c_void) {
    if owner.is_null() {
        return;
    }
    let owner = Box::from_raw(owner.cast::<ResponseOwner>());
    if let Some(state) = owner.state.upgrade() {
        state.cancel_response(owner.handle);
    }
}

struct HeadersOwner {
    state: std::sync::Weak<RuntimeState>,
    handle: u64,
}

/// Attach one Rust header-list handle to one engine-owned `Headers` object.
/// The weak state makes collection after adapter detach or runtime teardown a
/// no-op instead of retaining or dereferencing the runtime.
/// # Safety
/// `queue` must be a live pointer returned by `ibex2_queue_create`.
#[no_mangle]
pub unsafe extern "C" fn ibex2_headers_owner_create(
    queue: *const RuntimeState,
    handle: f64,
) -> *mut std::ffi::c_void {
    let Some(state) = clone_queue(queue) else {
        return std::ptr::null_mut();
    };
    if handle.fract() != 0.0 || !(1.0..=9_007_199_254_740_991.0).contains(&handle) {
        return std::ptr::null_mut();
    }
    let handle = handle as u64;
    if state.with_headers(handle, |_| ()).is_none() {
        return std::ptr::null_mut();
    }
    Box::into_raw(Box::new(HeadersOwner {
        state: Arc::downgrade(&state),
        handle,
    }))
    .cast()
}

/// Release a header list when its JavaScript `Headers` object is collected.
/// An explicit op 51 release removes the same row first, making this idempotent.
/// # Safety
/// `owner` must be null or an unfreed pointer from `ibex2_headers_owner_create`.
#[no_mangle]
pub unsafe extern "C" fn ibex2_headers_owner_destroy(owner: *mut std::ffi::c_void) {
    if owner.is_null() {
        return;
    }
    let owner = Box::from_raw(owner.cast::<HeadersOwner>());
    if let Some(state) = owner.state.upgrade() {
        state.drop_headers(owner.handle);
    }
}

struct CryptoKeyOwner {
    state: std::sync::Weak<RuntimeState>,
    handle: u64,
}

/// Attach one Rust key handle to one engine-owned `CryptoKey` object.
/// # Safety
/// `queue` must be a live pointer returned by `ibex2_queue_create`.
#[no_mangle]
pub unsafe extern "C" fn ibex2_crypto_key_owner_create(
    queue: *const RuntimeState,
    handle: f64,
) -> *mut std::ffi::c_void {
    let Some(state) = clone_queue(queue) else {
        return std::ptr::null_mut();
    };
    if handle.fract() != 0.0 || !(1.0..=9_007_199_254_740_991.0).contains(&handle) {
        return std::ptr::null_mut();
    }
    let handle = handle as u64;
    if state.with_crypto_key(handle, |_| ()).is_none() {
        return std::ptr::null_mut();
    }
    Box::into_raw(Box::new(CryptoKeyOwner {
        state: Arc::downgrade(&state),
        handle,
    }))
    .cast()
}

/// Release a key when its JavaScript `CryptoKey` is collected.
/// # Safety
/// `owner` must be null or an unfreed pointer from `ibex2_crypto_key_owner_create`.
#[no_mangle]
pub unsafe extern "C" fn ibex2_crypto_key_owner_destroy(owner: *mut std::ffi::c_void) {
    if owner.is_null() {
        return;
    }
    let owner = Box::from_raw(owner.cast::<CryptoKeyOwner>());
    if let Some(state) = owner.state.upgrade() {
        state.release_crypto_key(owner.handle);
    }
}

/// Test and diagnostics witness for the per-runtime key registry.
/// # Safety
/// `queue` must be null or a live pointer returned by `ibex2_queue_create`.
#[no_mangle]
pub unsafe extern "C" fn ibex2_crypto_key_count(queue: *const RuntimeState) -> usize {
    borrow_state(queue).map_or(0, RuntimeState::crypto_key_count)
}

/// Borrow the runtime state without taking ownership.
///
/// # Safety
/// `state` must be a live pointer from `ibex2_queue_create`.
pub unsafe fn borrow_state<'a>(state: *const RuntimeState) -> Option<&'a RuntimeState> {
    if state.is_null() {
        None
    } else {
        Some(&*state)
    }
}

/// Borrow a queue pointer as an `Arc` without consuming the caller's reference.
///
/// # Safety
/// `queue` must be a live pointer from `ibex2_queue_create`.
pub(crate) unsafe fn clone_queue(queue: *const RuntimeState) -> Option<Arc<RuntimeState>> {
    if queue.is_null() {
        return None;
    }
    Arc::increment_strong_count(queue);
    Some(Arc::from_raw(queue))
}

/// The ordering contract an engine adapter must satisfy.
///
/// **Normative source: LLP 0058.000.000 §8.** This restates the clauses the
/// tests hold this implementation to; where the two differ, the spec governs.
///
/// **C1.** Resolving a promise from a completion enqueues a microtask; it does
/// not run the continuation inline. A host call must never re-enter JavaScript
/// beneath itself.
///
/// **C2.** After a pump delivers completions, every microtask they enqueued —
/// transitively — drains before the pump returns. A pump that leaves
/// microtasks queued has moved work into the next macrotask and changed the
/// order the application sees.
///
/// **C3.** Microtasks already queued by JavaScript before the pump drain
/// **before** any microtask a completion enqueues during it. Completions join
/// the back of the queue; they do not jump it.
///
/// **C4.** Tasks are delivered in the order they were admitted (FIFO), across
/// timer deliveries and settlements alike — one queue, one order.
///
/// **C5.** A task is delivered to the runtime that admitted it, and to no
/// other. Queues are per-runtime.
///
/// **C6.** At most **one** host task runs per drive cycle, and a drive request
/// made while a cycle is running records a wakeup rather than nesting.
pub struct Pump;

impl Pump {
    pub const CONTRACT: &'static str =
        "C1 resolve enqueues, never re-enters; C2 pump drains transitively; \
         C3 pre-queued microtasks run first; C4 one FIFO across timers and \
         settlements; C5 tasks reach only their own runtime; \
         C6 one host task per drive, never nested";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "crypto")]
    #[test]
    fn runtime_shutdown_releases_every_crypto_key() {
        let state = RuntimeState::new(crate::transport::default_transport());
        let key = crate::stdlib::subtle::generate_key(
            crate::stdlib::subtle::GenerateAlgorithm::Hmac {
                hash: crate::stdlib::subtle::HashAlgorithm::Sha256,
                length_bits: Some(256),
            },
            false,
            &[crate::stdlib::subtle::KeyUsage::Sign],
        )
        .unwrap();
        state.store_crypto_key(key);
        assert_eq!(state.crypto_key_count(), 1);
        state.shutdown();
        assert_eq!(state.crypto_key_count(), 0);
    }

    #[test]
    fn runtime_shutdown_releases_every_header_list() {
        let state = RuntimeState::new(crate::transport::default_transport());
        for _ in 0..1000 {
            state.store_headers(crate::stdlib::fetch::Headers::new());
        }
        assert_eq!(state.live_headers(), 1000);
        state.shutdown();
        assert_eq!(state.live_headers(), 0);
    }

    #[test]
    fn shutdown_interrupts_body_without_waiting_on_its_registry_lock() {
        use crate::stdlib::{
            abort::AbortController,
            fetch::{Body, BodySource, Headers, StreamingResponse},
        };
        struct Waiting {
            wake: Arc<(Mutex<bool>, Condvar)>,
            entered: std::sync::mpsc::Sender<()>,
            _registration: crate::stdlib::abort::AbortRegistration,
        }
        impl BodySource for Waiting {
            fn read(&mut self, _: &mut [u8]) -> Result<usize, HostError> {
                self.entered.send(()).unwrap();
                let (lock, wake) = &*self.wake;
                let guard = lock.lock().unwrap();
                let (guard, timeout) = wake
                    .wait_timeout_while(guard, std::time::Duration::from_secs(2), |cancelled| {
                        !*cancelled
                    })
                    .unwrap();
                assert!(
                    !timeout.timed_out() && *guard,
                    "shutdown did not interrupt body"
                );
                Err(HostError::Failed("socket closed".into()))
            }
        }
        let state = Arc::new(RuntimeState::new(Box::new(
            crate::transport::dev_tcp::DevTcpTransport::new(),
        )));
        let control = AbortController::new();
        let wake = Arc::new((Mutex::new(false), Condvar::new()));
        let notify = wake.clone();
        let registration = control.signal().register(move || {
            let (lock, wake) = &*notify;
            *lock.lock().unwrap() = true;
            wake.notify_all();
        });
        let (entered, ready) = std::sync::mpsc::channel();
        let body = Body::new(
            Box::new(Waiting {
                wake,
                entered,
                _registration: registration,
            }),
            100,
            control.signal(),
        );
        let handle = state.store_response(
            StreamingResponse {
                status: 200,
                status_text: "OK".into(),
                headers: Headers::new(),
                body,
                url: "http://test/".into(),
                redirected: false,
            },
            control,
            None,
        );
        let reader_state = state.clone();
        let reader = std::thread::spawn(move || reader_state.read_response(handle));
        ready
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        let extra = state.create_control();
        assert!(
            state.control(extra).is_some(),
            "body read held the runtime registry"
        );
        state.shutdown();
        assert!(reader
            .join()
            .unwrap()
            .unwrap_err()
            .to_string()
            .starts_with("AbortError"));
        assert_eq!(state.live_responses(), 0);
        assert!(state.control(extra).is_none());
    }

    #[test]
    fn completions_come_back_in_publication_order() {
        let queue = CompletionQueue::new();
        queue.complete(1, Ok(HostValue::Number(1.0)));
        queue.complete(2, Ok(HostValue::Number(2.0)));
        queue.complete(3, Err(HostError::Failed("third".into())));

        let ids: Vec<u64> = (0..3)
            .map(|_| match queue.take().unwrap() {
                HostTask::Settlement(c) => c.task_id,
                other => panic!("unexpected {other:?}"),
            })
            .collect();
        assert_eq!(ids, vec![1, 2, 3]);
        assert!(queue.take().is_none());
    }

    #[test]
    fn timers_settlements_and_events_share_one_admission_sequence() {
        let state = Arc::new(RuntimeState::new(Box::new(
            crate::transport::dev_tcp::DevTcpTransport::new(),
        )));
        let (subscription_id, subscription) = state.subscribe_event().unwrap();

        state.queue.admit(HostTask::Timer { handle: 7 });
        state
            .queue
            .complete(8, Ok(HostValue::Str("settled".into())));
        assert!(state.publish_event(subscription_id, HostValue::Str("event".into())));

        let admitted: Vec<_> = (0..3)
            .map(|_| state.queue.take_sequenced().unwrap())
            .collect();
        assert_eq!(
            admitted
                .iter()
                .map(|task| task.sequence)
                .collect::<Vec<_>>(),
            [0, 1, 2]
        );
        assert!(matches!(admitted[0].task, HostTask::Timer { handle: 7 }));
        assert!(matches!(admitted[1].task, HostTask::Settlement(_)));
        assert!(matches!(
            admitted[2].task,
            HostTask::Event {
                subscription,
                payload: HostValue::Str(ref value)
            } if subscription == subscription_id && value == "event"
        ));
        drop(subscription);
    }

    #[test]
    fn unsubscribe_cancels_an_event_admitted_but_not_reserved() {
        let state = Arc::new(RuntimeState::new(Box::new(
            crate::transport::dev_tcp::DevTcpTransport::new(),
        )));
        let (subscription_id, subscription) = state.subscribe_event().unwrap();
        assert!(state.publish_event(subscription_id, HostValue::Str("queued".into())));
        assert_eq!(state.queue.len(), 1);

        subscription.unsubscribe();
        assert!(state.queue.take().is_none());
        assert!(!state.publish_event(subscription_id, HostValue::Str("late".into())));
    }

    #[test]
    fn a_host_that_stops_pumping_backpressures_the_js_event_fifo() {
        let state = Arc::new(RuntimeState::new(Box::new(
            crate::transport::dev_tcp::DevTcpTransport::new(),
        )));
        let (subscription_id, subscription) = state.subscribe_event().unwrap();
        let attempts = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let publisher = Arc::clone(&state);
        let publisher_attempts = Arc::clone(&attempts);
        let worker = std::thread::spawn(move || {
            for index in 0..(MAX_SUBSCRIPTION_EVENTS + 10) {
                publisher_attempts.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
                if !publisher
                    .publish_event(subscription_id, HostValue::Str(format!("event-{index}")))
                {
                    return false;
                }
            }
            true
        });
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while attempts.load(std::sync::atomic::Ordering::Acquire) <= MAX_SUBSCRIPTION_EVENTS
            && std::time::Instant::now() < deadline
        {
            std::thread::yield_now();
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
        assert_eq!(state.queue.len(), MAX_SUBSCRIPTION_EVENTS);
        assert_eq!(
            attempts.load(std::sync::atomic::Ordering::Acquire),
            MAX_SUBSCRIPTION_EVENTS + 1,
            "the next publication waits instead of growing the host FIFO"
        );
        subscription.unsubscribe();
        assert!(!worker.join().unwrap());
        assert!(state.queue.take().is_none());
    }

    #[test]
    fn shutdown_cancels_queued_events_and_refuses_new_subscriptions() {
        let state = Arc::new(RuntimeState::new(Box::new(
            crate::transport::dev_tcp::DevTcpTransport::new(),
        )));
        let (subscription_id, subscription) = state.subscribe_event().unwrap();
        assert!(state.publish_event(subscription_id, HostValue::Str("queued".into())));

        state.shutdown();

        assert!(state.queue.take().is_none());
        assert!(!state.publish_event(subscription_id, HostValue::Undefined));
        assert!(state.subscribe_event().is_none());
        drop(subscription);
    }

    #[test]
    fn shutdown_transition_serializes_with_event_reservation() {
        let state = Arc::new(RuntimeState::new(Box::new(
            crate::transport::dev_tcp::DevTcpTransport::new(),
        )));
        let (subscription_id, subscription) = state.subscribe_event().unwrap();
        assert!(state.publish_event(subscription_id, HostValue::Str("queued".into())));

        let (at_transition, transition_entered) = std::sync::mpsc::channel();
        let (release_transition, may_finish_transition) = std::sync::mpsc::channel();
        let may_finish_transition = Arc::new(Mutex::new(may_finish_transition));
        state.set_shutdown_transition_hook(Arc::new(move || {
            at_transition.send(()).unwrap();
            may_finish_transition.lock().unwrap().recv().unwrap();
        }));

        let shutdown_state = Arc::clone(&state);
        let shutdown = std::thread::spawn(move || shutdown_state.shutdown());
        transition_entered
            .recv_timeout(std::time::Duration::from_secs(2))
            .expect("shutdown did not reach its transition hook");

        let reservation_state = Arc::clone(&state);
        let (reservation_started, started) = std::sync::mpsc::channel();
        let (reservation_finished, finished) = std::sync::mpsc::channel();
        let reservation = std::thread::spawn(move || {
            reservation_started.send(()).unwrap();
            reservation_finished
                .send(reservation_state.take_task())
                .unwrap();
        });
        started.recv().unwrap();
        assert!(
            matches!(
                finished.recv_timeout(std::time::Duration::from_millis(200)),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout)
            ),
            "an event was reserved while the shutdown transition held its lock"
        );

        release_transition.send(()).unwrap();
        shutdown.join().unwrap();
        assert!(finished.recv().unwrap().is_none());
        reservation.join().unwrap();
        assert!(state.queue.is_empty());
        drop(subscription);
    }

    #[test]
    fn task_reservation_skips_an_event_whose_subscription_is_gone() {
        let state = Arc::new(RuntimeState::new(Box::new(
            crate::transport::dev_tcp::DevTcpTransport::new(),
        )));
        let (subscription_id, _subscription) = state.subscribe_event().unwrap();
        assert!(state.publish_event(subscription_id, HostValue::Str("stale".into())));
        state.subscriptions.lock().unwrap().remove(&subscription_id);

        assert!(state.take_task().is_none());
    }

    #[test]
    fn event_wake_can_unsubscribe_without_deadlocking_publication() {
        let state = Arc::new(RuntimeState::new(Box::new(
            crate::transport::dev_tcp::DevTcpTransport::new(),
        )));
        let (subscription_id, subscription) = state.subscribe_event().unwrap();
        state
            .queue
            .set_wake(Some(Arc::new(move || subscription.unsubscribe())));

        let publisher = Arc::clone(&state);
        let (finished, observed) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            finished
                .send(
                    publisher
                        .publish_event(subscription_id, HostValue::Str("canceled by wake".into())),
                )
                .unwrap();
        });
        assert_eq!(
            observed.recv_timeout(std::time::Duration::from_secs(2)),
            Ok(true),
            "wake deadlocked trying to unsubscribe"
        );
        worker.join().unwrap();
        state.queue.set_wake(None);
        assert!(state.queue.take().is_none());
        assert!(!state.publish_event(subscription_id, HostValue::Undefined));
    }

    #[test]
    fn event_wake_can_publish_without_deadlocking_or_recursing() {
        let state = Arc::new(RuntimeState::new(Box::new(
            crate::transport::dev_tcp::DevTcpTransport::new(),
        )));
        let (subscription_id, subscription) = state.subscribe_event().unwrap();
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let callback_calls = Arc::clone(&calls);
        let callback_state = Arc::downgrade(&state);
        state.queue.set_wake(Some(Arc::new(move || {
            if callback_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 {
                assert!(callback_state
                    .upgrade()
                    .unwrap()
                    .publish_event(subscription_id, HostValue::Str("from wake".into()),));
            }
        })));

        let publisher = Arc::clone(&state);
        let (finished, observed) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            let published =
                publisher.publish_event(subscription_id, HostValue::Str("outer".into()));
            finished.send(published).unwrap();
        });
        assert_eq!(
            observed.recv_timeout(std::time::Duration::from_secs(2)),
            Ok(true),
            "wake deadlocked trying to publish"
        );
        worker.join().unwrap();
        state.queue.set_wake(None);
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 2);
        assert_eq!(state.queue.len(), 2);
        drop(subscription);
    }

    #[test]
    fn completions_published_from_another_thread_are_visible() {
        let queue = Arc::new(CompletionQueue::new());
        let worker = Arc::clone(&queue);
        std::thread::spawn(move || {
            worker.complete(99, Ok(HostValue::Str("from a worker".into())));
        })
        .join()
        .expect("worker thread");

        match queue
            .take()
            .expect("a completion crossed the thread boundary")
        {
            HostTask::Settlement(completion) => {
                assert_eq!(completion.task_id, 99);
                assert_eq!(
                    completion.result.unwrap(),
                    HostValue::Str("from a worker".into())
                );
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    /// C5 — two queues do not see each other's work. This is the property whose
    /// absence made a global queue wrong.
    #[test]
    fn two_queues_do_not_steal_each_others_completions() {
        let first = CompletionQueue::new();
        let second = CompletionQueue::new();

        // Same task id in both, which is exactly what happens when two runtimes
        // each number their tasks from 1.
        first.complete(1, Ok(HostValue::Str("first".into())));
        second.complete(1, Ok(HostValue::Str("second".into())));

        let payload = |q: &CompletionQueue| match q.take().unwrap() {
            HostTask::Settlement(c) => c.result.unwrap(),
            other => panic!("unexpected {other:?}"),
        };
        assert_eq!(payload(&first), HostValue::Str("first".into()));
        assert_eq!(payload(&second), HostValue::Str("second".into()));
        assert!(first.take().is_none());
        assert!(second.take().is_none());
    }

    fn assert_last_owner_can_drop_from_wake(trigger: impl FnOnce(&RuntimeState) + Send + 'static) {
        let (finished, observed) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let state = Arc::new(RuntimeState::new(Box::new(
                crate::transport::dev_tcp::DevTcpTransport::new(),
            )));
            let owner = Arc::new(Mutex::new(Some(OwnerLease::new(Arc::clone(&state)))));
            let callback_owner = Arc::clone(&owner);
            state.queue.set_wake(Some(Arc::new(move || {
                drop(callback_owner.lock().unwrap().take());
            })));

            trigger(&state);
            finished.send(state.is_shutdown()).unwrap();
        });

        assert_eq!(
            observed.recv_timeout(std::time::Duration::from_secs(2)),
            Ok(true),
            "shutdown deadlocked while the wake callback released its own claim"
        );
    }

    #[test]
    fn wake_can_release_the_last_owner_during_settlement_admission() {
        assert_last_owner_can_drop_from_wake(|state| {
            state
                .queue
                .complete(1, Ok(HostValue::Str("finished".into())));
        });
    }

    #[test]
    fn wake_can_release_the_last_owner_during_timer_admission() {
        assert_last_owner_can_drop_from_wake(|state| {
            state.set_timer(0.0, false);
            assert_eq!(state.admit_due_timers(), 1);
        });
    }

    #[test]
    fn concurrent_publishers_do_not_deadlock_reentrant_owner_shutdown() {
        let state = Arc::new(RuntimeState::new(Box::new(
            crate::transport::dev_tcp::DevTcpTransport::new(),
        )));
        let owner = Arc::new(Mutex::new(Some(OwnerLease::new(Arc::clone(&state)))));
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let (holding_lock, lock_is_held) = std::sync::mpsc::channel();
        let (release_callback, callback_may_finish) = std::sync::mpsc::channel();
        let callback_may_finish = Arc::new(Mutex::new(callback_may_finish));
        let callback_owner = Arc::clone(&owner);
        let callback_calls = Arc::clone(&calls);
        let callback_release = Arc::clone(&callback_may_finish);
        state.queue.set_wake(Some(Arc::new(move || {
            callback_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let mut owner = callback_owner.lock().unwrap();
            holding_lock.send(()).unwrap();
            callback_release.lock().unwrap().recv().unwrap();
            drop(owner.take());
        })));

        let first_state = Arc::clone(&state);
        let (first_finished, first_observed) = std::sync::mpsc::channel();
        let first = std::thread::spawn(move || {
            first_state
                .queue
                .complete(1, Ok(HostValue::Str("first".into())));
            first_finished.send(()).unwrap();
        });
        lock_is_held
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();

        let (second_finished, second_observed) = std::sync::mpsc::channel();
        let second_state = Arc::clone(&state);
        let second = std::thread::spawn(move || {
            second_state
                .queue
                .complete(2, Ok(HostValue::Str("second".into())));
            second_finished.send(()).unwrap();
        });
        let second_result = second_observed.recv_timeout(std::time::Duration::from_secs(2));
        release_callback.send(()).unwrap();
        assert_eq!(
            second_result,
            Ok(()),
            "a publisher blocked behind the in-flight wake callback"
        );
        first_observed
            .recv_timeout(std::time::Duration::from_secs(2))
            .expect("re-entrant last-owner shutdown deadlocked");
        first.join().unwrap();
        second.join().unwrap();
        assert!(state.is_shutdown());
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[test]
    fn burst_admissions_coalesce_without_concurrent_or_lost_wakes() {
        const PUBLISHERS: usize = 32;
        let queue = Arc::new(CompletionQueue::new());
        let active = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let max_active = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let admissions_done = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let woke_after_last_admission = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (first_entered, first_is_blocked) = std::sync::mpsc::channel();
        let (release_first, first_may_return) = std::sync::mpsc::channel();
        let first_may_return = Arc::new(Mutex::new(first_may_return));

        let callback_active = Arc::clone(&active);
        let callback_max = Arc::clone(&max_active);
        let callback_calls = Arc::clone(&calls);
        let callback_done = Arc::clone(&admissions_done);
        let callback_after_last = Arc::clone(&woke_after_last_admission);
        let callback_release = Arc::clone(&first_may_return);
        queue.set_wake(Some(Arc::new(move || {
            let now = callback_active.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
            callback_max.fetch_max(now, std::sync::atomic::Ordering::SeqCst);
            let call = callback_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if callback_done.load(std::sync::atomic::Ordering::SeqCst) {
                callback_after_last.store(true, std::sync::atomic::Ordering::SeqCst);
            }
            if call == 0 {
                first_entered.send(()).unwrap();
                callback_release.lock().unwrap().recv().unwrap();
            }
            callback_active.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        })));

        let first_queue = Arc::clone(&queue);
        let first = std::thread::spawn(move || {
            first_queue.complete(0, Ok(HostValue::Undefined));
        });
        first_is_blocked
            .recv_timeout(std::time::Duration::from_secs(2))
            .expect("first wake did not begin");

        let start = Arc::new(std::sync::Barrier::new(PUBLISHERS + 1));
        let mut publishers = Vec::new();
        for task_id in 1..=PUBLISHERS {
            let queue = Arc::clone(&queue);
            let start = Arc::clone(&start);
            publishers.push(std::thread::spawn(move || {
                start.wait();
                queue.complete(task_id as u64, Ok(HostValue::Undefined));
            }));
        }
        start.wait();
        for publisher in publishers {
            publisher.join().unwrap();
        }
        admissions_done.store(true, std::sync::atomic::Ordering::SeqCst);
        release_first.send(()).unwrap();
        first.join().unwrap();

        assert_eq!(
            max_active.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "wake callbacks ran concurrently"
        );
        assert!(
            woke_after_last_admission.load(std::sync::atomic::Ordering::SeqCst),
            "the final coalesced wake was lost"
        );
        assert!(calls.load(std::sync::atomic::Ordering::SeqCst) >= 2);
    }

    #[test]
    fn completion_sampled_before_shutdown_cannot_publish_after_queue_closure() {
        let state = Arc::new(RuntimeState::new(Box::new(
            crate::transport::dev_tcp::DevTcpTransport::new(),
        )));
        let owner = OwnerLease::new(Arc::clone(&state));
        let wakes = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let observed_wakes = Arc::clone(&wakes);
        state.queue.set_wake(Some(Arc::new(move || {
            observed_wakes.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        })));
        let sampled_then_released = Arc::new(std::sync::Barrier::new(2));
        let worker_barrier = Arc::clone(&sampled_then_released);
        let worker_state = Arc::clone(&state);
        let worker = std::thread::spawn(move || {
            assert!(!worker_state.is_shutdown());
            worker_barrier.wait();
            worker_barrier.wait();
            worker_state
                .queue
                .complete(1, Ok(HostValue::Bytes(vec![1, 2, 3])));
        });

        sampled_then_released.wait();
        drop(owner);
        assert!(state.is_shutdown());
        sampled_then_released.wait();
        worker.join().unwrap();

        assert!(state.queue.is_empty(), "late completion entered the FIFO");
        assert_eq!(wakes.load(std::sync::atomic::Ordering::SeqCst), 0);
    }
}
