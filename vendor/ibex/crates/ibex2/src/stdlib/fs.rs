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
#[cfg(unix)]
use crate::grant::{Grant, PathPrefix};
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
    /// Exact patch 24 (exact2 LLP 1133.007 D6a): `stat` without following a
    /// link, a link's target, whether this user may read, write or execute a
    /// path, its permission bits, a symbolic link, a file's digest, and an
    /// advisory lock and its release.
    Lstat,
    Readlink,
    Access,
    Chmod,
    Symlink,
    Digest,
    Flock,
    Unlock,
}

impl FsOp {
    /// Which capability the operation needs, and on which path.
    ///
    /// `rename` and `copyFile` touch two paths and need write on the
    /// destination as well as read on the source. Rename additionally needs
    /// write on the source because it removes that directory entry.
    pub fn required(self) -> (bool, bool) {
        match self {
            FsOp::ReadFile
            | FsOp::ReadDir
            | FsOp::Stat
            | FsOp::Realpath
            | FsOp::Lstat
            | FsOp::Readlink
            | FsOp::Access
            | FsOp::Digest => (true, false),
            // A lock takes its file, made if absent; a link is written where it lives.
            FsOp::Chmod | FsOp::Symlink | FsOp::Flock => (false, true),
            // A lock's release names its handle, which its taking admitted.
            FsOp::Unlock => (false, false),
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
    /// Its permission bits (`0o7777`); on Windows `0o444` or `0o666`.
    pub mode: u32,
    /// An `lstat` of a symbolic link.
    pub is_symlink: bool,
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
        FsOp::Stat => std::fs::metadata(path)
            .map(|m| FsResult::Stat(stat_of(&m)))
            .map_err(failed),
        FsOp::Lstat => std::fs::symlink_metadata(path)
            .map(|m| FsResult::Stat(stat_of(&m)))
            .map_err(failed),
        FsOp::Readlink => std::fs::read_link(path)
            .map(|p| FsResult::Text(p.to_string_lossy().into_owned()))
            .map_err(failed),
        FsOp::Access | FsOp::Chmod | FsOp::Symlink | FsOp::Digest | FsOp::Flock | FsOp::Unlock => {
            super::fs_more::perform(op, path, data)
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

/// A `Stat` from a file's metadata.
pub(crate) fn stat_of(meta: &std::fs::Metadata) -> Stat {
    let modified_ms = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    #[cfg(unix)]
    let mode = std::os::unix::fs::PermissionsExt::mode(&meta.permissions()) & 0o7777;
    #[cfg(not(unix))]
    let mode = if meta.permissions().readonly() {
        0o444
    } else {
        0o666
    };
    Stat {
        size: meta.len(),
        is_file: meta.is_file(),
        is_directory: meta.is_dir(),
        modified_ms,
        mode,
        is_symlink: meta.file_type().is_symlink(),
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

/// The host's directories behind `home:/` and `bundle:/` (Exact patch 24:
/// exact2 LLP 1133.007 D6): the person's home directory and the app's
/// installed bundle, read only. A host that has neither leaves them unset,
/// and those paths refuse as unsupported.
#[derive(Debug, Clone, Default)]
pub struct Roots {
    pub home: Option<PathBuf>,
    pub bundle: Option<PathBuf>,
}

static ROOTS: std::sync::RwLock<Roots> = std::sync::RwLock::new(Roots {
    home: None,
    bundle: None,
});

/// Name the host's `home:/` and `bundle:/` directories, once a process.
pub fn set_roots(roots: Roots) {
    *ROOTS.write().unwrap_or_else(|e| e.into_inner()) = roots;
}

/// A namespace's name, its root as the host named it, and a path's
/// components below that root.
#[cfg(unix)]
type Namespaced = (&'static str, PathBuf, Vec<String>);

/// A `home:` or `bundle:` path's root and its components below it; `None`
/// for any other path. A `.` or `..` component is refused.
#[cfg(unix)]
fn namespaced(path: &str) -> Option<Result<Namespaced, HostError>> {
    let (namespace, rest) = match path.strip_prefix("home:/") {
        Some(rest) => ("home", rest),
        None => ("bundle", path.strip_prefix("bundle:/")?),
    };
    let roots = ROOTS.read().unwrap_or_else(|e| e.into_inner());
    let root = match namespace {
        "home" => roots.home.clone(),
        _ => roots.bundle.clone(),
    };
    Some((|| {
        let root = root.ok_or_else(|| {
            HostError::Failed(format!("unsupported: no {namespace}: directory here"))
        })?;
        let parts: Vec<String> = (rest.split('/').filter(|part| !part.is_empty()))
            .map(str::to_owned)
            .collect();
        if parts
            .iter()
            .any(|part| part == ".." || part == "." || part.contains('\0'))
        {
            return Err(HostError::Failed(format!(
                "TypeError: path has a . or .. part: {path}"
            )));
        }
        Ok((namespace, root, parts))
    })())
}

/// `path` below its root's retained descriptor, each component to be opened
/// `O_NOFOLLOW` (`app_fs::run_below`).
#[cfg(unix)]
fn below(path: &str) -> Result<(super::app_fs::Below, PathBuf), HostError> {
    let (name, root, parts) = namespaced(path).ok_or_else(|| {
        HostError::Failed(format!("unsupported: not a home: or bundle: path: {path}"))
    })??;
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_DIRECTORY | libc::O_CLOEXEC);
    }
    let failed = |e: std::io::Error| HostError::Failed(format!("{path}: {e}"));
    // The root is the host's (the account's home, the app's bundle), opened
    // as named; nothing below it is followed.
    let opened = options.open(&root).map_err(failed)?;
    let native = std::fs::canonicalize(&root)
        .map_err(failed)?
        .join(parts.join("/"));
    Ok((
        super::app_fs::Below {
            root: opened,
            parts,
            name,
        },
        native,
    ))
}

/// A `home:` or `bundle:` operation (exact2 LLP 1133.007 D6, Exact patch 24):
/// admitted as spelt, a `bundle:` path read only, then run below its root's
/// retained descriptor with every component opened `O_NOFOLLOW`, as an
/// `app:/` path is. A link anywhere below the root, one a grant's holder
/// planted included, is refused rather than followed, so the spelt path is
/// the one the operation reaches and admission needs no realized pass.
fn run_namespaced(
    grants: &GrantSet,
    op: FsOp,
    path: &str,
    destination: Option<&str>,
    data: Option<&[u8]>,
) -> Result<FsResult, HostError> {
    let namespace = |p: &str| p.starts_with("home:") || p.starts_with("bundle:");
    if !namespace(path) || destination.is_some_and(|d| !namespace(d)) {
        return Err(HostError::Failed(
            "unsupported: a home: or bundle: path pairs only with another".into(),
        ));
    }
    // A `.` or `..` part is refused before any grant is read.
    #[cfg(unix)]
    for p in [Some(path), destination].into_iter().flatten() {
        if let Some(Err(e)) = namespaced(p) {
            return Err(e);
        }
    }
    let (needs_read, needs_write) = op.required();
    // What is written: a one-path operation's path, a two-path one's
    // destination, and a rename's source, which it removes.
    let written = match destination {
        Some(to) => [Some(to), (op == FsOp::Rename).then_some(path)],
        None => [(needs_write && !needs_read).then_some(path), None],
    };
    if let Some(spelled) = written
        .into_iter()
        .flatten()
        .find(|p| p.starts_with("bundle:"))
    {
        return Err(HostError::Failed(format!(
            "denied: bundle: is read only: {spelled}"
        )));
    }
    admit_as(grants, op, Path::new(path), destination.map(Path::new))?;
    #[cfg(unix)]
    {
        let (source, native) = below(path)?;
        if op == FsOp::Realpath {
            source.open()?;
            return Ok(FsResult::Text(native.display().to_string()));
        }
        let target = destination.map(below).transpose()?.map(|(b, _)| b);
        super::app_fs::run_below(op, source, target, data.unwrap_or(&[]))
    }
    #[cfg(not(unix))]
    {
        let _ = data;
        Err(HostError::Failed(format!("unsupported: no {path} here")))
    }
}

/// Open `path` to read it, admitted under `fs.read` as `storage.fs` admits
/// it: an `app:/` path through the app's directories, a `home:` or `bundle:`
/// one below its root and a native one below its granted directory,
/// `O_NOFOLLOW` all the way. What a child's stdin reads from (exact2 LLP
/// 1133.007 D2's `exactStdinFrom`).
pub fn open_read(
    grants: &GrantSet,
    directories: Option<&super::app_fs::AppDirectories>,
    path: &str,
) -> Result<std::fs::File, HostError> {
    let regular = |file: std::fs::File| -> Result<std::fs::File, HostError> {
        match file.metadata() {
            Ok(m) if m.is_file() => Ok(file),
            _ => Err(HostError::Failed(format!("{path}: not a regular file"))),
        }
    };
    if path.starts_with("app:") {
        let directories = directories
            .ok_or_else(|| HostError::Failed("app directories are not configured".into()))?;
        return directories.open_file(grants, path);
    }
    if path.starts_with("home:") || path.starts_with("bundle:") {
        admit_as(grants, FsOp::ReadFile, Path::new(path), None)?;
        #[cfg(unix)]
        return regular(below(path)?.0.open()?);
        #[cfg(not(unix))]
        return Err(HostError::Failed(format!("unsupported: no {path} here")));
    }
    let native = normalize(path)?;
    #[cfg(unix)]
    {
        admit_as(grants, FsOp::ReadFile, &native, None)?;
        regular(
            native_below(grants, directories, &native, false)?
                .0
                .open()?,
        )
    }
    #[cfg(not(unix))]
    {
        admit(grants, FsOp::ReadFile, &native, None)?;
        regular(open_native(&native).map_err(|e| HostError::Failed(format!("{path}: {e}")))?)
    }
}

/// A native operation (exact2 LLP 1133.007 D6a, Exact patch 24), admitted as
/// spelt, then run below the directory a walk to each path starts from
/// (`native_below`), every component below it opened `O_NOFOLLOW`
/// (`app_fs::run_below`), as a `home:` path is. A link below that directory,
/// or on the way to it one this user can have made (`reach`), is refused
/// rather than followed, so the operation reaches the file it was admitted
/// for, never makes a dangling link's target, and needs no check of a
/// realized path.
#[cfg(unix)]
fn run_native(
    grants: &GrantSet,
    directories: Option<&super::app_fs::AppDirectories>,
    op: FsOp,
    path: &Path,
    destination: Option<&Path>,
    data: Option<&[u8]>,
) -> Result<FsResult, HostError> {
    let (needs_read, needs_write) = op.required();
    // A path is walked from the line its use needs: what is written from an
    // `fs.write` line, a rename's source too, which it removes.
    let written = needs_write && (destination.is_none() || op == FsOp::Rename);
    let (source, native) = native_below(grants, directories, path, written || !needs_read)?;
    if op == FsOp::Realpath {
        source.open()?;
        return Ok(FsResult::Text(native.display().to_string()));
    }
    let target = destination
        .map(|to| native_below(grants, directories, to, true))
        .transpose()?
        .map(|(below, _)| below);
    super::app_fs::run_below(op, source, target, data.unwrap_or(&[]))
}

/// `path` below the directory a walk to it starts from, and its native path.
/// The deepest native `fs.write` (`write`) or `fs.read` line that admits
/// `path` as spelt is reached (`reach`) without following a link this user
/// can have made or one below a directory the app may write; the walk
/// starts where that stopped, and goes through the rest of the line's names
/// and then `path`'s.
#[cfg(unix)]
fn native_below(
    grants: &GrantSet,
    directories: Option<&super::app_fs::AppDirectories>,
    path: &Path,
    write: bool,
) -> Result<(super::app_fs::Below, PathBuf), HostError> {
    let capability = if write { "fs.write" } else { "fs.read" };
    let denied = HostError::Denied { capability };
    let target = PathPrefix::new(&path.to_string_lossy()).ok_or(denied.clone())?;
    let line = (grants.iter())
        .filter_map(|grant| match (grant, write) {
            (Grant::FsWrite(line), true) | (Grant::FsRead(line), false) => Some(line),
            _ => None,
        })
        .filter(|line| line.components().first().is_some_and(String::is_empty))
        .filter(|line| target.components().starts_with(line.components()))
        .max_by_key(|line| line.components().len())
        .ok_or(denied.clone())?;
    let failed = |e: std::io::Error| HostError::Failed(format!("{}: {e}", path.display()));
    let line_path = line_native(line).ok_or(denied)?;
    let (root, at, mut parts) = reach(&line_path, &written(grants, directories)).map_err(failed)?;
    parts.extend(
        target.components()[line.components().len()..]
            .iter()
            .cloned(),
    );
    let native = parts.iter().fold(at, |at, part| at.join(part));
    Ok((
        super::app_fs::Below {
            root,
            parts,
            name: "granted",
        },
        native,
    ))
}

/// A grant line's directory as a native path: a native line as spelt, a
/// `home:` one below the host's home as named; `None` for any other.
#[cfg(unix)]
fn line_native(line: &PathPrefix) -> Option<PathBuf> {
    let (namespace, parts) = line.components().split_first()?;
    let root = match namespace.as_str() {
        "" => PathBuf::from("/"),
        "home:" => (ROOTS.read().unwrap_or_else(|e| e.into_inner()).home.clone())?,
        _ => return None,
    };
    Some(parts.iter().fold(root, |at, part| at.join(part)))
}

/// The directories the app may write natively, by identity (device and
/// inode): each native and `home:` `fs.write` line's, as named now, and the
/// app's own directories. A line below another that the app replaced with a
/// link adds the link's target, which only stops more walks.
#[cfg(unix)]
fn written(
    grants: &GrantSet,
    directories: Option<&super::app_fs::AppDirectories>,
) -> Vec<(u64, u64)> {
    use std::os::unix::fs::MetadataExt;
    let lines = grants.iter().filter_map(|grant| match grant {
        Grant::FsWrite(line) => line_native(line),
        _ => None,
    });
    (lines.filter_map(|dir| std::fs::metadata(dir).ok()))
        .map(|meta| (meta.dev(), meta.ino()))
        .chain(directories.map(|d| d.identities()).unwrap_or_default())
        .collect()
}

/// Open the directory `line` names from `/`, following its links only until
/// one of `stops` is reached, compared by identity, so a spelling that
/// differs (a link, case, Unicode normalization) still stops there. Returns
/// that directory, its path, and the names below it to walk `O_NOFOLLOW`
/// (none when no stop is on the way). A link on the way is followed only
/// when this user can neither have made nor replace it
/// (`made_by_this_user`), as a system link such as macOS's `/var`; any other
/// is refused, naming it. So no grants the app held before, in this launch,
/// an earlier one or another session, can have put a link on the way, and
/// a line spelt through the person's own link (`~/Dropbox`) is refused.
#[cfg(unix)]
fn reach(
    line: &Path,
    stops: &[(u64, u64)],
) -> std::io::Result<(std::fs::File, PathBuf, Vec<String>)> {
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::MetadataExt;
    let named = |path: &Path| -> std::collections::VecDeque<std::ffi::OsString> {
        (path.components())
            .filter_map(|part| match part {
                Component::Normal(name) => Some(name.to_os_string()),
                Component::ParentDir => Some("..".into()),
                _ => None,
            })
            .collect()
    };
    let root = || super::app_fs::open_directory(Path::new("/"));
    let (mut dir, mut at, mut todo, mut links) = (root()?, PathBuf::from("/"), named(line), 0);
    loop {
        let meta = dir.metadata()?;
        if stops.contains(&(meta.dev(), meta.ino())) {
            let mut below: Vec<String> = Vec::new();
            for part in todo {
                let part = part.into_string().map_err(|_| {
                    std::io::Error::new(std::io::ErrorKind::InvalidInput, "not UTF-8")
                })?;
                if part != ".." {
                    below.push(part);
                } else if below.pop().is_none() {
                    // A link's `..` above the stop: what it names is not below it.
                    return Err(std::io::Error::from_raw_os_error(libc::ELOOP));
                }
            }
            return Ok((dir, at, below));
        }
        let Some(part) = todo.pop_front() else {
            return Ok((dir, at, Vec::new()));
        };
        if part == ".." {
            dir = super::app_fs::directory_at(&dir, &part)?;
            at.pop();
            continue;
        }
        let error = match super::app_fs::directory_at(&dir, &part) {
            Ok(next) => {
                (dir, at) = (next, at.join(&part));
                continue;
            }
            Err(error) => error,
        };
        // A link: go on from where it points, as the kernel would, unless
        // this user can have made it.
        let target = super::app_fs::link_at(&dir, &part).map_err(|_| error)?;
        if made_by_this_user(&dir, &part)? {
            let link = at.join(&part);
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                format!(
                    "{} is a link this user could have made, not followed to a grant line; \
                     grant the directory it points to",
                    link.display()
                ),
            ));
        }
        links += 1;
        if links > 40 {
            return Err(std::io::Error::from_raw_os_error(libc::ELOOP));
        }
        if target.as_bytes().first() == Some(&b'/') {
            (dir, at) = (root()?, PathBuf::from("/"));
        }
        for part in named(Path::new(&target)).into_iter().rev() {
            todo.push_front(part);
        }
    }
}

/// Whether the link `name` in `dir` is one this process's user can have made
/// or replaced: the user owns the link or the directory (an owner may make
/// it writable), or may write the directory (`faccessat` with `AT_EACCESS`
/// counts groups and ACLs). Root may write nearly everything, so it follows
/// almost no link. Checked after `link_at`: a directory this user cannot
/// write keeps the link that was read.
#[cfg(unix)]
fn made_by_this_user(dir: &std::fs::File, name: &std::ffi::OsStr) -> std::io::Result<bool> {
    use std::os::fd::AsRawFd;
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::MetadataExt;
    // SAFETY: geteuid has no preconditions.
    let user = unsafe { libc::geteuid() };
    if dir.metadata()?.uid() == user {
        return Ok(true);
    }
    // SAFETY: the descriptor and the terminated name are live for the call.
    let writable = unsafe {
        libc::faccessat(
            dir.as_raw_fd(),
            c".".as_ptr(),
            libc::W_OK,
            crate::AT_EACCESS,
        )
    };
    if writable == 0 {
        return Ok(true);
    }
    // Only a refusal says the directory is closed to this user.
    let refused = std::io::Error::last_os_error().raw_os_error();
    if !matches!(refused, Some(libc::EACCES | libc::EROFS)) {
        return Ok(true);
    }
    let name = std::ffi::CString::new(name.as_bytes())
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "NUL in path"))?;
    let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
    // SAFETY: the descriptor, the terminated name and the buffer are live
    // throughout fstatat, which fills `stat` when it returns 0.
    let got = unsafe {
        libc::fstatat(
            dir.as_raw_fd(),
            name.as_ptr(),
            stat.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    };
    if got != 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: fstatat returned 0, so it filled `stat`.
    Ok(unsafe { stat.assume_init() }.st_uid == user)
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
    if op == FsOp::Unlock {
        return super::fs_more::perform(op, Path::new(path), data);
    }
    let namespace = |p: &str| p.starts_with("home:") || p.starts_with("bundle:");
    if namespace(path) || destination.is_some_and(namespace) {
        return run_namespaced(grants, op, path, destination, data);
    }
    #[cfg(windows)]
    {
        super::windows_fs::run_native(grants, op, path, destination, data)
    }
    #[cfg(not(windows))]
    {
        let path = normalize(path)?;
        let destination = destination.map(normalize).transpose()?;
        // Unix walks from the granted directory, refusing every link below
        // it, so the spelt path is the one reached (`run_native`).
        #[cfg(unix)]
        {
            admit_as(grants, op, &path, destination.as_deref())?;
            run_native(grants, directories, op, &path, destination.as_deref(), data)
        }
        #[cfg(not(unix))]
        {
            admit(grants, op, &path, destination.as_deref())?;
            perform(op, &path, destination.as_deref(), data)
        }
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
    copy_document, run_document, DocumentAccess, DocumentError, DocumentErrorKind, DocumentLease,
    Documents, DOCUMENT_PREFIX,
};

/// Dispatch with an optional chosen-document service. Mixed namespaces
/// refuse, except `copyFile` from a document into an app file.
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
        if let (FsOp::CopyFile, true, Some(to)) = (op, path.starts_with("doc:"), destination) {
            if to.starts_with("app:") {
                return copy_document(grants, directories, documents, path, to);
            }
        }
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
            FsResult::Stat(stat) => serde_json::to_vec(&serde_json::json!({"size":stat.size,"isFile":stat.is_file,"isDirectory":stat.is_directory,"modifiedMs":stat.modified_ms,"mode":stat.mode,"isSymbolicLink":stat.is_symlink})).ok().map(|s| s.len()),
            FsResult::Done => Some(4),
        };
        if size.is_none_or(|n| n > limits.bytes) {
            return Err(HostError::Failed(RESULT_LIMIT.into()));
        }
        Ok(result)
    })
}

#[cfg(all(test, unix))]
#[path = "fs_namespaces_tests.rs"]
mod namespaces_tests;

#[cfg(all(test, unix))]
#[path = "fs_native_tests.rs"]
mod native_tests;

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
