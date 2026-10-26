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
        if exact_data::image::available() {
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

/// The kept files an atomic write from TypeScript is replacing, by the
/// token the prelude holds until the write settles.
#[allow(clippy::type_complexity)]
static REPLACING: std::sync::Mutex<std::collections::BTreeMap<u64, (u64, String, ReplacedFiles)>> =
    std::sync::Mutex::new(std::collections::BTreeMap::new());
static NEXT_REPLACING: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
type ReplacedFiles = exact_data::documents::kept::Replaced;

/// `storage.fs.recentDocuments()` (`ask` empty) and `forgetDocument(path)`
/// (`ask` is `forget <path>`), for the session `owner` names when the grants
/// keep documents (LLP 1069.010.000 D5): the list as JSON, or nothing.
///
/// And the app's own `atomicWriteFile` of a kept file (D4), around the
/// write: `replacing <path>` reads what it replaces and answers a token, or
/// nothing when it replaces no kept file; `replaced <token>` carries the
/// entry to the file written; `dropped <token>` lets it go (the write failed).
pub(crate) fn kept(owner: Option<u64>, ask: &str) -> Result<Option<String>, String> {
    use exact_data::documents::kept;
    let owner =
        owner.ok_or("storage: recentDocuments and forgetDocument need the doc.keep grant")?;
    let replacing = || REPLACING.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(path) = ask.strip_prefix("replacing ") {
        // A save issued while an earlier one of the same file is in flight
        // replaces what that one writes: the earlier save may have renamed
        // its file and not yet carried the entry, so the identity at the path
        // is not yet the kept one. The queue runs the two in order.
        let earlier = || {
            (replacing().values())
                .find(|(o, p, _)| *o == owner && p == path)
                .map(|(.., r)| r.clone())
        };
        let Some(replaced) = earlier().or_else(|| kept::before_write(owner, path)) else {
            return Ok(None);
        };
        let token = NEXT_REPLACING.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        replacing().insert(token, (owner, path.to_owned(), replaced));
        return Ok(Some(token.to_string()));
    }
    for (how, carry) in [("replaced ", true), ("dropped ", false)] {
        if let Some(token) = ask.strip_prefix(how) {
            let held = token.parse().ok().and_then(|t: u64| replacing().remove(&t));
            if let Some((owner, _, replaced)) = held.filter(|_| carry) {
                kept::carry(owner, replaced);
            }
            return Ok(None);
        }
    }
    match ask.strip_prefix("forget ") {
        Some(path) => kept::forget(owner, path)
            .map(|()| None)
            .map_err(|_| format!("document ENOENT: {path} is not a document this app keeps")),
        // A session that keeps nothing (no store) has kept nothing.
        None => Ok(Some(
            kept::list(owner).map_or_else(|| "[]".into(), |list| list.to_string()),
        )),
    }
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
            selected_file: Some(name),
        } => DocumentLease::selected_file_named(directory, path, name, guard),
        Resolved::Real {
            directory,
            path,
            selected_file: None,
        } => DocumentLease::directory(directory, path, guard),
    }
}

/// The platform's image codec ([`exact_data::image`]), as ibex2's
/// (`fs.compressImage`, LLP 1069.002 A1; `fs.transformImage`, LLP 1133.019
/// D14): the one a Rust source's storage requests run too.
pub(crate) struct ImageCodec;
/// A transform's output side bounds the service; `compressImage`'s own
/// `maxDimension` is the codec's to check (8192).
fn image_limits() -> fs_image::WorkLimits {
    fs_image::WorkLimits {
        max_job_bytes: 640 << 20,
        max_dimension: exact_data::image::edit::MAX_SIDE,
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
        let call = request.call();
        exact_data::image::encode_request(
            bytes,
            request.transform,
            request.options.as_deref(),
            match request.format {
                fs_image::ImageFormat::Jpeg => exact_data::image::Format::Jpeg,
                fs_image::ImageFormat::Png => exact_data::image::Format::Png,
            },
            (request.max_dimension, request.max_bytes),
            exact_data::image::Deadline {
                at: control.deadline(),
                check: &|| {
                    control
                        .check()
                        .map_err(|e| exact_data::image::edit::failure_named(call, "timeout", e))
                },
                max_pixels: control
                    .limits()
                    .max_pixels
                    .min(exact_data::image::MAX_SOURCE_PIXELS),
                max_scratch_bytes: control.limits().max_scratch_bytes,
            },
        )
        .map(|c| fs_image::CompressedImage {
            bytes: c.bytes,
            width: c.width,
            height: c.height,
            report: c.report,
            extension: c.extension,
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

#[cfg(all(test, unix))]
mod tests {
    /// LLP 1069.010.000 D4: a save issued after an earlier one of the same
    /// kept file renamed its file but before it carried the entry still
    /// carries it, to the file the later one writes.
    #[test]
    fn a_save_issued_while_another_is_in_flight_carries_the_entry() {
        use exact_data::documents::kept;
        use std::os::unix::fs::MetadataExt;
        let dir = std::env::temp_dir().join(format!("exact-js-overlap-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("drawing.md");
        std::fs::write(&file, "first").unwrap();
        let set = dir.join("kept.json");
        kept::open(9140, "test.js.overlap", &set);
        let doc = kept::keep(9140, &file, "", "readwrite").unwrap();
        let replace = |text: &str| {
            std::fs::write(dir.join("next"), text).unwrap();
            std::fs::rename(dir.join("next"), &file).unwrap();
        };
        let ask = |ask: String| super::kept(Some(9140), &ask).unwrap();
        let a = ask(format!("replacing {doc}")).unwrap();
        replace("a");
        let b = ask(format!("replacing {doc}")).expect("the later save carries too");
        ask(format!("replaced {a}"));
        replace("b");
        ask(format!("replaced {b}"));
        let written: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&set).unwrap()).unwrap();
        assert_eq!(
            written["documents"][0]["inode"],
            std::fs::metadata(&file).unwrap().ino()
        );
        kept::end(9140);
        exact_data::documents::forget(9140);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
