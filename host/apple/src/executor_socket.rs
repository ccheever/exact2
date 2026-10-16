//! A resource's two-way socket, natively (LLP 1016.000 D7), over ibex2's
//! WebSocket transport (URLSession on Apple, the portable one elsewhere).
//! Its reader delivers `open`, then every frame in arrival order, text or
//! binary, never coalesced, and ends with the closing handshake's code and
//! reason. Frames wait for the runner in the stream's own queue, bounded at
//! 256 frames and 1 MiB besides the per-message ceiling; overflow ends the
//! stream refused after the frames already queued. A mutation's batch
//! ([`exact_runner::SocketSend`]) is checked whole, then sent in order.
use super::super::{complete, failed, has_ready, wake, Completed, Core, Shared};
use exact_runner::{FailureKind, Message, Outcome, SocketOp, SocketSend};
use ibex2::stdlib::{
    abort::AbortController,
    websocket::{Event, MessageSender, MessageSource},
};
use std::sync::Arc;

/// Undelivered frames a socket may hold, by count and by bytes (D7).
const RECEIVE_FRAMES: usize = 256;
const RECEIVE_BYTES: usize = 1 << 20;
/// What a mutation's batch and the transport's buffer may hold together.
const SEND_BYTES: usize = 1 << 20;

/// An open socket's sending half, while its reader runs.
pub(crate) struct Socket {
    sender: Arc<dyn MessageSender>,
    /// A close was sent: nothing more is.
    closing: bool,
}

/// Close the sockets of `tickets` (every one, for `None`) with 1000 before
/// their streams are aborted, as the web JS target's release does: the far
/// side sees a closing handshake, not a dropped connection.
pub(crate) fn close(shared: &Shared, tickets: Option<&[u64]>) {
    let mut sockets = shared.sockets.lock().unwrap();
    for (ticket, socket) in sockets.iter_mut() {
        if !socket.closing && tickets.is_none_or(|t| t.contains(ticket)) {
            socket.closing = true;
            let _ = socket.sender.close(Some(1000), "");
        }
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
/// mutations' batches until then.
pub(super) fn read(
    shared: &Shared,
    ticket: u64,
    mut socket: Box<dyn MessageSource>,
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
        if matches!(frame(shared, ticket, open), Frame::Gone) {
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
            match frame(shared, ticket, m) {
                Frame::Queued => {}
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

/// Queue one frame behind the ones the runner has not taken: never
/// coalesced. Its bytes are the socket's own bound, beside the stream's
/// reservation, which covers one message.
fn frame(shared: &Shared, ticket: u64, message: Message) -> Frame {
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
        .fold((0, 0), |(n, bytes), done| match &done.outcome {
            Outcome::Message(m) => (n + 1, bytes + size(m)),
            _ => (n, bytes),
        });
    if frames >= RECEIVE_FRAMES || bytes + size(&message) > RECEIVE_BYTES {
        return Frame::Overflow;
    }
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
    /// Send one mutation's socket batch, whose commit stood: every socket
    /// it names must still be open and wanted, and each must take its
    /// share without its buffer passing 1 MiB, or nothing is sent. For each
    /// socket, the frames it accepted and the bytes it now buffers.
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
            if sockets[ticket].sender.buffered_amount() + bytes > SEND_BYTES {
                return Err("QuotaExceededError: socket buffer exceeds 1 MiB".into());
            }
        }
        for (ticket, _, op) in &batch.ops {
            let socket = sockets.get_mut(ticket).ok_or_else(not_open)?;
            let sent = match op {
                SocketOp::Text(text) => socket.sender.send_text(text),
                SocketOp::Binary(bytes) => socket.sender.send_binary(bytes),
                SocketOp::Close { code, reason } => {
                    socket.closing = true;
                    socket.sender.close(Some(*code), reason)
                }
            };
            sent.map_err(|e| e.to_string())?;
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
