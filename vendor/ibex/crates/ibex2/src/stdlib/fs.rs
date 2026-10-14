//! `fs` — delegating, **capability-bearing** (LLP 0059.000 §3.11).
//!
//! Promise-based only. **No synchronous variants**: `readFileSync` and its
//! family are the largest single source of blocking in Node programs, and
//! LLP 0059.000 §1.1 permits a synchronous host call only for an op that never
//! leaves the calling thread. This one leaves it. That is a deliberate
//! divergence from Node and is documented as one.
//!
//! **Capability:** `fs.read` and `fs.write`, granted per path prefix. This is
//! the half of the supply-chain story that per-origin cannot express — the
//! package that cannot read `~/.ssh` — and it is why `PathPrefix` compares
//! whole components rather than string prefixes.
//!
//! `app:/data`, `app:/cache`, and `app:/tmp` resolve through explicit host
//! directory handles. Native absolute paths retain their separate grants.
//! `run` is the shared executor for Rust and JavaScript callers.
//!
//! @ref LLP 0059.000#311-fs--delegating-capability-bearing-author-required — the surface
//! @ref LLP 0067#3-the-check — per-prefix is a parameterized grant

#[cfg(not(windows))]
use std::path::Component;
use std::path::{Path, PathBuf};

use crate::boundary::HostError;
use crate::grant::{GrantSet, Operation};

// @ref LLP 0068.001#4-bounded-access-and-image-work — the storage envelope is capped at 16 MiB. A readFile
// result is {"base64":"..."}, whose fixed JSON overhead is 13 bytes.
pub const RESULT_BYTES: usize = 16 << 20;
pub const READ_BYTES: usize = ((RESULT_BYTES - 13) / 4) * 3;
/// Serialized storage envelope budget. Read bytes include base64 expansion and JSON overhead.
#[derive(Debug, Clone, Copy)]
pub struct ResultLimits {
    pub bytes: usize,
}
impl Default for ResultLimits {
    fn default() -> Self {
        Self {
            bytes: RESULT_BYTES,
        }
    }
}
thread_local! { static RESULT_BUDGET: std::cell::Cell<usize> = const { std::cell::Cell::new(RESULT_BYTES) }; }
// Executors are synchronous. A stack-scoped budget reaches platform enumeration helpers
// without changing the existing public FsOp ABI; nested calls restore the previous value.
fn with_result_limits<T>(
    limits: ResultLimits,
    call: impl FnOnce() -> Result<T, HostError>,
) -> Result<T, HostError> {
    if limits.bytes < 16 || limits.bytes > RESULT_BYTES {
        return Err(HostError::InvalidArgument(
            "storage result limit must be between 16 bytes and 16 MiB".into(),
        ));
    }
    struct Restore(usize);
    impl Drop for Restore {
        fn drop(&mut self) {
            RESULT_BUDGET.with(|limit| limit.set(self.0));
        }
    }
    let _restore = Restore(RESULT_BUDGET.with(|limit| limit.replace(limits.bytes)));
    call()
}
pub(crate) const RESULT_LIMIT: &str = "storage result exceeds its byte limit";

/// A same-handle bounded read. The callback runs between chunks with no Ibex lock held.
pub fn read_bounded_file(
    mut file: std::fs::File,
    cap: u64,
    canceled: &dyn Fn() -> bool,
) -> std::io::Result<Vec<u8>> {
    use std::io::Read;
    let meta = file.metadata()?;
    if !meta.is_file() {
        return Err(std::io::Error::other("read needs a regular file"));
    }
    if meta.len() > cap {
        return Err(std::io::Error::other(RESULT_LIMIT));
    }
    let cap = usize::try_from(cap).map_err(|_| std::io::Error::other(RESULT_LIMIT))?;
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 64 * 1024];
    loop {
        if canceled() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                "read abandoned",
            ));
        }
        let remaining = cap.saturating_sub(bytes.len());
        let count = chunk.len().min(remaining.saturating_add(1));
        let n = file.read(&mut chunk[..count])?;
        if n == 0 {
            return Ok(bytes);
        }
        if n > remaining {
            return Err(std::io::Error::other(RESULT_LIMIT));
        }
        bytes.extend_from_slice(&chunk[..n]);
    }
}

pub(crate) fn read_result(file: std::fs::File) -> std::io::Result<Vec<u8>> {
    read_bounded_file(
        file,
        RESULT_BUDGET.with(|limit| ((limit.get() - 13) / 4 * 3) as u64),
        &|| false,
    )
}

