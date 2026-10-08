//! A request body read from an app file (LLP 1108 D6 R2): `fetch(url,
//! {exactBodyFrom: "app:/tmp/…"})` from TypeScript, `Request::body_from`
//! from Rust. The worker that runs the request reads the file just before
//! it goes out, under the same `fs.read` grant and app-directory handles a
//! source's `readFile` uses, at most [`MAX_BODY_FROM_BYTES`]. Any refusal
//! (no app files here, a denied path, a missing file, one over the bound)
//! is the request's failure, and nothing is sent. The bytes never cross the
//! source, so a 2 MB upload costs its step nothing.
//!
//! Memory: the file is read whole into the request, as a byte body would
//! have been, at most 64 MiB per running request. Like other allocations
//! during native work it is bounded by the worker and stream counts, not by
//! the lane's byte reservations (the core's `Core` comment).
use exact_runner::{FailureKind, Outcome, Request, MAX_BODY_FROM_BYTES};
use ibex2::stdlib::app_fs::AppDirectories;
use std::path::PathBuf;

/// The app's `app:/data`, `app:/cache` and `app:/tmp` handles, opened once
/// when the host names them (or why they would not open); `None` on a host
/// or drive that has no app files.
pub(super) type Roots<'a> = Option<&'a Result<AppDirectories, String>>;

/// Open the app's directories, made as storage makes them, so their handles
/// pin the roots from now on: a root renamed or replaced later (by a
/// symlink to another app's tree, say) is not followed, as `readFile`'s
/// handles do not follow it.
pub(super) fn open(roots: &[PathBuf; 3]) -> Result<AppDirectories, String> {
    for root in roots {
        std::fs::create_dir_all(root).map_err(|e| format!("app storage: {e}"))?;
    }
    AppDirectories::new(&roots[0], &roots[1], &roots[2]).map_err(|e| e.to_string())
}

/// Read `request.body_from` into `request.body` under `grants` (the app's,
/// narrowed by the request's own scope). A request without one is untouched.
pub(super) fn resolve(
    roots: Roots<'_>,
    grants: &str,
    request: &mut Request,
) -> Result<(), Outcome> {
    let Some(path) = request.body_from.clone() else {
        return Ok(());
    };
    if let Some(why) = request.body_from_refusal() {
        return Err(refused(why.to_string()));
    }
    let bytes = read(roots, grants, request.grants.as_deref(), &path)?;
    request.body = bytes;
    request.body_from = None;
    Ok(())
}

fn refused(message: String) -> Outcome {
    Outcome::Failed {
        kind: FailureKind::Refused,
        message,
    }
}

/// The file's bytes, or the refusal that names why not.
pub(super) fn read(
    roots: Roots<'_>,
    grants: &str,
    scope: Option<&str>,
    path: &str,
) -> Result<Vec<u8>, Outcome> {
    use ibex2::{
        boundary::HostError,
        stdlib::fs::{FsOp, FsResult},
    };
    let named = |e: HostError| {
        match e {
        // As `readFile`'s denial reads (js/src/prelude.js `deniedPath`).
        HostError::Denied { capability } => refused(format!(
            "exactBodyFrom {path}: denied: {capability} {path}: no grant covers it; grant `{capability} {path}`"
        )),
        e => refused(format!("exactBodyFrom {path}: {e}")),
    }
    };
    let Some(directories) = roots else {
        return Err(Outcome::Failed {
            kind: FailureKind::Unsupported,
            message: format!("exactBodyFrom {path}: this host has no app files (no storage here)"),
        });
    };
    let admitted = exact_data::storage::scope(grants, scope)
        .map_err(|e| refused(format!("exactBodyFrom {path}: {e}")))?;
    let grants = ibex2::grant::GrantSet::parse(&exact_runner::io_grants(admitted))
        .map_err(|e| refused(format!("the app's grants did not parse: {e}")))?;
    let directories = directories.as_ref().map_err(|e| {
        refused(format!(
            "exactBodyFrom {path}: the app's files would not open: {e}"
        ))
    })?;
    // `stat` admits the read and names a missing file or a folder before
    // anything is read; the read itself is capped, so a file that grew since
    // is refused without being read whole.
    match ibex2::stdlib::fs::run(&grants, Some(directories), FsOp::Stat, path, None, None)
        .map_err(named)?
    {
        FsResult::Stat(stat) if !stat.is_file => {
            return Err(refused(format!("exactBodyFrom {path}: not a file")));
        }
        FsResult::Stat(stat) if stat.size > MAX_BODY_FROM_BYTES => {
            return Err(too_large(path, format!("{} bytes is", stat.size)));
        }
        _ => {}
    }
    #[cfg(unix)]
    let bytes = directories
        .read_capped(&grants, path, MAX_BODY_FROM_BYTES + 1)
        .map_err(named)?;
    // Elsewhere (Windows) there is no read capped on the opened handle yet,
    // and an uncapped read could pass the bound for a file that grew since
    // `stat`: refused, naming why, rather than read whole.
    #[cfg(not(unix))]
    let bytes: Vec<u8> = {
        let _ = &directories;
        return Err(Outcome::Failed {
            kind: FailureKind::Unsupported,
            message: format!(
                "exactBodyFrom {path}: not on this host yet: it has no capped file read"
            ),
        });
    };
    if bytes.len() as u64 > MAX_BODY_FROM_BYTES {
        return Err(too_large(path, "it grew to more bytes than".into()));
    }
    Ok(bytes)
}

fn too_large(path: &str, size: String) -> Outcome {
    refused(format!(
        "exactBodyFrom {path}: too-large: {size} over {MAX_BODY_FROM_BYTES}"
    ))
}
