//! The process owns exactly two persistent PNG workers. Waiting work is source
//! identity/metadata; encoded bytes and raster buffers never enter this queue.
use super::{assets::ImageInput, png_decode, remote, svg_picture, Assets, Bitmap};
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
#[cfg(test)]
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
    /// A URL's download: the body a worker reads once it is settled.
    remote: Option<Arc<remote::Load>>,
    /// Its download failed in a way asking again may mend (`Failure::transient`):
    /// not before this (the server's `Retry-After`, if it gave one).
    transient: Mutex<Option<std::time::Instant>>,
    /// Why it failed, its image's `error` (no link's query: `remote::without_queries`).
    message: Mutex<Option<String>>,
}
impl SourceOwner {
    /// The file it reads, when it is one (not an update's bytes): a URL's
    /// is its spool file.
    pub(super) fn file(&self) -> Option<std::path::PathBuf> {
        if let Some(load) = &self.remote {
            return match load.outcome()? {
                Ok(body) => Some(body.path.clone()),
                Err(_) => None,
            };
        }
        match self.assets.upgrade()?.image_input(&self.name)? {
            ImageInput::Path(path) => Some(path),
            ImageInput::Bytes(_) => None,
        }
    }

    /// The asset name it was asked for.
    pub(super) fn name(&self) -> &str {
        &self.name
    }

