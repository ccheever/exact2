//! @ref LLP 0068#windows-app-storage-qualification — logical app paths on Windows.
use crate::stdlib::fs::{FsOp, FsResult};
use crate::stdlib::windows_directory::{self, identity, Directory};
use crate::stdlib::windows_fs::{error, token, Target};
use crate::{
    boundary::HostError,
    grant::{GrantSet, Operation},
};
use std::{
    fs::File,
    io,
    path::{Path, PathBuf},
    sync::Arc,
};
#[derive(Clone, Debug)]
pub struct AppDirectories {
    roots: Arc<[Directory; 3]>,
    paths: Arc<[PathBuf; 3]>,
}
fn text(path: &Path) -> Result<&str, HostError> {
    path.to_str().ok_or_else(|| error("non-Unicode path"))
}
impl AppDirectories {
    pub fn new(
        data: impl AsRef<Path>,
        cache: impl AsRef<Path>,
        tmp: impl AsRef<Path>,
    ) -> Result<Self, HostError> {
        let paths = [
            data.as_ref().to_owned(),
            cache.as_ref().to_owned(),
            tmp.as_ref().to_owned(),
        ];
        let roots = [
            Directory::root(text(&paths[0])?, false).map_err(error)?,
            Directory::root(text(&paths[1])?, false).map_err(error)?,
            Directory::root(text(&paths[2])?, false).map_err(error)?,
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
        let mut components = tail.split('/').filter(|part| !part.is_empty());
        let index = match components.next() {
            Some("data") => 0,
            Some("cache") => 1,
            Some("tmp") => 2,
            _ => return Err(error("unknown app directory")),
        };
        let parts: Vec<_> = components.collect();
        // Pure lexical validation; no filesystem access precedes grant admission.
        for part in &parts {
            windows_directory::validate_name(part).map_err(error)?;
        }
        Ok((index, parts))
    }
    fn parent(&self, path: &str) -> Result<(Directory, String), HostError> {
        let (index, parts) = self.parse(path)?;
        if parts.is_empty() {
            return Err(error("operation needs a path below the app root"));
        }
        self.roots[index]
            .parent(&parts.join("/"), false)
            .map_err(error)
    }
    fn open(&self, path: &str) -> Result<File, HostError> {
        let (index, parts) = self.parse(path)?;
        if parts.is_empty() {
            return self.roots[index].0.try_clone().map_err(error);
        }
        let (parent, leaf) = self.parent(path)?;
        parent.entry(&leaf).map_err(error)
    }
    fn sqlite_path(&self, path: &str) -> Result<PathBuf, HostError> {
        let (index, parts) = self.parse(path)?;
        let (pinned, _) = self.parent(path)?;
        let physical = self.paths[index].join(parts.join("/"));
        let current = Directory::root(text(physical.parent().unwrap())?, false).map_err(error)?;
        if identity(&current.0).map_err(error)? != identity(&pinned.0).map_err(error)? {
            return Err(error("app directory was replaced"));
        }
        validate_sqlite_location(&physical)?;
        Ok(physical)
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
        let check = |write, p: &str| {
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
        if op == FsOp::Realpath {
            self.open(path)?;
            let (index, parts) = self.parse(path)?;
            let base = ["data", "cache", "tmp"][index];
            return Ok(FsResult::Text(if parts.is_empty() {
                format!("app:/{base}")
            } else {
                format!("app:/{base}/{}", parts.join("/"))
            }));
        }
        let (index, parts) = self.parse(path)?;
        if op == FsOp::Mkdir {
            let mut dir = self.roots[index].try_clone().map_err(error)?;
            for part in parts {
                dir = dir.child(part, true).map_err(error)?;
            }
            return Ok(FsResult::Done);
        }
        let source = Target::pin(&self.roots[index], &parts).map_err(error)?;
        let target = destination
            .map(|path| {
                let (index, parts) = self.parse(path)?;
                Target::pin(&self.roots[index], &parts).map_err(error)
            })
            .transpose()?;
        crate::stdlib::windows_fs::perform(op, &source, target.as_ref(), data)
    }
}
/// Validate a native SQLite location for a trusted provider. The embedder must
/// still keep its ancestry stable for the whole connection: SQLite uses paths.
pub fn validate_sqlite_location(path: &Path) -> Result<(), HostError> {
    let parent = path
        .parent()
        .ok_or_else(|| error("database needs a parent"))?;
    let leaf = path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| error("database needs a filename"))?;
    let dir = Directory::root(text(parent)?, false).map_err(error)?;
    for suffix in ["", "-journal", "-wal", "-shm"] {
        match dir.kind(&format!("{leaf}{suffix}")) {
            Ok(kind) if !kind.regular => {
                return Err(error("database and sidecars must be regular files"))
            }
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(error(e)),
        }
    }
    Ok(())
}
/// Admit the logical SQLite namespace. Native Windows grant spellings remain unsupported.
pub fn resolve_sqlite(
    grants: &GrantSet,
    directories: Option<&AppDirectories>,
    path: &str,
) -> Result<PathBuf, HostError> {
    if !path.starts_with("app:/") {
        return Err(error("native Windows SQLite grants are not supported"));
    }
    crate::boundary::admit(grants, &Operation::SqliteOpen { path: path.into() })?;
    directories
        .ok_or_else(|| error("app directories are not configured"))?
        .sqlite_path(path)
}
pub(crate) fn atomic_native(path: &Path, data: &[u8]) -> Result<FsResult, HostError> {
    let dir = Directory::root(
        text(
            path.parent()
                .ok_or_else(|| error("atomic write needs a parent"))?,
        )?,
        false,
    )
    .map_err(error)?;
    let leaf = path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| error("atomic write needs a filename"))?;
    dir.write(leaf, data, &token().map_err(error)?)
        .map_err(error)?;
    Ok(FsResult::Done)
}

#[cfg(test)]
#[path = "app_fs_windows_tests.rs"]
mod tests;
