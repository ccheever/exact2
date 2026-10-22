//! A source's identity (LLP 1121 D15) and the named package that owns its
//! assets (stage 13): where a file is alone, never the route a `use`
//! reached it by.

use crate::resolve::Origin;
use std::path::{Path, PathBuf};

/// A source's file as a component's identity names it (LLP 1121 D15),
/// from where the file is alone, never the route a `use` reached it by: a
/// file in a package (the nearest directory above it, below the app's own
/// and no higher than a linked package's root, whose `package.json` has a
/// name: under `node_modules`, linked from elsewhere, or vendored) under
/// the package's name, else the file relative to the app's directory (the
/// root linked into the app's tree, where the link sits). None for a file
/// outside the app in no named package: its place from
/// the app would spell this machine's directories, so it is refused
/// ([`UNNAMED`]).
pub(super) fn home(path: &Path, origin: &Origin, app_root: &Path) -> Option<String> {
    named_home(path, origin, app_root).map(|(home, _)| home)
}

/// [`home`], and the named package it is under, by its name and directory:
/// the package that owns the file's assets (LLP 1121 stage 13 round 2,
/// Astra 1), a vendored package's own whichever route reached it; none for
/// the app's own file.
#[allow(clippy::type_complexity)]
pub(super) fn named_home(
    path: &Path,
    origin: &Origin,
    app_root: &Path,
) -> Option<(String, Option<(String, PathBuf)>)> {
    let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let app = app_root
        .canonicalize()
        .unwrap_or_else(|_| app_root.to_path_buf());
    let relative = |root: &Path| {
        path.strip_prefix(root)
            .ok()
            .map(|p| p.to_string_lossy().replace('\\', "/"))
    };
    let named = |root: &Path| {
        std::fs::read_to_string(root.join("package.json"))
            .ok()
            .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
            .and_then(|m| m["name"].as_str().map(str::to_string))
    };
    // A directory above a linked package's root is none of the package's.
    let root = match origin {
        Origin::Package { root, .. } => Some(root.as_path()),
        _ => None,
    };
    for dir in path.ancestors().skip(1) {
        // The app's own directory and those above it are no package of it,
        // unless the package holds the app (a package's own test app).
        let ours = root.is_some_and(|r| dir.starts_with(r));
        if !ours && app.starts_with(dir) {
            break;
        }
        if let (Some(name), Some(rest)) = (named(dir), relative(dir)) {
            let home = format!("{name}/{rest}");
            return Some((home, Some((name, dir.to_path_buf()))));
        }
        if root == Some(dir) {
            break;
        }
    }
    if let Some(rest) = relative(&app) {
        return Some((rest, None));
    }
    match origin {
        Origin::Builtin => Some((path.to_string_lossy().into_owned(), None)),
        _ => None,
    }
}

/// [`home`] for the root file linked into the app's tree from elsewhere
/// (conform's corpus fixtures: `conformance/tabs.contract`, a link to the
/// corpus's), by `given`, the path it was named by: the app's own, named
/// where the link sits. None for a package's file, or a link outside. A
/// file a `use` reaches is its canonical path's, refused outside the app
/// by its `use` (`contract-use-path`).
pub(super) fn linked_home(given: &Path, origin: &Origin, app_root: &Path) -> Option<String> {
    if matches!(origin, Origin::Package { .. }) {
        return None;
    }
    let app = app_root.canonicalize().ok()?;
    // A bare file name's directory is the working one, as `app_root`'s is.
    let dir = given.parent().filter(|d| !d.as_os_str().is_empty());
    let dir = dir.unwrap_or(Path::new(".")).canonicalize().ok()?;
    let at = dir.join(given.file_name()?);
    at.strip_prefix(&app)
        .ok()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
}
