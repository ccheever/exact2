//! A resource's two-way socket (LLP 1016.000 D7). A mutation's source
//! stages operations on a socket handle it kept from a resource's mapper;
//! the runner addresses them to the stream ticket of the resource that
//! owns the socket now, and hands them to the host only once the
//! mutation's commit stands. A refused commit takes them back with
//! everything else (`commit.rs`'s checkpoint).
use super::*;
use crate::request::{SocketOp, SocketSend};

/// What `state.streams` shows of a resource's socket.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SocketState {
    /// The WebSocket `readyState` as the runner knows it: 0 until the
    /// socket's `open` is delivered, then 1. Its end takes the stream away.
    pub ready_state: u8,
    /// Frames the host has accepted for sending.
    pub sent: u64,
    /// The transport's buffered bytes after the last publication.
    pub buffered: u64,
}

impl<D: DataSource> Runner<D> {
    /// Address the operations mutation `m`'s answer staged to the open
    /// sockets of the resources they name, for publication once the commit
    /// stands. A resource whose socket is not open now refuses the whole
    /// send, as the handle's `InvalidStateError` would have.
    pub(super) fn stage_sockets(
        &mut self,
        m: usize,
        staged: Vec<(usize, SocketOp)>,
    ) -> Result<(), String> {
        if staged.is_empty() {
            return Ok(());
        }
        let mut ops = Vec::with_capacity(staged.len());
        for (i, op) in staged {
            let open = self.pending.iter().find(|p| {
                p.target == Target::Resource(i)
                    && p.stream
                        .and_then(|s| s.socket)
                        .is_some_and(|s| s.ready_state == 1)
            });
            let Some(p) = open else {
                return Err("InvalidStateError: the socket is not open".into());
            };
            ops.push((p.ticket, self.target_name(p.target), op));
        }
        let mutation = self.plan.str(self.plan.mutations[m].name).to_string();
        self.sockets.push(SocketSend { mutation, ops });
        Ok(())
    }

    /// The socket operations of the commits that stood since the last
    /// take, one batch per mutation, in commit order. A host takes them
    /// after it has let go of the tickets the runner no longer holds, so a
    /// socket the same commit released refuses its batch.
    pub fn take_socket_sends(&mut self) -> Vec<SocketSend> {
        std::mem::take(&mut self.sockets)
    }

    /// Hand each batch of the commits that stood to `send`, the host's
    /// executor, and record what it made of it ([`Runner::socket_published`]).
    /// A host calls it after it has let go of the tickets the runner released.
    pub fn publish_sockets(
        &mut self,
        mut send: impl FnMut(&SocketSend) -> Result<Vec<(u64, u64, u64)>, String>,
    ) {
        for batch in self.take_socket_sends() {
            let result = send(&batch);
            self.socket_published(&batch, result);
        }
    }

    /// What the host made of a batch [`Runner::take_socket_sends`] gave it:
    /// for each socket, the frames it accepted and the bytes its transport
    /// buffers now; or why it refused the whole batch, which sent nothing
    /// (D7: the commit stood, and acceptance is not server acknowledgement).
    pub fn socket_published(
        &mut self,
        batch: &SocketSend,
        result: Result<Vec<(u64, u64, u64)>, String>,
    ) {
        let sockets = match result {
            Ok(sockets) => sockets,
            Err(why) => {
                let line = exact_num::text!(
                    "send {} failed: {}; socket frames were refused",
                    batch.mutation,
                    why
                );
                return self.log(line);
            }
        };
        for (_, name, op) in &batch.ops {
            self.log(exact_num::text!(
                "socket {}: {} bytes ({})",
                op.name(),
                op.size(),
                name
            ));
        }
        for (ticket, sent, buffered) in sockets {
            let socket = self
                .pending
                .iter_mut()
                .find(|p| p.ticket == ticket)
                .and_then(|p| p.stream.as_mut()?.socket.as_mut());
            if let Some(socket) = socket {
                socket.sent += sent;
                socket.buffered = buffered;
            }
        }
    }
}

/// A stream's count for a request: a two-way socket's state too.
pub(super) fn stream_count(request: &crate::Request) -> Option<StreamCount> {
    request.stream.then(|| StreamCount {
        socket: request.socket.then(SocketState::default),
        ..StreamCount::default()
    })
}

/// A delivered message's effect on its socket's state: `open` opens it.
pub(super) fn socket_message(count: &mut StreamCount, message: &crate::Message) {
    if let Some(socket) = count.socket.as_mut() {
        if message.event == "open" {
            socket.ready_state = 1;
        }
    }
}
