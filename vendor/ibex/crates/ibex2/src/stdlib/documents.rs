//! Chosen documents retain capabilities, never physical pathnames.
//! @ref LLP 0068.001#3-chosen-document-provider — admission precedes trusted minting.
use super::fs::{FsOp, FsResult, Stat};
use crate::{
    boundary::{self, HostError},
    grant::{GrantSet, Operation},
};
use std::{fs::File, sync::Arc};

pub const DOCUMENT_PREFIX: &str = "doc:/";
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentAccess {
    Read,
    Write,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentErrorKind {
    NotFound,
    PermissionDenied,
    InvalidPath,
    Unsupported,
    IsDirectory,
    NotDirectory,
    DirectoryNotEmpty,
    AlreadyExists,
    ResultLimit,
    Io,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DocumentError {
    pub kind: DocumentErrorKind,
}
impl DocumentError {
    pub fn new(kind: DocumentErrorKind) -> Self {
        Self { kind }
    }
    fn guest(self, path: &str) -> HostError {
        if self.kind == DocumentErrorKind::ResultLimit {
            return HostError::Failed(super::fs::RESULT_LIMIT.into());
        }
        HostError::Failed(format!(
            "document {}: {path}",
            match self.kind {
                DocumentErrorKind::NotFound => "ENOENT",
                DocumentErrorKind::PermissionDenied => "EACCES",
                DocumentErrorKind::InvalidPath => "EINVAL",
                DocumentErrorKind::Unsupported => "ENOTSUP",
                DocumentErrorKind::IsDirectory => "EISDIR",
                DocumentErrorKind::NotDirectory => "ENOTDIR",
                DocumentErrorKind::DirectoryNotEmpty => "ENOTEMPTY",
                DocumentErrorKind::AlreadyExists => "EEXIST",
                DocumentErrorKind::ResultLimit => unreachable!("handled above"),
                DocumentErrorKind::Io => "EIO",
            }
        ))
    }
}
impl From<std::io::Error> for DocumentError {
    fn from(error: std::io::Error) -> Self {
        use std::io::ErrorKind;
        // Preserve only Ibex's constant quota marker. Native diagnostic text
        // remains private, since it can contain the physical selected path.
        if error.kind() == ErrorKind::Other && error.to_string() == super::fs::RESULT_LIMIT {
            return Self::new(DocumentErrorKind::ResultLimit);
        }
        Self::new(match error.kind() {
            ErrorKind::IsADirectory => DocumentErrorKind::IsDirectory,
            ErrorKind::NotADirectory => DocumentErrorKind::NotDirectory,
            ErrorKind::DirectoryNotEmpty => DocumentErrorKind::DirectoryNotEmpty,
            ErrorKind::AlreadyExists => DocumentErrorKind::AlreadyExists,
            ErrorKind::NotFound => DocumentErrorKind::NotFound,
            ErrorKind::PermissionDenied => DocumentErrorKind::PermissionDenied,
            ErrorKind::Unsupported => DocumentErrorKind::Unsupported,
            _ => DocumentErrorKind::Io,
        })
    }
}
impl std::fmt::Display for DocumentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "document {:?}", self.kind)
    }
}
impl std::error::Error for DocumentError {}

