//! Optional app storage executor. The runner and portable Rust module own no I/O.
//! @ref LLP 1027.001 D2 — requests travel as values; native work stays on workers.
use exact_plan::{Plan, Value};
use exact_runner::{
    Answer, DataError, DataSource, Dispatch, InFlight, Interrupt, Outcome, Placement, Store, Target,
};
use std::{
    collections::{BTreeMap, HashSet},
    path::PathBuf,
};
#[cfg(not(target_arch = "wasm32"))]
#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;
#[cfg(all(test, windows))]
mod windows_native_tests;

/// Host-configured app directories; recorded without opening them.
#[derive(Clone)]
pub struct Directories {
    /// Durable app data.
    pub data: PathBuf,
    /// Evictable cache.
    pub cache: PathBuf,
    /// App temporary files.
    pub temporary: PathBuf,
}

enum Pending {
    Child(u64),
    #[cfg(not(target_arch = "wasm32"))]
    Storage(Vec<u8>, String),
    /// A send's storage request made before activation (a document the
    /// host hands over at launch, before first pixel: studio diary R14),
    /// with the grants it asked under: checked and run once storage is
    /// ready, never refused for being early.
    #[cfg(not(target_arch = "wasm32"))]
    Early(Vec<u8>, Option<String>),
}

/// A source with optional host-owned storage. Native storage uses the existing
/// host worker. On the web the dedicated request is executed by browser storage.
pub struct Storage<D> {
    source: D,
    directories: Option<Directories>,
    active: bool,
    effects: bool,
    next: u64,
    pending: BTreeMap<u64, Pending>,
    alive: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// A scripted drive (`EXACT_AGENT=1`): storage is the scratch store it
    /// names, else none.
    agent: bool,
    /// Early requests dispatch held until activation, in order.
    held: Vec<u64>,
}
impl<D> Storage<D> {
    /// Wrap a source without creating storage or starting any thread.
    pub fn new(source: D) -> Self {
        Self {
            source,
            directories: None,
            active: false,
            effects: false,
            next: 0,
            pending: BTreeMap::new(),
            alive: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true)),
            agent: std::env::var("EXACT_AGENT").as_deref() == Ok("1"),
            held: Vec::new(),
        }
    }
}
impl<D: Default> Default for Storage<D> {
    fn default() -> Self {
        Self::new(D::default())
    }
}
impl<D: DataSource> Storage<D> {
    fn step(
        &mut self,
        store: &mut Store,
        result: Result<Answer, DataError>,
    ) -> Result<Answer, DataError> {
        self.step_for(None, store, result)
    }

