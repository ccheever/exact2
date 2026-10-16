//! Where the picker's files land on Apple (LLP 1069.002 D4, D7): the app's
//! `app:/data`, `app:/cache` and `app:/tmp` as storage configures them, or,
//! for a scripted drive with no scratch store, a directory of this
//! process's own. The Swift host copies into the file a `pickedPath` reply
//! names and resolves an `image`/`video` `app:/` source against the roots,
//! which it learns at boot (`appRoots`).

use exact_runner::DataError;
use ibex2::stdlib::app_fs::AppDirectories;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;

static ROOTS: Mutex<Option<[PathBuf; 3]>> = Mutex::new(None);

/// The app's `app:/data`, `app:/cache` and `app:/tmp` (LLP 1027.001): under
/// the user's Library, or a scripted drive's scratch tree (`--storage`),
/// apart from the app's real files, with that tree when an authored test
/// starts it empty (`EXACT_AGENT_STORAGE_FRESH`). `None`: an app with no id,
/// or a drive with no scratch store, which has no storage.
#[allow(clippy::type_complexity)]
pub(crate) fn app_dirs(app_id: &str) -> Result<Option<([PathBuf; 3], Option<PathBuf>)>, DataError> {
    // Scripted drives must not read or write the developer's app files;
    // one that names a scratch tree gets storage there instead.
    let scratch = match std::env::var_os("EXACT_AGENT") {
        Some(_) => match agent_scratch()? {
            Some(name) => Some(name),
            None => return Ok(None),
        },
        None => None,
    };
    if app_id.is_empty() {
        return Ok(None);
    }
    if matches!(app_id, "." | "..")
        || !app_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b".-_".contains(&b))
    {
        return Err(DataError::Unavailable("unsafe app storage identity".into()));
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .ok_or_else(|| DataError::Unavailable("app storage needs an absolute HOME".into()))?;
    let mut data = home
        .join("Library/Application Support/exact")
        .join(app_id)
        .join("data");
    let mut cache = home.join("Library/Caches/exact").join(app_id);
    let mut fresh = None;
    if let Some(name) = scratch {
        cache = cache.join("agent").join(name);
        if std::env::var_os("EXACT_AGENT_STORAGE_FRESH").is_some() {
            fresh = Some(cache.clone());
        }
        data = cache.join("data");
    }
    // Sibling roots keep app:/cache grants from implicitly reaching tmp.
    // The user's cache base avoids a predictable shared /tmp directory.
    Ok(Some((
        [data, cache.join("cache"), cache.join("temporary")],
        fresh,
    )))
}

/// The scratch directory a named agent drive keeps `secret.keep` in
/// (`<scratch>/secrets` and `<scratch>/kv`), beside `app:/data`. `None` when
/// this drive names no scratch store, or the app has no id: secrets stay in
/// memory and nothing is written. Not the Keychain, and not `EXACT_STORE=real`.
pub(crate) fn agent_secret_root(app_id: &str) -> Option<PathBuf> {
    if std::env::var_os("EXACT_AGENT").is_none() || app_id.is_empty() {
        return None;
    }
    // `roots[0]` is `<scratch>/data`. Secrets live in the scratch tree, so
    // a fresh drive's emptying of that tree ([`empty_fresh_tree`]) takes them too.
    let (roots, _) = app_dirs(app_id).ok().flatten()?;
    roots[0].parent().map(|path| path.to_path_buf())
}

/// The scratch trees a fresh drive has emptied in this process.
static EMPTIED: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

/// Empty a fresh drive's scratch tree (`EXACT_AGENT_STORAGE_FRESH`) once a
/// process: at the first boot that reads its store, before the snapshot and
/// before any commit can write a secret or a kept answer into it. The
/// post-pixel activation and any later boot here leave it as it is; emptying
/// it again there removed what a commit in between had written while memory
/// still held it, so a relaunch read nothing back.
pub(crate) fn empty_fresh_tree(app_id: &str) -> Result<(), DataError> {
    let Some((_, Some(tree))) = app_dirs(app_id)? else {
        return Ok(());
    };
    let mut emptied = EMPTIED.lock().unwrap_or_else(|e| e.into_inner());
    if emptied.contains(&tree) {
        return Ok(());
    }
    match std::fs::remove_dir_all(&tree) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
            return Err(DataError::Unavailable(format!(
                "EXACT_AGENT_STORAGE_FRESH: could not empty {}: {e}",
                tree.display()
            )));
        }
        _ => {}
    }
    emptied.push(tree);
    Ok(())
}

