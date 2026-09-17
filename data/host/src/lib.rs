//! Optional app storage executor. The runner and portable Rust module own no I/O.
//! @ref LLP 1027.001 D2 — requests travel as values; native work stays on workers.
use exact_plan::{Plan, Value};
use exact_runner::{Answer, DataError, DataSource, Outcome, Store};
use std::{collections::BTreeMap, path::PathBuf};
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
    fn ready(&self) -> bool {
        self.active && self.source.ready()
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
    fn continuation_token(&mut self, token: u64) -> Option<u64> {
        match self.pending.remove(&token)? {
            Pending::Child(token) => self.source.continuation_token(token),
            #[cfg(not(target_arch = "wasm32"))]
            Pending::Storage(..) => None,
        }
    }
}
impl<D> Drop for Storage<D> {
    fn drop(&mut self) {
        self.alive
            .store(false, std::sync::atomic::Ordering::Release)
    }
}
