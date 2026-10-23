//! D4's address rules: a `unix:` path resolved before it is granted, its
//! parent made or checked, and every ancestor safe from other users.
use std::{
    ffi::OsStr,
    os::unix::{
        ffi::OsStrExt,
        fs::{DirBuilderExt, MetadataExt, PermissionsExt},
    },
    path::{Path, PathBuf},
};

/// `sun_path`'s size, which holds the path and its NUL.
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub(crate) const SUN_PATH: usize = 104;
#[cfg(not(any(target_os = "macos", target_os = "ios")))]
pub(crate) const SUN_PATH: usize = 108;

/// The staging name's length beside the final component (`.s` and 8 hex).
pub(crate) const STAGING: usize = 10;

/// Why a listener stopped, as the `ERROR` event words it.
#[derive(Debug)]
pub(crate) struct Failure {
    pub kind: &'static str,
    pub message: String,
}

impl Failure {
    pub(crate) fn refused(message: impl Into<String>) -> Self {
        Failure {
            kind: "Refused",
            message: message.into(),
        }
    }
    pub(crate) fn os(what: &str, error: std::io::Error) -> Self {
        Failure {
            kind: "Network",
            message: format!("{what}: {error}"),
        }
    }
}

/// An address that passed D4 and D11: the socket's resolved path and its
/// parent, which exists, is the user's and is `0700`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Resolved {
    pub path: PathBuf,
    pub parent: PathBuf,
}

/// The path of `address`: `unix:` and an absolute path with a final
/// component and no `.`, `..` or NUL.
pub(crate) fn path_of(address: &[u8]) -> Result<PathBuf, Failure> {
    let Some(rest) = address.strip_prefix(b"unix:") else {
        let scheme = address.split(|&b| b == b':').next().unwrap_or_default();
        return Err(Failure {
            kind: "Unsupported",
            message: format!(
                "{}: addresses: only unix: is built (LLP 1133.005 D5)",
                String::from_utf8_lossy(scheme)
            ),
        });
    };
    if rest.contains(&0) {
        return Err(Failure::refused("a unix: path cannot hold a NUL byte"));
    }
    let path = Path::new(OsStr::from_bytes(rest));
    if !path.is_absolute() {
        return Err(Failure::refused("a unix: path must be absolute"));
    }
    // By the bytes: `Path::components` drops an inner `.`.
    if rest.ends_with(b"/")
        || rest
            .split(|&b| b == b'/')
            .any(|segment| segment == b"." || segment == b"..")
    {
        return Err(Failure::refused(
            "a unix: path names a file, with no . or .. segment",
        ));
    }
    Ok(path.to_path_buf())
}

/// `path` with its existing ancestors resolved (`realpath`), the rest as
/// written. A symlink in an ancestor cannot lead it outside a grant.
pub(crate) fn resolve_existing(path: &Path) -> Result<PathBuf, Failure> {
    let mut rest = Vec::new();
    let mut at = path;
    loop {
        match std::fs::canonicalize(at) {
            Ok(resolved) => {
                let mut out = resolved;
                out.extend(rest.iter().rev());
                return Ok(out);
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let Some(name) = at.file_name() else {
                    return Err(Failure::os("resolving the path", e));
                };
                rest.push(name.to_os_string());
                at = at.parent().ok_or_else(|| Failure::os("resolving", e))?;
            }
            Err(e) => return Err(Failure::os(&format!("resolving {}", at.display()), e)),
        }
    }
}