/// A scripted drive's scratch storage (`EXACT_AGENT_STORAGE=<name>`): a tree
/// of its own under the cache base, so a drive can exercise storage without
/// touching the app's real files. Absent, a drive has no storage.
fn agent_scratch() -> Result<Option<String>, DataError> {
    let Some(name) = std::env::var_os("EXACT_AGENT_STORAGE") else {
        return Ok(None);
    };
    match name.to_str() {
        Some(name)
            if !matches!(name, "" | "." | "..")
                && name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b".-_".contains(&b)) =>
        {
            Ok(Some(name.to_owned()))
        }
        _ => Err(DataError::Unavailable(
            "EXACT_AGENT_STORAGE: one name of letters, digits, '.', '-' or '_'".into(),
        )),
    }
}

/// The app's directories at boot, before storage is configured after first
/// pixel: an `image "app:/data/…"` resolves from the first frame, whether or
/// not anything was picked in this process (D7; recipes F18). Nothing is
/// emptied until storage is configured ([`set_roots`]).
pub fn know_roots(app_id: &str) {
    if let Ok(Some((roots, _))) = app_dirs(app_id) {
        *ROOTS.lock().unwrap_or_else(|e| e.into_inner()) = Some(roots);
    }
}

/// Record the app's directories, as storage is configured, and empty
/// `app:/tmp/picked/`: the last launch's picks are gone (D4). Their handles
/// are pinned here for a share's reads ([`pin`]).
pub fn set_roots(data: PathBuf, cache: PathBuf, temporary: PathBuf) {
    let _ = std::fs::remove_dir_all(temporary.join("picked"));
    let roots = [data, cache, temporary];
    pin(&roots);
    *ROOTS.lock().unwrap_or_else(|e| e.into_inner()) = Some(roots);
}

fn roots() -> [PathBuf; 3] {
    let mut held = ROOTS.lock().unwrap_or_else(|e| e.into_inner());
    held.get_or_insert_with(|| {
        let base = std::env::temp_dir().join(format!("exact-agent-{}", std::process::id()));
        [
            base.join("data"),
            base.join("cache"),
            base.join("temporary"),
        ]
    })
    .clone()
}

/// A `pickedPath` reply (`{"path":"app:/tmp/picked/…"}`) with the file it
/// names and the three roots, for the Swift host.
pub fn with_files(reply: String) -> String {
    let [_, _, temporary] = roots();
    let Some(rest) = reply
        .find("app:/tmp/picked/")
        .map(|i| &reply[i + "app:/tmp/".len()..])
        .and_then(|r| r.split('"').next())
    else {
        return reply;
    };
    let file = temporary.join(rest);
    let mut s = reply.trim_end_matches('}').to_owned();
    s.push_str(",\"file\":");
    exact_runner::agent::quote(&file.to_string_lossy(), &mut s);
    s.push(',');
    s.push_str(&roots_reply()[1..]);
    s
}

/// `{"roots":{"data":…,"cache":…,"tmp":…}}`, for the Swift host (`appRoots`,
/// and beside each picked name).
pub fn roots_reply() -> String {
    let mut s = String::from("{\"roots\":{");
    for (i, (name, root)) in ["data", "cache", "tmp"].iter().zip(roots()).enumerate() {
        s.push_str(if i == 0 { "\"" } else { ",\"" });
        s.push_str(name);
        s.push_str("\":");
        exact_runner::agent::quote(&root.to_string_lossy(), &mut s);
    }
    s.push_str("}}");
    s
}

/// The session's document calls (LLP 1069.010 D1), `owner` its number:
/// `openDocument` (`{"path"}`: a host route — Finder, the command line,
/// ⌘O, Open Recent, a path typed into `open-file`) answers `{"path":
/// "doc:/…"}` for an app granted `fs.read doc:/`, and the path unchanged
/// for one that reads its files itself; `mintDocument` (`{"path"}`: what a
/// picker returned, exactly) answers `{"doc"}`; `forgetDocuments` drops
/// every handle the session minted.
pub fn documents(op: &str, request: &str, grants: &str) -> String {
    use exact_runner::agent::{error, field_num, field_str, quote};
    let owner = field_num(request, "owner").unwrap_or(0.0) as u64;
    let path = field_str(request, "path").unwrap_or_default();
    let reply = |key: &str, value: &str| {
        let mut s = format!("{{\"{key}\":");
        quote(value, &mut s);
        s.push('}');
        s
    };
    match op {
        "forgetDocuments" => {
            exact_data::documents::forget(owner);
            "{}".into()
        }
        "openDocument" if !exact_runner::save_file::covered(grants, "fs.read", "doc:/") => {
            reply("path", &path)
        }
        "openDocument" => match exact_data::documents::open_route(&path, owner) {
            Some(doc) => reply("path", &doc),
            None => reply("path", &path),
        },
        _ => match exact_data::documents::mint(std::path::Path::new(&path), owner) {
            Some(doc) => reply("doc", &doc),
            None => error(&format!("{path} is not a file or folder")),
        },
    }
}

