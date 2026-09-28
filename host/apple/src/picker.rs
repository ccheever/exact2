//! Where the picker's files land on Apple (LLP 1069.002 D4, D7): the app's
//! `app:/data`, `app:/cache` and `app:/tmp` as storage configures them, or,
//! for a scripted drive with no scratch store, a directory of this
//! process's own. The Swift host copies into the file a `pickedPath` reply
//! names and resolves an `image`/`video` `app:/` source against the roots.

use std::path::PathBuf;
use std::sync::Mutex;

static ROOTS: Mutex<Option<[PathBuf; 3]>> = Mutex::new(None);

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
    let [data, cache, temporary] = roots();
    let Some(rest) = reply
        .find("app:/tmp/picked/")
        .map(|i| &reply[i + "app:/tmp/".len()..])
        .and_then(|r| r.split('"').next())
    else {
        return reply;
    };
    let file = temporary.join(rest);
    let mut s = reply.trim_end_matches('}').to_owned();
    let quote =
        |p: &std::path::Path, s: &mut String| exact_runner::agent::quote(&p.to_string_lossy(), s);
    s.push_str(",\"file\":");
    quote(&file, &mut s);
    s.push_str(",\"roots\":{\"data\":");
    quote(&data, &mut s);
    s.push_str(",\"cache\":");
    quote(&cache, &mut s);
    s.push_str(",\"tmp\":");
    quote(&temporary, &mut s);
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