fn open_native(path: &Path) -> std::io::Result<std::fs::File> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC);
    }
    options.open(path)
}

pub(crate) fn push_name(
    names: &mut Vec<String>,
    used: &mut usize,
    name: String,
) -> std::io::Result<()> {
    let encoded = serde_json::to_string(&name)
        .map_err(std::io::Error::other)?
        .len();
    *used = used
        .checked_add(encoded)
        .and_then(|n| n.checked_add(usize::from(!names.is_empty())))
        .ok_or_else(|| std::io::Error::other(RESULT_LIMIT))?;
    if *used > RESULT_BUDGET.with(|limit| limit.get() - 2) {
        return Err(std::io::Error::other(RESULT_LIMIT));
    }
    names.push(name);
    Ok(())
}

/// The operations, each one a distinct host op.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsOp {
    ReadFile,
    WriteFile,
    AppendFile,
    AtomicWriteFile,
    ReadDir,
    Mkdir,
    Remove,
    Stat,
    Rename,
    CopyFile,
    Realpath,
}

impl FsOp {
    /// Which capability the operation needs, and on which path.
    ///
    /// `rename` and `copyFile` touch two paths and need write on the
    /// destination as well as read on the source. Rename additionally needs
    /// write on the source because it removes that directory entry.
    pub fn required(self) -> (bool, bool) {
        match self {
            FsOp::ReadFile | FsOp::ReadDir | FsOp::Stat | FsOp::Realpath => (true, false),
            FsOp::WriteFile
            | FsOp::AppendFile
            | FsOp::AtomicWriteFile
            | FsOp::Mkdir
            | FsOp::Remove => (false, true),
            // read the source, write the destination
            FsOp::Rename | FsOp::CopyFile => (true, true),
        }
    }

    pub fn takes_second_path(self) -> bool {
        matches!(self, FsOp::Rename | FsOp::CopyFile)
    }
}

/// Normalize an absolute path lexically, refusing anything relative or
/// unresolved.
///
/// Lexically, not through the filesystem: resolving `..` by asking the OS would
/// let a symlink change what a grant covers between the check and the use.
#[cfg(windows)]
pub fn normalize(path: &str) -> Result<PathBuf, HostError> {
    super::windows_path::NativePath::parse(path, true)
        .map(|path| PathBuf::from(path.spelling()))
        .map_err(|error| HostError::Failed(format!("TypeError: {error}")))
}

#[cfg(not(windows))]
pub fn normalize(path: &str) -> Result<PathBuf, HostError> {
    if !path.starts_with('/') {
        return Err(HostError::Failed(format!(
            "TypeError: path must be absolute: {path}"
        )));
    }
    let mut parts: Vec<String> = Vec::new();
    for component in Path::new(path).components() {
        match component {
            Component::RootDir => {}
            Component::CurDir => {}
            Component::ParentDir => {
                if parts.pop().is_none() {
                    return Err(HostError::Failed(format!(
                        "TypeError: path escapes the root: {path}"
                    )));
                }
            }
            Component::Normal(part) => parts.push(part.to_string_lossy().into_owned()),
            Component::Prefix(_) => {
                return Err(HostError::Failed(format!(
                    "TypeError: unsupported path: {path}"
                )))
            }
        }
    }
    Ok(PathBuf::from(format!("/{}", parts.join("/"))))
}

/// A path as the filesystem will actually resolve it: its longest existing
/// prefix canonicalized (symlinks followed, case settled), the rest appended
/// as written.
///
/// The lexical check alone let a symlink *inside* a granted prefix reach
/// outside it — `/data/link -> /etc/passwd` read `/etc/passwd` under a
/// `/data` grant, and a module with write on `/data` could make that link
/// itself. So a request is admitted only if both its spelling and its real
/// path are covered, each against the grant's own spelling and real path,
/// which is what keeps a grant on `/var/...` working when `/var` is itself a
/// symlink to `/private/var`.
pub fn realize(path: &Path) -> PathBuf {
    let mut existing = path;
    let mut rest: Vec<std::ffi::OsString> = Vec::new();
    loop {
        if let Ok(canonical) = std::fs::canonicalize(existing) {
            let mut out = canonical;
            for component in rest.iter().rev() {
                out.push(component);
            }
            return out;
        }
        match (existing.file_name(), existing.parent()) {
            (Some(name), Some(parent)) => {
                rest.push(name.to_os_string());
                existing = parent;
            }
            _ => return path.to_path_buf(),
        }
    }
}

