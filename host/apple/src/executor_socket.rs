//! A resource's two-way socket, natively (LLP 1016.000 D7), over ibex2's
//! WebSocket transport (URLSession on Apple, the portable one elsewhere).
//! Its reader delivers `open`, then every frame in arrival order, text or
//! binary, never coalesced, and ends with the closing handshake's code and
//! reason. Frames wait for the runner in the stream's own queue, bounded at
//! 256 frames and 1 MiB beyond the per-message ceiling; overflow ends the
//! stream refused after the frames already queued. A mutation's or a
//! mapper call's batch ([`exact_runner::SocketSend`]) is checked whole,
//! then sent in order.
use super::super::{complete, failed, has_ready, wake, Completed, Core, Shared};
use exact_runner::{FailureKind, Message, Outcome, SocketOp, SocketSend};
use ibex2::stdlib::{
    abort::AbortController,
    websocket::{Event, MessageSender, MessageSource},
};
use std::sync::Arc;

/// Undelivered frames a socket may hold (D7).
const RECEIVE_FRAMES: usize = 256;
/// Bytes allowed beyond the stream's per-message ceiling (D7).
const RECEIVE_BYTES: usize = 1 << 20;
/// What one batch and the transport's buffer may hold together:
/// bytes (D7), and frames, the bound past which both ibex2 transports close
/// the connection rather than queue (WHATWG's "full").
const SEND_BYTES: usize = 1 << 20;
const SEND_FRAMES: usize = 256;

/// An open socket's sending half, while its reader runs.
pub(crate) struct Socket {
    sender: Arc<dyn MessageSender>,
    /// A close was sent: nothing more is.
    closing: bool,
}

/// Close the sockets of `tickets` (every one, for `None`) with 1000 before
/// their streams are aborted, as the web JS target's release does: the far
/// side sees a closing handshake, not a dropped connection. The tickets of
/// the sockets it closed.
pub(crate) fn close(shared: &Shared, tickets: Option<&[u64]>) -> Vec<u64> {
    let mut sockets = shared.sockets.lock().unwrap();
    let mut closed = Vec::new();
    for (ticket, socket) in sockets.iter_mut() {
        if !socket.closing && tickets.is_none_or(|t| t.contains(ticket)) {
            socket.closing = true;
            let _ = socket.sender.close(Some(1000), "");
            closed.push(*ticket);
        }
    }
    closed
}

/// How long a released socket's close frame has to reach the wire before
/// its stream is aborted: the portable transport queues it, and an abort
/// shuts the connection at once.
const CLOSE_GRACE: std::time::Duration = std::time::Duration::from_millis(250);

/// Abort the work let go, each with its stream's ticket when it is one: a
/// socket is closed with 1000 first (unless a mutation already closed it),
/// and aborted after [`CLOSE_GRACE`] (the reader ends sooner when the far
/// side answers the close).
pub(crate) fn release(shared: &Shared, aborts: Vec<(Option<u64>, AbortController)>) {
    let streams: Vec<u64> = aborts.iter().filter_map(|(t, _)| *t).collect();
    let _ = close(shared, Some(&streams));
    let sockets: Vec<u64> = {
        let open = shared.sockets.lock().unwrap();
        streams
            .into_iter()
            .filter(|t| open.contains_key(t))
            .collect()
    };
    let (later, now): (Vec<_>, Vec<_>) =
        (aborts.into_iter()).partition(|(t, _)| t.is_some_and(|t| sockets.contains(&t)));
    for (_, abort) in now {
        abort.abort();
    }
    if later.is_empty() {
        return;
    }
    let aborts: Vec<AbortController> = later.into_iter().map(|(_, abort)| abort).collect();
    let deferred = aborts.clone();
    let spawned = std::thread::Builder::new()
        .name("exact-socket-close".into())
        .spawn(move || {
            std::thread::sleep(CLOSE_GRACE);
            deferred.iter().for_each(AbortController::abort);
        });
    // No thread: abort now rather than never.
    if spawned.is_err() {
        aborts.iter().for_each(AbortController::abort);
    }
}