    fn step_for(
        &mut self,
        target: Option<Target>,
        store: &mut Store,
        result: Result<Answer, DataError>,
    ) -> Result<Answer, DataError> {
        // The web's storage is the browser's, ready before any send.
        #[cfg(target_arch = "wasm32")]
        let _ = target;
        let mut answer = result?;
        if let Answer::Later(request) = &mut answer {
            if request.http != exact_runner::HttpScheduling::Ordered
                && (request.storage.is_some() || request.continuation.is_some())
            {
                return Err(unavailable("independent scheduling is HTTP-only"));
            }
            if let Some(payload) = &request.storage {
                store.observe_external_read();
                // A send before activation waits for storage (studio diary
                // R14: the document a launch hands over, sent to the data
                // module before first pixel, was refused and lost); a
                // resource is asked again at `data_ready` instead.
                #[cfg(not(target_arch = "wasm32"))]
                if !self.active && matches!(target, Some(Target::Mutation(_))) {
                    if payload.len() > exact_data::storage::MAX_BYTES {
                        return Err(unavailable("storage request exceeds its byte limit"));
                    }
                    self.next = self
                        .next
                        .checked_add(1)
                        .ok_or_else(|| unavailable("storage token space exhausted"))?;
                    self.pending.insert(
                        self.next,
                        Pending::Early(payload.clone(), request.grants.clone()),
                    );
                    *request = exact_runner::Request::continuation(self.next);
                    return Ok(answer);
                }
                if !self.effects {
                    return Err(unavailable(
                        "storage is unavailable during bake or validation",
                    ));
                }
                exact_data::storage::scope(self.grants(), request.grants.as_deref())
                    .map_err(unavailable)?;
                if payload.len() > exact_data::storage::MAX_BYTES {
                    return Err(unavailable("storage request exceeds its byte limit"));
                }
                std::str::from_utf8(payload)
                    .map_err(|_| unavailable("storage request must be UTF-8"))?;
                #[cfg(not(target_arch = "wasm32"))]
                {
                    // A chosen document or a Windows disk path is not app
                    // storage: it needs no app directories (LLP 1069.010 D1).
                    // A drive that names no scratch store has no directories
                    // either: its request is answered with the web's refusal,
                    // which the module can handle (`native::agent_refusal`).
                    if self.directories.is_none()
                        && !native::independent_storage(payload)
                        && !self.agent
                    {
                        return Err(unavailable(
                            "storage is unavailable in an unconfigured host",
                        ));
                    }
                    let scope = request
                        .grants
                        .clone()
                        .unwrap_or_else(|| self.grants().into());
                    self.next = self
                        .next
                        .checked_add(1)
                        .ok_or_else(|| unavailable("storage token space exhausted"))?;
                    self.pending
                        .insert(self.next, Pending::Storage(payload.clone(), scope));
                    *request = exact_runner::Request::continuation(self.next);
                    return Ok(answer);
                }
            }
            if let Some(token) = request.continuation {
                self.next = self
                    .next
                    .checked_add(1)
                    .ok_or_else(|| unavailable("storage token space exhausted"))?;
                self.pending.insert(self.next, Pending::Child(token));
                request.continuation = Some(self.next);
            }
        }
        Ok(answer)
    }
}
fn unavailable(s: impl Into<String>) -> DataError {
    DataError::Unavailable(s.into())
}
impl<D: DataSource> DataSource for Storage<D> {
    fn preload(&self) -> Result<bool, DataError> {
        self.source.preload()
    }
    fn query(&mut self, name: &str, args: &[Value]) -> Result<Value, DataError> {
        self.source.query(name, args)
    }
    fn answer(
        &mut self,
        store: &mut Store,
        name: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        let answer = self.source.answer(store, name, args);
        self.step(store, answer)
    }
    fn parse(
        &mut self,
        store: &mut Store,
        name: &str,
        args: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        let answer = self.source.parse(store, name, args, outcome);
        self.step(store, answer)
    }
    fn answer_for(
        &mut self,
        target: Target,
        store: &mut Store,
        name: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        let answer = self.source.answer_for(target, store, name, args);
        self.step_for(Some(target), store, answer)
    }
    fn parse_for(
        &mut self,
        target: Target,
        store: &mut Store,
        name: &str,
        args: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        let answer = self.source.parse_for(target, store, name, args, outcome);
        self.step_for(Some(target), store, answer)
    }
    fn app_id(&self) -> &str {
        self.source.app_id()
    }
    fn grants(&self) -> &str {
        self.source.grants()
    }
    fn revision(&self) -> Option<&str> {
        self.source.revision()
    }
    fn bind(&mut self, plan: &Plan) {
        self.source.bind(plan)
    }
    fn adopt(&mut self, source: &str, args: &[Value], value: &Value) {
        self.source.adopt(source, args, value)
    }
    fn ready(&self) -> bool {
        self.active && self.source.ready()
    }
    fn canvas_surfaces(&self) -> Vec<(String, usize)> {
        self.source.canvas_surfaces()
    }
    fn draw(
        &mut self,
        request: &exact_runner::DrawRequest<'_>,
        ctx: &exact_runner::exact_canvas::Context2d,
    ) -> exact_runner::Drawn {
        self.source.draw(request, ctx)
    }
    fn canvases_retired(&mut self, retired: &[(u64, u32)]) {
        self.source.canvases_retired(retired)
    }
    fn configure_storage(
        &mut self,
        data: PathBuf,
        cache: PathBuf,
        temporary: PathBuf,
    ) -> Result<(), DataError> {
        if self.active {
            return Err(unavailable("configure storage before activation"));
        }
        self.source
            .configure_storage(data.clone(), cache.clone(), temporary.clone())?;
        self.directories = Some(Directories {
            data,
            cache,
            temporary,
        });
        Ok(())
    }
    fn activate(&mut self) -> Result<(), DataError> {
        self.source.activate()?;
        self.active = true;
        self.effects = true;
        Ok(())
    }
    fn activate_for_validation(&mut self) -> Result<(), DataError> {
        self.source.activate_for_validation()?;
        self.directories = None;
        self.active = true;
        self.effects = false;
        Ok(())
    }
    fn replacement(&self, plan: &[u8], receipt: &str, module: Vec<u8>) -> Result<Self, DataError> {
        let mut next = Self::new(self.source.replacement(plan, receipt, module)?);
        next.directories = self.directories.clone();
        next.agent = self.agent;
        Ok(next)
    }
    fn placement(&self) -> Placement {
        self.source.placement()
    }
    fn interrupt(&self) -> Option<Interrupt> {
        self.source.interrupt()
    }
    fn native(&self) -> Option<exact_runner::Native> {
        self.source.native()
    }
    /// Continuation tokens are this source's own: each goes back to its
    /// child's where the child handed one out (a storage request's never
    /// did, and one already dispatched can't be told any more), and an entry
    /// the runner no longer has in flight is let go.
    fn forgotten(&mut self, in_flight: &[InFlight<'_>]) {
        let view: Vec<InFlight<'_>> = in_flight
            .iter()
            .map(|f| InFlight {
                continuation: f
                    .continuation
                    .and_then(|outer| match self.pending.get(&outer) {
                        Some(Pending::Child(child)) => Some(*child),
                        _ => None,
                    }),
                ..*f
            })
            .collect();
        let tokens: HashSet<u64> = in_flight.iter().filter_map(|f| f.continuation).collect();
        self.pending.retain(|outer, _| tokens.contains(outer));
        let pending = &self.pending;
        self.held.retain(|t| pending.contains_key(t));
        self.source.forgotten(&view);
    }

    fn dispatch(&mut self, token: u64, store: &Store) -> Dispatch {
        match self.pending.get(&token) {
            Some(Pending::Child(child)) => {
                let child = *child;
                let dispatch = self.source.dispatch(child, store);
                if !matches!(dispatch, Dispatch::Held) {
                    self.pending.remove(&token);
                }
                dispatch
            }
            #[cfg(not(target_arch = "wasm32"))]
            Some(Pending::Early(..)) if !self.effects => {
                self.held.push(token);
                Dispatch::Held
            }
            #[cfg(not(target_arch = "wasm32"))]
            Some(Pending::Storage(..) | Pending::Early(..)) => match self.continuation(token) {
                Some(work) => Dispatch::Run(exact_runner::Work::Now(work)),
                None => Dispatch::Missing,
            },
            None => Dispatch::Missing,
        }
    }

    fn release(&mut self, store: &Store) -> Vec<(u64, Dispatch)> {
        let mut released = Vec::new();
        // Early sends, in order, once storage is ready.
        #[cfg(not(target_arch = "wasm32"))]
        if self.effects {
            for token in std::mem::take(&mut self.held) {
                if let Some(work) = self.continuation(token) {
                    released.push((token, Dispatch::Run(exact_runner::Work::Now(work))));
                }
            }
        }
        for (child, dispatch) in self.source.release(store) {
            let outer = self
                .pending
                .iter()
                .find(|(_, p)| matches!(p, Pending::Child(c) if *c == child))
                .map(|(outer, _)| *outer);
            let Some(outer) = outer else {
                continue;
            };
            if !matches!(dispatch, Dispatch::Held) {
                self.pending.remove(&outer);
            }
            released.push((outer, dispatch));
        }
        released
    }

    fn discard(&mut self, token: u64) {
        if let Some(Pending::Child(child)) = self.pending.remove(&token) {
            self.source.discard(child);
        }
    }

    fn continuation(&mut self, token: u64) -> Option<Box<dyn FnOnce() -> Outcome + Send>> {
        match self.pending.remove(&token)? {
            Pending::Child(token) => self.source.continuation(token),
            #[cfg(not(target_arch = "wasm32"))]
            Pending::Early(payload, asked) => {
                // The checks a send made after activation meets at once.
                let scope = exact_data::storage::scope(self.grants(), asked.as_deref()).map(|_| ());
                let configured = self.directories.is_some()
                    || native::independent_storage(&payload)
                    || self.agent;
                let refusal = match (scope, std::str::from_utf8(&payload).is_ok()) {
                    (Err(e), _) => Some((exact_runner::FailureKind::Refused, e)),
                    (_, false) => Some((
                        exact_runner::FailureKind::Refused,
                        "storage request must be UTF-8".into(),
                    )),
                    (Ok(()), true) if !configured => Some((
                        exact_runner::FailureKind::Unsupported,
                        "storage is unavailable in an unconfigured host".into(),
                    )),
                    (Ok(()), true) => {
                        let grants = asked.unwrap_or_else(|| self.grants().into());
                        self.pending
                            .insert(token, Pending::Storage(payload, grants));
                        return self.continuation(token);
                    }
                };
                let (kind, message) = refusal?;
                Some(Box::new(move || Outcome::Failed { kind, message }))
            }
            #[cfg(not(target_arch = "wasm32"))]
            Pending::Storage(payload, grants) => {
                let paths = self.directories.clone();
                let alive = self.alive.clone();
                let refused =
                    paths.is_none() && self.agent && !native::independent_storage(&payload);
                Some(Box::new(move || {
                    if !alive.load(std::sync::atomic::Ordering::Acquire) {
                        return Outcome::Failed {
                            kind: exact_runner::FailureKind::Aborted,
                            message: "storage source unloaded".into(),
                        };
                    }
                    if refused {
                        return native::agent_refusal();
                    }
                    native::run(paths.as_ref(), &grants, &payload)
                }))
            }
        }
    }
}
impl<D> Drop for Storage<D> {
    fn drop(&mut self) {
        self.alive
            .store(false, std::sync::atomic::Ordering::Release)
    }
}
