//! The storage the host hands the app's native module (LLP 1154 D3): the
//! app's three roots, and SQLite files admitted under the app's grants and
//! reserved from Exact's own SQLite while the module holds them.
//!
//! The module may link a SQLite of its own. Two SQLite libraries in one
//! process are safe only while no file is open in both: each keeps its own
//! POSIX advisory locks, and closing a descriptor drops every lock the
//! process holds on the file (<https://www.sqlite.org/howtocorrupt.html>,
//! §2.2.1). So a file is held by the module or by Exact, never by both:
//! [`NativeStorage::sqlite`] refuses a file the app's `storage.sqlite` has
//! open, and [`Reserving`] refuses the app's `storage.sqlite.open` of a
//! file the module holds.

use ibex2::{
    boundary::{admit, HostError},
    grant::{GrantSet, Operation},
    stdlib::{
        app_fs::{resolve_sqlite, AppDirectories},
        sqlite::{self, Command, ExecuteResult, Location, Rows, StatementInfo, Value},
    },
};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

/// Who has a SQLite file open, by its real path. Process-wide: the two
/// libraries' exclusion is the process's, whichever session opened the file.
static OPEN: Mutex<BTreeMap<PathBuf, Holder>> = Mutex::new(BTreeMap::new());

enum Holder {
    /// The native module, through one [`SqliteFile`].
    Module,
    /// Exact's SQLite, through this many open connections.
    Exact(usize),
}

fn open() -> std::sync::MutexGuard<'static, BTreeMap<PathBuf, Holder>> {
    OPEN.lock().unwrap_or_else(|e| e.into_inner())
}

/// One file however its directories are spelled (`/tmp` and `/private/tmp`
/// on macOS): the parent canonicalized, which exists once it is admitted.
fn key(path: &Path) -> PathBuf {
    match (path.parent().map(std::fs::canonicalize), path.file_name()) {
        (Some(Ok(parent)), Some(name)) => parent.join(name),
        _ => path.to_path_buf(),
    }
}

/// What [`crate::NativeModule::configure_storage`] receives: the app's
/// data, cache and temporary roots, and its SQLite files under its grants.
/// Recording it opens nothing.
pub struct NativeStorage {
    roots: [PathBuf; 3],
    grants: GrantSet,
}

impl NativeStorage {
    pub(crate) fn new(roots: [PathBuf; 3], io_grants: &str) -> Result<Self, String> {
        let grants = GrantSet::parse(io_grants).map_err(|e| e.to_string())?;
        Ok(Self { roots, grants })
    }

    /// The app's data root (`app:/data`).
    pub fn data(&self) -> &Path {
        &self.roots[0]
    }

    /// The app's cache root (`app:/cache`).
    pub fn cache(&self) -> &Path {
        &self.roots[1]
    }

    /// The app's temporary root (`app:/tmp`).
    pub fn temporary(&self) -> &Path {
        &self.roots[2]
    }

    /// Admit `sqlite.open <app_path>` under the app's grants and reserve the
    /// file for the module until the answer drops: meanwhile the app's own
    /// `storage.sqlite.open` of it is refused. Refused when no grant covers
    /// the path, or when the app's storage has the file open. The module
    /// opens the real path ([`SqliteFile::path`]) with its own SQLite, and
    /// closes it before dropping the answer.
    pub fn sqlite(&self, app_path: &str) -> Result<SqliteFile, String> {
        // Admitted before even the roots are created.
        admit(
            &self.grants,
            &Operation::SqliteOpen {
                path: app_path.into(),
            },
        )
        .map_err(|e| match e {
            // The file and the lines that would admit it, as the web's storage says.
            HostError::Denied { .. } => {
                let dir = app_path.rsplit_once('/').map_or(app_path, |(dir, _)| dir);
                format!("{e} {app_path}: no grant covers it; grant `sqlite.open {app_path}`, or `sqlite.open {dir}` for every file there (a grant covers its path and what is below it, by whole names)")
            }
            e => e.to_string(),
        })?;
        for root in &self.roots {
            std::fs::create_dir_all(root).map_err(|e| format!("app storage: {e}"))?;
        }
        let directories = AppDirectories::new(&self.roots[0], &self.roots[1], &self.roots[2])
            .map_err(|e| e.to_string())?;
        let path = resolve_sqlite(&self.grants, Some(&directories), app_path)
            .map_err(|e| e.to_string())?;
        let key = key(&path);
        let mut open = open();
        match open.get(&key) {
            Some(Holder::Module) => {
                return Err(format!(
                    "{app_path} is already open in the app's native module"
                ))
            }
            Some(Holder::Exact(_)) => {
                return Err(format!(
                    "{app_path} is open in the app's own storage (storage.sqlite); a file is open in one SQLite at a time, so close it there first"
                ))
            }
            None => {}
        }
        open.insert(key.clone(), Holder::Module);
        Ok(SqliteFile {
            path,
            key,
            _directories: directories,
        })
    }
}

