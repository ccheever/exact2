//! The process owns exactly two persistent PNG workers. Waiting work is source
//! identity/metadata; encoded bytes and raster buffers never enter this queue.
use super::{assets::ImageInput, png_decode, Assets, Bitmap};
use crate::wake::Stream as UnixStream;
use exact_raster::{
    DecodePermit, Gate, RasterSession, Refusal, ResidentBytes, COLD_ENTRIES, PENDING_JOBS,
    SUBSCRIPTIONS,
};
use std::collections::BTreeMap;
use std::io::{Cursor, Read, Seek, Write};
#[cfg(unix)]
use std::os::fd::{AsRawFd, RawFd};
use std::sync::{Arc, Condvar, Mutex, OnceLock, Weak};
use std::time::Duration;

#[derive(Clone, Copy, Debug)]
pub(super) enum Prepared {
    Queued,
    Reading,
    Ready(png_decode::Header),
    Failed(Refusal),
}
pub(super) struct SourceOwner {
    pub id: u64,
    generation: u64,
    name: String,
    assets: Weak<Assets>,
    prepared: Mutex<Prepared>,
}
impl SourceOwner {
    /// The file it reads, when it is one (not an update's bytes).
    pub(super) fn file(&self) -> Option<std::path::PathBuf> {
        match self.assets.upgrade()?.image_input(&self.name)? {
            ImageInput::Path(path) => Some(path),
            ImageInput::Bytes(_) => None,
        }
    }