/// Check an operation's paths against the caller's grants.
///
/// Every path is checked, as spelt and as realized (`realize`). For a two-path
/// operation both are, with the right capability on each.
pub fn admit(
    grants: &GrantSet,
    op: FsOp,
    path: &Path,
    destination: Option<&Path>,
) -> Result<(), HostError> {
    admit_as(grants, op, path, destination)?;
    let realized = grants.realized_fs();
    let real_path = realize(path);
    let real_destination = destination.map(realize);
    admit_as(&realized, op, &real_path, real_destination.as_deref())
}

pub(super) fn admit_as(
    grants: &GrantSet,
    op: FsOp,
    path: &Path,
    destination: Option<&Path>,
) -> Result<(), HostError> {
    let (needs_read, needs_write) = op.required();
    let as_string = |p: &Path| p.to_string_lossy().into_owned();

    match (needs_read, needs_write, destination) {
        // Rename also removes the source, requiring source write authority.
        (true, true, Some(destination)) => {
            if op == FsOp::Rename {
                crate::boundary::admit(
                    grants,
                    &Operation::FsWrite {
                        path: as_string(path),
                    },
                )?;
            }
            crate::boundary::admit(
                grants,
                &Operation::FsRead {
                    path: as_string(path),
                },
            )?;
            crate::boundary::admit(
                grants,
                &Operation::FsWrite {
                    path: as_string(destination),
                },
            )
        }
        (true, true, None) => Err(HostError::InvalidArgument(
            "this operation needs a destination path".into(),
        )),
        (true, false, _) => crate::boundary::admit(
            grants,
            &Operation::FsRead {
                path: as_string(path),
            },
        ),
        _ => crate::boundary::admit(
            grants,
            &Operation::FsWrite {
                path: as_string(path),
            },
        ),
    }
}

/// What `stat` reports. Deliberately small: size, kind, and modification time
/// are what the measured uses need, and a full `Stats` object is a Node-ism
/// with a large surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stat {
    pub size: u64,
    pub is_file: bool,
    pub is_directory: bool,
    pub modified_ms: u64,
}

