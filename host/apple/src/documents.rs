//! The documents a person chose, as the Swift host asks for them (@ref LLP
//! 1069.010 D1; LLP 1069.010.000 D3–D6): minted for the session that asks,
//! and, for an app granted `doc.keep`, kept across launches in a set beside
//! its `app:/` roots (`kept-documents.json`), never inside them. Under the
//! agent that is the drive's scratch store, and a drive with none keeps
//! nothing.
//!
//! The Swift host makes and resolves the security-scoped bookmarks and
//! reads `NSDocumentController`'s recents, the list that decides what stays
//! kept (D6); what a bookmark found is checked here against the object
//! recorded ([`kept::restore`]).

use exact_data::documents::{self, kept};
use exact_runner::agent::{error, field_num, field_str, quote};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;

/// The calls [`answer`] answers.
const OPS: [&str; 7] = [
    "openDocument",
    "mintDocument",
    "forgetDocuments",
    "keptDocuments",
    "keptResolved",
    "keepDocument",
    "keptRecents",
];

/// Whether an app keeps the documents a person chooses (D7): on a Mac; iOS
/// keeps nothing until an iOS document app asks (the LLP's Rulings).
fn keeps(grants: &str) -> bool {
    cfg!(target_os = "macos") && grants.lines().any(|line| line.trim() == "doc.keep")
}

/// Where `app_id` keeps its set: beside its `app:/` roots, or a drive's
/// scratch store's; `None` where it has none.
fn store_file(app_id: &str) -> Option<PathBuf> {
    let ([data, ..], _) = crate::picker::app_dirs(app_id).ok().flatten()?;
    Some(data.parent()?.join("kept-documents.json"))
}

/// The session's document calls, `owner` its number:
///
/// - `openDocument` (`{"path"}`: a host route — Finder, the command line,
///   ⌘O, Open Recent, a path typed into `open-file`) answers `{"path":
///   "doc:/…"}` for an app granted `fs.read doc:/`, and the path unchanged
///   for one that reads its files itself; `mintDocument` (`{"path"}`: what
///   a picker returned, exactly) answers `{"doc"}`; `forgetDocuments` drops
///   every handle the session minted that its app does not keep.
/// - `keptDocuments`, as the session's data activates: `{"keep": false}`
///   for an app that keeps nothing; else the records this launch has yet to
///   find (`restore`: `n`, `bookmark`, `device`, `inode`), which the host
///   resolves and answers with `keptResolved` (`found`: `n`, `path`, a
///   refreshed `bookmark`; `recent`: the paths the platform's list holds).
/// - `keepDocument` (`{"path", "bookmark", "recent"}`): what the person
///   chose, kept; `{"doc"}` names it. `keptRecents` (`{"recent"}`): the
///   platform's list changed.
///
/// `None` for any other call.
pub fn answer<D: exact_runner::DataSource>(
    op: Option<&str>,
    request: &str,
    runner: &exact_runner::Runner<D>,
) -> Option<String> {
    let op = op.filter(|op| OPS.contains(op))?;
    Some(documents(
        op,
        request,
        runner.data_ref(),
        &runner.native_slot(),
    ))
}

fn documents<D: exact_runner::DataSource>(
    op: &str,
    request: &str,
    data: &D,
    native: &exact_runner::Native,
) -> String {
    let owner = field_num(request, "owner").unwrap_or(0.0) as u64;
    let path = field_str(request, "path").unwrap_or_default();
    let reply = |key: &str, value: &str| {
        let mut s = format!("{{\"{key}\":");
        quote(value, &mut s);
        s.push('}');
        s
    };
    // Read the kept set before the session mints anything, so no `<n>` is
    // one an earlier launch gave (D4).
    if keeps(data.grants()) && op != "forgetDocuments" {
        if let Some(file) = store_file(data.app_id()) {
            kept::open(owner, data.app_id(), &file);
        }
    }
    let fields: Value = serde_json::from_str(request).unwrap_or(Value::Null);
    let recent = || -> Vec<PathBuf> {
        fields["recent"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(PathBuf::from)
            .collect()
    };
    match op {
        "forgetDocuments" => {
            kept::end(owner);
            documents::forget(owner);
            "{}".into()
        }
        "openDocument" if !exact_runner::save_file::covered(data.grants(), "fs.read", "doc:/") => {
            reply("path", &path)
        }
        "openDocument" => match documents::open_route(&path, owner) {
            Some(doc) => reply("path", &doc),
            None => reply("path", &path),
        },
        "mintDocument" => match documents::mint(std::path::Path::new(&path), owner) {
            Some(doc) => reply("doc", &doc),
            None => error(&format!("{path} is not a file or folder")),
        },
        "keptDocuments" if kept::list(owner).is_none() => "{\"keep\":false}".into(),
        "keptDocuments" => {
            let native = native.clone();
            kept::listen(
                owner,
                Some(Arc::new(move || native.changed(kept::LIST_TOPIC))),
            );
            let restore: Vec<Value> = kept::to_restore(owner)
                .into_iter()
                .map(|r| {
                    serde_json::json!({
                        "n": r.n, "bookmark": r.bookmark,
                        "device": r.identity.device, "inode": r.identity.inode,
                    })
                })
                .collect();
            serde_json::json!({ "keep": true, "restore": restore }).to_string()
        }
        "keptResolved" => {
            let found: Vec<(u64, PathBuf, Option<String>)> = fields["found"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|f| {
                    Some((
                        f["n"].as_u64()?,
                        PathBuf::from(f["path"].as_str()?),
                        f["bookmark"].as_str().map(str::to_owned),
                    ))
                })
                .collect();
            // What the list no longer holds is never restored (D6).
            kept::held(owner, &recent());
            kept::restore(owner, &found);
            "{}".into()
        }
        // What the panels and Finder grant on a Mac is reading and writing.
        "keepDocument" => {
            let bookmark = field_str(request, "bookmark").unwrap_or_default();
            let doc = kept::keep(owner, std::path::Path::new(&path), &bookmark, "readwrite");
            kept::held(owner, &recent());
            doc.map_or_else(|| "{}".into(), |doc| reply("doc", &doc))
        }
        _ => {
            kept::held(owner, &recent());
            "{}".into()
        }
    }
}

/// The Swift host's hook for a document the app forgot (`forgetDocument`):
/// it takes the object off `NSDocumentController`'s recents.
pub fn on_forgotten(forgotten: extern "C" fn(u64, u64)) {
    kept::on_forgotten(Arc::new(move |identity, _| {
        forgotten(identity.device, identity.inode)
    }));
}