/// An opaque scope/security lease. Its destructor runs after actual native work.
pub type DocumentGuard = Arc<dyn Send + Sync>;
#[derive(Clone)]
pub struct DocumentLease {
    target: Target,
    _guard: DocumentGuard,
}
#[derive(Clone)]
enum Target {
    Root(String),
    Directory {
        directory: Arc<File>,
        relative: String,
    },
    File {
        directory: Arc<File>,
        leaf: String,
        /// The name the guest path gives it: `leaf`, unless the provider
        /// kept the selection under the name it was chosen by.
        name: String,
    },
}
impl DocumentLease {
    pub fn root(entry: String, guard: DocumentGuard) -> Result<Self, DocumentError> {
        validate_relative(&entry, false)?;
        if entry.contains('/') {
            return Err(DocumentError::new(DocumentErrorKind::InvalidPath));
        }
        Ok(Self {
            target: Target::Root(entry),
            _guard: guard,
        })
    }
    /// `directory` is the chosen directory; `relative` is beneath it, not an absolute path.
    pub fn directory(
        directory: Arc<File>,
        relative: String,
        guard: DocumentGuard,
    ) -> Result<Self, DocumentError> {
        validate_relative(&relative, true)?;
        Ok(Self {
            target: Target::Directory {
                directory,
                relative,
            },
            _guard: guard,
        })
    }
    /// A file selection identifies exactly one leaf name in its retained parent.
    pub fn selected_file(
        directory: Arc<File>,
        leaf: String,
        guard: DocumentGuard,
    ) -> Result<Self, DocumentError> {
        let name = leaf.clone();
        Self::selected_file_named(directory, leaf, name, guard)
    }
    /// A file selection the guest names `name` while it is `leaf` in its
    /// retained parent: one the provider kept across a rename (Exact patch 15).
    pub fn selected_file_named(
        directory: Arc<File>,
        leaf: String,
        name: String,
        guard: DocumentGuard,
    ) -> Result<Self, DocumentError> {
        for part in [&leaf, &name] {
            validate_relative(part, false)?;
            if part.contains('/') {
                return Err(DocumentError::new(DocumentErrorKind::InvalidPath));
            }
        }
        Ok(Self {
            target: Target::File {
                directory,
                leaf,
                name,
            },
            _guard: guard,
        })
    }
}
/// Providers enforce session identity, forgetting, and access mode. Handles are
/// never reassigned in one provider's lifetime. Errors deliberately contain no diagnostic path.
pub type Documents =
    dyn Fn(&str, DocumentAccess) -> Result<DocumentLease, DocumentError> + Send + Sync;
fn validate_relative(path: &str, empty: bool) -> Result<(), DocumentError> {
    if (path.is_empty() && !empty)
        || path.contains(['\\', '\0'])
        || (!path.is_empty()
            && path
                .split('/')
                .any(|p| p.is_empty() || p == "." || p == ".."))
    {
        return Err(DocumentError::new(DocumentErrorKind::InvalidPath));
    }
    Ok(())
}
fn components(path: &str) -> Result<Vec<&str>, DocumentError> {
    let tail = path
        .strip_prefix(DOCUMENT_PREFIX)
        .ok_or_else(|| DocumentError::new(DocumentErrorKind::InvalidPath))?;
    let tail = tail.strip_suffix('/').unwrap_or(tail);
    validate_relative(tail, false)?;
    Ok(tail.split('/').collect())
}
pub fn run_document(
    grants: &GrantSet,
    documents: Option<&Documents>,
    op: FsOp,
    path: &str,
    data: Option<&[u8]>,
) -> Result<FsResult, HostError> {
    // All admission precedes provider table lookup and file metadata/open calls.
    let (read, write) = op.required();
    if read {
        boundary::admit(grants, &Operation::FsRead { path: path.into() })?;
    }
    if write {
        boundary::admit(grants, &Operation::FsWrite { path: path.into() })?;
    }
    let parts = components(path).map_err(|e| e.guest(path))?;
    if matches!(op, FsOp::Rename | FsOp::CopyFile | FsOp::Realpath) {
        return Err(DocumentError::new(DocumentErrorKind::Unsupported).guest(path));
    }
    let documents =
        documents.ok_or_else(|| DocumentError::new(DocumentErrorKind::Unsupported).guest(path))?;
    let lease = documents(
        path,
        if write {
            DocumentAccess::Write
        } else {
            DocumentAccess::Read
        },
    )
    .map_err(|e| e.guest(path))?;
    match &lease.target {
        Target::Root(entry) => match op {
            FsOp::ReadDir => {
                let mut names = Vec::new();
                super::fs::push_name(&mut names, &mut 0, entry.clone())
                    .map_err(|e| DocumentError::from(e).guest(path))?;
                Ok(FsResult::Names(names))
            }
            FsOp::Stat => Ok(FsResult::Stat(Stat {
                size: 0,
                is_file: false,
                is_directory: true,
                modified_ms: 0,
            })),
            _ => Err(DocumentError::new(DocumentErrorKind::PermissionDenied).guest(path)),
        },
        Target::File {
            directory,
            leaf,
            name,
        } => {
            if parts.len() != 2
                || parts[1] != name
                || matches!(op, FsOp::Remove | FsOp::Mkdir | FsOp::ReadDir)
            {
                return Err(DocumentError::new(DocumentErrorKind::PermissionDenied).guest(path));
            }
            super::app_fs::document(directory, leaf, op, data)
                .map_err(|e| DocumentError::from(e).guest(path))
        }
        Target::Directory {
            directory,
            relative,
        } => {
            if op == FsOp::Remove && (parts.len() <= 2 || relative.is_empty()) {
                return Err(DocumentError::new(DocumentErrorKind::PermissionDenied).guest(path));
            }
            super::app_fs::document(directory, relative, op, data)
                .map(|result| match result {
                    FsResult::Stat(stat) if stat.is_directory => {
                        FsResult::Stat(Stat { size: 0, ..stat })
                    }
                    other => other,
                })
                .map_err(|e| DocumentError::from(e).guest(path))
        }
    }
}