/// What became of one frame.
enum Frame {
    Queued,
    /// Nobody wants more: forgotten, retired, ended.
    Gone,
    /// The queue is full.
    Overflow,
}

/// Read the open socket `ticket` to its end. Its sender is registered for
/// batches until then.
pub(super) fn read(
    shared: &Shared,
    ticket: u64,
    mut socket: Box<dyn MessageSource>,
    limit: usize,
    abort: AbortController,
) {
    if let Some(sender) = socket.sender() {
        let socket = Socket {
            sender,
            closing: false,
        };
        shared.sockets.lock().unwrap().insert(ticket, socket);
    }
    let gone = || failed(FailureKind::Aborted, "the stream was let go");
    let run = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let open = Message {
            event: "open".into(),
            ..Message::default()
        };
        if matches!(frame(shared, ticket, open, limit), Frame::Gone) {
            return gone();
        }
        loop {
            let m = match socket.next_event() {
                Ok(Event::Message(ibex2::stdlib::websocket::Message::Text(data))) => Message {
                    data,
                    ..Message::default()
                },
                Ok(Event::Message(ibex2::stdlib::websocket::Message::Binary(bytes))) => Message {
                    binary: Some(bytes),
                    ..Message::default()
                },
                Ok(Event::Close {
                    code,
                    reason,
                    was_clean,
                }) => {
                    return Outcome::Closed {
                        code,
                        reason,
                        was_clean,
                    }
                }
                Ok(Event::Open { .. } | Event::Error(_)) => continue,
                Err(_) if abort.signal().aborted() => {
                    return failed(FailureKind::Aborted, "native request aborted")
                }
                // Both transports word a frame over the ceiling so (ibex2
                // `MessageSource::next_event`); the frames before it are queued.
                Err(e) if e.to_string().contains("exceeded its configured limit") => {
                    return failed(
                        FailureKind::Refused,
                        "a message exceeds the response ceiling",
                    )
                }
                Err(e) => return failed(FailureKind::Network, e.to_string()),
            };
            match frame(shared, ticket, m, limit) {
                Frame::Queued => {}
                // Released, its close on the way: frames are dropped, and the
                // socket is kept until the far side answers or the abort comes,
                // so the close is not cut off.
                Frame::Gone if closing(shared, ticket) => {}
                Frame::Gone => return gone(),
                Frame::Overflow => {
                    return failed(
                        FailureKind::Refused,
                        "socket receive queue exceeds 256 frames or 1 MiB",
                    )
                }
            }
        }
    }))
    .unwrap_or_else(|_| failed(FailureKind::Aborted, "the stream reader panicked"));
    shared.sockets.lock().unwrap().remove(&ticket);
    drop(socket);
    complete(shared, ticket, run);
}

/// Whether `ticket`'s socket has sent its close.
fn closing(shared: &Shared, ticket: u64) -> bool {
    let sockets = shared.sockets.lock().unwrap();
    sockets.get(&ticket).is_some_and(|socket| socket.closing)
}

/// Queue one frame behind the ones the runner has not taken: never
/// coalesced. The stream's reservation covers one message up to its ceiling;
/// `RECEIVE_BYTES` is the additional byte allowance.
fn frame(shared: &Shared, ticket: u64, message: Message, limit: usize) -> Frame {
    let mut state = shared.state.lock().unwrap();
    if state.retired {
        return Frame::Gone;
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
        return Frame::Gone;
    };
    let size = |m: &Message| m.binary.as_ref().map_or(m.data.len(), Vec::len);
    let (frames, bytes) = state.completed[lane]
        .iter()
        .filter(|done| done.ticket == ticket && done.message)
        .fold((0, 0usize), |(n, bytes), done| match &done.outcome {
            // `open` is no protocol frame: only those count against the bound.
            Outcome::Message(m) if m.event != "open" => (n + 1, bytes.saturating_add(size(m))),
            _ => (n, bytes),
        });
    if frames >= RECEIVE_FRAMES
        || bytes.saturating_add(size(&message)) > RECEIVE_BYTES.saturating_add(limit)
    {
        return Frame::Overflow;
    }
    exact_runner::stream_timing::received(ticket);
    state.completed[lane].push_back(Completed {
        elapsed_ms,
        ticket,
        outcome: Outcome::Message(message),
        bytes: 0,
        message: true,
        stream: true,
        light: false,
        again: false,
    });
    if has_ready(&state) {
        wake(&mut state);
    }
    Frame::Queued
}