    /// The asset name it was asked for.
    pub(super) fn name(&self) -> &str {
        &self.name
    }
}
struct State {
    sources: BTreeMap<u64, Weak<SourceOwner>>,
    next_source: u64,
    next_generation: u64,
}
pub(super) struct Backend {
    pub session: RasterSession,
    state: Mutex<State>,
    pub changed: Condvar,
    pub revision: Mutex<u64>,
    requests: Mutex<std::collections::BTreeSet<exact_raster::RequestId>>,
    budget_notice: Mutex<Option<(u64, u64, u64)>>,
    wake_read: Mutex<UnixStream>,
    wake_write: Mutex<UnixStream>,
    #[cfg(test)]
    pub hook: Mutex<Option<Arc<DecodeHook>>>,
}
#[cfg(test)]
type DecodeHook = dyn Fn(&str, &Arc<Bitmap>) + Send + Sync;
impl Backend {
    pub fn new() -> Arc<Self> {
        Self::for_workers(Workers::process())
    }
    fn for_workers(workers: &Workers) -> Arc<Self> {
        let (wake_read, wake_write) = UnixStream::pair().expect("raster wake pair");
        wake_read.set_nonblocking(true).unwrap();
        wake_write.set_nonblocking(true).unwrap();
        let backend = Arc::new(Self {
            session: workers.gate.session(),
            state: Mutex::new(State {
                sources: BTreeMap::new(),
                next_source: 1,
                next_generation: 1,
            }),
            changed: Condvar::new(),
            revision: Mutex::new(0),
            requests: Mutex::new(Default::default()),
            budget_notice: Mutex::new(None),
            wake_read: Mutex::new(wake_read),
            wake_write: Mutex::new(wake_write),
            #[cfg(test)]
            hook: Mutex::new(None),
        });
        workers
            .backends
            .lock()
            .unwrap()
            .push(Arc::downgrade(&backend));
        backend
    }
    pub fn generation(&self) -> u64 {
        let mut state = self.state.lock().unwrap();
        let generation = state.next_generation;
        state.next_generation = generation
            .checked_add(1)
            .expect("raster generation exhausted");
        generation
    }
    pub fn source(
        &self,
        generation: u64,
        name: &str,
        assets: &Arc<Assets>,
    ) -> Result<Arc<SourceOwner>, Refusal> {
        let mut state = self.state.lock().unwrap();
        let mut queued = false;
        state.sources.retain(|_, source| source.strong_count() > 0);
        // Keep upgraded owners until AFTER releasing the index lock: their
        // asset descriptor may have an arbitrary final destructor.
        let owners: Vec<_> = state.sources.values().filter_map(Weak::upgrade).collect();
        let result = if let Some(source) = owners
            .iter()
            .find(|s| s.generation == generation && s.name == name)
        {
            Ok(source.clone())
        } else if state.sources.len() >= SUBSCRIPTIONS + COLD_ENTRIES + 2
            || owners
                .iter()
                .filter(|s| {
                    matches!(
                        *s.prepared.lock().unwrap(),
                        Prepared::Queued | Prepared::Reading
                    )
                })
                .count()
                >= PENDING_JOBS
        {
            Err(Refusal::QueueFull)
        } else if let Some(next) = state.next_source.checked_add(1) {
            let source = Arc::new(SourceOwner {
                id: state.next_source,
                generation,
                name: name.to_owned(),
                assets: Arc::downgrade(assets),
                prepared: Mutex::new(Prepared::Queued),
            });
            state.next_source = next;
            state.sources.insert(source.id, Arc::downgrade(&source));
            queued = true;
            Ok(source)
        } else {
            Err(Refusal::Overflow)
        };
        drop(state);
        drop(owners);
        // A header to read: a worker reads it now, not at its backstop timeout.
        if queued {
            self.session.wake();
        }
        result
    }
    fn owners(&self) -> Vec<Arc<SourceOwner>> {
        let mut state = self.state.lock().unwrap();
        state.sources.retain(|_, source| source.strong_count() > 0);
        state.sources.values().filter_map(Weak::upgrade).collect()
    }
    pub fn prepared(&self, id: u64) -> Option<Prepared> {
        let source = self
            .state
            .lock()
            .unwrap()
            .sources
            .get(&id)
            .and_then(Weak::upgrade)?;
        let prepared = *source.prepared.lock().unwrap();
        Some(prepared)
    }
    pub fn retire(&self, generation: u64) {
        self.session.retire_generation(generation);
        // Weak identities survive while an old provider still owns its pixels.
        self.sources();
    }
    pub fn sources(&self) -> usize {
        let mut state = self.state.lock().unwrap();
        state.sources.retain(|_, source| source.strong_count() > 0);
        state.sources.len()
    }
    pub fn request(
        &self,
        demand: exact_raster::Demand,
    ) -> Result<exact_raster::RequestId, Refusal> {
        let id = self.session.request(demand)?;
        self.requests.lock().unwrap().insert(id);
        Ok(id)
    }
    pub fn forget(&self, id: exact_raster::RequestId) {
        self.requests.lock().unwrap().remove(&id);
    }
    pub fn cancel(&self, id: exact_raster::RequestId) {
        self.forget(id);
        self.session.cancel(id);
    }
    #[cfg(unix)]
    pub fn wake_fd(&self) -> RawFd {
        self.wake_read.lock().unwrap().as_raw_fd()
    }
    pub fn drain_wake(&self) {
        let mut bytes = [0; 128];
        let mut input = self.wake_read.lock().unwrap();
        while input.read(&mut bytes).is_ok_and(|n| n > 0) {}
    }
    fn notify(&self) {
        *self.revision.lock().unwrap() += 1;
        self.changed.notify_all();
        let _ = self.wake_write.lock().unwrap().write(&[1]);
    }
    fn notify_budget_wait(&self) {
        let mut requests = self.requests.lock().unwrap();
        requests.retain(|id| self.session.status(*id).is_some());
        let id = requests.iter().find(|id| {
            self.session.status(**id) == Some(exact_raster::RequestStatus::WaitingBudget)
        });
        let stats = self.session.stats();
        let signature = id.map(|id| (id.get(), stats.resident_bytes, stats.reserved_bytes));
        let mut previous = self.budget_notice.lock().unwrap();
        if *previous != signature {
            *previous = signature;
            if signature.is_some() {
                self.notify();
            }
        }
    }
    fn prepare(&self) -> bool {
        let owners = self.owners();
        let source = owners.iter().find(|source| {
            let mut prepared = source.prepared.lock().unwrap();
            if !matches!(*prepared, Prepared::Queued) {
                return false;
            }
            *prepared = Prepared::Reading;
            true
        });
        let Some(source) = source else {
            return false;
        };
        let result =
            open(source).and_then(|(mut input, bytes)| png_decode::inspect(&mut input, bytes));
        *source.prepared.lock().unwrap() = match result {
            Ok(header) => Prepared::Ready(header),
            Err(e) => Prepared::Failed(e),
        };
        self.notify();
        true
    }
    fn decode(&self, permit: DecodePermit) {
        let key = permit.key();
        let source = self
            .state
            .lock()
            .unwrap()
            .sources
            .get(&key.source)
            .and_then(Weak::upgrade);
        let result = source.ok_or(Refusal::Stale).and_then(|source| {
            if source.generation != key.generation || permit.is_cancelled() {
                return Err(Refusal::Stale);
            }
            let Prepared::Ready(header) = *source.prepared.lock().unwrap() else {
                return Err(Refusal::Stale);
            };
            let plan = png_decode::DecodePlan::new(header, (key.pixels.width, key.pixels.height))?;
            if plan.cost != permit.cost() || header.metadata != permit.metadata() {
                return Err(Refusal::ConflictingMetadata);
            }
            let (input, bytes) = open(&source)?;
            if bytes != header.metadata.encoded_bytes {
                return Err(Refusal::ConflictingMetadata);
            }
            // This charge precedes the decoder and output allocations. On an
            // error decode_rows destroys both before the permit can retire.
            let charge = permit.allocation_charge();
            let pixels = png_decode::decode_rows(input, &plan, || permit.is_cancelled())?;
            let bitmap = Arc::new(Bitmap::from_source(
                pixels,
                plan.natural(),
                charge,
                source.clone(),
            ));
            #[cfg(test)]
            {
                let hook = self.hook.lock().unwrap().clone();
                if let Some(hook) = hook {
                    hook(&source.name, &bitmap);
                }
            }
            Ok(bitmap)
        });
        match result {
            Ok(bitmap) => {
                let bytes = bitmap.as_ref().as_ref().len() as u64;
                let _ = permit.complete(
                    bitmap,
                    ResidentBytes {
                        output: bytes,
                        copy: 0,
                    },
                );
            }
            Err(_) => permit.fail(),
        }
        self.notify();
    }
}
impl Drop for Backend {
    fn drop(&mut self) {
        self.session.shutdown();
    }
}

