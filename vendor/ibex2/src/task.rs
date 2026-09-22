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

use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex};

use crate::boundary::{HostError, HostValue};

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
}

/// Completions waiting for **one** runtime.
///
/// Per-runtime, not process-global. A shared queue lets two runtimes in the
/// same process take each other's completions, and their task ids collide
/// because each numbers its own tasks from 1. That is not a theoretical
/// concern — it showed up the moment two runtimes existed at once.
#[derive(Default)]
pub struct CompletionQueue {
    ready: Mutex<VecDeque<HostTask>>,
    /// Lets an embedder block until there is something to pump instead of
    /// spinning. A runtime that polls in a loop burns a core to do nothing,
    /// which is the default failure mode of this design.
    signal: Condvar,
    wake: Mutex<Option<Arc<dyn Fn() + Send + Sync>>>,
}

impl std::fmt::Debug for CompletionQueue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompletionQueue")
            .field("ready", &self.ready)
            .finish_non_exhaustive()
    }
}

impl CompletionQueue {
    pub fn new() -> Self {
        Self::default()
    }

    /// Schedule the caller's executor when a task arrives. The callback runs
    /// on the publishing thread, without queue locks; it must not enter JSI.
    /// A callback already in flight may finish after replacement.
    pub fn set_wake(&self, wake: Option<Arc<dyn Fn() + Send + Sync>>) {
        *self.wake.lock().expect("completion wake poisoned") = wake;
    }

    /// Publish a settlement. Callable from any thread.
    pub fn complete(&self, task_id: u64, result: Result<HostValue, HostError>) {
        self.admit(HostTask::Settlement(Completion { task_id, result }));
    }

    /// Admit a task to the FIFO. Order of admission is the order of delivery.
    pub fn admit(&self, task: HostTask) {
        self.ready
            .lock()
            .expect("completion queue poisoned")
            .push_back(task);
        self.signal.notify_all();
        let wake = self.wake.lock().expect("completion wake poisoned").clone();
        if let Some(wake) = wake {
            wake();
        }
    }

    /// Take the next admitted task, if any. Called on the JavaScript thread.
    ///
    /// FIFO across BOTH kinds: a timer admitted before a settlement runs before
    /// it. The order an application observes is the order things became ready,
    /// not an artifact of which queue the driver happened to look at first.
    pub fn take(&self) -> Option<HostTask> {
        self.ready
            .lock()
            .expect("completion queue poisoned")
            .pop_front()
    }

    pub fn len(&self) -> usize {
        self.ready.lock().expect("completion queue poisoned").len()
    }