/// Run an operation whose paths have already been normalized and admitted.
pub fn perform(
    op: FsOp,
    path: &Path,
    destination: Option<&Path>,
    data: Option<&[u8]>,
) -> Result<FsResult, HostError> {
    let failed = |e: std::io::Error| HostError::Failed(format!("{}: {e}", path.display()));
    match op {
        FsOp::AtomicWriteFile => super::app_fs::atomic_native(path, data.unwrap_or(&[])),
        FsOp::ReadFile => open_native(path)
            .and_then(read_result)
            .map(FsResult::Bytes)
            .map_err(failed),
        FsOp::WriteFile => std::fs::write(path, data.unwrap_or(&[]))
            .map(|_| FsResult::Done)
            .map_err(failed),
        FsOp::AppendFile => {
            use std::io::Write;
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .and_then(|mut file| file.write_all(data.unwrap_or(&[])))
                .map(|_| FsResult::Done)
                .map_err(failed)
        }
        FsOp::ReadDir => {
            let mut names = Vec::new();
            let mut used = 0;
            for entry in std::fs::read_dir(path).map_err(failed)? {
                let entry = entry.map_err(failed)?;
                push_name(
                    &mut names,
                    &mut used,
                    entry.file_name().to_string_lossy().into_owned(),
                )
                .map_err(failed)?;
            }
            // Sorted, so a directory listing does not depend on filesystem
            // ordering and a test can assert it.
            names.sort();
            Ok(FsResult::Names(names))
        }
        FsOp::Mkdir => std::fs::create_dir_all(path)
            .map(|_| FsResult::Done)
            .map_err(failed),
        #[cfg(unix)]
        FsOp::Remove => super::app_fs::remove_native(path),
        #[cfg(not(unix))]
        FsOp::Remove => {
            let result = if path.is_dir() {
                std::fs::remove_dir_all(path)
            } else {
                std::fs::remove_file(path)
            };
            match result {
                Ok(()) => Ok(FsResult::Done),
                // Removing what is not there is not an error, matching
                // `rm({force: true})`, which is the only shape v1 offers.
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(FsResult::Done),
                Err(e) => Err(failed(e)),
            }
        }
        FsOp::Stat => {
            let meta = std::fs::metadata(path).map_err(failed)?;
            let modified_ms = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            Ok(FsResult::Stat(Stat {
                size: meta.len(),
                is_file: meta.is_file(),
                is_directory: meta.is_dir(),
                modified_ms,
            }))
        }
        FsOp::Rename => std::fs::rename(
            path,
            destination
                .ok_or_else(|| HostError::InvalidArgument("rename needs a destination".into()))?,
        )
        .map(|_| FsResult::Done)
        .map_err(failed),
        FsOp::CopyFile => std::fs::copy(
            path,
            destination
                .ok_or_else(|| HostError::InvalidArgument("copyFile needs a destination".into()))?,
        )
        .map(|_| FsResult::Done)
        .map_err(failed),
        FsOp::Realpath => std::fs::canonicalize(path)
            .map(|p| FsResult::Text(p.to_string_lossy().into_owned()))
            .map_err(failed),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FsResult {
    Done,
    Bytes(Vec<u8>),
    Text(String),
    Names(Vec<String>),
    Stat(Stat),
}

/// Execute a filesystem operation with authority checked in its original namespace.
pub fn run(
    grants: &GrantSet,
    directories: Option<&super::app_fs::AppDirectories>,
    op: FsOp,
    path: &str,
    destination: Option<&str>,
    data: Option<&[u8]>,
) -> Result<FsResult, HostError> {
    if path.starts_with("app:") || destination.is_some_and(|p| p.starts_with("app:")) {
        let directories = directories
            .ok_or_else(|| HostError::Failed("app directories are not configured".into()))?;
        return directories.run(grants, op, path, destination, data);
    }
    if path.starts_with("doc:") || destination.is_some_and(|p| p.starts_with("doc:")) {
        return Err(HostError::Failed("unsupported: no document service".into()));
    }
    #[cfg(windows)]
    {
        super::windows_fs::run_native(grants, op, path, destination, data)
    }
    #[cfg(not(windows))]
    {
        let path = normalize(path)?;
        let destination = destination.map(normalize).transpose()?;
        admit(grants, op, &path, destination.as_deref())?;
        perform(op, &path, destination.as_deref(), data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use crate::grant::{Grant, PathPrefix};

    #[cfg(unix)]
    fn granted(read: &str, write: &str) -> GrantSet {
        GrantSet::none()
            .with(Grant::FsRead(PathPrefix::new(read).unwrap()))
            .with(Grant::FsWrite(PathPrefix::new(write).unwrap()))
    }

    #[test]
    #[cfg(unix)]
    fn paths_are_normalized_lexically_and_must_be_absolute() {
        assert_eq!(normalize("/a/b/../c").unwrap(), PathBuf::from("/a/c"));
        assert_eq!(normalize("/a/./b").unwrap(), PathBuf::from("/a/b"));
        assert_eq!(normalize("/a//b").unwrap(), PathBuf::from("/a/b"));
        assert!(normalize("relative/path").is_err());
        assert!(normalize("/../escape").is_err());
    }

    /// The traversal that a grant check on the raw string would admit.
    #[test]
    #[cfg(unix)]
    fn a_traversal_cannot_reach_outside_its_prefix() {
        let grants = granted("/data", "/data");
        let path = normalize("/data/../etc/passwd").unwrap();
        assert_eq!(path, PathBuf::from("/etc/passwd"));
        assert!(
            admit(&grants, FsOp::ReadFile, &path, None).is_err(),
            "normalization must happen before the check, not after"
        );
    }

    #[test]
    #[cfg(unix)]
    fn read_and_write_need_their_own_grants() {
        let read_only = GrantSet::none().with(Grant::FsRead(PathPrefix::new("/data").unwrap()));
        let p = normalize("/data/x").unwrap();
        assert!(admit(&read_only, FsOp::ReadFile, &p, None).is_ok());
        assert!(admit(&read_only, FsOp::WriteFile, &p, None).is_err());
        assert!(admit(&read_only, FsOp::Remove, &p, None).is_err());
        assert!(admit(&read_only, FsOp::Mkdir, &p, None).is_err());
    }

    /// The case a single-path check gets silently wrong: a read-only grant must
    /// not be able to MOVE a file, and a write grant on the destination must
    /// not license reading a source it was never given.
    #[test]
    #[cfg(unix)]
    fn a_two_path_operation_checks_both_paths() {
        let grants = granted("/src", "/dst");
        let source = normalize("/src/a").unwrap();
        let destination = normalize("/dst/a").unwrap();
        assert!(admit(&grants, FsOp::Rename, &source, Some(&destination)).is_err());
        assert!(admit(&grants, FsOp::CopyFile, &source, Some(&destination)).is_ok());

        // Writing somewhere ungranted.
        let elsewhere = normalize("/other/a").unwrap();
        assert!(admit(&grants, FsOp::Rename, &source, Some(&elsewhere)).is_err());
        // Reading from somewhere ungranted.
        assert!(admit(&grants, FsOp::CopyFile, &elsewhere, Some(&destination)).is_err());
        // And a two-path op with no destination is an argument error, not a
        // silently single-path one.
        assert!(admit(&grants, FsOp::Rename, &source, None).is_err());
    }

    #[test]
    #[cfg(unix)]
    fn prefixes_match_whole_components() {
        let grants = granted("/data", "/data");
        assert!(admit(&grants, FsOp::ReadFile, Path::new("/data/x"), None).is_ok());
        assert!(admit(&grants, FsOp::ReadFile, Path::new("/data2/x"), None).is_err());
        assert!(admit(&grants, FsOp::ReadFile, Path::new("/database/x"), None).is_err());
    }

    #[test]
    fn operations_round_trip_on_a_real_directory() {
        let dir = std::env::temp_dir().join(format!("ibex2-fs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        perform(FsOp::Mkdir, &dir, None, None).expect("mkdir");

        let file = dir.join("a.txt");
        perform(FsOp::WriteFile, &file, None, Some(b"hello")).expect("write");
        assert_eq!(
            perform(FsOp::ReadFile, &file, None, None).unwrap(),
            FsResult::Bytes(b"hello".to_vec())
        );

        perform(FsOp::AppendFile, &file, None, Some(b" there")).expect("append");
        assert_eq!(
            perform(FsOp::ReadFile, &file, None, None).unwrap(),
            FsResult::Bytes(b"hello there".to_vec())
        );

        let copy = dir.join("b.txt");
        perform(FsOp::CopyFile, &file, Some(&copy), None).expect("copy");
        assert_eq!(
            perform(FsOp::ReadDir, &dir, None, None).unwrap(),
            FsResult::Names(vec!["a.txt".into(), "b.txt".into()])
        );

        match perform(FsOp::Stat, &file, None, None).unwrap() {
            FsResult::Stat(stat) => {
                assert_eq!(stat.size, 11);
                assert!(stat.is_file && !stat.is_directory);
            }
            other => panic!("unexpected {other:?}"),
        }

        perform(FsOp::Remove, &copy, None, None).expect("rm");
        // Removing what is gone is not an error.
        perform(FsOp::Remove, &copy, None, None).expect("rm again");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

// @ref LLP 0068.001#3-chosen-document-provider — one provider contract for both doors.
pub use super::documents::{
    run_document, DocumentAccess, DocumentError, DocumentErrorKind, DocumentLease, Documents,
    DOCUMENT_PREFIX,
};

/// Dispatch with an optional chosen-document service. Mixed namespaces refuse.
#[allow(clippy::too_many_arguments)]
pub fn run_with_documents(
    grants: &GrantSet,
    directories: Option<&super::app_fs::AppDirectories>,
    documents: Option<&Documents>,
    op: FsOp,
    path: &str,
    destination: Option<&str>,
    data: Option<&[u8]>,
) -> Result<FsResult, HostError> {
    if path.starts_with("doc:") || destination.is_some_and(|p| p.starts_with("doc:")) {
        if destination.is_some() {
            return Err(HostError::Failed(
                "unsupported: two-path document operation".into(),
            ));
        }
        return run_document(grants, documents, op, path, data);
    }
    run(grants, directories, op, path, destination, data)
}

/// Execute with a consumer-selected envelope, refusing during reads/enumeration.
/// Bounds may tighten the default 16 MiB envelope; Rust bounded reads are independent.
#[allow(clippy::too_many_arguments)]
pub fn run_with_limits(
    grants: &GrantSet,
    directories: Option<&super::app_fs::AppDirectories>,
    documents: Option<&Documents>,
    limits: ResultLimits,
    op: FsOp,
    path: &str,
    destination: Option<&str>,
    data: Option<&[u8]>,
) -> Result<FsResult, HostError> {
    with_result_limits(limits, || {
        let result =
            run_with_documents(grants, directories, documents, op, path, destination, data)?;
        let size = match &result {
            FsResult::Bytes(bytes) => bytes
                .len()
                .checked_add(2)
                .and_then(|n| n.checked_div(3))
                .and_then(|n| n.checked_mul(4))
                .and_then(|n| n.checked_add(13)),
            FsResult::Text(text) => serde_json::to_string(text).ok().map(|s| s.len()),
            FsResult::Names(names) => serde_json::to_vec(names).ok().map(|s| s.len()),
            FsResult::Stat(stat) => serde_json::to_vec(&serde_json::json!({"size":stat.size,"isFile":stat.is_file,"isDirectory":stat.is_directory,"modifiedMs":stat.modified_ms})).ok().map(|s| s.len()),
            FsResult::Done => Some(4),
        };
        if size.is_none_or(|n| n > limits.bytes) {
            return Err(HostError::Failed(RESULT_LIMIT.into()));
        }
        Ok(result)
    })
}

#[cfg(all(test, unix))]
mod bounded_tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    fn fixture() -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "ibex-bounds-{}",
            super::super::crypto::random_uuid().unwrap()
        ));
        std::fs::create_dir(&p).unwrap();
        p
    }
    #[test]
    fn growth_after_metadata_is_stopped_at_the_probe_byte_and_cancel_stops_next_chunk() {
        let root = fixture();
        let path = root.join("grow");
        std::fs::write(&path, b"x").unwrap();
        let calls = AtomicUsize::new(0);
        let file = std::fs::File::open(&path).unwrap();
        let err = read_bounded_file(file, 4, &|| {
            if calls.fetch_add(1, Ordering::Relaxed) == 0 {
                std::fs::OpenOptions::new()
                    .write(true)
                    .open(&path)
                    .unwrap()
                    .set_len(100)
                    .unwrap();
            }
            false
        })
        .unwrap_err();
        assert!(err.to_string().contains(RESULT_LIMIT));
        let file = std::fs::File::open(&path).unwrap();
        let calls = AtomicUsize::new(0);
        assert_eq!(
            read_bounded_file(file, 100, &|| calls.fetch_add(1, Ordering::Relaxed) == 1)
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::Interrupted
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn configured_envelopes_count_base64_and_escaped_names_before_retaining_them() {
        let root = fixture();
        let path = root.join("bytes");
        std::fs::write(&path, b"abcd").unwrap();
        let grants = GrantSet::parse(&format!("fs.read \"{}\"", root.display())).unwrap();
        assert!(run_with_limits(
            &grants,
            None,
            None,
            ResultLimits { bytes: 17 },
            FsOp::ReadFile,
            path.to_str().unwrap(),
            None,
            None
        )
        .is_err());
        std::fs::write(&path, b"abc").unwrap();
        assert_eq!(
            run_with_limits(
                &grants,
                None,
                None,
                ResultLimits { bytes: 17 },
                FsOp::ReadFile,
                path.to_str().unwrap(),
                None,
                None
            )
            .unwrap(),
            FsResult::Bytes(b"abc".to_vec())
        );
        with_result_limits(ResultLimits { bytes: 16 }, || {
            let mut names = vec![];
            let mut used = 0;
            push_name(&mut names, &mut used, "a".into()).unwrap();
            assert!(push_name(&mut names, &mut used, "\n\n\n\n\n\n".into()).is_err());
            assert_eq!(names, ["a"]);
            Ok(())
        })
        .unwrap();
        assert_eq!(RESULT_BUDGET.with(|limit| limit.get()), RESULT_BYTES);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn bounded_reader_refuses_nonregular_opened_handles() {
        let root = fixture();
        assert!(read_bounded_file(std::fs::File::open(&root).unwrap(), 8, &|| false).is_err());
        let mut fds = [0; 2];
        assert_eq!(unsafe { libc::pipe(fds.as_mut_ptr()) }, 0);
        use std::os::fd::FromRawFd;
        let reader = unsafe { std::fs::File::from_raw_fd(fds[0]) };
        let writer = unsafe { std::fs::File::from_raw_fd(fds[1]) };
        assert!(read_bounded_file(reader, 8, &|| false).is_err());
        drop(writer);
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(all(test, windows))]
#[path = "windows_document_tests.rs"]
mod windows_document_tests;
