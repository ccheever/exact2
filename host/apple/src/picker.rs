//! Where the picker's files land on Apple (LLP 1069.002 D4, D7): the app's
//! `app:/data`, `app:/cache` and `app:/tmp` as storage configures them, or,
//! for a scripted drive with no scratch store, a directory of this
//! process's own. The Swift host copies into the file a `pickedPath` reply
//! names and resolves an `image`/`video` `app:/` source against the roots,
//! which it learns at boot (`appRoots`).

use exact_runner::DataError;
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
    #[cfg(target_os = "android")]
    let (mut data, mut cache) = {
        // The Android Context owns these roots. The retained host supplies the
        // same XDG inputs as the native-window Android host, inside its sandbox.
        let base = |name: &str| {
            std::env::var_os(name)
                .map(PathBuf::from)
                .filter(|p| p.is_absolute())
                .ok_or_else(|| DataError::Unavailable(format!("Android app storage needs {name}")))
        };
        (
            base("XDG_DATA_HOME")?
                .join("exact")
                .join(app_id)
                .join("data"),
            base("XDG_CACHE_HOME")?.join("exact").join(app_id),
        )
    };
    #[cfg(not(target_os = "android"))]
    let (mut data, mut cache) = {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .ok_or_else(|| DataError::Unavailable("app storage needs an absolute HOME".into()))?;
        let data = home
            .join("Library/Application Support/exact")
            .join(app_id)
            .join("data");
        let cache = home.join("Library/Caches/exact").join(app_id);
        (data, cache)
    };
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

/// Record the app's directories and empty `app:/tmp/picked/`: the last
/// launch's picks are gone (D4).
pub fn set_roots(data: PathBuf, cache: PathBuf, temporary: PathBuf) {
    let _ = std::fs::remove_dir_all(temporary.join("picked"));
    *ROOTS.lock().unwrap_or_else(|e| e.into_inner()) = Some([data, cache, temporary]);
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