trait Input: Read + Seek + Send {}
impl<T: Read + Seek + Send> Input for T {}
fn open(source: &SourceOwner) -> Result<(Box<dyn Input>, u64), Refusal> {
    let assets = source.assets.upgrade().ok_or(Refusal::Stale)?;
    match assets
        .image_input(&source.name)
        .ok_or(Refusal::DecodeFailed)?
    {
        ImageInput::Bytes(bytes) => {
            let len = bytes.len() as u64;
            if len > exact_raster::MAX_ENCODED_BYTES {
                return Err(Refusal::EncodedLimit);
            }
            Ok((Box::new(Cursor::new(bytes)), len))
        }
        ImageInput::Path(path) => {
            let file = crate::file::open_regular(&path).map_err(|_| Refusal::DecodeFailed)?;
            let metadata = file.metadata().map_err(|_| Refusal::DecodeFailed)?;
            if !metadata.is_file() {
                return Err(Refusal::DecodeFailed);
            }
            if metadata.len() > exact_raster::MAX_ENCODED_BYTES {
                return Err(Refusal::EncodedLimit);
            }
            Ok((Box::new(file), metadata.len()))
        }
    }
}

struct Workers {
    gate: Gate,
    backends: Mutex<Vec<Weak<Backend>>>,
    cursor: Mutex<usize>,
}
impl Workers {
    fn process() -> &'static Arc<Self> {
        static WORKERS: OnceLock<Arc<Workers>> = OnceLock::new();
        WORKERS.get_or_init(|| {
            let workers = Arc::new(Self {
                gate: Gate::process(),
                backends: Mutex::new(Vec::new()),
                cursor: Mutex::new(0),
            });
            for n in 0..2 {
                let owner = workers.clone();
                std::thread::Builder::new()
                    .name(format!("exact-png-{n}"))
                    .spawn(move || {
                        #[cfg(target_os = "android")]
                        crate::android::background_priority();
                        owner.run()
                    })
                    .expect("PNG worker");
            }
            workers
        })
    }
    fn sessions(&self) -> Vec<Arc<Backend>> {
        let mut backends = self.backends.lock().unwrap();
        backends.retain(|weak| weak.strong_count() > 0);
        backends.iter().filter_map(Weak::upgrade).collect()
    }
    fn complete(&self, permit: DecodePermit) {
        if let Some(backend) = self
            .sessions()
            .into_iter()
            .find(|b| b.session.id() == permit.session_id())
        {
            backend.decode(permit);
        } else {
            permit.fail();
        }
    }
    fn run(&self) {
        let mut metadata_turn = false;
        loop {
            self.turn(&mut metadata_turn, || {});
        }
    }
    // A single production scheduling turn. The callback is a deterministic test
    // seam for work arriving after the empty metadata scan and before waiting.
    fn turn(&self, metadata_turn: &mut bool, before_wait: impl FnOnce()) {
        // One metadata turn between decode turns, even under continuous
        // known-source pressure. A busy metadata queue also cannot starve decode.
        if !*metadata_turn {
            *metadata_turn = true;
            if let Some(permit) = self.gate.next_decode() {
                self.complete(permit);
                return;
            }
        }
        *metadata_turn = false;
        let backends = self.sessions();
        for backend in &backends {
            backend.notify_budget_wait();
        }
        let start = {
            let mut at = self.cursor.lock().unwrap();
            *at = at.wrapping_add(1);
            *at
        };
        if !backends.is_empty()
            && (0..backends.len()).any(|i| backends[(start + i) % backends.len()].prepare())
        {
            return;
        }
        drop(backends);
        before_wait();
        // Requests, cancels, freed budget and sources to read all wake a
        // worker, which takes the next turn at once (a header to read is no
        // decode: waiting on for one held every first picture to the
        // timeout); the timeout is only a backstop, so it is long enough not
        // to be a poll.
        if let Some(permit) = self.gate.wait_work(Duration::from_millis(200)) {
            *metadata_turn = true;
            self.complete(permit);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_live_source_cap_refuses_atomically_and_dead_indices_are_reclaimed() {
        let backend = Backend::new();
        let assets = Arc::new(Assets::embedded(Default::default()));
        let generation = backend.generation();
        let cap = SUBSCRIPTIONS + COLD_ENTRIES + 2;
        let mut owners = Vec::new();
        for n in 0..cap {
            let owner = backend.source(generation, &n.to_string(), &assets).unwrap();
            *owner.prepared.lock().unwrap() = Prepared::Failed(Refusal::DecodeFailed);
            owners.push(owner);
        }
        let next = backend.state.lock().unwrap().next_source;
        assert!(matches!(
            backend.source(generation, "overflow", &assets),
            Err(Refusal::QueueFull)
        ));
        assert_eq!(backend.sources(), cap);
        assert_eq!(backend.state.lock().unwrap().next_source, next);
        drop(owners.pop());
        // A metadata scan can briefly own the dropped source. Let it finish.
        let start = std::time::Instant::now();
        while backend.sources() == cap {
            assert!(
                start.elapsed() < Duration::from_secs(60),
                "hang, not a budget"
            );
            std::thread::yield_now();
        }
        let replacement = backend.source(generation, "replacement", &assets).unwrap();
        assert_eq!(replacement.id, next);
        assert_eq!(backend.sources(), cap);
    }
}

#[cfg(test)]
#[path = "workers/fairness_tests.rs"]
mod fairness_tests;
