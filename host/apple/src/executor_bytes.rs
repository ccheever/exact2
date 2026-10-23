//! A byte stream, natively (LLP 1016.000, the amendment of 2026-10-23): a
//! TypeScript stream's 2xx body that is not `text/event-stream`. Its reader
//! appends each read to the one message the runner has not taken, so the
//! messages follow the runner's pace, not the transport's chunking, and no
//! byte is dropped or reordered. What waits is bounded by the stream's
//! ceiling, at most 1 MiB; a read that would pass it
//! ends the stream refused after the bytes already queued.
use super::super::{complete, failed, has_ready, wake, Completed, Shared};
use exact_runner::{FailureKind, Message, Outcome};
use ibex2::stdlib::{abort::AbortController, fetch::StreamingResponse};

/// The undelivered bytes a byte stream may hold, unless its ceiling is lower.
const RECEIVE_BYTES: usize = 1 << 20;

/// What became of one read.
enum Read {
    Queued,
    /// Nobody wants more: forgotten, retired, ended.
    Gone,
    /// The bytes waiting would pass the bound.
    Overflow,
}

/// Read the open byte stream `ticket` to its end. It ends when the body
/// does, the connection drops, the runner falls a bound behind, or the
/// runner lets the ticket go (forgetting aborts the read).
pub(super) fn read(
    shared: &Shared,
    ticket: u64,
    response: StreamingResponse,
    limit: usize,
    abort: AbortController,
) {
    let bound = limit.min(RECEIVE_BYTES);
    let mut body = response.body;
    let run = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // No one read can pass the bound alone.
        let mut chunk = vec![0; bound.min(64 << 10)];
        loop {
            match body.read(&mut chunk) {
                Ok(0) => return failed(FailureKind::Network, "the byte stream ended"),
                Ok(n) => match append(shared, ticket, &chunk[..n], bound) {
                    Read::Queued => {}
                    Read::Gone => return failed(FailureKind::Aborted, "the stream was let go"),
                    Read::Overflow if bound == RECEIVE_BYTES => {
                        return failed(
                            FailureKind::Refused,
                            "byte stream receive queue exceeds 1 MiB",
                        )
                    }
                    Read::Overflow => {
                        return failed(
                            FailureKind::Refused,
                            "byte stream receive queue exceeds the response ceiling",
                        )
                    }
                },
                Err(_) if abort.signal().aborted() => {
                    return failed(FailureKind::Aborted, "native request aborted")
                }
                Err(e) => {
                    return failed(
                        FailureKind::Network,
                        e.to_string().chars().take(2048).collect::<String>(),
                    )
                }
            }
        }
    }))
    .unwrap_or_else(|_| failed(FailureKind::Aborted, "the stream reader panicked"));
    body.cancel();
    complete(shared, ticket, run);
}

/// Append `bytes` to the message of `ticket` the runner has not taken, or
/// queue a new one.
fn append(shared: &Shared, ticket: u64, bytes: &[u8], bound: usize) -> Read {
    let mut state = shared.state.lock().unwrap();
    if state.retired {
        return Read::Gone;
    }
    let Some((lane, elapsed_ms)) = state
        .running
        .iter()
        .find(|run| run.ticket == ticket && !run.forgotten)
        .map(|run| {
            (
                run.lane,
                run.started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
            )
        })
    else {
        return Read::Gone;
    };
    let waiting = state.completed[lane]
        .iter_mut()
        .find_map(|done| match &mut done.outcome {
            Outcome::Message(Message {
                binary: Some(queued),
                ..
            }) if done.ticket == ticket && done.message => Some(queued),
            _ => None,
        });
    match waiting {
        Some(queued) if queued.len() + bytes.len() > bound => return Read::Overflow,
        Some(queued) => queued.extend_from_slice(bytes),
        None => {
            exact_runner::stream_timing::received(ticket);
            state.completed[lane].push_back(Completed {
                elapsed_ms,
                ticket,
                outcome: Outcome::Message(Message {
                    binary: Some(bytes.to_vec()),
                    ..Message::default()
                }),
                // Its bytes are the stream's own bound, beside the reservation.
                bytes: 0,
                message: true,
                stream: true,
                light: false,
                again: false,
            })
        }
    }
    if has_ready(&state) {
        wake(&mut state);
    }
    Read::Queued
}