impl Core {
    /// Each open socket's buffered bytes, by its stream ticket, read live
    /// from its transport: the runner's snapshot at each turn's start.
    pub(crate) fn socket_reader(&self) -> exact_runner::SocketReader {
        let shared = self.shared.clone();
        std::sync::Arc::new(move |ticket| {
            let sockets = shared.sockets.lock().unwrap();
            (sockets.get(&ticket)).map(|socket| socket.sender.buffered_amount() as u64)
        })
    }

    /// Send one mutation's or mapper call's socket batch, whose commit
    /// stood: every socket it names must still be open and wanted, and each
    /// must take its share without its buffer passing 1 MiB or 256 frames,
    /// or nothing is sent. For each socket, the frames it accepted and the
    /// bytes it now buffers.
    pub(crate) fn socket(&self, batch: &SocketSend) -> Result<Vec<(u64, u64, u64)>, String> {
        let not_open = || "InvalidStateError: the socket is not open".to_string();
        {
            let state = self.shared.state.lock().unwrap();
            let wanted =
                |t: u64| (state.running.iter()).any(|run| run.ticket == t && !run.forgotten);
            if state.retired || !batch.ops.iter().all(|(t, ..)| wanted(*t)) {
                return Err(not_open());
            }
        }
        let mut sockets = self.shared.sockets.lock().unwrap();
        let mut totals: Vec<(u64, usize, u64)> = Vec::new();
        for (ticket, _, op) in &batch.ops {
            let socket = sockets.get(ticket).filter(|s| !s.closing);
            socket.ok_or_else(not_open)?;
            match totals.iter_mut().find(|(t, ..)| t == ticket) {
                Some(total) => total.1 += op.size(),
                None => totals.push((*ticket, op.size(), 0)),
            }
        }
        for (ticket, bytes, _) in &totals {
            let sender = &sockets[ticket].sender;
            if sender.buffered_amount() + bytes > SEND_BYTES {
                return Err("QuotaExceededError: socket buffer exceeds 1 MiB".into());
            }
            let frames = (batch.ops.iter())
                .filter(|(t, _, op)| t == ticket && !matches!(op, SocketOp::Close { .. }))
                .count();
            if sender.buffered_messages() + frames > SEND_FRAMES {
                return Err("QuotaExceededError: socket buffer exceeds 256 frames".into());
            }
        }
        for (done, (ticket, _, op)) in batch.ops.iter().enumerate() {
            let socket = sockets.get_mut(ticket).ok_or_else(not_open)?;
            let sent = match op {
                SocketOp::Text(text) => socket.sender.send_text(text),
                SocketOp::Binary(bytes) => socket.sender.send_binary(bytes),
                SocketOp::Close { code, reason } => {
                    socket.closing = true;
                    socket.sender.close(Some(*code), reason)
                }
            };
            // The transport failed partway: what it took is gone, and the
            // journal says how much.
            sent.map_err(|e| format!("{e} (after {done} of {} operations)", batch.ops.len()))?;
            if !matches!(op, SocketOp::Close { .. }) {
                let total = totals.iter_mut().find(|(t, ..)| t == ticket);
                total.expect("totalled above").2 += 1;
            }
        }
        Ok(totals
            .into_iter()
            .map(|(ticket, _, sent)| {
                let buffered = sockets
                    .get(&ticket)
                    .map_or(0, |s| s.sender.buffered_amount());
                (ticket, sent, buffered as u64)
            })
            .collect())
    }
}

#[cfg(test)]
#[path = "executor_socket_tests.rs"]
mod tests;
