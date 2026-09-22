//! Host-selected app directories, anchored by directory descriptors.
//! @ref LLP 0059.000#311-fs--delegating-capability-bearing-author-required — app paths
use crate::stdlib::fs::{FsOp, FsResult, Stat};
use crate::{
    boundary::HostError,
    grant::{GrantSet, Operation},
};
use std::{
    ffi::{CStr, CString},
    fs::File,
    io::{self, Read, Write},
    os::fd::{AsRawFd, FromRawFd},
    path::Path,
    sync::Arc,
};

/// The host supplies existing absolute directories. Handles pin the selected
/// directories even if their names are subsequently moved or replaced.
#[derive(Clone, Debug)]
pub struct AppDirectories {
    roots: Arc<[File; 3]>,
    paths: Arc<[std::path::PathBuf; 3]>,
}
fn error(e: impl std::fmt::Display) -> HostError {
    HostError::Failed(format!("filesystem: {e}"))
}
fn name(s: &str) -> io::Result<CString> {
    CString::new(s).map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "NUL in path"))
}
fn open_at(dir: &File, path: &str, flags: i32) -> io::Result<File> {
    let path = name(path)?;
    // SAFETY: the descriptor and terminated pathname are live throughout openat.
    let fd = unsafe {
        libc::openat(
            dir.as_raw_fd(),
            path.as_ptr(),
            flags | libc::O_CLOEXEC | libc::O_NOFOLLOW,
            0o600,
        )
    };
    if fd < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(unsafe { File::from_raw_fd(fd) })
    }
}
fn directory(dir: &File, path: &str) -> io::Result<File> {
    open_at(dir, path, libc::O_RDONLY | libc::O_DIRECTORY)
}
fn mkdir(dir: &File, path: &str) -> io::Result<()> {
    let path = name(path)?;
    if unsafe { libc::mkdirat(dir.as_raw_fd(), path.as_ptr(), 0o700) } == 0 {
        Ok(())
    } else {
        let e = io::Error::last_os_error();
        if e.kind() == io::ErrorKind::AlreadyExists {
            Ok(())
        } else {
            Err(e)
        }
    }
}
fn unlink(dir: &File, path: &str, is_dir: bool) -> io::Result<()> {
    let path = name(path)?;
    if unsafe {
        libc::unlinkat(
            dir.as_raw_fd(),
            path.as_ptr(),
            if is_dir { libc::AT_REMOVEDIR } else { 0 },
        )
    } == 0
    {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}
fn rename(from: &File, source: &str, to: &File, target: &str) -> io::Result<()> {
    let source = name(source)?;
    let target = name(target)?;
    if unsafe {
        libc::renameat(
            from.as_raw_fd(),
            source.as_ptr(),
            to.as_raw_fd(),
            target.as_ptr(),
        )
    } == 0
    {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}
fn names(dir: &File) -> io::Result<Vec<String>> {
    let duplicate = directory(dir, ".")?;
    use std::os::fd::IntoRawFd;
    let fd = duplicate.into_raw_fd();
    let stream = unsafe { libc::fdopendir(fd) };
    if stream.is_null() {
        unsafe { libc::close(fd) };
        return Err(io::Error::last_os_error());
    }
    let mut out = Vec::new();
    loop {
        let entry = unsafe { libc::readdir(stream) };
        if entry.is_null() {
            break;
        }
        let s = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) }
            .to_string_lossy()
            .into_owned();
        if s != "." && s != ".." {
            out.push(s);
        }
    }
    unsafe { libc::closedir(stream) };
    out.sort();
    Ok(out)
}
fn remove(dir: &File, path: &str) -> io::Result<()> {
    match directory(dir, path) {
        Ok(child) => {
            for entry in names(&child)? {
                remove(&child, &entry)?;
            }
            unlink(dir, path, true)
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(_) => unlink(dir, path, false),
    }
}
fn atomic(dir: &File, target: &str, data: &[u8]) -> io::Result<()> {
    let mut bytes = [0; 16];
    getrandom::getrandom(&mut bytes).map_err(|e| io::Error::other(e.to_string()))?;
    let temp = format!(
        ".ibex-{}",
        bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
    );
    let mut file = open_at(dir, &temp, libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL)?;
    let result = (|| {
        file.write_all(data)?;
        file.sync_all()?;
        rename(dir, &temp, dir, target)
    })();
    if result.is_err() {
        let _ = unlink(dir, &temp, false);
    }
    result
}

impl AppDirectories {
    pub fn new(
        data: impl AsRef<Path>,
        cache: impl AsRef<Path>,
        tmp: impl AsRef<Path>,
    ) -> Result<Self, HostError> {
        fn root(path: &Path) -> Result<File, HostError> {
            if !path.is_absolute() {
                return Err(error("app directory must be absolute"));
            }
            let file = File::open(path).map_err(error)?;
            if !file.metadata().map_err(error)?.is_dir() {
                return Err(error("app root is not a directory"));
            }
            Ok(file)
        }
        let roots = [
            root(data.as_ref())?,
            root(cache.as_ref())?,
            root(tmp.as_ref())?,
        ];
        let paths = [
            std::fs::canonicalize(data).map_err(error)?,
            std::fs::canonicalize(cache).map_err(error)?,
            std::fs::canonicalize(tmp).map_err(error)?,
        ];
        Ok(Self {
            roots: Arc::new(roots),
            paths: Arc::new(paths),
        })
    }
    fn parse<'a>(&self, path: &'a str) -> Result<(usize, Vec<&'a str>), HostError> {
        let tail = path
            .strip_prefix("app:/")
            .ok_or_else(|| error("cannot mix app and native paths"))?;
        let mut components = tail.split('/').filter(|c| !c.is_empty());
        let index = match components.next() {
            Some("data") => 0,
            Some("cache") => 1,
            Some("tmp") => 2,
            _ => return Err(error("unknown app directory")),
        };
        let parts: Vec<_> = components.collect();
        if parts
            .iter()
            .any(|s| *s == ".." || *s == "." || s.contains('\0'))
        {
            return Err(error("app path contains traversal or NUL"));
        }
        Ok((index, parts))
    }
    pub(crate) fn open_parent(&self, path: &str) -> Result<(File, String), HostError> {
        let (index, parts) = self.parse(path)?;
        let (leaf, parents) = parts
            .split_last()
            .ok_or_else(|| error("operation needs a path below the app root"))?;
        let mut dir = self.roots[index].try_clone().map_err(error)?;
        for part in parents {
            dir = directory(&dir, part).map_err(error)?;
        }
        Ok((dir, (*leaf).to_owned()))
    }
    /// SQLite's native VFS needs a physical filename. The host must keep this
    /// directory ancestry stable while a database is open. FS operations do not
    /// have this restriction: they use the retained descriptors directly.
    pub(crate) fn sqlite_path(&self, path: &str) -> Result<std::path::PathBuf, HostError> {
        use std::os::unix::fs::MetadataExt;
        let (index, parts) = self.parse(path)?;
        let (parent, leaf) = self.open_parent(path)?;
        let physical = self.paths[index].join(parts.join("/"));
        let actual = std::fs::metadata(physical.parent().unwrap()).map_err(error)?;
        let pinned = parent.metadata().map_err(error)?;
        if (actual.dev(), actual.ino()) != (pinned.dev(), pinned.ino()) {
            return Err(error("app directory was replaced"));
        }
        match open_at(&parent, &leaf, libc::O_RDONLY | libc::O_NONBLOCK) {
            Ok(file) if !file.metadata().map_err(error)?.is_file() => {
                return Err(error("database is not a regular file"))
            }
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(error(e)),
        }
        Ok(physical)
    }
    fn open(&self, path: &str) -> Result<File, HostError> {
        let (index, parts) = self.parse(path)?;
        if parts.is_empty() {
            return self.roots[index].try_clone().map_err(error);
        }
        let (parent, leaf) = self.open_parent(path)?;
        open_at(&parent, &leaf, libc::O_RDONLY | libc::O_NONBLOCK).map_err(error)
    }
    pub(crate) fn run(
        &self,
        grants: &GrantSet,
        op: FsOp,
        path: &str,
        destination: Option<&str>,
        data: Option<&[u8]>,
    ) -> Result<FsResult, HostError> {
        self.parse(path)?;
        let check = |write: bool, p: &str| {
            crate::boundary::admit(
                grants,
                &if write {
                    Operation::FsWrite { path: p.into() }
                } else {
                    Operation::FsRead { path: p.into() }
                },
            )
        };
        let (read, write) = op.required();
        if read {
            check(false, path)?;
        }
        if op.takes_second_path() {
            let to = destination.ok_or_else(|| error("operation needs destination"))?;
            self.parse(to)?;
            check(true, to)?;
            if op == FsOp::Rename {
                check(true, path)?;
            }
        } else if write {
            check(true, path)?;
        }
        let data = data.unwrap_or(&[]);
        match op {
            FsOp::ReadFile => {
                let mut bytes = Vec::new();
                let mut file = self.open(path)?;
                if !file.metadata().map_err(error)?.is_file() {
                    return Err(error("read needs a regular file"));
                }
                file.read_to_end(&mut bytes).map_err(error)?;
                Ok(FsResult::Bytes(bytes))
            }
            FsOp::ReadDir => Ok(FsResult::Names(names(&self.open(path)?).map_err(error)?)),
            FsOp::Stat => {
                let m = self.open(path)?.metadata().map_err(error)?;
                Ok(FsResult::Stat(Stat {
                    size: m.len(),
                    is_file: m.is_file(),
                    is_directory: m.is_dir(),
                    modified_ms: m
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_millis() as u64)
                        .unwrap_or(0),
                }))
            }
            FsOp::Realpath => {
                self.open(path)?;
                let (i, parts) = self.parse(path)?;
                let root = ["data", "cache", "tmp"][i];
                Ok(FsResult::Text(if parts.is_empty() {
                    format!("app:/{root}")
                } else {
                    format!("app:/{root}/{}", parts.join("/"))
                }))
            }
            FsOp::Mkdir => {
                let (i, parts) = self.parse(path)?;
                let mut dir = self.roots[i].try_clone().map_err(error)?;
                for part in parts {
                    mkdir(&dir, part).map_err(error)?;
                    dir = directory(&dir, part).map_err(error)?;
                }
                Ok(FsResult::Done)
            }
            FsOp::CopyFile => {
                let mut source = self.open(path)?;
                let (parent, leaf) = self.open_parent(destination.unwrap())?;
                let mut target = open_at(
                    &parent,
                    &leaf,
                    libc::O_WRONLY | libc::O_CREAT | libc::O_NONBLOCK,
                )
                .map_err(error)?;
                use std::os::unix::fs::MetadataExt;
                let a = source.metadata().map_err(error)?;
                let b = target.metadata().map_err(error)?;
                if !a.is_file() || !b.is_file() || (a.dev(), a.ino()) == (b.dev(), b.ino()) {
                    return Err(error("copy requires distinct regular files"));
                }
                target.set_len(0).map_err(error)?;
                io::copy(&mut source, &mut target).map_err(error)?;
                Ok(FsResult::Done)
            }
            _ => {
                let (parent, leaf) = self.open_parent(path)?;
                match op {
                    FsOp::WriteFile | FsOp::AppendFile => {
                        let flags = libc::O_WRONLY
                            | libc::O_CREAT
                            | libc::O_NONBLOCK
                            | if op == FsOp::AppendFile {
                                libc::O_APPEND
                            } else {
                                0
                            };
                        let mut file = open_at(&parent, &leaf, flags).map_err(error)?;
                        if !file.metadata().map_err(error)?.is_file() {
                            return Err(error("write needs a regular file"));
                        }
                        if op == FsOp::WriteFile {
                            file.set_len(0).map_err(error)?;
                        }
                        file.write_all(data).map_err(error)?;
                    }
                    FsOp::AtomicWriteFile => atomic(&parent, &leaf, data).map_err(error)?,
                    FsOp::Remove => remove(&parent, &leaf).map_err(error)?,
                    FsOp::Rename => {
                        let (to, target) = self.open_parent(destination.unwrap())?;
                        rename(&parent, &leaf, &to, &target).map_err(error)?;
                    }
                    _ => unreachable!(),
                }
                Ok(FsResult::Done)
            }
        }
    }
}
pub(crate) fn atomic_native(path: &Path, data: &[u8]) -> Result<FsResult, HostError> {
    let parent = path
        .parent()
        .ok_or_else(|| error("atomic write needs a parent"))?;
    let leaf = path
        .file_name()
        .and_then(|p| p.to_str())
        .ok_or_else(|| error("atomic write needs a filename"))?;
    atomic(&File::open(parent).map_err(error)?, leaf, data).map_err(error)?;
    Ok(FsResult::Done)
}

