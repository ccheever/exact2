//! Documents the person chose (LLP 1069.010 D1): a `doc:/<n>/<name>` path
//! names a file or folder a host minted when the person picked it, opened
//! it from the system, or typed it into the app's `open-file` field. The
//! real location stays here, behind `<n>`; an app holds only the name and
//! reaches the bytes through `storage.fs` under `fs.read doc:/` or
//! `fs.write doc:/` (the `File System Access API`'s `FileSystemHandle`, the
//! web's name for the same thing).
//!
//! A `doc:` path exists only once a host has minted it, so a grant over
//! `doc:/` admits what the person chose and never a location the app
//! names. `..`, `.` and empty segments are refused, as `PathPrefix`
//! refuses them. Each minting host owns its entries and forgets them when
//! its session ends ([`forget`]).

use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// The namespace every minted path starts with.
pub const PREFIX: &str = "doc:/";

struct Entry {
    real: PathBuf,
    name: String,
    owner: u64,
}

/// Every entry minted in this process, `<n>` its index + 1; a forgotten
/// entry leaves a hole, so a number is never reused for another file.
static TABLE: Mutex<Vec<Option<Entry>>> = Mutex::new(Vec::new());

fn table() -> std::sync::MutexGuard<'static, Vec<Option<Entry>>> {
    TABLE.lock().unwrap_or_else(|e| e.into_inner())
}

/// Mint (or find) the handle for `real`, a file or folder the person chose,
/// on behalf of `owner` (a session): `doc:/<n>/<name>`. `None` for a path
/// with no final name (`/`).
pub fn mint(real: &Path, owner: u64) -> Option<String> {
    // One file is one handle however it was spelt (`/tmp` is `/private/tmp`
    // on a Mac); a file that does not exist yet (a save) keeps its spelling.
    let canonical = std::fs::canonicalize(real).ok();
    let real = canonical.as_deref().unwrap_or(real);
    let name = real.file_name()?.to_string_lossy().into_owned();
    if name.is_empty() || name == "." || name == ".." {
        return None;
    }
    let mut t = table();
    let n = match t.iter().position(|e| {
        e.as_ref()
            .is_some_and(|e| e.owner == owner && e.real == real)
    }) {
        Some(i) => i + 1,
        None => {
            t.push(Some(Entry {
                real: real.to_path_buf(),
                name: name.clone(),
                owner,
            }));
            t.len()
        }
    };
    Some(format!("{PREFIX}{n}/{name}"))
}

/// What a host's own routes deliver for a real path (Finder, Open With,
/// the command line, ⌘O, Open Recent, a path typed into the `open-file`
/// field): a folder is its own handle; a file is delivered beneath a handle
/// to its folder, so a reader can list what is beside it. `None` for a path
/// that is not absolute or does not exist.
pub fn open_route(path: &str, owner: u64) -> Option<String> {
    let real = Path::new(path);
    if !real.is_absolute() {
        return None;
    }
    if std::fs::metadata(real).ok()?.is_dir() {
        return mint(real, owner);
    }
    let name = real.file_name()?.to_string_lossy().into_owned();
    mint(real.parent()?, owner).map(|folder| format!("{folder}/{name}"))
}

/// Forget every handle `owner` minted: its session ended.
pub fn forget(owner: u64) {
    for slot in table().iter_mut() {
        if slot.as_ref().is_some_and(|e| e.owner == owner) {
            *slot = None;
        }
    }
}

/// Whether `path` is in the `doc:` namespace.
pub fn is_document(path: &str) -> bool {
    path.starts_with(PREFIX)
}

/// Where a `doc:` path leads.
#[derive(Debug, Clone, PartialEq)]
pub enum Resolved {
    /// `doc:/<n>`: the handle's own directory, holding only its entry.
    Root(String),
    /// The chosen file or folder, or a path beneath a chosen folder.
    Real(PathBuf),
}

/// Resolve a `doc:` path to the real location behind it, refusing a handle
/// never minted (or forgotten), a name that is not the handle's, and any
/// `.`, `..` or empty segment.
pub fn resolve(path: &str) -> Result<Resolved, String> {
    let rest = path
        .strip_prefix(PREFIX)
        .ok_or_else(|| format!("{path} is not a document path"))?;
    let rest = rest.strip_suffix('/').unwrap_or(rest);
    let mut parts = rest.split('/');
    let n: usize = parts
        .next()
        .and_then(|n| n.parse().ok())
        .filter(|n| *n > 0)
        .ok_or_else(|| format!("{path} names no document"))?;
    let segments: Vec<&str> = parts.collect();
    if segments
        .iter()
        .any(|s| s.is_empty() || *s == "." || *s == ".." || s.contains('\0'))
    {
        return Err(format!(
            "{path}: a document path has no `.`, `..` or empty segment"
        ));
    }
    let t = table();
    let entry = t.get(n - 1).and_then(Option::as_ref).ok_or_else(|| {
        format!("{path}: no such document (it was never opened, or its window closed)")
    })?;
    let Some((first, beneath)) = segments.split_first() else {
        return Ok(Resolved::Root(entry.name.clone()));
    };
    if *first != entry.name {
        return Err(format!("{path}: no such document"));
    }
    Ok(Resolved::Real(
        beneath.iter().fold(entry.real.clone(), |p, s| p.join(s)),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_minted_path_resolves_beneath_its_handle_and_nowhere_else() {
        let folder = Path::new("/tmp/exact-docs-test/notes");
        let doc = mint(folder, 7001).unwrap();
        assert!(doc.starts_with("doc:/") && doc.ends_with("/notes"), "{doc}");
        assert_eq!(mint(folder, 7001).unwrap(), doc, "minting again reuses it");
        assert_eq!(
            resolve(&format!("{doc}/a/b.md")).unwrap(),
            Resolved::Real(folder.join("a").join("b.md"))
        );
        let n = doc.trim_start_matches(PREFIX).split('/').next().unwrap();
        assert_eq!(
            resolve(&format!("doc:/{n}")).unwrap(),
            Resolved::Root("notes".into())
        );
        assert!(resolve(&format!("{doc}/../x")).is_err());
        assert!(resolve(&format!("doc:/{n}/other/x")).is_err());
        assert!(resolve("doc:/0/x").is_err());
        assert!(resolve("app:/data/x").is_err());
        forget(7001);
        assert!(resolve(&doc).is_err(), "a forgotten handle reads nothing");
        assert!(mint(Path::new("/"), 7001).is_none());
    }
}
