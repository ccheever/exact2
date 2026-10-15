//! Executor-neutral, bounded image work with an observable publication decision.
//! @ref LLP 0068.001#5-work-identity-and-the-cancellation-decision — cancellation never waits for publication.
use super::app_fs::AppDirectories;
use crate::{
    boundary::HostError,
    grant::{GrantSet, Operation},
};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex, Weak,
    },
    time::{Duration, Instant},
};

pub const MAX_SOURCE_BYTES: u64 = 64 * 1024 * 1024;
pub const TRIAL_BUDGET: Duration = Duration::from_secs(20);
/// Retained provider diagnostics cannot make completion records unbounded.
pub const MAX_ERROR_BYTES: usize = 4096;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompressedImage {
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageFile {
    pub width: u32,
    pub height: u32,
    pub size: u64,
}
#[derive(Debug, Clone, Copy)]
pub struct ImageRequest {
    pub max_dimension: u32,
    pub max_bytes: u64,
}
/// Provider declarations used for preallocation checks and Ibex admission.
/// The provider must check decoded pixels before decoding, bound retained encoded
/// output, and account for all explicitly sized decoded/intermediate buffers in
/// `max_scratch_bytes`. Only the borrowed source and returned encoded output are
/// excluded; Ibex charges those separately. For an opaque native codec, scratch
/// is a conservative admission estimate: the provider must document its measured
/// envelope and limitations. Ibex does not enforce a native allocator/RSS cap;
/// an embedder requiring one needs an allocation-controlled or isolated provider.
/// A codec must not retain inputs or its owned workspace after return.
#[derive(Debug, Clone, Copy)]
pub struct CodecLimits {
    pub max_pixels: u64,
    pub max_scratch_bytes: u64,
}
pub trait ImageCodec: Send + Sync {
    fn limits(&self) -> CodecLimits;
    fn encode(
        &self,
        source: &[u8],
        request: ImageRequest,
        control: &WorkControl,
    ) -> Result<CompressedImage, HostError>;
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkLimits {
    pub workers: usize,
    pub completions: usize,
    pub max_job_bytes: u64,
    pub max_output_bytes: u64,
    pub max_dimension: u32,
}
impl Default for WorkLimits {
    fn default() -> Self {
        Self {
            workers: 4,
            completions: 16,
            max_job_bytes: 256 << 20,
            max_output_bytes: 64 << 20,
            max_dimension: 16_384,
        }
    }
}
#[derive(Clone)]
pub struct ImageService {
    pub(crate) codec: Arc<dyn ImageCodec>,
    pub(crate) limits: WorkLimits,
    codec_limits: CodecLimits,
}
impl ImageService {
    pub fn new(codec: Arc<dyn ImageCodec>, limits: WorkLimits) -> Result<Self, HostError> {
        let codec_limits = codec.limits(); // provider callback outside every registry lock
        if limits.workers == 0
            || limits.completions == 0
            || limits.max_dimension == 0
            || limits.max_output_bytes == 0
            || codec_limits.max_pixels == 0
            || codec_limits.max_scratch_bytes == 0
            || MAX_SOURCE_BYTES
                .checked_add(codec_limits.max_scratch_bytes)
                .and_then(|n| n.checked_add(limits.max_output_bytes))
                .is_none_or(|n| n > limits.max_job_bytes)
        {
            return Err(HostError::InvalidArgument(
                "invalid image work limits".into(),
            ));
        }
        Ok(Self {
            codec,
            limits,
            codec_limits,
        })
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WorkId {
    pub generation: u64,
    pub sequence: u64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkState {
    Queued,
    Running,
    CommitStarted,
    Settled,
    Abandoned,
}
pub type WorkOutcome = Result<ImageFile, HostError>;
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CancelResult {
    Abandoned,
    CommitStarted,
    Settled(WorkOutcome),
}
struct Record {
    state: WorkState,
    outcome: Option<WorkOutcome>,
}
struct Work {
    id: WorkId,
    record: Mutex<Record>,
    registry: Weak<Registry>,
}
#[derive(Clone)]
pub struct WorkHandle(Arc<Work>);
impl WorkHandle {
    pub fn id(&self) -> WorkId {
        self.0.id
    }
    pub fn state(&self) -> WorkState {
        self.0
            .record
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .state
    }
    pub fn cancel(&self) -> CancelResult {
        let mut record = self.0.record.lock().unwrap_or_else(|e| e.into_inner());
        match record.state {
            WorkState::Queued | WorkState::Running => {
                record.state = WorkState::Abandoned;
                record.outcome = Some(Err(failed("abandoned")));
                CancelResult::Abandoned
            }
            WorkState::Abandoned => CancelResult::Abandoned,
            WorkState::CommitStarted => CancelResult::CommitStarted,
            WorkState::Settled => {
                CancelResult::Settled(record.outcome.clone().expect("settled outcome"))
            }
        }
    }
    /// Remove the completion charge only when its terminal outcome is collected.
    /// This handle still retains its small history; it cannot cancel any other identity.
    pub fn take_outcome(&self) -> Option<WorkOutcome> {
        let outcome = self
            .0
            .record
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .outcome
            .clone();
        if outcome.is_some() {
            self.discard();
        }
        outcome
    }
    /// Explicitly discard future delivery/history accounting, without claiming abandonment.
    /// A running worker retains its independent worker/memory charge.
    pub fn discard(&self) {
        if let Some(registry) = self.0.registry.upgrade() {
            let mut state = registry.state.lock().unwrap_or_else(|e| e.into_inner());
            let (charged, retired) = match state.work.get_mut(&self.id().sequence) {
                Some(entry) => {
                    let charged = entry.completion;
                    entry.completion = false;
                    (charged, !entry.worker)
                }
                None => (false, false),
            };
            if charged {
                state.completions -= 1;
            }
            let removed = if retired {
                state.work.remove(&self.id().sequence)
            } else {
                None
            };
            drop(state);
            drop(removed);
        }
    }
    fn start(&self) -> bool {
        let mut record = self.0.record.lock().unwrap_or_else(|e| e.into_inner());
        if record.state != WorkState::Queued {
            return false;
        }
        record.state = WorkState::Running;
        true
    }
    fn reserve_commit(&self, deadline: Instant) -> bool {
        let mut record = self.0.record.lock().unwrap_or_else(|e| e.into_inner());
        if record.state != WorkState::Running {
            return false;
        }
        if Instant::now() >= deadline {
            record.state = WorkState::Abandoned;
            record.outcome = Some(Err(failed("timeout: deadline expired before publication")));
            return false;
        }
        record.state = WorkState::CommitStarted;
        true
    }
    fn settle(&self, outcome: WorkOutcome) -> WorkOutcome {
        let mut record = self.0.record.lock().unwrap_or_else(|e| e.into_inner());
        if record.state == WorkState::Abandoned {
            return record.outcome.clone().expect("abandoned outcome");
        }
        record.state = WorkState::Settled;
        record.outcome = Some(outcome.clone());
        outcome
    }
}
struct Registry {
    generation: u64,
    limits: WorkLimits,
    state: Mutex<RegistryState>,
}
struct RegistryState {
    next: u64,
    workers: usize,
    closed: bool,
    completions: usize,
    work: HashMap<u64, Entry>,
}
struct Entry {
    handle: WorkHandle,
    completion: bool,
    worker: bool,
}
static GENERATION: AtomicU64 = AtomicU64::new(1);
#[derive(Clone)]
pub struct ImageWork(Arc<Registry>);
impl ImageWork {
    pub fn new(limits: WorkLimits) -> Self {
        Self(Arc::new(Registry {
            generation: GENERATION
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
                .expect("image owner identity exhausted"),
            limits,
            state: Mutex::new(RegistryState {
                next: 1,
                workers: 0,
                completions: 0,
                closed: false,
                work: HashMap::new(),
            }),
        }))
    }
    pub fn outstanding_workers(&self) -> usize {
        self.0
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .workers
    }
    pub fn pending_completions(&self) -> usize {
        self.0
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .completions
    }
    pub fn handles(&self) -> Vec<WorkHandle> {
        self.0
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .work
            .values()
            .map(|entry| entry.handle.clone())
            .collect()
    }
    pub fn close(&self) {
        let handles = {
            let mut state = self.0.state.lock().unwrap_or_else(|e| e.into_inner());
            state.closed = true;
            state.completions = 0;
            std::mem::take(&mut state.work)
        };
        for (_, entry) in handles {
            entry.handle.cancel();
        }
    }
    fn admit(&self) -> Result<(WorkHandle, Worker), HostError> {
        let mut state = self.0.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.closed
            || state.workers >= self.0.limits.workers
            || state.completions >= self.0.limits.completions
        {
            return Err(failed("work capacity unavailable"));
        }
        let sequence = state.next;
        state.next = state
            .next
            .checked_add(1)
            .ok_or_else(|| failed("work identity exhausted"))?;
        let handle = WorkHandle(Arc::new(Work {
            id: WorkId {
                generation: self.0.generation,
                sequence,
            },
            record: Mutex::new(Record {
                state: WorkState::Queued,
                outcome: None,
            }),
            registry: Arc::downgrade(&self.0),
        }));
        state.workers += 1;
        state.completions += 1;
        state.work.insert(
            sequence,
            Entry {
                handle: handle.clone(),
                completion: true,
                worker: true,
            },
        );
        Ok((handle, Worker(self.0.clone(), sequence)))
    }
}
struct Worker(Arc<Registry>, u64);
impl Drop for Worker {
    fn drop(&mut self) {
        let mut state = self.0.state.lock().unwrap_or_else(|e| e.into_inner());
        state.workers -= 1;
        let discarded = state.work.get_mut(&self.1).is_some_and(|entry| {
            entry.worker = false;
            !entry.completion
        });
        let removed = if discarded {
            state.work.remove(&self.1)
        } else {
            None
        };
        drop(state);
        drop(removed);
    }
}
/// Cooperative provider view; no paths, destination, grants or JSI values.
pub struct WorkControl {
    handle: WorkHandle,
    deadline: Instant,
    limits: CodecLimits,
}
impl WorkControl {
    pub fn deadline(&self) -> Instant {
        self.deadline
    }
    pub fn limits(&self) -> CodecLimits {
        self.limits
    }
    pub fn canceled(&self) -> bool {
        self.handle.state() == WorkState::Abandoned
    }
    /// Call BEFORE starting every decode/encode trial and between long-running chunks.
    pub fn check(&self) -> Result<(), HostError> {
        if self.canceled() {
            Err(failed("work abandoned"))
        } else if Instant::now() >= self.deadline {
            Err(failed("timeout: the trial deadline expired"))
        } else {
            Ok(())
        }
    }
}
/// A bounded admission created before dispatch. Dropping it before execution abandons it.
pub struct ImageJob {
    handle: WorkHandle,
    service: ImageService,
    directories: Arc<AppDirectories>,
    grants: Arc<GrantSet>,
    from: String,
    to: String,
    request: ImageRequest,
    // Last: accounting retires only after provider/leases/owned allocations drop.
    worker: Option<Worker>,
}
fn failed(detail: &str) -> HostError {
    HostError::Failed(format!("compressImage: {detail}"))
}
fn source_error(error: HostError, control: &WorkControl) -> HostError {
    // This exact, internal reader marker is the source quota. Arbitrary I/O or
    // provider text containing these words must retain its original meaning.
    if matches!(&error, HostError::Failed(text)
        if text.strip_prefix("filesystem: ") == Some(crate::stdlib::fs::RESULT_LIMIT))
    {
        failed("too-large: the source is over 64 MiB")
    } else if !control.canceled() && Instant::now() >= control.deadline {
        failed("timeout: the source read ran past the trial deadline")
    } else {
        error
    }
}
fn bounded_error(error: HostError) -> HostError {
    fn bound(mut text: String) -> String {
        let mut end = MAX_ERROR_BYTES.min(text.len());
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
        text.shrink_to_fit();
        text
    }
    match error {
        HostError::Failed(text) => HostError::Failed(bound(text)),
        HostError::InvalidArgument(text) => HostError::InvalidArgument(bound(text)),
        HostError::DeniedRedirect { capability, origin } => HostError::DeniedRedirect {
            capability,
            origin: bound(origin),
        },
        other => other,
    }
}
impl ImageJob {
    #[allow(clippy::too_many_arguments)]
    pub fn prepare(
        work: &ImageWork,
        service: Option<&ImageService>,
        directories: Option<Arc<AppDirectories>>,
        grants: Arc<GrantSet>,
        from: &str,
        to: &str,
        request: ImageRequest,
    ) -> Result<Self, HostError> {
        if from.len() > 32_768 || to.len() > 32_768 {
            return Err(failed("image path exceeds its bound"));
        }
        crate::boundary::admit(&grants, &Operation::FsRead { path: from.into() })?;
        crate::boundary::admit(&grants, &Operation::FsWrite { path: to.into() })?;
        let service = service.ok_or_else(|| failed("unsupported: no image codec"))?;
        if work.0.limits != service.limits {
            return Err(failed("work registry and codec service limits differ"));
        }
        if !from.starts_with("app:/")
            || !to.starts_with("app:/")
            || from.len() > 32_768
            || to.len() > 32_768
        {
            return Err(failed("requires bounded app:/ paths"));
        }
        if request.max_dimension == 0
            || request.max_dimension > service.limits.max_dimension
            || request.max_bytes == 0
            || request.max_bytes > service.limits.max_output_bytes
            || u64::from(request.max_dimension)
                .checked_mul(u64::from(request.max_dimension))
                .is_none()
        {
            return Err(failed("invalid image dimensions or byte bound"));
        }
        let directories =
            directories.ok_or_else(|| failed("app directories are not configured"))?;
        let (handle, worker) = work.admit()?;
        Ok(Self {
            handle,
            worker: Some(worker),
            service: service.clone(),
            directories,
            grants,
            from: from.into(),
            to: to.into(),
            request,
        })
    }
    pub fn handle(&self) -> WorkHandle {
        self.handle.clone()
    }
    pub fn execute(self) -> WorkOutcome {
        self.execute_with_commit_observer(|_| {})
    }
    /// Trusted native instrumentation after the publication decision, before its write.
    /// Runs outside every state lock; cancellation at this point reports CommitStarted.
    pub fn execute_with_commit_observer(self, observer: impl FnOnce(WorkId)) -> WorkOutcome {
        if !self.handle.start() {
            return self.handle.take_snapshot();
        }
        let outcome =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.perform(observer)))
                .unwrap_or_else(|_| Err(failed("image provider or commit observer panicked")));
        self.handle.settle(outcome)
    }
    fn perform(&self, observer: impl FnOnce(WorkId)) -> WorkOutcome {
        let control = WorkControl {
            handle: self.handle.clone(),
            deadline: Instant::now() + TRIAL_BUDGET,
            limits: self.service.codec_limits,
        };
        control.check()?;
        // Pin the destination parent before provider work; no later namespace lookup retargets it.
        let target = self.directories.prepare_atomic(&self.grants, &self.to)?;
        let source = self
            .directories
            .read_bounded(&self.grants, &self.from, MAX_SOURCE_BYTES, &|| {
                control.check().is_err()
            })
            .map_err(|error| source_error(error, &control))?;
        control.check()?;
        let image = self
            .service
            .codec
            .encode(&source, self.request, &control)
            .map_err(bounded_error)?;
        drop(source);
        let pixels = u64::from(image.width)
            .checked_mul(u64::from(image.height))
            .ok_or_else(|| failed("invalid codec dimensions"))?;
        if image.width == 0
            || image.height == 0
            || image.width > self.request.max_dimension
            || image.height > self.request.max_dimension
            || pixels > self.service.codec_limits.max_pixels
            || image.bytes.len() as u64 > self.request.max_bytes
        {
            return Err(failed("codec exceeded its output contract"));
        }
        if !self.handle.reserve_commit(control.deadline) {
            return Err(failed("abandoned before publication"));
        }
        // No state/registry lock spans filesystem calls. A losing cancel immediately sees CommitStarted.
        observer(self.handle.id());
        target.write(&image.bytes)?;
        Ok(ImageFile {
            width: image.width,
            height: image.height,
            size: image.bytes.len() as u64,
        })
    }
}
impl WorkHandle {
    fn take_snapshot(&self) -> WorkOutcome {
        self.0
            .record
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .outcome
            .clone()
            .unwrap_or_else(|| Err(failed("work did not start")))
    }
}
impl Drop for ImageJob {
    fn drop(&mut self) {
        if self.worker.is_some() {
            self.handle.cancel();
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::{
        path::PathBuf,
        sync::{atomic::AtomicUsize, mpsc},
    };
    struct Fixture {
        path: PathBuf,
        dirs: Arc<AppDirectories>,
        grants: Arc<GrantSet>,
    }
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "ibex-image-{}",
                crate::stdlib::crypto::random_uuid().unwrap()
            ));
            for root in ["data", "cache", "tmp"] {
                std::fs::create_dir_all(path.join(root)).unwrap();
            }
            std::fs::write(path.join("tmp/in"), b"new").unwrap();
            std::fs::write(path.join("data/out"), b"old").unwrap();
            let dirs = Arc::new(
                AppDirectories::new(path.join("data"), path.join("cache"), path.join("tmp"))
                    .unwrap(),
            );
            Self {
                path,
                dirs,
                grants: Arc::new(GrantSet::parse("fs.read app:/\nfs.write app:/").unwrap()),
            }
        }
        fn job(&self, work: &ImageWork, service: &ImageService) -> ImageJob {
            ImageJob::prepare(
                work,
                Some(service),
                Some(self.dirs.clone()),
                self.grants.clone(),
                "app:/tmp/in",
                "app:/data/out",
                ImageRequest {
                    max_dimension: 16,
                    max_bytes: 32,
                },
            )
            .unwrap()
        }
        fn output(&self) -> Vec<u8> {
            std::fs::read(self.path.join("data/out")).unwrap()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
    struct Codec<F>(F);
    impl<F: Fn(&[u8], &WorkControl) -> Result<CompressedImage, HostError> + Send + Sync> ImageCodec
        for Codec<F>
    {
        fn limits(&self) -> CodecLimits {
            CodecLimits {
                max_pixels: 1024,
                max_scratch_bytes: 1024,
            }
        }
        fn encode(
            &self,
            source: &[u8],
            _: ImageRequest,
            control: &WorkControl,
        ) -> Result<CompressedImage, HostError> {
            (self.0)(source, control)
        }
    }
    fn encoded(source: &[u8], _: &WorkControl) -> Result<CompressedImage, HostError> {
        Ok(CompressedImage {
            bytes: source.to_vec(),
            width: 2,
            height: 1,
        })
    }
    fn service<
        F: Fn(&[u8], &WorkControl) -> Result<CompressedImage, HostError> + Send + Sync + 'static,
    >(
        codec: F,
        limits: WorkLimits,
    ) -> ImageService {
        ImageService::new(Arc::new(Codec(codec)), limits).unwrap()
    }
    #[test]
    fn oversized_source_reports_image_quota_before_the_codec_and_keeps_the_target() {
        let f = Fixture::new();
        std::fs::File::create(f.path.join("tmp/in"))
            .unwrap()
            .set_len(MAX_SOURCE_BYTES + 1)
            .unwrap();
        let limits = WorkLimits::default();
        let work = ImageWork::new(limits);
        let service = service(|_, _| panic!("over-quota input reached codec"), limits);
        let result = f.job(&work, &service).execute();
        assert_eq!(result, Err(failed("too-large: the source is over 64 MiB")));
        assert_eq!(f.output(), b"old");
        assert_eq!(work.outstanding_workers(), 0);
    }
    #[test]
    fn source_quota_and_deadlines_keep_distinct_stable_error_codes() {
        let work = ImageWork::new(WorkLimits::default());
        let (handle, _worker) = work.admit().unwrap();
        assert!(handle.start());
        let mut control = WorkControl {
            handle: handle.clone(),
            deadline: Instant::now() + Duration::from_secs(60),
            limits: CodecLimits {
                max_pixels: 1024,
                max_scratch_bytes: 1024,
            },
        };
        let lookalike = HostError::Failed(format!(
            "unrelated detail: {}",
            crate::stdlib::fs::RESULT_LIMIT
        ));
        assert_eq!(source_error(lookalike.clone(), &control), lookalike);
        assert_eq!(
            source_error(
                HostError::Failed(format!("filesystem: {}", crate::stdlib::fs::RESULT_LIMIT)),
                &control
            ),
            failed("too-large: the source is over 64 MiB")
        );
        control.deadline = Instant::now();
        assert_eq!(
            control.check(),
            Err(failed("timeout: the trial deadline expired"))
        );
        assert_eq!(
            source_error(HostError::Failed("interrupted read".into()), &control),
            failed("timeout: the source read ran past the trial deadline")
        );
        assert!(!handle.reserve_commit(control.deadline));
        assert_eq!(
            handle.take_outcome(),
            Some(Err(failed("timeout: deadline expired before publication")))
        );
        assert_eq!(control.check(), Err(failed("work abandoned")));
    }
    #[test]
    fn provider_diagnostics_are_bounded_before_completion_retention() {
        let f = Fixture::new();
        let limits = WorkLimits::default();
        let work = ImageWork::new(limits);
        let codec = service(|_, _| Err(HostError::Failed("😀".repeat(8192))), limits);
        let job = f.job(&work, &codec);
        let handle = job.handle();
        let error = job.execute().unwrap_err();
        assert!(error.to_string().len() <= MAX_ERROR_BYTES);
        assert!(
            handle
                .take_outcome()
                .unwrap()
                .unwrap_err()
                .to_string()
                .len()
                <= MAX_ERROR_BYTES
        );
        assert_eq!(f.output(), b"old");
        // The provider can return any HostError variant. Its redirect-origin
        // string is retained just like an ordinary failure's diagnostic.
        let codec = service(
            |_, _| {
                Err(HostError::DeniedRedirect {
                    capability: "net.fetch",
                    origin: "😀".repeat(8192),
                })
            },
            limits,
        );
        let job = f.job(&work, &codec);
        let handle = job.handle();
        assert!(job.execute().is_err());
        let HostError::DeniedRedirect { origin, .. } = handle.take_outcome().unwrap().unwrap_err()
        else {
            panic!("provider error classification changed")
        };
        assert!(origin.len() <= MAX_ERROR_BYTES);
        assert!(origin.capacity() <= MAX_ERROR_BYTES);
        assert_eq!(f.output(), b"old");
    }
    #[test]
    fn codec_and_publication_observer_panics_settle_and_release_workers() {
        let f = Fixture::new();
        let limits = WorkLimits::default();
        let work = ImageWork::new(limits);
        let codec = service(|_, _| panic!("codec panic fixture"), limits);
        let job = f.job(&work, &codec);
        let handle = job.handle();
        assert!(job.execute().is_err());
        assert_eq!(work.outstanding_workers(), 0);
        assert!(matches!(handle.cancel(), CancelResult::Settled(Err(_))));
        handle.take_outcome();
        let codec = service(encoded, limits);
        let job = f.job(&work, &codec);
        let handle = job.handle();
        assert!(job
            .execute_with_commit_observer(|_| panic!("observer panic fixture"))
            .is_err());
        assert_eq!(work.outstanding_workers(), 0);
        assert!(matches!(handle.cancel(), CancelResult::Settled(Err(_))));
        assert_eq!(f.output(), b"old");
        handle.take_outcome();
        assert_eq!(work.pending_completions(), 0);
    }
    #[test]
    fn queued_abandonment_never_calls_codec_or_writes_and_keeps_worker_until_retirement() {
        let f = Fixture::new();
        let limits = WorkLimits::default();
        let work = ImageWork::new(limits);
        let service = service(
            |_, _| panic!("queued abandoned work called provider"),
            limits,
        );
        let job = f.job(&work, &service);
        let handle = job.handle();
        assert_eq!(handle.cancel(), CancelResult::Abandoned);
        assert_eq!(work.outstanding_workers(), 1);
        assert!(job.execute().is_err());
        assert_eq!(work.outstanding_workers(), 0);
        assert_eq!(work.pending_completions(), 1);
        assert!(handle.take_outcome().unwrap().is_err());
        assert_eq!(work.pending_completions(), 0);
        assert_eq!(f.output(), b"old");
    }
    #[test]
    fn discarding_delivery_does_not_lose_teardown_authority_over_a_running_worker() {
        let f = Fixture::new();
        let limits = WorkLimits::default();
        let work = ImageWork::new(limits);
        let (entered, waiting) = mpsc::channel();
        let (release, blocked) = mpsc::channel();
        let blocked = Mutex::new(blocked);
        let service = service(
            move |source, control| {
                entered.send(()).unwrap();
                blocked.lock().unwrap().recv().unwrap();
                encoded(source, control)
            },
            limits,
        );
        let job = f.job(&work, &service);
        let handle = job.handle();
        let thread = std::thread::spawn(move || job.execute());
        waiting.recv_timeout(Duration::from_secs(10)).unwrap();
        handle.discard();
        assert_eq!(work.pending_completions(), 0);
        assert_eq!(work.handles().len(), 1);
        work.close();
        assert_eq!(handle.state(), WorkState::Abandoned);
        assert_eq!(work.outstanding_workers(), 1);
        release.send(()).unwrap();
        assert!(thread.join().unwrap().is_err());
        assert_eq!(f.output(), b"old");
        assert_eq!(work.outstanding_workers(), 0);
    }
    #[test]
    fn running_abandonment_prevents_late_publication_and_a_stalled_codec_keeps_capacity() {
        let f = Fixture::new();
        let limits = WorkLimits {
            workers: 1,
            ..Default::default()
        };
        let work = ImageWork::new(limits);
        let (entered, waiting) = mpsc::channel();
        let (release, blocked) = mpsc::channel();
        let blocked = Mutex::new(blocked);
        let service = service(
            move |source, control| {
                entered.send(()).unwrap();
                blocked.lock().unwrap().recv().unwrap();
                assert!(control.canceled());
                encoded(source, control)
            },
            limits,
        );
        let job = f.job(&work, &service);
        let handle = job.handle();
        let thread = std::thread::spawn(move || job.execute());
        waiting.recv_timeout(Duration::from_secs(10)).unwrap();
        assert_eq!(handle.cancel(), CancelResult::Abandoned);
        handle.discard();
        assert_eq!(work.pending_completions(), 0);
        assert_eq!(work.outstanding_workers(), 1);
        assert!(ImageJob::prepare(
            &work,
            Some(&service),
            Some(f.dirs.clone()),
            f.grants.clone(),
            "app:/tmp/in",
            "app:/data/out",
            ImageRequest {
                max_dimension: 16,
                max_bytes: 32
            }
        )
        .is_err());
        release.send(()).unwrap();
        assert!(thread.join().unwrap().is_err());
        assert_eq!(f.output(), b"old");
        assert_eq!(work.outstanding_workers(), 0);
    }
    #[test]
    fn losing_cancel_is_immediate_and_preserves_real_publication_outcome() {
        let f = Fixture::new();
        let work = ImageWork::new(WorkLimits::default());
        let (handle, worker) = work.admit().unwrap();
        assert!(handle.start());
        let target = f.dirs.prepare_atomic(&f.grants, "app:/data/out").unwrap();
        assert!(handle.reserve_commit(Instant::now() + Duration::from_secs(10)));
        let (release, blocked) = mpsc::channel();
        let publishing = handle.clone();
        let thread = std::thread::spawn(move || {
            let _worker = worker;
            blocked.recv().unwrap();
            target.write(b"published").unwrap();
            publishing.settle(Ok(ImageFile {
                width: 1,
                height: 1,
                size: 9,
            }))
        });
        // Publication cannot progress until this sends. A blocking cancel deadlocks this witness.
        assert_eq!(handle.cancel(), CancelResult::CommitStarted);
        assert_eq!(f.output(), b"old");
        release.send(()).unwrap();
        let actual = thread.join().unwrap();
        assert_eq!(f.output(), b"published");
        assert_eq!(handle.cancel(), CancelResult::Settled(actual.clone()));
        assert_eq!(handle.take_outcome(), Some(actual));
        assert_eq!(work.pending_completions(), 0);
    }
    #[test]
    fn settled_completion_budget_is_independent_of_available_workers_and_stale_handles() {
        let f = Fixture::new();
        let limits = WorkLimits {
            completions: 1,
            ..Default::default()
        };
        let work = ImageWork::new(limits);
        let service = service(encoded, limits);
        let job = f.job(&work, &service);
        let old = job.handle();
        assert!(job.execute().is_ok());
        assert_eq!(work.outstanding_workers(), 0);
        assert_eq!(work.pending_completions(), 1);
        assert!(ImageJob::prepare(
            &work,
            Some(&service),
            Some(f.dirs.clone()),
            f.grants.clone(),
            "app:/tmp/in",
            "app:/data/out",
            ImageRequest {
                max_dimension: 16,
                max_bytes: 32
            }
        )
        .is_err());
        old.take_outcome().unwrap().unwrap();
        let job = f.job(&work, &service);
        let new = job.handle();
        assert_ne!(old.id(), new.id());
        assert!(matches!(old.cancel(), CancelResult::Settled(Ok(_))));
        assert_eq!(new.state(), WorkState::Queued);
        drop(job);
        let other = ImageWork::new(limits);
        let other_job = f.job(&other, &service);
        assert_ne!(new.id().generation, other_job.handle().id().generation);
    }
    #[test]
    fn closing_work_refuses_admission_but_preserves_committed_results() {
        let f = Fixture::new();
        let limits = WorkLimits::default();
        let work = ImageWork::new(limits);
        let service = service(encoded, limits);
        let job = f.job(&work, &service);
        let handle = job.handle();
        let result = job.execute();
        work.close();
        assert_eq!(handle.cancel(), CancelResult::Settled(result));
        assert!(ImageJob::prepare(
            &work,
            Some(&service),
            Some(f.dirs.clone()),
            f.grants.clone(),
            "app:/tmp/in",
            "app:/data/out",
            ImageRequest {
                max_dimension: 16,
                max_bytes: 32
            }
        )
        .is_err());
    }
    #[test]
    fn missing_codec_and_denied_write_do_no_source_io_and_bad_output_keeps_old_target() {
        let f = Fixture::new();
        let limits = WorkLimits::default();
        let work = ImageWork::new(limits);
        let err = ImageJob::prepare(
            &work,
            None,
            Some(f.dirs.clone()),
            f.grants.clone(),
            "app:/tmp/missing",
            "app:/data/out",
            ImageRequest {
                max_dimension: 1,
                max_bytes: 1,
            },
        )
        .err()
        .unwrap();
        assert!(err.to_string().contains("unsupported"));
        let count = Arc::new(AtomicUsize::new(0));
        let seen = count.clone();
        let service = service(
            move |_, _| {
                seen.fetch_add(1, Ordering::Relaxed);
                Ok(CompressedImage {
                    bytes: vec![0; 33],
                    width: 1,
                    height: 1,
                })
            },
            limits,
        );
        let err = ImageJob::prepare(
            &work,
            Some(&service),
            Some(f.dirs.clone()),
            Arc::new(GrantSet::parse("fs.read app:/").unwrap()),
            "app:/tmp/missing",
            "app:/data/out",
            ImageRequest {
                max_dimension: 1,
                max_bytes: 1,
            },
        )
        .err()
        .unwrap();
        assert!(matches!(
            err,
            HostError::Denied {
                capability: "fs.write"
            }
        ));
        assert!(f.job(&work, &service).execute().is_err());
        assert_eq!(count.load(Ordering::Relaxed), 1);
        assert_eq!(f.output(), b"old");
    }
    #[test]
    fn destination_parent_is_pinned_before_codec_and_host_owner_drop_cancels_queued_work() {
        let f = Fixture::new();
        let limits = WorkLimits::default();
        let work = ImageWork::new(limits);
        let path = f.path.clone();
        let service = service(
            move |source, control| {
                std::fs::rename(path.join("data"), path.join("saved")).unwrap();
                std::fs::create_dir(path.join("data")).unwrap();
                encoded(source, control)
            },
            limits,
        );
        f.job(&work, &service).execute().unwrap();
        assert!(!f.path.join("data/out").exists());
        assert_eq!(std::fs::read(f.path.join("saved/out")).unwrap(), b"new");
        let host = crate::host::Host::new()
            .with_app_directories((*f.dirs).clone())
            .with_image_codec(Arc::new(Codec(encoded)), limits)
            .unwrap();
        let bound = host.endow((*f.grants).clone());
        let job = bound
            .fs
            .prepare_image(
                "app:/tmp/in",
                "app:/data/out",
                ImageRequest {
                    max_dimension: 2,
                    max_bytes: 32,
                },
            )
            .unwrap();
        let handle = job.handle();
        drop(bound);
        assert_eq!(handle.state(), WorkState::Abandoned);
        assert!(job.execute().is_err());
    }
}
