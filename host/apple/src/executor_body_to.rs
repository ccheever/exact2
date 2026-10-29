//! A reply's body written to an app file (LLP 1108 D6 R2, ruled 2026-10-26):
//! `fetch(url, {exactBodyTo: "app:/…"})` from TypeScript, `Request::body_to`
//! from Rust. Before the request is sent, the worker admits `fs.write` on the
//! path and creates a staging file beside it through the app's pinned
//! directory handles (Ibex patch 19), so a denied path or a missing parent is
//! the request's failure and nothing is sent. A 2xx body goes to the staging
//! file a chunk at a time as it arrives, never held whole, and the staging
//! file replaces the target once the body is complete. A failure, an abort,
//! the deadline or a body over the bound drops the staging file, and the
//! target is left as it was. A reply that is not 2xx is read into the
//! response as any reply is, at most [`BODY_TO_HELD_BYTES`], and the file is
//! untouched. The bytes written travel as the response's [`BODY_WRITTEN`]
//! header.
//!
//! Memory and the lane: the request is charged what it can hold, a reply that
//! is not 2xx (`executor_core.rs` `reservation`), never the bound on what it
//! writes, so a large download neither waits for the lane's whole budget nor
//! is refused for it.
//!
//! Time: the writes run on the worker between the transport's reads, so the
//! deadline and an abort stop the body at its next chunk; a write stalled on
//! the disk holds this request's worker until it returns.
use super::body;
use exact_runner::{
    FailureKind, HttpScheduling, Outcome, Request, BODY_TO_HELD_BYTES, BODY_WRITTEN,
    MAX_BODY_TO_BYTES,
};
use ibex2::boundary::HostError;
use ibex2::stdlib::app_fs::Staged;
use ibex2::stdlib::fetch::{over_limit, Response, StreamingResponse};

/// What one read of the body takes before it is written.
const CHUNK: usize = 64 << 10;

/// The most of a reply `request` may read: its `max_response_bytes`, else
/// 1 GiB for a body written to a file and 64 MiB for any other.
pub(super) fn limit(request: &Request) -> usize {
    match request.http {
        HttpScheduling::Independent { max_response_bytes } => max_response_bytes as usize,
        HttpScheduling::Ordered if request.body_to.is_some() => MAX_BODY_TO_BYTES as usize,
        HttpScheduling::Ordered => super::MAX_BODY,
    }
}

/// The staging file for `request`'s body, made before the request is sent;
/// `None` for a request whose body is not written to a file.
pub(super) fn stage(
    roots: body::Roots<'_>,
    grants: &str,
    request: &Request,
) -> Result<Option<Staged>, Outcome> {
    let Some(path) = request.body_to.as_deref() else {
        return Ok(None);
    };
    if let Some(why) = request.body_to_refusal() {
        return Err(Outcome::Failed {
            kind: FailureKind::Refused,
            message: why.into(),
        });
    }
    let what = "exactBodyTo";
    let (granted, directories) =
        body::open_as(what, roots, grants, request.grants.as_deref(), path)?;
    directories
        .stage(&granted, path)
        .map(Some)
        .map_err(|e| body::named_as(what, path, e))
}

/// The response: with `staged`, a 2xx body written there and published, its
/// size in the [`BODY_WRITTEN`] header and no body; any other reply read into
/// memory, at most [`BODY_TO_HELD_BYTES`]. Past `limit` either is
/// [`over_limit`]`(limit)`, as a reply read into memory is.
pub(super) fn receive(
    mut r: StreamingResponse,
    staged: Option<Staged>,
    limit: usize,
) -> Result<Response, HostError> {
    let Some(mut staged) = staged else {
        return r.collect();
    };
    let held = limit.min(BODY_TO_HELD_BYTES as usize);
    let (mut chunk, mut body, mut written) = (vec![0; CHUNK], Vec::new(), 0u64);
    loop {
        let n = r.body.read(&mut chunk)?;
        if n == 0 {
            break;
        }
        if r.ok() {
            staged.write_all(&chunk[..n])?;
            written += n as u64;
        } else if body.len() + n > held {
            r.body.cancel();
            return Err(over_limit(limit));
        } else {
            body.extend_from_slice(&chunk[..n]);
        }
    }
    let (ok, mut headers) = (r.ok(), r.headers);
    if ok {
        staged.publish()?;
        headers.set(BODY_WRITTEN, &written.to_string());
    }
    Ok(Response {
        status: r.status,
        status_text: r.status_text,
        headers,
        body,
        url: r.url,
        redirected: r.redirected,
    })
}