/// The agent's `appFile` ([`app_file`]) and `shareFile` ([`share_copy`],
/// through the roots of the asking session's own app, `data`'s).
pub fn file_op<D: exact_runner::DataSource>(op: &str, request: &str, data: &D) -> String {
    match op {
        "appFile" => app_file(&exact_runner::agent::field_str(request, "path").unwrap_or_default()),
        _ => {
            let roots = app_dirs(data.app_id())
                .ok()
                .flatten()
                .map(|(roots, _)| roots);
            share_copy(request, data.grants(), roots.as_ref())
        }
    }
}

/// `{"file":"/…"}`: the file behind an `app:/data|cache|tmp/…` path, which
/// an export copies out (LLP 1069.010 D3); an error for anything else.
pub fn app_file(path: &str) -> String {
    let [data, cache, temporary] = roots();
    let resolved = path.strip_prefix("app:/").and_then(|rest| {
        let (dir, rest) = rest.split_once('/')?;
        let root = match dir {
            "data" => data,
            "cache" => cache,
            "tmp" => temporary,
            _ => return None,
        };
        let parts: Vec<&str> = rest.split('/').collect();
        if parts
            .iter()
            .any(|p| p.is_empty() || *p == "." || *p == ".." || p.contains('\0'))
        {
            return None;
        }
        Some(parts.iter().fold(root, |p, part| p.join(part)))
    });
    match resolved {
        Some(file) => {
            let mut s = String::from("{\"file\":");
            exact_runner::agent::quote(&file.to_string_lossy(), &mut s);
            s.push('}');
            s
        }
        None => exact_runner::agent::error(&format!("{path} names no app file")),
    }
}

/// Each app's directories as its storage opened them, by their paths, pinned
/// by their handles from then on (LLP 1125 D12): a share reads through its
/// own session's and never reopens a root by name, so a root or an ancestor
/// of it renamed or replaced later (a link to another tree) changes nothing
/// a share reads, and another session's storage (LLP 1031: two sessions in
/// one process, perhaps two apps) is not this one's. An app's entry is set
/// when its storage is configured ([`set_roots`]), as storage reopens its own.
static PINNED: Mutex<BTreeMap<[PathBuf; 3], Result<AppDirectories, String>>> =
    Mutex::new(BTreeMap::new());

/// Pin `roots`' handles for a share's reads ([`PINNED`]).
fn pin(roots: &[PathBuf; 3]) {
    let opened = open_pinned(roots);
    PINNED
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(roots.clone(), opened);
}

/// The largest app file a share copies (LLP 1125 D12): the copy runs on the
/// thread that asked, the main one, so a larger file is refused by name
/// rather than holding the interface while it copies.
pub const SHARE_COPY_MAX: u64 = 64 << 20;

/// The roots' handles, opened as the request executor opens them
/// (`executor_body::open`): made if missing, a root that is itself a link
/// refused.
fn open_pinned(roots: &[PathBuf; 3]) -> Result<AppDirectories, String> {
    if roots
        .iter()
        .any(|r| std::fs::symlink_metadata(r).is_ok_and(|m| m.is_symlink()))
    {
        return Err("an app root is a symbolic link".into());
    }
    for root in roots {
        std::fs::create_dir_all(root).map_err(|e| e.to_string())?;
    }
    AppDirectories::new(&roots[0], &roots[1], &roots[2]).map_err(|e| e.to_string())
}