/// Whether `grants` (one `unix:<prefix>` a line) admits the resolved
/// `path`: any path for a bare `unix:`, else one under a prefix, compared
/// by whole segments once the prefix is resolved too (D11).
pub(crate) fn granted(grants: &str, path: &Path) -> Result<bool, Failure> {
    for line in grants.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let Some(prefix) = line.strip_prefix("unix:") else {
            continue;
        };
        if prefix.is_empty() {
            return Ok(true);
        }
        let prefix = Path::new(prefix);
        if !prefix.is_absolute() {
            continue;
        }
        if path.starts_with(resolve_existing(prefix)?) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// D4 and D11 for `address`: the resolved path, inside a grant, short
/// enough for `sun_path` with its staging name; every ancestor of its
/// parent the user's or root's and not writable by others unless sticky;
/// the parent made `0700` when missing (its own parent must exist), or an
/// existing directory (never a symlink) the user owns, set to `0700`.
pub(crate) fn prepare(address: &[u8], grants: &str) -> Result<Resolved, Failure> {
    let written = path_of(address)?;
    let written_parent = written
        .parent()
        .expect("an absolute file path has a parent");
    let name = written.file_name().expect("a final component");
    // The parent is checked by `lstat` as written: a symlink is refused,
    // never followed (T3's `prepareDirectory`).
    match std::fs::symlink_metadata(written_parent) {
        Ok(meta) if meta.file_type().is_symlink() => {
            return Err(Failure::refused(format!(
                "{} is a symlink, not a directory",
                written_parent.display()
            )))
        }
        _ => {}
    }
    let parent = resolve_existing(written_parent)?;
    let path = parent.join(name);
    if !granted(grants, &path)? {
        return Err(Failure::refused(format!(
            "unix:{} is outside the net.listen grants",
            path.display()
        )));
    }
    let longest = path
        .as_os_str()
        .len()
        .max(parent.as_os_str().len() + 1 + STAGING);
    // The staging name beside it must fit too; it is no longer than a
    // final component of ten bytes.
    if longest >= SUN_PATH {
        return Err(Failure::refused(format!(
            "unix:{} is too long: a socket path is at most {} bytes here, resolved",
            path.display(),
            SUN_PATH - 1
        )));
    }
    let uid = unsafe { libc::getuid() };
    let mut ancestor = parent.parent();
    while let Some(dir) = ancestor {
        if let Ok(meta) = std::fs::metadata(dir) {
            let mode = meta.mode();
            if meta.uid() != uid && meta.uid() != 0 {
                return Err(Failure::refused(format!(
                    "{} belongs to another user",
                    dir.display()
                )));
            }
            if mode & 0o022 != 0 && mode & 0o1000 == 0 {
                return Err(Failure::refused(format!(
                    "{} is writable by other users and not sticky",
                    dir.display()
                )));
            }
        }
        ancestor = dir.parent();
    }
    match std::fs::symlink_metadata(&parent) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            std::fs::DirBuilder::new()
                .mode(0o700)
                .create(&parent)
                .map_err(|e| Failure::os(&format!("making {}", parent.display()), e))?;
        }
        Err(e) => return Err(Failure::os(&format!("reading {}", parent.display()), e)),
        Ok(_) => {}
    }
    let meta = std::fs::symlink_metadata(&parent)
        .map_err(|e| Failure::os(&format!("reading {}", parent.display()), e))?;
    if !meta.file_type().is_dir() {
        return Err(Failure::refused(format!(
            "{} is not a directory",
            parent.display()
        )));
    }
    if meta.uid() != uid {
        return Err(Failure::refused(format!(
            "{} belongs to another user",
            parent.display()
        )));
    }
    if meta.mode() & 0o777 != 0o700 {
        std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o700))
            .map_err(|e| Failure::os(&format!("setting {} to 0700", parent.display()), e))?;
    }
    Ok(Resolved { path, parent })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("exact-listen-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn an_address_is_unix_and_an_absolute_file_path() {
        assert_eq!(path_of(b"unix:/a/b.sock").unwrap(), Path::new("/a/b.sock"));
        assert_eq!(
            path_of(b"tcp://127.0.0.1:0").unwrap_err().kind,
            "Unsupported"
        );
        for bad in [
            &b"unix:a.sock"[..],
            b"unix:/a/",
            b"unix:/a/../b",
            b"unix:/a/./b",
            b"unix:/a\0b",
        ] {
            assert_eq!(path_of(bad).unwrap_err().kind, "Refused", "{bad:?}");
        }
    }

    #[test]
    fn the_parent_is_made_0700_and_a_symlinked_parent_refused() {
        let root = scratch("parent");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o755)).unwrap();
        let address = format!("unix:{}/d/x.sock", root.display());
        let r = prepare(address.as_bytes(), "unix:").unwrap();
        let mode = std::fs::metadata(&r.parent).unwrap().mode() & 0o777;
        assert_eq!(mode, 0o700);
        assert_eq!(
            r.path,
            std::fs::canonicalize(&root).unwrap().join("d/x.sock")
        );
        std::os::unix::fs::symlink(root.join("d"), root.join("link")).unwrap();
        let linked = format!("unix:{}/link/x.sock", root.display());
        let refused = prepare(linked.as_bytes(), "unix:").unwrap_err();
        assert!(refused.message.contains("symlink"), "{refused:?}");
        std::fs::write(root.join("file"), b"").unwrap();
        let file = format!("unix:{}/file/x.sock", root.display());
        assert!(prepare(file.as_bytes(), "unix:")
            .unwrap_err()
            .message
            .contains("not a directory"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_grant_admits_by_resolved_segments() {
        let root = scratch("grant");
        std::fs::create_dir_all(root.join("ok")).unwrap();
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o755)).unwrap();
        // A symlink in an ancestor cannot lead outside the grant.
        std::fs::create_dir_all(root.join("elsewhere")).unwrap();
        std::os::unix::fs::symlink(root.join("elsewhere"), root.join("ok/escape")).unwrap();
        let grant = format!("unix:{}/ok", root.display());
        let inside = format!("unix:{}/ok/d/x.sock", root.display());
        assert!(prepare(inside.as_bytes(), &grant).is_ok());
        let sibling = format!("unix:{}/okay/x.sock", root.display());
        assert!(prepare(sibling.as_bytes(), &grant)
            .unwrap_err()
            .message
            .contains("outside"));
        let escaped = format!("unix:{}/ok/escape/d/x.sock", root.display());
        assert!(prepare(escaped.as_bytes(), &grant)
            .unwrap_err()
            .message
            .contains("outside"));
        assert!(prepare(inside.as_bytes(), "")
            .unwrap_err()
            .message
            .contains("outside"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_path_too_long_for_sun_path_is_refused_before_anything_is_made() {
        let root = scratch("long");
        let long = format!("unix:{}/{}/x.sock", root.display(), "d".repeat(SUN_PATH));
        let refused = prepare(long.as_bytes(), "unix:").unwrap_err();
        assert!(refused.message.contains("too long"), "{refused:?}");
        assert!(!root.exists());
    }

    #[test]
    fn an_ancestor_writable_by_others_and_not_sticky_is_refused() {
        let root = scratch("open");
        std::fs::create_dir_all(root.join("shared")).unwrap();
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::set_permissions(root.join("shared"), std::fs::Permissions::from_mode(0o777))
            .unwrap();
        let address = format!("unix:{}/shared/d/x.sock", root.display());
        assert!(prepare(address.as_bytes(), "unix:")
            .unwrap_err()
            .message
            .contains("writable"));
        std::fs::set_permissions(root.join("shared"), std::fs::Permissions::from_mode(0o1777))
            .unwrap();
        assert!(prepare(address.as_bytes(), "unix:").is_ok());
        let _ = std::fs::remove_dir_all(&root);
    }
}
