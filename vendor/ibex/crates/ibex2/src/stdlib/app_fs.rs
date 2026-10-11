//! Host-selected data, cache, and temporary directories.
#[cfg(unix)]
#[path = "app_fs_unix.rs"]
mod implementation;
#[cfg(windows)]
#[path = "app_fs_windows.rs"]
mod implementation;
#[cfg(any(unix, windows))]
pub use implementation::*;
#[cfg(any(unix, windows))]
pub(crate) use implementation::{atomic_native, document};

#[cfg(not(any(unix, windows)))]
mod implementation {
    use crate::{
        boundary::HostError,
        grant::GrantSet,
        stdlib::fs::{FsOp, FsResult},
    };
    use std::path::{Path, PathBuf};
    fn unsupported() -> HostError {
        HostError::Failed("app filesystem requires Unix directory capabilities".into())
    }
    #[derive(Clone, Debug)]
    pub struct AppDirectories;
    impl AppDirectories {
        pub fn new(
            _data: impl AsRef<Path>,
            _cache: impl AsRef<Path>,
            _tmp: impl AsRef<Path>,
        ) -> Result<Self, HostError> {
            Err(unsupported())
        }
        pub(crate) fn run(
            &self,
            _grants: &GrantSet,
            _op: FsOp,
            _path: &str,
            _destination: Option<&str>,
            _data: Option<&[u8]>,
        ) -> Result<FsResult, HostError> {
            Err(unsupported())
        }
    }
    pub fn resolve_sqlite(
        _grants: &GrantSet,
        _directories: Option<&AppDirectories>,
        _path: &str,
    ) -> Result<PathBuf, HostError> {
        Err(unsupported())
    }
    pub(crate) fn atomic_native(_path: &Path, _data: &[u8]) -> Result<FsResult, HostError> {
        Err(unsupported())
    }
    pub(crate) fn document(
        _root: &std::fs::File,
        _path: &str,
        _op: FsOp,
        _data: Option<&[u8]>,
    ) -> std::io::Result<FsResult> {
        Err(std::io::Error::other(
            "documents require native directory capabilities",
        ))
    }
}
#[cfg(not(any(unix, windows)))]
pub use implementation::*;
#[cfg(not(any(unix, windows)))]
pub(crate) use implementation::{atomic_native, document};