/// `{"file":to}`: a copy at `to` of the app file `path` (LLP 1125 D12), read
/// as `readFile` reads it: admitted by the app's `fs.read` grants and opened
/// relative to the pinned roots' handles without following a symbolic link
/// at any component, so a link in the app's tree cannot put a file outside
/// the grant on the share sheet. Only a regular file of at most
/// [`SHARE_COPY_MAX`] bytes is copied; `to` must not exist. An error for
/// anything else. `request` is `{"op":"shareFile","path":…,"to":…}`;
/// `roots` the asking session's app's, `None` where it has no storage.
pub fn share_copy(request: &str, grants: &str, roots: Option<&[PathBuf; 3]>) -> String {
    let field = |k| exact_runner::agent::field_str(request, k).unwrap_or_default();
    let (path, to) = (field("path"), field("to"));
    let pinned = roots.and_then(|r| {
        PINNED
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(r)
            .cloned()
    });
    let copied = match pinned {
        Some(Ok(directories)) => copy_confined(
            &directories,
            &path,
            std::path::Path::new(&to),
            grants,
            SHARE_COPY_MAX,
        ),
        Some(Err(e)) => Err(format!("the app's files would not open: {e}")),
        None => Err("this host has no app files".into()),
    };
    match copied {
        Ok(()) => {
            let mut s = String::from("{\"file\":");
            exact_runner::agent::quote(&to, &mut s);
            s.push('}');
            s
        }
        Err(e) => exact_runner::agent::error(&format!("{path}: {e}")),
    }
}

