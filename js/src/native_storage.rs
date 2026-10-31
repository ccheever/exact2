//! The storage the host hands the app's native module (LLP 1154 D3): the
//! app's three roots, and SQLite files admitted under the app's grants and
//! reserved from Exact's own SQLite while the module holds them.
//!
//! The module may link a SQLite of its own, and two in one process are safe
//! only while no file is open in both. Every Exact SQLite connection, the
//! TypeScript storage's and a Rust source's, opens through
//! `ibex2_sqlite::SqliteProvider`, which refuses a file reserved here, and
//! [`NativeStorage::sqlite`] refuses a file one of them has open
//! (`ibex2_sqlite::reserve`, Exact's local Ibex patch 25).

use ibex2::{
    boundary::{admit, HostError},
    grant::{GrantSet, Operation},
    stdlib::app_fs::{resolve_sqlite, AppDirectories},
};
use std::path::{Path, PathBuf};

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
    /// file for the module: refused when no grant covers the path, or when
    /// Exact's SQLite has the file open. The module holds the [`SqliteFile`]
    /// guard while it uses the file, opens its real path
    /// ([`SqliteFile::path`]) with its own SQLite, and closes that before
    /// dropping the guard; meanwhile the app's own `storage.sqlite.open` of
    /// the file is refused. The reservation ends when the guard drops.
    /// An absent file is created empty, so that it is one file under every
    /// name from the start. Neither this nor a refusal opens an existing file.
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
        let reservation = ibex2_sqlite::reserve(&path).map_err(|refused| match refused {
            ibex2_sqlite::Refused::Reserved => {
                format!("{app_path} is already open in the app's native module")
            }
            ibex2_sqlite::Refused::Open => format!(
                "{app_path} is open in the app's own storage; a file is open in one SQLite at a time, so close it there first"
            ),
            ibex2_sqlite::Refused::Create(why) => format!("{app_path}: {why}"),
        })?;
        Ok(SqliteFile {
            path,
            reservation,
            _directories: directories,
        })
    }
}

/// A SQLite file admitted to the native module and reserved for it. It
/// holds the app's directories, so the file's real path stays where it was
/// resolved, while it lives; dropping it ends the reservation.
pub struct SqliteFile {
    path: PathBuf,
    reservation: ibex2_sqlite::Reservation,
    _directories: AppDirectories,
}

impl SqliteFile {
    /// The file's real path, for the module's own SQLite to open.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Whether the file existed before [`NativeStorage::sqlite`]; else the
    /// host made it, empty, which SQLite opens as a new database.
    pub fn existed(&self) -> bool {
        self.reservation.existed()
    }
}
