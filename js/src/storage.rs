//! Host configuration and worker-side readiness for storage continuations.
//! @ref LLP 1027 D1a / D10 — effects leave the data seam as host work.
use exact_runner::{FailureKind, Outcome, Response};
use ibex2::{
    bindings::Context,
    grant::GrantSet,
    stdlib::{
        app_fs::AppDirectories,
        fs::{DocumentError, DocumentLease},
        fs_image::{self, CancelResult},
    },
};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

#[derive(Clone)]
pub(crate) struct Directories {
    pub data: PathBuf,
    pub cache: PathBuf,
    pub temporary: PathBuf,
}

/// What a waiter that took a compression's right to write away fails with
/// (LLP 1069.002 A1.5); the prelude settles that operation on it.
pub(crate) const IMAGE_ABANDONED: &str =
    "compressImage: timeout: the storage wait ran out; nothing was written";

/// How long a storage step is waited for before its answer fails.
pub(crate) const WAIT: Duration = Duration::from_secs(30);

pub(crate) struct Session {
    pub context: Arc<Context>,
    alive: Arc<AtomicBool>,
    wait: Duration,
}

impl Session {
    /// A storage session under `grants`: the app's directories where the
    /// host configured them, and always the documents the person chose
    /// (`doc:`, LLP 1069.010 D1), which need none, as a Rust source's do.
    pub fn open(grants: &str, wait: Duration, owner: u64) -> Result<Self, String> {
        let grants = GrantSet::parse(grants).map_err(|e| e.to_string())?;
        let context = Context::new(grants);
        context
            .set_storage_limits(ibex2::stdlib::fs::ResultLimits {
                bytes: exact_data::storage::MAX_BYTES,
            })
            .map_err(|e| e.to_string())?;
        context
            .set_sqlite_provider(Arc::new(ibex2_sqlite::SqliteProvider))
            .map_err(|e| e.to_string())?;
        context
            .set_documents(Arc::new(move |path, _access| documents(path, owner)))
            .map_err(|e| e.to_string())?;
        // Only where there is a codec: elsewhere ibex2 refuses the call as
        // unsupported before reading the source (LLP 1069.002 A1.3).
        if cfg!(target_vendor = "apple") {
            context
                .set_image_codec(Arc::new(ImageCodec), image_limits())
                .map_err(|e| e.to_string())?;
        }
        Ok(Self {
            context: Arc::new(context),
            alive: Arc::new(AtomicBool::new(true)),
            wait,
        })
    }

    /// Activate app directories after the native module has received their
    /// paths, but before storage is materialized in JavaScript.
    pub fn configure(&self, paths: &Directories, read_only: bool) -> Result<(), String> {
        for path in [&paths.data, &paths.cache, &paths.temporary] {
            if !path.is_absolute() {
                return Err("app storage roots must be absolute".into());
            }
            if !read_only {
                std::fs::create_dir_all(path).map_err(|e| format!("app storage: {e}"))?;
            }
        }
        #[cfg(unix)]
        let directories = if read_only {
            AppDirectories::snapshot(&paths.data, &paths.cache, &paths.temporary)
        } else {
            AppDirectories::new(&paths.data, &paths.cache, &paths.temporary)
        };
        #[cfg(not(unix))]
        let directories = AppDirectories::new(&paths.data, &paths.cache, &paths.temporary);
        self.context
            .set_app_directories(directories.map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())
    }

