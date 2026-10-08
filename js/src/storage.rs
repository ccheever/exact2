//! Host configuration and worker-side readiness for storage continuations.
//! @ref LLP 1027 D1a / D10 — effects leave the data seam as host work.
use exact_runner::{FailureKind, Outcome, Response};
use ibex2::{
    bindings::Context,
    grant::GrantSet,
    stdlib::{
        app_fs::AppDirectories,
        fs::{Abandoned, CompressedImage, Document},
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
    pub fn open(grants: &str, wait: Duration) -> Result<Self, String> {
        let grants = GrantSet::parse(grants).map_err(|e| e.to_string())?;
        let context = Context::new(grants);
        context
            .set_sqlite_provider(Arc::new(ibex2_sqlite::SqliteProvider))
            .map_err(|e| e.to_string())?;
        context
            .set_documents(Arc::new(documents))
            .map_err(|e| e.to_string())?;
        // Only where there is a codec: elsewhere ibex2 refuses the call as
        // unsupported before reading the source (LLP 1069.002 A1.3).
        if cfg!(target_vendor = "apple") {
            context
                .set_image_codec(Arc::new(image_codec))
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
    pub fn configure(&self, paths: &Directories) -> Result<(), String> {
        for path in [&paths.data, &paths.cache, &paths.temporary] {
            if !path.is_absolute() {
                return Err("app storage roots must be absolute".into());
            }
            std::fs::create_dir_all(path).map_err(|e| format!("app storage: {e}"))?;
        }
        self.context
            .set_app_directories(
                AppDirectories::new(&paths.data, &paths.cache, &paths.temporary)
                    .map_err(|e| e.to_string())?,
            )
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
        let context = self.context.clone();
        let alive = self.alive.clone();
        let wait = self.wait;
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
                if Instant::now() >= deadline {
                    // Giving up: an `fs.compressImage` that has not written
                    // loses the right to, so nothing lands after this failure
                    // and the queue moves on safely; one that has written is
                    // waited for (LLP 1069.002 A1.5).
                    let message = match context.abandon_image_work() {
                        Abandoned::Written => {
                            deadline = Instant::now() + Duration::from_secs(5);
                            continue;
                        }
                        Abandoned::Abandoned => {
                            "compressImage: timeout: the storage wait ran out; nothing was written"
                        }
                        Abandoned::Nothing => "storage continuation timed out",
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
fn documents(path: &str) -> Result<Document, String> {
    use exact_data::documents::{resolve, Resolved};
    Ok(match resolve(path)? {
        Resolved::Root(name) => Document::Root(name),
        Resolved::Real(real) => Document::Real(real),
    })
}

/// The platform's image codec ([`exact_data::image`]), as ibex2's
/// (`fs.compressImage`, LLP 1069.002 A1): the one a Rust source's storage
/// requests run too.
pub(crate) fn image_codec(
    bytes: &[u8],
    max_dimension: u32,
    max_bytes: u64,
    deadline: Instant,
) -> Result<CompressedImage, String> {
    exact_data::image::compress(bytes, max_dimension, max_bytes, deadline).map(|c| {
        CompressedImage {
            bytes: c.bytes,
            width: c.width,
            height: c.height,
        }
    })
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
        // An `fs.compressImage` still running when its module is unloaded
        // writes nothing; one mid-write is let finish (LLP 1069.002 A1.5).
        let _ = self.context.abandon_image_work();
    }
}