    /// A URL's body this host cannot decode: let go of its spool file (the
    /// cache's hold too, so the URL is fetched again when next asked for),
    /// and say so once.
    fn release(&self, refusal: Refusal, downloads: &remote::Downloads) {
        let message = format!("not an image this host decodes ({refusal:?})");
        let failure = remote::Failure {
            refusal,
            message: message.clone(),
            transient: false,
            after: None,
        };
        let Some(body) = self.remote.as_ref().and_then(|load| load.release(failure)) else {
            return;
        };
        downloads.forget(&self.name, &body);
        eprintln!(
            "exact: image {}: {message}",
            remote::without_query(&self.name)
        );
        *self.message.lock().unwrap() = Some(message);
    }
}
struct State {
    sources: BTreeMap<u64, Weak<SourceOwner>>,
    next_source: u64,
    next_generation: u64,
}
pub(super) struct Backend {
    pub session: RasterSession,
    /// Where a URL's body comes from.
    pub downloads: Arc<remote::Downloads>,
    /// This backend, for a download to wake its workers when it lands.
    me: Weak<Backend>,
    state: Mutex<State>,
    pub changed: Condvar,
    pub revision: Mutex<u64>,
    requests: Mutex<std::collections::BTreeSet<exact_raster::RequestId>>,
    budget_notice: Mutex<Option<(u64, u64, u64)>>,
    wake_read: Mutex<UnixStream>,
    wake_write: Mutex<UnixStream>,
    /// The device scale each SVG picture was asked at, by source and
    /// pixels: with the key's pixels it gives the CSS size the scene
    /// resolves at (LLP 1011.001 D3), which the key does not carry.
    scales: Mutex<BTreeMap<(u64, exact_raster::PixelSize), f32>>,
    /// SVG pictures with text, resolved on a worker and waiting for the
    /// paint thread ([`Backend::finish_pictures`]).
    pictures: Mutex<Vec<svg_picture::Pending>>,
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
        let backend = Arc::new_cyclic(|me| Self {
            session: workers.gate.session(),
            downloads: remote::Downloads::process().clone(),
            me: me.clone(),
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
            scales: Mutex::new(BTreeMap::new()),
            pictures: Mutex::new(Vec::new()),
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
    /// `urgent`: a URL whose picture fills the screen, downloaded first
    /// (`Downloads::load`).
    pub fn source(
        self: &Arc<Self>,
        generation: u64,
        name: &str,
        assets: &Arc<Assets>,
        urgent: bool,
    ) -> Result<Arc<SourceOwner>, Refusal> {
        Workers::process().start();
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
            // Now one that fills the screen: its download, still waiting, goes first.
            if let Some(load) = source.remote.as_ref().filter(|_| urgent) {
                self.downloads.promote(load);
            }
            Ok(source.clone())
        } else if state.sources.len() >= SUBSCRIPTIONS + COLD_ENTRIES + 2
            || owners
                .iter()
                .filter(|s| {
                    // A download is bounded by the downloads' own admission.
                    !s.remote.as_ref().is_some_and(|load| !load.settled())
                        && matches!(
                            *s.prepared.lock().unwrap(),
                            Prepared::Queued | Prepared::Reading
                        )
                })
                .count()
                >= PENDING_JOBS
        {
            Err(Refusal::QueueFull)
        } else if let Some(next) = state.next_source.checked_add(1) {
            // A URL downloads now, off the workers; one reads its header
            // once it is settled (`prepare`).
            let remote = if remote::is_remote(name) {
                let me = self.me.clone();
                self.downloads
                    .load(name, urgent, move || {
                        if let Some(backend) = me.upgrade() {
                            backend.session.wake();
                        }
                    })
                    .map(Some)
            } else {
                Ok(None)
            };
            remote.map(|remote| {
                let source = Arc::new(SourceOwner {
                    id: state.next_source,
                    generation,
                    name: name.to_owned(),
                    assets: Arc::downgrade(assets),
                    prepared: Mutex::new(Prepared::Queued),
                    remote,
                    transient: Default::default(),
                    message: Default::default(),
                });
                state.next_source = next;
                state.sources.insert(source.id, Arc::downgrade(&source));
                queued = true;
                source
            })
        } else {
            Err(Refusal::Overflow)
        };
        drop(state);
        drop(owners);
        // Header work is outside the decode gate; announce it after insertion.
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
    /// Whether source `id`'s download failed in a way asking again may mend:
    /// the earliest it may be asked again, if so.
    pub fn transient(&self, id: u64) -> Option<std::time::Instant> {
        let source = (self.state.lock().unwrap().sources.get(&id)).and_then(Weak::upgrade)?;
        let after = *source.transient.lock().unwrap();
        after
    }
    /// Why source `id` failed, as its image's `error` says it, if it said more
    /// than its refusal.
    pub fn message(&self, id: u64) -> Option<String> {
        let source = (self.state.lock().unwrap().sources.get(&id)).and_then(Weak::upgrade)?;
        let message = source.message.lock().unwrap().clone();
        message
    }
    /// `source`'s download, if it still waits for a thread, goes first: its
    /// picture now fills the screen ([`remote::Downloads::promote`]).
    pub fn promote(&self, source: &SourceOwner) {
        if let Some(load) = &source.remote {
            self.downloads.promote(load);
        }
    }
    /// Source `id`, failed, is found no more: the next ask for its name
    /// downloads it again (a picture still on screen, `image.rs`).
    pub fn evict(&self, id: u64) {
        self.state.lock().unwrap().sources.remove(&id);
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
        Workers::process().start();
        let id = self.session.request(demand)?;
        self.requests.lock().unwrap().insert(id);
        Ok(id)
    }
    /// The device scale an SVG picture of `source` at `pixels` is asked at.
    pub fn note_scale(&self, source: u64, pixels: exact_raster::PixelSize, scale: f32) {
        let mut scales = self.scales.lock().unwrap();
        if scales.len() >= 4 * (SUBSCRIPTIONS + COLD_ENTRIES) {
            scales.clear();
        }
        scales.insert((source, pixels), scale);
    }
    /// Paint the SVG pictures waiting for the paint thread with `text`,
    /// completing their permits as a worker completes a raster's.
    pub fn finish_pictures(&self, text: &crate::text::Shared) {
        let waiting = std::mem::take(&mut *self.pictures.lock().unwrap());
        if waiting.is_empty() {
            return;
        }
        for pending in waiting {
            svg_picture::finish(text, pending);
        }
        self.notify();
    }
    /// How many pictures wait for the paint thread.
    #[cfg(test)]
    pub fn pictures_waiting(&self) -> usize {
        self.pictures.lock().unwrap().len()
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
            if source.remote.as_ref().is_some_and(|load| !load.settled()) {
                return false;
            }
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
        let result = match source.remote.as_ref().and_then(|load| load.outcome()) {
            // The reason is the image's `error` and the log's, once for the
            // source; neither quotes a link's query (a CDN's signature).
            Some(Err(failure)) => {
                let message = remote::redacted(&failure.message, &source.name);
                eprintln!(
                    "exact: image {}: {message}",
                    remote::without_query(&source.name)
                );
                *source.message.lock().unwrap() = Some(message);
                *source.transient.lock().unwrap() = failure
                    .transient
                    .then(|| failure.after.unwrap_or_else(std::time::Instant::now));
                Err(failure.refusal)
            }
            _ => open(source)
                .and_then(|(mut input, bytes)| png_decode::inspect(&mut input, bytes))
                .inspect_err(|e| source.release(*e, &self.downloads)),
        };
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
        let mut later = None;
        let decoded_source = source.clone();
        let result = source.ok_or(Refusal::Stale).and_then(|source| {
            if source.generation != key.generation || permit.is_cancelled() {
                return Err(Refusal::Stale);
            }
            let Prepared::Ready(header) = *source.prepared.lock().unwrap() else {
                return Err(Refusal::Stale);
            };
            let plan = png_decode::DecodePlan::of_key(header, &key)?;
            if plan.cost != permit.cost() || header.metadata != permit.metadata() {
                return Err(Refusal::ConflictingMetadata);
            }
            let (mut input, bytes) = open(&source)?;
            if bytes != header.metadata.encoded_bytes {
                return Err(Refusal::ConflictingMetadata);
            }
            if header.is_svg() {
                // The parse and the scene precede the charge: the picture is
                // allocated by the painter, after it (as `charge` precedes
                // a raster's output).
                let scale = self
                    .scales
                    .lock()
                    .unwrap()
                    .get(&(key.source, key.pixels))
                    .copied()
                    .unwrap_or(1.0);
                let mut text = Vec::with_capacity(bytes as usize);
                input
                    .read_to_end(&mut text)
                    .map_err(|_| Refusal::DecodeFailed)?;
                let decoded = svg_picture::decode(&source.name, &text, &plan, scale, || {
                    permit.is_cancelled()
                })?;
                return match decoded {
                    svg_picture::Decoded::Pixels(pixels) => {
                        let charge = permit.allocation_charge();
                        Ok(Arc::new(Bitmap::from_source(
                            pixels.into(),
                            plan.natural(),
                            plan.crop,
                            charge,
                            source.clone(),
                        )))
                    }
                    svg_picture::Decoded::Later(scene) => {
                        later = Some((scene, scale, plan, source.clone()));
                        Err(Refusal::Paused)
                    }
                };
            }
            // This charge precedes the decoder and output allocations. On an
            // error decode_rows destroys both before the permit can retire.
            let charge = permit.allocation_charge();
            let pixels = super::jpeg_decode::decode_pixels(input, &plan, || permit.is_cancelled())?;
            let bitmap = Arc::new(Bitmap::from_source(
                pixels,
                plan.natural(),
                plan.crop,
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
                let bytes = bitmap.bytes();
                let _ = permit.complete(
                    bitmap,
                    ResidentBytes {
                        output: bytes,
                        copy: 0,
                    },
                );
            }
            Err(Refusal::Paused) if later.is_some() => {
                // A picture with text waits for the paint thread, its permit
                // with it (LLP 1011.001 D3).
                let (scene, scale, plan, source) = later.unwrap();
                self.pictures.lock().unwrap().push(svg_picture::Pending {
                    permit,
                    scene,
                    css: svg_picture::css_size(plan.pixels, scale),
                    scale,
                    natural: plan.natural(),
                    source,
                });
            }
            Err(e) => {
                if e == Refusal::DecodeFailed {
                    if let Some(source) = &decoded_source {
                        source.release(e, &self.downloads);
                    }
                }
                permit.fail()
            }
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
    if let Some(load) = &source.remote {
        let Some(Ok(body)) = load.outcome() else {
            return Err(Refusal::DecodeFailed);
        };
        let file = crate::file::open_regular(&body.path).map_err(|_| Refusal::DecodeFailed)?;
        if file.metadata().map_err(|_| Refusal::DecodeFailed)?.len() != body.len {
            return Err(Refusal::DecodeFailed);
        }
        return Ok((Box::new(file), body.len));
    }
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
            Arc::new(Self {
                gate: Gate::process(),
                backends: Mutex::new(Vec::new()),
                cursor: Mutex::new(0),
            })
        })
    }
    /// Start the process's two decoders, once: at the first picture source
    /// or request, so an app that shows no picture carries no decoder.
    fn start(self: &Arc<Self>) {
        static STARTED: std::sync::Once = std::sync::Once::new();
        STARTED.call_once(|| {
            for n in 0..2 {
                let owner = self.clone();
                std::thread::Builder::new()
                    .name(format!("exact-png-{n}"))
                    .spawn(move || {
                        #[cfg(target_os = "android")]
                        crate::android::background_priority();
                        owner.run()
                    })
                    .expect("PNG worker");
            }
        });
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
        // Include work announced during either scan in the eventual wait.
        let observed = self.gate.wake_sequence();
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
        // Requests, cancellation, freed budget and source metadata all notify
        // this gate. An idle decoder needs no periodic wake.
        if let Some(permit) = self.gate.wait_work_since(observed, None) {
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
            let owner = backend
                .source(generation, &n.to_string(), &assets, false)
                .unwrap();
            *owner.prepared.lock().unwrap() = Prepared::Failed(Refusal::DecodeFailed);
            owners.push(owner);
        }
        let next = backend.state.lock().unwrap().next_source;
        assert!(matches!(
            backend.source(generation, "overflow", &assets, false),
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
        let replacement = backend
            .source(generation, "replacement", &assets, false)
            .unwrap();
        assert_eq!(replacement.id, next);
        assert_eq!(backend.sources(), cap);
    }
}

#[cfg(test)]
#[path = "workers/fairness_tests.rs"]
mod fairness_tests;