fn copy_confined(
    directories: &AppDirectories,
    path: &str,
    to: &std::path::Path,
    grants: &str,
    max: u64,
) -> Result<(), String> {
    use std::io::Read;
    let grants = ibex2::grant::GrantSet::parse(&exact_runner::io_grants(grants))
        .map_err(|e| format!("the app's grants did not parse: {e}"))?;
    let from = directories
        .open_file(&grants, path)
        .map_err(|e| e.to_string())?;
    let too_large = || format!("larger than the {} MB a share copies", max >> 20);
    if from.metadata().map_err(|e| e.to_string())?.len() > max {
        return Err(too_large());
    }
    let mut copy = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(to)
        .map_err(|e| e.to_string())?;
    // A file that grows while it is copied stops one byte past the bound.
    let copied = std::io::copy(&mut from.take(max + 1), &mut copy).map_err(|e| e.to_string());
    let failed = match copied {
        Ok(n) if n > max => Some(too_large()),
        Ok(_) => None,
        Err(e) => Some(e),
    };
    if let Some(e) = failed {
        drop(copy);
        let _ = std::fs::remove_file(to);
        return Err(e);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    /// A fresh tree: `data`, `cache`, `tmp`, an outside file, and the copy's place.
    fn tree(name: &str) -> (PathBuf, [PathBuf; 3]) {
        let base =
            std::env::temp_dir().join(format!("exact-share-copy-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let roots = [base.join("data"), base.join("cache"), base.join("tmp")];
        for r in &roots {
            std::fs::create_dir_all(r).unwrap();
        }
        std::fs::create_dir_all(roots[0].join("backups")).unwrap();
        std::fs::write(roots[0].join("backups/notes.json"), b"{}").unwrap();
        std::fs::create_dir_all(base.join("outside/backups")).unwrap();
        std::fs::write(base.join("outside/secret"), b"secret").unwrap();
        std::fs::write(base.join("outside/backups/notes.json"), b"other").unwrap();
        (base, roots)
    }

    const GRANTS: &str = "fs.read app:/data\n";

    #[test]
    fn a_share_copies_only_a_granted_regular_file_and_follows_no_link() {
        let (base, roots) = tree("links");
        let dirs = open_pinned(&roots).unwrap();
        let to = |n: &str| base.join(n);
        let copy = |path: &str, n: &str| copy_confined(&dirs, path, &to(n), GRANTS, 1 << 20);
        assert_eq!(copy("app:/data/backups/notes.json", "a"), Ok(()));
        assert_eq!(std::fs::read(to("a")).unwrap(), b"{}");
        // A leaf link to a file outside the grant.
        symlink(base.join("outside/secret"), roots[0].join("backups/leaf")).unwrap();
        assert!(copy("app:/data/backups/leaf", "b").is_err());
        // An ancestor link to a folder outside it.
        symlink(base.join("outside"), roots[0].join("folder")).unwrap();
        assert!(copy("app:/data/folder/secret", "c").is_err());
        // A folder, a file no grant covers, and a copy that would overwrite.
        assert!(copy("app:/data/backups", "d").is_err());
        std::fs::write(roots[1].join("x"), b"x").unwrap();
        assert!(copy("app:/cache/x", "e").is_err());
        assert!(copy("app:/data/backups/notes.json", "a").is_err());
        // The file the path named replaced by a link: still refused, since the
        // leaf is opened without following it at the moment of the read.
        std::fs::rename(roots[0].join("backups/notes.json"), base.join("kept")).unwrap();
        symlink(
            base.join("outside/secret"),
            roots[0].join("backups/notes.json"),
        )
        .unwrap();
        assert!(copy("app:/data/backups/notes.json", "f").is_err());
        for n in ["b", "c", "d", "e", "f"] {
            assert!(!to(n).exists(), "{n} was written");
        }
        // Grants that do not parse admit nothing, whatever else they say.
        let broken = "fs.read app:/data\nfs.write relative";
        std::fs::write(roots[0].join("backups/plain.json"), b"{}").unwrap();
        assert!(copy_confined(
            &dirs,
            "app:/data/backups/plain.json",
            &to("g"),
            broken,
            1 << 20
        )
        .is_err());
        let _ = std::fs::remove_dir_all(&base);
    }

    /// The roots are the handles storage pinned: a data root swapped later
    /// for a link to another tree (or any directory) changes nothing a share
    /// reads, and a root that is a link when pinned is refused.
    #[test]
    fn a_root_swapped_after_pinning_is_not_followed() {
        let (base, roots) = tree("swap");
        let dirs = open_pinned(&roots).unwrap();
        std::fs::rename(&roots[0], base.join("moved")).unwrap();
        symlink(base.join("outside"), &roots[0]).unwrap();
        assert_eq!(
            copy_confined(
                &dirs,
                "app:/data/backups/notes.json",
                &base.join("a"),
                GRANTS,
                1 << 20
            ),
            Ok(())
        );
        assert_eq!(
            std::fs::read(base.join("a")).unwrap(),
            b"{}",
            "the pinned tree's file"
        );
        assert!(open_pinned(&roots).is_err(), "a root that is a link");
        let _ = std::fs::remove_dir_all(&base);
    }

    /// Two sessions' apps pinned in one process (LLP 1031): each share reads
    /// its own app's tree, and roots never pinned have no files.
    #[test]
    fn each_session_shares_through_its_own_pinned_roots() {
        let (a, roots_a) = tree("session-a");
        let (b, roots_b) = tree("session-b");
        std::fs::write(roots_b[0].join("backups/notes.json"), b"b's").unwrap();
        pin(&roots_a);
        pin(&roots_b);
        let ask = |to: &PathBuf| {
            format!(
                r#"{{"op":"shareFile","path":"app:/data/backups/notes.json","to":"{}"}}"#,
                to.display()
            )
        };
        assert!(share_copy(&ask(&a.join("a")), GRANTS, Some(&roots_a)).contains("\"file\""));
        assert!(share_copy(&ask(&b.join("b")), GRANTS, Some(&roots_b)).contains("\"file\""));
        assert_eq!(std::fs::read(a.join("a")).unwrap(), b"{}");
        assert_eq!(std::fs::read(b.join("b")).unwrap(), b"b's");
        let unpinned = [a.join("x"), a.join("y"), a.join("z")];
        assert!(share_copy(&ask(&a.join("c")), GRANTS, Some(&unpinned)).contains("no app files"));
        assert!(share_copy(&ask(&a.join("d")), GRANTS, None).contains("no app files"));
        let _ = std::fs::remove_dir_all(&a);
        let _ = std::fs::remove_dir_all(&b);
    }

    /// A file larger than the bound is refused by name and nothing is left.
    #[test]
    fn a_file_past_the_bound_is_refused() {
        let (base, roots) = tree("bound");
        let dirs = open_pinned(&roots).unwrap();
        std::fs::write(roots[0].join("big"), vec![7u8; (1 << 20) + 1]).unwrap();
        let e =
            copy_confined(&dirs, "app:/data/big", &base.join("a"), GRANTS, 1 << 20).unwrap_err();
        assert_eq!(e, "larger than the 1 MB a share copies");
        assert!(!base.join("a").exists());
        std::fs::write(roots[0].join("big"), vec![7u8; 1 << 20]).unwrap();
        assert_eq!(
            copy_confined(&dirs, "app:/data/big", &base.join("a"), GRANTS, 1 << 20),
            Ok(())
        );
        let _ = std::fs::remove_dir_all(&base);
    }
}