    /// Block until at least one completion is ready, or the timeout elapses.
    ///
    /// This is what the Condvar is for. An embedder that polls in a loop burns
    /// a core to do nothing; one that blocks here wakes exactly when there is
    /// work. Returns whether anything is ready.
    pub fn wait(&self, timeout: std::time::Duration) -> bool {
        let ready = self.ready.lock().expect("completion queue poisoned");
        if !ready.is_empty() {
            return true;
        }
        let (ready, _) = self
            .signal
            .wait_timeout(ready, timeout)
            .expect("completion queue poisoned");
        !ready.is_empty()
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
    #[cfg(all(feature = "hermes", target_os = "linux"))]
    pub(crate) intl: crate::stdlib::intl::Registry,
    #[cfg(all(feature = "hermes", target_os = "linux"))]
    pub(crate) intl_datetime: crate::stdlib::intl_datetime::Registry,
    pub(crate) sqlite: crate::sqlite_abi::Registry,
    app_directories: std::sync::OnceLock<crate::stdlib::app_fs::AppDirectories>,
    responses: Mutex<std::collections::HashMap<u64, Arc<StoredResponse>>>,
    controls: Mutex<std::collections::HashMap<u64, crate::stdlib::abort::AbortController>>,
    shutdown: std::sync::atomic::AtomicBool,
    /// Header lists JavaScript holds by handle, for the same reason responses
    /// are: a header list is not a primitive and §1.1 forbids serializing.
    headers: Mutex<std::collections::HashMap<u64, crate::stdlib::fetch::Headers>>,
    /// The timer wheel (LLP 0059.000 §3.2). Rust owns *when*; the engine keeps
    /// the closures and owns *what*.
    timers: Mutex<crate::stdlib::timers::Timers>,
    /// The runtime's monotonic origin, so `now()` is milliseconds since boot —
    /// the same base `performance.now` will read (§2).
    started: std::time::Instant,
    /// Where modules are loaded from, and what each one may reach.
    loader: Mutex<Option<LoaderConfig>>,
    /// One `Arc` per distinct grant set, for the runtime's life. Two modules
    /// with equal grants get the same pointer, and the engine side keys the
    /// bindings it builds — `fetch`, `fs`, `process` — on that pointer, so
    /// they are built once per grant set rather than once per module. Never
    /// cleared: a set is immutable, and a binding built for it is right for
    /// every module that ever receives it.
    interned_grants:
        Mutex<std::collections::HashMap<crate::grant::GrantSet, Arc<crate::grant::GrantSet>>>,
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
    transport: Box<dyn crate::stdlib::fetch::Transport>,
}

struct StoredResponse {
    response: Mutex<crate::stdlib::fetch::StreamingResponse>,
    control: crate::stdlib::abort::AbortController,
    control_handle: Option<u64>,
}

impl RuntimeState {
    pub fn new(transport: Box<dyn crate::stdlib::fetch::Transport>) -> Self {
        Self {
            queue: CompletionQueue::new(),
            #[cfg(all(feature = "hermes", target_os = "linux"))]
            intl: crate::stdlib::intl::Registry::new(),
            #[cfg(all(feature = "hermes", target_os = "linux"))]
            intl_datetime: crate::stdlib::intl_datetime::Registry::new(),
            sqlite: crate::sqlite_abi::Registry::default(),
            app_directories: std::sync::OnceLock::new(),
            responses: Mutex::new(std::collections::HashMap::new()),
            controls: Mutex::new(std::collections::HashMap::new()),
            shutdown: std::sync::atomic::AtomicBool::new(false),
            headers: Mutex::new(std::collections::HashMap::new()),
            timers: Mutex::new(crate::stdlib::timers::Timers::new()),
            started: std::time::Instant::now(),
            loader: Mutex::new(None),
            interned_grants: Mutex::new(std::collections::HashMap::new()),
            driving: std::sync::atomic::AtomicBool::new(false),
            in_flight: std::sync::atomic::AtomicUsize::new(0),
            next_handle: std::sync::atomic::AtomicU64::new(1),
            transport,
        }
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
    pub fn set_sqlite_provider(
        &self,
        provider: Arc<dyn crate::stdlib::sqlite::Provider>,
    ) -> Result<(), HostError> {
        self.sqlite.set_provider(provider)
    }

    pub fn transport(&self) -> &dyn crate::stdlib::fetch::Transport {
        self.transport.as_ref()
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
        self.sqlite.shutdown();
        self.shutdown
            .store(true, std::sync::atomic::Ordering::Release);
        let controls = std::mem::take(&mut *self.controls.lock().unwrap());
        for control in controls.into_values() {
            control.abort();
        }
        let responses = std::mem::take(&mut *self.responses.lock().unwrap());
        for response in responses.into_values() {
            response.control.abort();
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
}

/// Where the loader reads from, and the authority it hands each module.
#[derive(Debug)]
pub struct LoaderConfig {
    pub root: crate::loader::Root,
    pub grants: crate::loader::ModuleGrants,
    /// Compiles module wrappers ahead of time. Absent means source loading,
    /// which `rules/RULES.md` forbids for anything shippable and which exists
    /// only so a machine without hermesc can still run something.
    pub compiler: Option<crate::bytecode::Compiler>,
    /// Refuse to compile on demand: every module must already be built.
    pub precompiled_only: bool,
    /// Resolved specifier to artifact key, written by the build. When present,
    /// a module is found without reading or hashing its source.
    pub manifest: Option<crate::bytecode::Manifest>,
    /// What resolution asked the filesystem, for this loader's life.
    pub cache: crate::loader::ResolveCache,
    /// The build's artifacts in one file, read once. A key it lacks falls
    /// back to the artifact's own file.
    pub bundle: Option<crate::bytecode::Bundle>,
}

impl RuntimeState {
    pub fn set_loader(&self, config: LoaderConfig) {
        *self.loader.lock().expect("loader poisoned") = Some(config);
    }

    /// Resolve a specifier and produce the module's executable form.
    ///
    /// Resolution refuses anything outside the root before a file is opened, so
    /// a traversal is a loader error rather than a filesystem question. The
    /// second element is Hermes bytecode when a compiler is configured and
    /// wrapped source otherwise; the caller distinguishes them by the HBC
    /// magic rather than by asking.
    pub fn load_module(&self, from: &str, specifier: &str) -> Result<(String, Vec<u8>), String> {
        let guard = self.loader.lock().expect("loader poisoned");
        let config = guard.as_ref().ok_or("no loader configured")?;
        // The build already resolved this edge, if there was a build: no
        // resolver, no directory listing, no realpath. Otherwise resolve.
        let resolved = match config
            .manifest
            .as_ref()
            .and_then(|manifest| manifest.edge(from, specifier))
        {
            Some(resolved) => resolved.to_string(),
            None => crate::loader::resolve_in(&config.cache, &config.root, from, specifier)?,
        };

        // The fast path: the build already said which artifact this is, so the
        // source file is never opened.
        if let (Some(compiler), Some(manifest)) = (&config.compiler, &config.manifest) {
            if let Some(key) = manifest.get(&resolved) {
                if let Some(bytes) = config.bundle.as_ref().and_then(|bundle| bundle.get(key)) {
                    return Ok((resolved, bytes.to_vec()));
                }
                let bytes = compiler
                    .by_key(key)
                    .map_err(|e| format!("{resolved}: {e}"))?;
                return Ok((resolved, bytes));
            }
            if config.precompiled_only {
                return Err(format!("{resolved}: not in the build manifest"));
            }
        }

        #[cfg(not(feature = "loader"))]
        #[allow(clippy::needless_return)]
        {
            return Err(format!(
                "{resolved}: not in the build manifest, and this build has no loader — it runs precompiled artifacts only"
            ));
        }
        #[cfg(feature = "loader")]
        let path = config.root.join(resolved.trim_start_matches("./"));
        #[cfg(feature = "loader")]
        let source = std::fs::read_to_string(&path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        #[cfg(feature = "loader")]
        {
            // The wrapper is built HERE, once, because it is what gets compiled —
            // the artifact is the wrapper, so a second definition of it elsewhere
            // would be a second definition of the thing the cache is keyed on.
            let wrapped = crate::loader::lower_and_wrap(&source, &resolved)?;

            match &config.compiler {
                Some(compiler) => {
                    let bytes = if config.precompiled_only {
                        compiler.cached_only(&wrapped)
                    } else {
                        compiler.compile(&wrapped)
                    }
                    .map_err(|e| format!("{resolved}: {e}"))?;
                    Ok((resolved, bytes))
                }
                None => Ok((resolved, wrapped.into_bytes())),
            }
        }
    }

    /// The authority for one module, as an owned handle the binding keeps.
    pub fn grants_for(&self, specifier: &str) -> Arc<crate::grant::GrantSet> {
        let guard = self.loader.lock().expect("loader poisoned");
        let set = match guard.as_ref() {
            Some(config) => config.grants.for_module(specifier).clone(),
            None => crate::grant::GrantSet::none(),
        };
        let mut interned = self
            .interned_grants
            .lock()
            .expect("interned grants poisoned");
        if let Some(existing) = interned.get(&set) {
            return Arc::clone(existing);
        }
        let shared = Arc::new(set.clone());
        interned.insert(set, Arc::clone(&shared));
        shared
    }
}

/// Create a queue and hand ownership to the caller as a raw pointer.
///
/// # Safety
/// The result must be released exactly once with `ibex2_queue_destroy`.
#[no_mangle]
pub extern "C" fn ibex2_queue_create() -> *const RuntimeState {
    Arc::into_raw(Arc::new(RuntimeState::new(
        crate::transport::default_transport(),
    )))
}

/// # Safety
/// `queue` must come from `ibex2_queue_create` and not have been destroyed.
#[no_mangle]
pub unsafe extern "C" fn ibex2_queue_destroy(queue: *const RuntimeState) {
    if !queue.is_null() {
        let state = Arc::from_raw(queue);
        state.shutdown();
        drop(state);
    }
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
}