/// A SQLite file admitted to the native module and reserved for it. It
/// holds the app's directories, so the file's real path stays where it was
/// resolved, while it lives; dropping it ends the reservation.
pub struct SqliteFile {
    path: PathBuf,
    key: PathBuf,
    _directories: AppDirectories,
}

impl SqliteFile {
    /// The file's real path, for the module's own SQLite to open.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for SqliteFile {
    fn drop(&mut self) {
        let mut open = open();
        if matches!(open.get(&self.key), Some(Holder::Module)) {
            open.remove(&self.key);
        }
    }
}

/// Exact's SQLite provider, refusing a file the native module holds and
/// counting the files it has open for [`NativeStorage::sqlite`].
pub(crate) struct Reserving(pub ibex2_sqlite::SqliteProvider);

impl sqlite::Provider for Reserving {
    fn open(&self, location: Location) -> Result<Arc<dyn sqlite::Connection>, HostError> {
        let key = key(&location.path);
        {
            let mut open = open();
            match open.get_mut(&key) {
                Some(Holder::Module) => {
                    return Err(HostError::Failed(format!(
                        "SQLite: {} is open in the app's native module, which holds it until it lets it go; a file is open in one SQLite at a time",
                        location.path.display()
                    )))
                }
                Some(Holder::Exact(count)) => *count += 1,
                None => {
                    open.insert(key.clone(), Holder::Exact(1));
                }
            }
        }
        let held = Held {
            key,
            released: AtomicBool::new(false),
        };
        // A failed open releases its count as `held` drops.
        let connection = self.0.open(location)?;
        Ok(Arc::new(Connection { connection, held }))
    }
}

/// One count of a file Exact's SQLite has open, released once: at close,
/// or after the connection itself, when the last handle to it drops.
struct Held {
    key: PathBuf,
    released: AtomicBool,
}

impl Held {
    fn release(&self) {
        if self.released.swap(true, Ordering::AcqRel) {
            return;
        }
        let mut open = open();
        if let Some(Holder::Exact(count)) = open.get_mut(&self.key) {
            *count -= 1;
            if *count == 0 {
                open.remove(&self.key);
            }
        }
    }
}

impl Drop for Held {
    fn drop(&mut self) {
        self.release();
    }
}

struct Connection {
    connection: Arc<dyn sqlite::Connection>,
    held: Held,
}

impl sqlite::Connection for Connection {
    fn prepare(&self, sql: &str) -> Result<StatementInfo, HostError> {
        self.connection.prepare(sql)
    }
    fn execute(&self, sql: &str, params: &[Value]) -> Result<ExecuteResult, HostError> {
        self.connection.execute(sql, params)
    }
    fn query(&self, sql: &str, params: &[Value]) -> Result<Rows, HostError> {
        self.connection.query(sql, params)
    }
    fn transaction(&self, commands: &[Command]) -> Result<Vec<ExecuteResult>, HostError> {
        self.connection.transaction(commands)
    }
    fn close(&self) -> Result<(), HostError> {
        // A connection that did not close stays counted until it drops.
        self.connection.close()?;
        self.held.release();
        Ok(())
    }
}
