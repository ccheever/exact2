//! @ref LLP 0068#proposed-windows-native-filesystem-grants — admit names, then retain parents.
use super::{
    fs::{FsOp, FsResult, Stat},
    windows_directory::{identity, Directory},
    windows_path::NativePath,
};
use crate::{
    boundary::HostError,
    grant::{GrantSet, Operation},
};
use std::{
    fs::File,
    io::{self, Write},
    os::windows::io::AsRawHandle,
};
use windows_sys::Win32::Storage::FileSystem::GetFinalPathNameByHandleW;

pub(super) fn error(e: impl std::fmt::Display) -> HostError {
    HostError::Failed(format!("filesystem: {e}"))
}
pub(super) fn token() -> io::Result<String> {
    let mut bytes = [0; 24];
    getrandom::getrandom(&mut bytes).map_err(|e| io::Error::other(e.to_string()))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// All ancestors remain owned until the operation ends. `leaf == None` names
/// the held root object and is never a mutable directory entry.
pub(super) struct Target {
    parents: Vec<Directory>,
    leaf: Option<String>,
}
impl Target {
    pub fn pin(root: &Directory, parts: &[&str]) -> io::Result<Self> {
        let mut parents = vec![root.try_clone()?];
        let (leaf, ancestors) = match parts.split_last() {
            Some((leaf, ancestors)) => (Some((*leaf).to_owned()), ancestors),
            None => (None, &[][..]),
        };
        for name in ancestors {
            parents.push(parents.last().unwrap().child(name, false)?);
        }
        Ok(Self { parents, leaf })
    }
    fn parent(&self) -> &Directory {
        self.parents.last().unwrap()
    }
    fn entry(&self) -> Result<(&Directory, &str), HostError> {
        Ok((
            self.parent(),
            self.leaf
                .as_deref()
                .ok_or_else(|| error("operation needs a path below the root"))?,
        ))
    }
    fn open(&self) -> Result<File, HostError> {
        match &self.leaf {
            Some(leaf) => self.parent().entry(leaf),
            None => self.parent().0.try_clone(),
        }
        .map_err(error)
    }
    fn directory(&self) -> io::Result<Directory> {
        match &self.leaf {
            Some(leaf) => self.parent().child(leaf, false),
            None => self.parent().try_clone(),
        }
    }
}
fn remove(parent: &Directory, leaf: &str) -> io::Result<()> {
    let kind = match parent.kind(leaf) {
        Ok(kind) => kind,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    if kind.directory {
        let child = parent.child(leaf, false)?;
        for name in child.names()? {
            remove(&child, &name)?;
        }
    }
    parent.unlink(leaf)
}

/// Shared by app and native path admission; every operand is already pinned.
pub(super) fn perform(
    op: FsOp,
    path: &Target,
    destination: Option<&Target>,
    data: Option<&[u8]>,
) -> Result<FsResult, HostError> {
    let data = data.unwrap_or(&[]);
    match op {
        FsOp::ReadFile => {
            let (p, n) = path.entry()?;
            Ok(FsResult::Bytes(p.read(n).map_err(error)?))
        }
        FsOp::ReadDir => Ok(FsResult::Names(
            path.directory().map_err(error)?.names().map_err(error)?,
        )),
        FsOp::Stat => {
            let m = path.open()?.metadata().map_err(error)?;
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
        FsOp::CopyFile => {
            let (from, name) = path.entry()?;
            let mut source = from.file(name, libc::O_RDONLY).map_err(error)?;
            let (to, leaf) = destination
                .ok_or_else(|| error("copy needs destination"))?
                .entry()?;
            let mut target = to
                .file(leaf, libc::O_WRONLY | libc::O_CREAT)
                .map_err(error)?;
            if identity(&source).map_err(error)? == identity(&target).map_err(error)? {
                return Err(error("copy requires distinct regular files"));
            }
            target.set_len(0).map_err(error)?;
            io::copy(&mut source, &mut target).map_err(error)?;
            Ok(FsResult::Done)
        }
        FsOp::WriteFile | FsOp::AppendFile => {
            let (parent, leaf) = path.entry()?;
            let mut file = parent
                .file(
                    leaf,
                    libc::O_WRONLY
                        | libc::O_CREAT
                        | if op == FsOp::AppendFile {
                            libc::O_APPEND
                        } else {
                            0
                        },
                )
                .map_err(error)?;
            if op == FsOp::WriteFile {
                file.set_len(0).map_err(error)?;
            }
            file.write_all(data).map_err(error)?;
            Ok(FsResult::Done)
        }
        FsOp::AtomicWriteFile => {
            let (parent, leaf) = path.entry()?;
            parent
                .write(leaf, data, &token().map_err(error)?)
                .map_err(error)?;
            Ok(FsResult::Done)
        }
        FsOp::Remove => {
            let (p, n) = path.entry()?;
            remove(p, n).map_err(error)?;
            Ok(FsResult::Done)
        }
        FsOp::Rename => {
            let (parent, leaf) = path.entry()?;
            let (target, to) = destination
                .ok_or_else(|| error("rename needs destination"))?
                .entry()?;
            let source = parent.removable(leaf).map_err(error)?;
            match target.kind(to) {
                Ok(_) => {}
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                Err(e) => return Err(error(e)),
            }
            target.rename(&source, to).map_err(error)?;
            Ok(FsResult::Done)
        }
        FsOp::Realpath => {
            let file = path.open()?;
            let mut buffer = vec![0; 512];
            loop {
                // SAFETY: the held handle and writable UTF-16 buffer remain live.
                let length = unsafe {
                    GetFinalPathNameByHandleW(
                        file.as_raw_handle(),
                        buffer.as_mut_ptr(),
                        buffer.len() as u32,
                        0,
                    )
                } as usize;
                if length == 0 {
                    return Err(error(io::Error::last_os_error()));
                }
                if length < buffer.len() {
                    return Ok(FsResult::Text(
                        String::from_utf16(&buffer[..length]).map_err(error)?,
                    ));
                }
                buffer.resize(length + 1, 0);
            }
        }
        FsOp::Mkdir => unreachable!("mkdir admission walks only authorized missing components"),
    }
}

pub(super) fn run_native(
    grants: &GrantSet,
    op: FsOp,
    path: &str,
    destination: Option<&str>,
    data: Option<&[u8]>,
) -> Result<FsResult, HostError> {
    if !op.takes_second_path() && destination.is_some() {
        return Err(HostError::InvalidArgument(
            "operation does not take a destination".into(),
        ));
    }
    let path = NativePath::parse(path, true).map_err(error)?;
    let destination = destination
        .map(|p| NativePath::parse(p, true))
        .transpose()
        .map_err(error)?;
    super::fs::admit_as(
        grants,
        op,
        std::path::Path::new(&path.spelling()),
        destination
            .as_ref()
            .map(|p| std::path::PathBuf::from(p.spelling()))
            .as_deref(),
    )?;
    // One root lookup per drive. Both roots and operand ancestors stay retained
    // until after the common operation, even across namespace replacement.
    let root = Directory::native_drive(path.drive).map_err(error)?;
    if op == FsOp::Mkdir {
        let mut parents = vec![root];
        for (index, part) in path.parts.iter().enumerate() {
            let may_create = grants.permits(&Operation::FsWrite {
                path: path.prefix(index + 1),
            });
            parents.push(
                parents
                    .last()
                    .unwrap()
                    .child(part, may_create)
                    .map_err(error)?,
            );
        }
        return Ok(FsResult::Done);
    }
    let source = match Target::pin(
        &root,
        &path.parts.iter().map(String::as_str).collect::<Vec<_>>(),
    ) {
        Ok(source) => source,
        Err(e) if op == FsOp::Remove && e.kind() == io::ErrorKind::NotFound => {
            return Ok(FsResult::Done)
        }
        Err(e) => return Err(error(e)),
    };
    let other_root = destination
        .as_ref()
        .filter(|p| p.drive != path.drive)
        .map(|p| Directory::native_drive(p.drive))
        .transpose()
        .map_err(error)?;
    let target = destination
        .as_ref()
        .map(|p| {
            Target::pin(
                other_root.as_ref().unwrap_or(&root),
                &p.parts.iter().map(String::as_str).collect::<Vec<_>>(),
            )
        })
        .transpose()
        .map_err(error)?;
    perform(op, &source, target.as_ref(), data)
}

#[cfg(test)]
#[path = "windows_fs_tests.rs"]
mod tests;