/// Admit a database in its logical namespace before selecting a native filename.
/// The host owns the selected ancestry for the lifetime of the connection.
pub fn resolve_sqlite(
    grants: &GrantSet,
    directories: Option<&AppDirectories>,
    path: &str,
) -> Result<std::path::PathBuf, HostError> {
    if path.starts_with("app:") {
        crate::boundary::admit(grants, &Operation::SqliteOpen { path: path.into() })?;
        directories
            .ok_or_else(|| error("app directories are not configured"))?
            .sqlite_path(path)
    } else {
        let path = crate::stdlib::fs::normalize(path)?;
        crate::boundary::admit(
            grants,
            &Operation::SqliteOpen {
                path: path.to_string_lossy().into_owned(),
            },
        )?;
        let real = crate::stdlib::fs::realize(&path);
        crate::boundary::admit(
            &grants.realized_fs(),
            &Operation::SqliteOpen {
                path: real.to_string_lossy().into_owned(),
            },
        )?;
        Ok(real)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stdlib::fs::run;
    use std::path::PathBuf;
    struct Fixture {
        path: PathBuf,
        dirs: AppDirectories,
    }
    impl Fixture {
        fn new() -> Self {
            let id = crate::stdlib::crypto::random_uuid().unwrap();
            let path = std::env::temp_dir().join(format!("ibex-app-fs-{id}"));
            for root in ["data", "cache", "tmp"] {
                std::fs::create_dir_all(path.join(root)).unwrap();
            }
            let dirs = AppDirectories::new(path.join("data"), path.join("cache"), path.join("tmp"))
                .unwrap();
            Self { path, dirs }
        }
        fn call(
            &self,
            op: FsOp,
            path: &str,
            destination: Option<&str>,
            data: Option<&[u8]>,
        ) -> Result<FsResult, HostError> {
            run(
                &GrantSet::parse("fs.read app:/\nfs.write app:/").unwrap(),
                Some(&self.dirs),
                op,
                path,
                destination,
                data,
            )
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
    #[test]
    fn app_roots_are_explicit_and_separate_from_native_authority() {
        let f = Fixture::new();
        let logical = GrantSet::parse("fs.read app:/data\nfs.write app:/data").unwrap();
        assert!(run(
            &logical,
            None,
            FsOp::WriteFile,
            "app:/data/file",
            None,
            Some(b"x")
        )
        .is_err());
        assert!(run(
            &logical,
            Some(&f.dirs),
            FsOp::WriteFile,
            "app:/cache/file",
            None,
            Some(b"x")
        )
        .is_err());
        assert!(run(
            &logical,
            Some(&f.dirs),
            FsOp::WriteFile,
            f.path.join("data/file").to_str().unwrap(),
            None,
            Some(b"x")
        )
        .is_err());
        let native = GrantSet::parse(&format!("fs.write {}", f.path.display())).unwrap();
        assert!(run(
            &native,
            Some(&f.dirs),
            FsOp::WriteFile,
            "app:/data/file",
            None,
            Some(b"x")
        )
        .is_err());
        assert!(AppDirectories::new("relative", "relative", "relative").is_err());
        assert!(AppDirectories::new(f.path.join("absent"), &f.path, &f.path).is_err());
    }
    #[test]
    fn all_app_operations_round_trip_without_exposing_native_paths() {
        let f = Fixture::new();
        f.call(FsOp::Mkdir, "app:/data/deep/dir", None, None)
            .unwrap();
        f.call(FsOp::WriteFile, "app:/data/deep/dir/a", None, Some(b"one"))
            .unwrap();
        f.call(FsOp::AppendFile, "app:/data/deep/dir/a", None, Some(b"two"))
            .unwrap();
        assert_eq!(
            f.call(FsOp::ReadFile, "app:/data/deep/dir/a", None, None)
                .unwrap(),
            FsResult::Bytes(b"onetwo".to_vec())
        );
        f.call(
            FsOp::CopyFile,
            "app:/data/deep/dir/a",
            Some("app:/cache/copy"),
            None,
        )
        .unwrap();
        f.call(
            FsOp::Rename,
            "app:/cache/copy",
            Some("app:/tmp/moved"),
            None,
        )
        .unwrap();
        assert_eq!(
            f.call(FsOp::Realpath, "app:/tmp//moved", None, None)
                .unwrap(),
            FsResult::Text("app:/tmp/moved".into())
        );
        assert_eq!(
            f.call(FsOp::ReadDir, "app:/tmp", None, None).unwrap(),
            FsResult::Names(vec!["moved".into()])
        );
        let FsResult::Stat(stat) = f.call(FsOp::Stat, "app:/tmp/moved", None, None).unwrap() else {
            panic!()
        };
        assert_eq!(stat.size, 6);
        assert!(stat.is_file);
        f.call(FsOp::Remove, "app:/data/deep", None, None).unwrap();
        f.call(FsOp::Remove, "app:/data/deep", None, None).unwrap();
        assert_eq!(
            f.call(FsOp::ReadDir, "app:/data", None, None).unwrap(),
            FsResult::Names(vec![])
        );
    }
    #[test]
    fn traversal_and_symlink_escape_fail_closed_and_root_replacement_stays_pinned() {
        use std::os::unix::fs::symlink;
        let f = Fixture::new();
        std::fs::write(f.path.join("cache/secret"), b"private").unwrap();
        symlink(f.path.join("cache"), f.path.join("data/link")).unwrap();
        symlink(f.path.join("cache/secret"), f.path.join("data/file")).unwrap();
        for path in [
            "app:/data/../cache/secret",
            "app:/data/link/secret",
            "app:/data/file",
            "app:/data/./file",
        ] {
            assert!(f.call(FsOp::ReadFile, path, None, None).is_err(), "{path}");
            assert!(
                f.call(FsOp::WriteFile, path, None, Some(b"wrong")).is_err(),
                "{path}"
            );
        }
        assert_eq!(
            std::fs::read(f.path.join("cache/secret")).unwrap(),
            b"private"
        );
        std::fs::rename(f.path.join("data"), f.path.join("old-data")).unwrap();
        symlink(f.path.join("cache"), f.path.join("data")).unwrap();
        f.call(FsOp::WriteFile, "app:/data/new", None, Some(b"pinned"))
            .unwrap();
        assert!(f.path.join("old-data/new").exists());
        assert!(!f.path.join("cache/new").exists());
        assert!(f.dirs.sqlite_path("app:/data/new.db").is_err());
    }
    #[test]
    fn rename_requires_source_write_and_copy_does_not() {
        let f = Fixture::new();
        f.call(FsOp::WriteFile, "app:/data/a", None, Some(b"keep"))
            .unwrap();
        let grants = GrantSet::parse("fs.read app:/data\nfs.write app:/cache").unwrap();
        assert!(run(
            &grants,
            Some(&f.dirs),
            FsOp::Rename,
            "app:/data/a",
            Some("app:/cache/a"),
            None
        )
        .is_err());
        run(
            &grants,
            Some(&f.dirs),
            FsOp::CopyFile,
            "app:/data/a",
            Some("app:/cache/a"),
            None,
        )
        .unwrap();
        assert_eq!(std::fs::read(f.path.join("data/a")).unwrap(), b"keep");
    }
    #[test]
    fn atomic_replace_is_complete_under_concurrency_and_cleans_up_failure() {
        let f = Fixture::new();
        f.call(
            FsOp::AtomicWriteFile,
            "app:/data/value",
            None,
            Some(&vec![0; 8192]),
        )
        .unwrap();
        let dirs = &f.dirs;
        std::thread::scope(|scope| {
            for value in 1..5u8 {
                scope.spawn(move || {
                    let grants = GrantSet::parse("fs.write app:/data/value").unwrap();
                    for _ in 0..16 {
                        run(
                            &grants,
                            Some(dirs),
                            FsOp::AtomicWriteFile,
                            "app:/data/value",
                            None,
                            Some(&vec![value; 8192]),
                        )
                        .unwrap();
                    }
                });
            }
            for _ in 0..128 {
                let bytes = std::fs::read(f.path.join("data/value")).unwrap();
                assert_eq!(bytes.len(), 8192);
                assert!(bytes.iter().all(|b| *b == bytes[0]));
            }
        });
        f.call(FsOp::Mkdir, "app:/data/is-directory", None, None)
            .unwrap();
        assert!(f
            .call(
                FsOp::AtomicWriteFile,
                "app:/data/is-directory",
                None,
                Some(b"failure")
            )
            .is_err());
        assert_eq!(
            f.call(FsOp::ReadDir, "app:/data", None, None).unwrap(),
            FsResult::Names(vec!["is-directory".into(), "value".into()])
        );
        let native = f.path.join("cache/native");
        atomic_native(&native, b"old").unwrap();
        atomic_native(&native, b"new").unwrap();
        assert_eq!(std::fs::read(native).unwrap(), b"new");
    }
    #[test]
    fn sqlite_uses_only_its_own_grant_in_original_namespace() {
        let f = Fixture::new();
        assert!(resolve_sqlite(
            &GrantSet::parse("fs.write app:/data").unwrap(),
            Some(&f.dirs),
            "app:/data/db"
        )
        .is_err());
        let grants = GrantSet::parse("sqlite.open app:/data").unwrap();
        assert_eq!(
            resolve_sqlite(&grants, Some(&f.dirs), "app:/data/db").unwrap(),
            std::fs::canonicalize(f.path.join("data"))
                .unwrap()
                .join("db")
        );
        assert!(resolve_sqlite(&grants, Some(&f.dirs), "app:/cache/db").is_err());
        assert!(resolve_sqlite(&grants, Some(&f.dirs), "app:/data/../cache/db").is_err());
    }
}
