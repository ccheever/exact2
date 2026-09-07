//! Host configuration and worker-side readiness for storage continuations.
//! @ref LLP 1027 D1a / D10 — effects leave the data seam as host work.
use exact_runner::{FailureKind, Outcome, Response};
use ibex2::{bindings::Context, grant::GrantSet, stdlib::app_fs::AppDirectories};
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

pub(crate) struct Session {
    pub context: Arc<Context>,
    alive: Arc<AtomicBool>,
}

impl Session {
    pub fn open(paths: &Directories, grants: &str) -> Result<Self, String> {
        let grants = GrantSet::parse(grants).map_err(|e| e.to_string())?;
        for path in [&paths.data, &paths.cache, &paths.temporary] {
            if !path.is_absolute() {
                return Err("app storage roots must be absolute".into());
            }
            std::fs::create_dir_all(path).map_err(|e| format!("app storage: {e}"))?;
        }
        let context = Context::new(grants);
        context
            .set_app_directories(
                AppDirectories::new(&paths.data, &paths.cache, &paths.temporary)
                    .map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
        context
            .set_sqlite_provider(Arc::new(ibex2_sqlite::SqliteProvider))
            .map_err(|e| e.to_string())?;
        Ok(Self {
            context: Arc::new(context),
            alive: Arc::new(AtomicBool::new(true)),
        })
    }

    pub fn continuation(&self) -> Box<dyn FnOnce() -> Outcome + Send> {
        let context = self.context.clone();
        let alive = self.alive.clone();
        Box::new(move || {
            let deadline = Instant::now() + Duration::from_secs(30);
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
                    return Outcome::Failed {
                        kind: FailureKind::Aborted,
                        message: "storage continuation timed out".into(),
                    };
                }
            }
        })
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.alive.store(false, Ordering::Release);
    }
}
