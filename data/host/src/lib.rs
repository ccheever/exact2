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
mod native;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;

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
        let mut answer = result?;
        if let Answer::Later(request) = &mut answer {
            if request.http != exact_runner::HttpScheduling::Ordered
                && (request.storage.is_some() || request.continuation.is_some())
            {
                return Err(unavailable("independent scheduling is HTTP-only"));
            }
            if let Some(payload) = &request.storage {
                store.observe_external_read();
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
                    if self.directories.is_none() {
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
        self.step(store, answer)
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
        self.step(store, answer)
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
        Ok(next)
    }
    fn placement(&self) -> Placement {
        self.source.placement()
    }
    fn interrupt(&self) -> Option<Interrupt> {
        self.source.interrupt()
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
            Some(Pending::Storage(..)) => match self.continuation(token) {
                Some(work) => Dispatch::Run(exact_runner::Work::Now(work)),
                None => Dispatch::Missing,
            },
            None => Dispatch::Missing,
        }
    }

    fn release(&mut self, store: &Store) -> Vec<(u64, Dispatch)> {
        let mut released = Vec::new();
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
            Pending::Storage(payload, grants) => {
                let paths = self.directories.clone()?;
                let alive = self.alive.clone();
                Some(Box::new(move || {
                    if !alive.load(std::sync::atomic::Ordering::Acquire) {
                        return Outcome::Failed {
                            kind: exact_runner::FailureKind::Aborted,
                            message: "storage source unloaded".into(),
                        };
                    }
                    native::run(&paths, &grants, &payload)
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