    /// Wait on this thread, until `deadline`, for a completion to deliver
    /// (or nothing left in flight): teardown's bounded wait (LLP 1097 D10).
    pub fn wait_until(&self, deadline: Instant) -> bool {
        loop {
            if self.context.is_idle() || self.context.wait(Duration::from_millis(25)) {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
        }
    }

    pub fn continuation(&self) -> Box<dyn FnOnce() -> Outcome + Send> {
        self.continuation_for(None)
    }

    /// A waiter for an answer's step, which gives up without taking any
    /// compression's right once `retired` is set (its call was let go, and
    /// another waiter now watches the work: LLP 1069.002 A1.5).
    pub fn continuation_for(
        &self,
        retired: Option<Arc<AtomicBool>>,
    ) -> Box<dyn FnOnce() -> Outcome + Send> {
        let context = self.context.clone();
        let alive = self.alive.clone();
        let wait = self.wait;
        // Capture only work visible to this owner-thread turn. A retired
        // waiter can never discover or cancel a later generation's jobs.
        let work = context.image_work().handles();
        Box::new(move || {
            let mut deadline = Instant::now() + wait;
            loop {
                if !alive.load(Ordering::Acquire) {
                    return Outcome::Failed {
                        kind: FailureKind::Aborted,
                        message: "storage runtime was unloaded".into(),
                    };
                }
                // Another ready call may already have completed this answer;
                // the owner thread still has to inspect its JS continuation.
                if context.is_idle() || context.wait(Duration::from_millis(25)) {
                    return Outcome::Response(Response {
                        status: 204,
                        headers: vec![],
                        body: vec![],
                    });
                }
                let live = || retired.as_ref().is_none_or(|r| !r.load(Ordering::Acquire));
                let retired_outcome = || Outcome::Failed {
                    kind: FailureKind::Aborted,
                    message: "storage continuation retired: its call was let go".into(),
                };
                if !live() {
                    return retired_outcome();
                }
                if Instant::now() >= deadline {
                    let mut abandoned = false;
                    let mut committed = false;
                    for handle in &work {
                        if !live() {
                            return retired_outcome();
                        }
                        match handle.cancel() {
                            CancelResult::Abandoned => abandoned = true,
                            CancelResult::CommitStarted | CancelResult::Settled(Ok(_)) => {
                                committed = true
                            }
                            CancelResult::Settled(Err(_)) => {}
                        }
                    }
                    if !live() {
                        return retired_outcome();
                    }
                    // Cancellation that loses publication returns immediately.
                    // This embedder waits outside Ibex locks and delivers the
                    // actual result instead of claiming nothing was written.
                    if committed || (!abandoned && context.wait(Duration::ZERO)) {
                        deadline = Instant::now() + Duration::from_secs(5);
                        continue;
                    }
                    let message = if abandoned {
                        IMAGE_ABANDONED
                    } else {
                        "storage continuation timed out"
                    };
                    return Outcome::Failed {
                        kind: FailureKind::Aborted,
                        message: message.into(),
                    };
                }
            }
        })
    }
}

/// Storage's availability as the prelude's refusal code (kanban F28): `bake`
/// where storage is not live; none where the module reaches it; else `agent`
/// for a drive that names no scratch store, or `unsupported`.
pub(crate) fn refusal(live: bool, reaches: bool, agent: bool) -> Result<Option<String>, String> {
    if !live {
        return Err("bake".into());
    }
    Ok((!reaches).then(|| if agent { "agent" } else { "unsupported" }.into()))
}

/// The documents a host minted ([`exact_data::documents`]), as ibex2's
/// table: the one a Rust source's storage requests resolve through.
fn documents(path: &str, owner: u64) -> Result<DocumentLease, DocumentError> {
    use exact_data::documents::{resolve, Resolved};
    let resolved = resolve(path, owner).map_err(DocumentError::from)?;
    let guard = std::sync::Arc::new(());
    match resolved {
        Resolved::Root(name) => DocumentLease::root(name, guard),
        Resolved::Real {
            directory,
            path,
            selected_file: true,
        } => DocumentLease::selected_file(directory, path, guard),
        Resolved::Real {
            directory,
            path,
            selected_file: false,
        } => DocumentLease::directory(directory, path, guard),
    }
}

/// The platform's image codec ([`exact_data::image`]), as ibex2's
/// (`fs.compressImage`, LLP 1069.002 A1): the one a Rust source's storage
/// requests run too.
pub(crate) struct ImageCodec;
fn image_limits() -> fs_image::WorkLimits {
    fs_image::WorkLimits {
        max_job_bytes: 640 << 20,
        max_dimension: exact_data::image::MAX_DIMENSION,
        ..fs_image::WorkLimits::default()
    }
}
impl fs_image::ImageCodec for ImageCodec {
    fn limits(&self) -> fs_image::CodecLimits {
        fs_image::CodecLimits {
            max_pixels: exact_data::image::MAX_SOURCE_PIXELS,
            max_scratch_bytes: 512 << 20,
        }
    }
    fn encode(
        &self,
        bytes: &[u8],
        request: fs_image::ImageRequest,
        control: &fs_image::WorkControl,
    ) -> Result<fs_image::CompressedImage, ibex2::boundary::HostError> {
        exact_data::image::compress_controlled(
            bytes,
            request.max_dimension,
            request.max_bytes,
            control.deadline(),
            &|| {
                control
                    .check()
                    .map_err(|e| exact_data::image::failure("timeout", e))
            },
            control.limits().max_pixels,
            control.limits().max_scratch_bytes,
        )
        .map(|c| fs_image::CompressedImage {
            bytes: c.bytes,
            width: c.width,
            height: c.height,
        })
        .map_err(ibex2::boundary::HostError::Failed)
    }
}

/// Whether `grants` reach documents (`fs.read doc:/`, `fs.write doc:/`):
/// such a module gets a storage session even with no app directories.
pub(crate) fn reaches_documents(grants: &str) -> bool {
    grants.lines().any(|line| {
        let mut words = line.split_whitespace();
        matches!(words.next(), Some("fs.read" | "fs.write"))
            && words.next().is_some_and(|p| p.starts_with("doc:"))
    })
}

impl Drop for Session {
    fn drop(&mut self) {
        self.alive.store(false, Ordering::Release);
        // Close admission and abandon cancellable work without waiting under
        // a shared lock. An already-started publication retains its real result.
        self.context.image_work().close();
    }
}