/// `copyFile` from a chosen document into the app's own directories (Exact
/// patch 18). The source is admitted and opened as `readFile` admits and
/// opens it, after `fs.write` on the destination is admitted; the host copies
/// the bytes, which never reach the guest, so no result envelope bounds them.
pub fn copy_document(
    grants: &GrantSet,
    directories: Option<&super::app_fs::AppDirectories>,
    documents: Option<&Documents>,
    path: &str,
    destination: &str,
) -> Result<FsResult, HostError> {
    boundary::admit(grants, &Operation::FsRead { path: path.into() })?;
    let parts = components(path).map_err(|e| e.guest(path))?;
    let directories = directories
        .ok_or_else(|| HostError::Failed("app directories are not configured".into()))?;
    let documents =
        documents.ok_or_else(|| DocumentError::new(DocumentErrorKind::Unsupported).guest(path))?;
    directories.copy_in(grants, destination, || {
        let lease = documents(path, DocumentAccess::Read).map_err(|e| e.guest(path))?;
        let file = match &lease.target {
            Target::Root(_) => Err(DocumentError::new(DocumentErrorKind::PermissionDenied)),
            Target::File {
                directory,
                leaf,
                name,
            } if parts.len() == 2 && parts[1] == name => {
                super::app_fs::document_file(directory, leaf).map_err(DocumentError::from)
            }
            Target::File { .. } => Err(DocumentError::new(DocumentErrorKind::PermissionDenied)),
            Target::Directory {
                directory,
                relative,
            } => super::app_fs::document_file(directory, relative).map_err(DocumentError::from),
        }
        .map_err(|e| e.guest(path))?;
        Ok((file, lease))
    })
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::{
        path::PathBuf,
        sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    };
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let p = std::env::temp_dir().join(format!(
                "ibex-doc-{}",
                crate::stdlib::crypto::random_uuid().unwrap()
            ));
            std::fs::create_dir(&p).unwrap();
            std::fs::write(p.join("chosen"), b"old").unwrap();
            Self(p)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn grants() -> GrantSet {
        GrantSet::parse("fs.read doc:/1\nfs.write doc:/1").unwrap()
    }
    #[test]
    fn document_result_limits_keep_the_same_safe_error_as_other_storage() {
        let f = Fixture::new();
        std::fs::write(f.0.join("chosen"), [0; 64]).unwrap();
        let retained = Arc::new(File::open(&f.0).unwrap());
        let provider = move |_: &str, _: DocumentAccess| {
            DocumentLease::selected_file(retained.clone(), "chosen".into(), Arc::new(()))
        };
        let error = super::super::fs::run_with_limits(
            &grants(),
            None,
            Some(&provider),
            super::super::fs::ResultLimits { bytes: 64 },
            FsOp::ReadFile,
            "doc:/1/chosen",
            None,
            None,
        )
        .unwrap_err();
        assert_eq!(
            error,
            HostError::Failed(super::super::fs::RESULT_LIMIT.into())
        );
        let native_error = DocumentError::from(std::io::Error::other("/private/selected/file"));
        assert_eq!(native_error.kind, DocumentErrorKind::Io);
        assert!(!native_error
            .guest("doc:/1/chosen")
            .to_string()
            .contains("/private"));
    }
    #[test]
    fn grants_and_path_validation_precede_provider_and_namespaces_are_disjoint() {
        let provider = |_: &str, _: DocumentAccess| -> Result<DocumentLease, DocumentError> {
            panic!("provider touched before admission");
        };
        for path in [
            "doc:/10/chosen",
            "doc:/1/../chosen",
            "doc:/1//chosen",
            "doc:/1/chosen\\x",
            "doc:/1/chosen\0",
        ] {
            assert!(run_document(&grants(), Some(&provider), FsOp::ReadFile, path, None).is_err());
        }
        assert!(run_document(
            &GrantSet::none(),
            Some(&provider),
            FsOp::ReadFile,
            "doc:/1/chosen",
            None
        )
        .is_err());
        assert!(!GrantSet::parse("fs.read /\nfs.read app:/")
            .unwrap()
            .permits(&Operation::FsRead {
                path: "doc:/1/chosen".into()
            }));
    }
    #[test]
    fn selected_leaf_remains_in_retained_parent_after_rename_and_cannot_escape() {
        let f = Fixture::new();
        let retained = Arc::new(File::open(&f.0).unwrap());
        let provider = move |_: &str, _: DocumentAccess| {
            DocumentLease::selected_file(retained.clone(), "chosen".into(), Arc::new(()))
        };
        let moved = f.0.with_extension("moved");
        std::fs::rename(&f.0, &moved).unwrap();
        std::fs::create_dir(&f.0).unwrap();
        std::fs::write(f.0.join("chosen"), b"replacement").unwrap();
        assert_eq!(
            run_document(
                &grants(),
                Some(&provider),
                FsOp::ReadFile,
                "doc:/1/chosen",
                None
            )
            .unwrap(),
            FsResult::Bytes(b"old".to_vec())
        );
        run_document(
            &grants(),
            Some(&provider),
            FsOp::AtomicWriteFile,
            "doc:/1/chosen",
            Some(b"saved"),
        )
        .unwrap();
        assert_eq!(std::fs::read(moved.join("chosen")).unwrap(), b"saved");
        assert_eq!(std::fs::read(f.0.join("chosen")).unwrap(), b"replacement");
        for (op, path) in [
            (FsOp::ReadFile, "doc:/1/chosen/other"),
            (FsOp::Remove, "doc:/1/chosen"),
            (FsOp::Realpath, "doc:/1/chosen"),
        ] {
            assert!(run_document(&grants(), Some(&provider), op, path, None).is_err());
        }
        std::fs::remove_file(moved.join("chosen")).unwrap();
        std::os::unix::fs::symlink(f.0.join("chosen"), moved.join("chosen")).unwrap();
        let e = run_document(
            &grants(),
            Some(&provider),
            FsOp::ReadFile,
            "doc:/1/chosen",
            None,
        )
        .unwrap_err();
        assert!(!e.to_string().contains(f.0.to_str().unwrap()));
        std::fs::remove_dir_all(moved).unwrap();
    }
    #[test]
    fn directory_descendants_refuse_symlinks_and_nonrecursive_remove_respects_selection() {
        let f = Fixture::new();
        std::fs::create_dir(f.0.join("folder")).unwrap();
        std::fs::write(f.0.join("folder/inside"), b"inside").unwrap();
        std::os::unix::fs::symlink("folder", f.0.join("link")).unwrap();
        let root = Arc::new(File::open(&f.0).unwrap());
        let provider = move |path: &str, _: DocumentAccess| {
            DocumentLease::directory(
                root.clone(),
                path.strip_prefix("doc:/1/root")
                    .unwrap()
                    .trim_start_matches('/')
                    .into(),
                Arc::new(()),
            )
        };
        for path in ["doc:/1/root/link/inside", "doc:/1/root/link"] {
            assert!(run_document(&grants(), Some(&provider), FsOp::ReadFile, path, None).is_err());
        }
        let e = run_document(
            &grants(),
            Some(&provider),
            FsOp::ReadFile,
            "doc:/1/root/folder",
            None,
        )
        .unwrap_err();
        assert!(e.to_string().contains("EISDIR"));
        assert!(run_document(
            &grants(),
            Some(&provider),
            FsOp::Remove,
            "doc:/1/root/folder",
            None
        )
        .is_err());
        assert!(f.0.join("folder/inside").exists());
        run_document(
            &grants(),
            Some(&provider),
            FsOp::Remove,
            "doc:/1/root/folder/inside",
            None,
        )
        .unwrap();
        run_document(
            &grants(),
            Some(&provider),
            FsOp::Remove,
            "doc:/1/root/folder",
            None,
        )
        .unwrap();
    }
    #[test]
    fn copy_file_takes_a_document_into_an_app_file_under_both_grants() {
        use super::super::fs::run_with_documents;
        let f = Fixture::new();
        std::fs::create_dir(f.0.join("folder")).unwrap();
        std::fs::write(f.0.join("folder/inside"), b"inside").unwrap();
        std::os::unix::fs::symlink("folder/inside", f.0.join("link")).unwrap();
        let app = f.0.join("app");
        for root in ["data", "cache", "tmp"] {
            std::fs::create_dir_all(app.join(root)).unwrap();
        }
        let dirs = super::super::app_fs::AppDirectories::new(
            app.join("data"),
            app.join("cache"),
            app.join("tmp"),
        )
        .unwrap();
        let root = Arc::new(File::open(&f.0).unwrap());
        let touched = Arc::new(AtomicUsize::new(0));
        let seen = touched.clone();
        let provider = move |path: &str, access: DocumentAccess| {
            seen.fetch_add(1, Ordering::Relaxed);
            assert_eq!(access, DocumentAccess::Read);
            match path.strip_prefix("doc:/1/") {
                Some(rest) if rest.starts_with("chosen") => {
                    DocumentLease::selected_file(root.clone(), "chosen".into(), Arc::new(()))
                }
                Some(rest) if rest.starts_with("root") => DocumentLease::directory(
                    root.clone(),
                    rest[4..].trim_start_matches('/').into(),
                    Arc::new(()),
                ),
                _ => Err(DocumentError::new(DocumentErrorKind::NotFound)),
            }
        };
        let both = GrantSet::parse("fs.read doc:/1\nfs.write app:/data").unwrap();
        let copy = |grants: &GrantSet, from: &str, to: &str| {
            run_with_documents(
                grants,
                Some(&dirs),
                Some(&provider),
                FsOp::CopyFile,
                from,
                Some(to),
                None,
            )
        };
        std::fs::write(app.join("data/old"), b"a longer earlier file").unwrap();
        assert_eq!(
            copy(&both, "doc:/1/chosen", "app:/data/old").unwrap(),
            FsResult::Done
        );
        assert_eq!(std::fs::read(app.join("data/old")).unwrap(), b"old");
        copy(&both, "doc:/1/root/folder/inside", "app:/data/new").unwrap();
        assert_eq!(std::fs::read(app.join("data/new")).unwrap(), b"inside");
        let before = touched.load(Ordering::Relaxed);
        // Each grant is admitted before the provider is asked.
        for (grants, to) in [
            ("fs.read doc:/1\nfs.write app:/cache", "app:/data/x"),
            ("fs.read app:/data\nfs.write app:/data", "app:/data/x"),
        ] {
            let e = copy(&GrantSet::parse(grants).unwrap(), "doc:/1/chosen", to).unwrap_err();
            assert!(e.to_string().contains("denied"), "{e}");
        }
        assert_eq!(touched.load(Ordering::Relaxed), before);
        // The refusals `readFile` gives the same source.
        for (from, says) in [
            ("doc:/1/root/link", "ELOOP"),
            ("doc:/1/root/folder", "EISDIR"),
            ("doc:/1/root", "EISDIR"),
            ("doc:/1/chosen/other", "EACCES"),
            ("doc:/1/gone", "ENOENT"),
        ] {
            let e = copy(&both, from, "app:/data/x").unwrap_err().to_string();
            assert!(
                e.starts_with("document ") && !e.contains(f.0.to_str().unwrap()),
                "{from}: {e}"
            );
            if says != "ELOOP" {
                assert!(e.contains(says), "{from}: {e}");
            }
        }
        assert!(copy(&both, "doc:/1/../chosen", "app:/data/x").is_err());
        assert!(!app.join("data/x").exists());
        // Only into an app file: another document, or a disk path, is refused.
        let doc_write = GrantSet::parse("fs.read doc:/1\nfs.write doc:/1\nfs.write /").unwrap();
        for (from, to) in [
            ("doc:/1/chosen", "doc:/1/root/copy"),
            ("doc:/1/chosen", "/tmp/copy"),
        ] {
            assert!(copy(&doc_write, from, to).is_err());
        }
        assert!(!f.0.join("copy").exists());
        for op in [FsOp::Rename, FsOp::Realpath] {
            assert!(run_with_documents(
                &both,
                Some(&dirs),
                Some(&provider),
                op,
                "doc:/1/chosen",
                (op == FsOp::Rename).then_some("app:/data/moved"),
                None,
            )
            .is_err());
        }
        assert!(f.0.join("chosen").exists());
    }
    #[test]
    fn provider_enforces_access_sessions_forgetting_and_guard_lifetime() {
        struct Guard(Arc<AtomicUsize>);
        impl Drop for Guard {
            fn drop(&mut self) {
                self.0.fetch_add(1, Ordering::Relaxed);
            }
        }
        let f = Fixture::new();
        let root = Arc::new(File::open(&f.0).unwrap());
        let forgotten = Arc::new(AtomicBool::new(false));
        let dead = forgotten.clone();
        let dropped = Arc::new(AtomicUsize::new(0));
        let seen = dropped.clone();
        let provider = move |path: &str, access: DocumentAccess| {
            if dead.load(Ordering::Acquire) || !path.starts_with("doc:/1/") {
                return Err(DocumentError::new(DocumentErrorKind::NotFound));
            }
            if access == DocumentAccess::Write {
                return Err(DocumentError::new(DocumentErrorKind::PermissionDenied));
            }
            DocumentLease::selected_file(
                root.clone(),
                "chosen".into(),
                Arc::new(Guard(seen.clone())),
            )
        };
        assert!(run_document(
            &grants(),
            Some(&provider),
            FsOp::WriteFile,
            "doc:/1/chosen",
            Some(b"no")
        )
        .is_err());
        run_document(
            &grants(),
            Some(&provider),
            FsOp::ReadFile,
            "doc:/1/chosen",
            None,
        )
        .unwrap();
        assert_eq!(dropped.load(Ordering::Relaxed), 1);
        forgotten.store(true, Ordering::Release);
        assert!(run_document(
            &grants(),
            Some(&provider),
            FsOp::ReadFile,
            "doc:/1/chosen",
            None
        )
        .is_err());
        assert_eq!(std::fs::read(f.0.join("chosen")).unwrap(), b"old");
    }
}
