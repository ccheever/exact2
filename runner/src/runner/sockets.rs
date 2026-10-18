//! A resource's two-way socket (LLP 1016.000 D7). A mutation's source
//! stages operations on a socket handle it kept from a resource's mapper,
//! and the mapper itself may stage replies on its own socket as it answers
//! the socket's `open` or a `message`. The runner addresses them to the
//! stream ticket of the resource that owns the socket now, and hands them
//! to the host only once the commit that staged them stands. A refused
//! commit takes them back with everything else (`commit.rs`'s checkpoint).
//! An answer in flight sends too (the amendment of 2026-10-18): each turn's
//! operations are one batch, sent once the commit that ran the turn stands
//! whatever the answer turns out to be.
use super::*;
use crate::request::{SocketOp, SocketOrigin, SocketSend};

/// The host's live count of the bytes a socket's transport buffers, by the
/// socket's stream ticket; `None` for a ticket it holds no socket for.
pub type SocketReader = std::sync::Arc<dyn Fn(u64) -> Option<u64> + Send + Sync>;

/// What `state.streams` shows of a resource's socket.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SocketState {
    /// The WebSocket `readyState` as the runner knows it: 0 until the
    /// socket's `open` is delivered, then 1. Its end takes the stream away.
    pub ready_state: u8,
    /// Frames the host has accepted for sending.
    pub sent: u64,
    /// The transport's buffered bytes as of the last turn or publication.
    pub buffered: u64,
}

impl<D: DataSource> Runner<D> {
    /// Read each socket's buffered bytes from the host's transport with
    /// `reader`, at the start of every turn that may read a handle.
    pub fn set_socket_reader(&mut self, reader: SocketReader) {
        self.socket_reader = Some(reader);
    }

    /// Before every turn that may read a handle (a mutation's, a mapper's,
    /// an answer's): each open socket's buffered bytes, fresh from the
    /// transport, as its handle's `bufferedAmount` reads them for the whole
    /// turn (WHATWG's count at the task's start, plus what the turn itself
    /// stages), with the batches of earlier turns the host has not taken.
    /// Without a reader, the count the last publication left.
    pub(super) fn snapshot_sockets(&mut self) {
        let mut snapshot = Vec::new();
        for p in &mut self.pending {
            let (Target::Resource(i), Some(socket)) =
                (p.target, p.stream.as_mut().and_then(|s| s.socket.as_mut()))
            else {
                continue;
            };
            if socket.ready_state == 1 {
                if let Some(buffered) = self.socket_reader.as_ref().and_then(|r| r(p.ticket)) {
                    socket.buffered = buffered;
                }
                let held = (self.sockets.iter().flat_map(|b| &b.ops))
                    .filter(|(t, ..)| *t == p.ticket)
                    .map(|(.., op)| op.size() as u64);
                snapshot.push((i, socket.buffered + held.sum::<u64>()));
            }
        }
        self.store.set_socket_buffered(snapshot);
    }

    /// Socket `ticket`'s buffered bytes now: the host's count, else `last`.
    pub fn socket_buffered(&self, ticket: u64, last: u64) -> u64 {
        (self.socket_reader.as_ref())
            .and_then(|read| read(ticket))
            .unwrap_or(last)
    }

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
        let ops = self.address(staged)?;
        let mutation = self.plan.str(self.plan.mutations[m].name).to_string();
        let origin = SocketOrigin::Mutation(mutation);
        self.sockets.push(SocketSend { origin, ops });
        Ok(())
    }

    /// Each staged operation addressed to the open socket of the resource
    /// it names; refused whole when one of them is not open.
    #[allow(clippy::type_complexity)]
    fn address(
        &self,
        staged: Vec<(usize, SocketOp)>,
    ) -> Result<Vec<(u64, String, SocketOp)>, String> {
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
        Ok(ops)
    }

    /// Address what `target`'s answer sent in the turn just run, one batch
    /// in the order it made them, behind the batches of earlier turns. It
    /// goes out once the commit that ran the turn stands, whatever the
    /// turn's answer was ([`Runner::refuse_batches`]); a batch with an
    /// operation on a socket that is not open is refused whole, and said.
    pub(super) fn stage_answer(&mut self, target: Target) {
        let staged = self.store.take_answer_sockets();
        if staged.is_empty() {
            return;
        }
        let origin = SocketOrigin::Answer(self.target_name(target));
        match self.address(staged) {
            Ok(ops) => self.sockets.push(SocketSend { origin, ops }),
            Err(why) => self.log(refused(&origin, &why)),
        }
    }

    /// A refused commit takes back the batches its turns staged after
    /// `from`, as it takes back their fetches: the answers whose turns sent
    /// them are let go with it. An answer's are said, since its code went
    /// on as if they were sent (a mapper's are said by [`Runner::message`]).
    pub(super) fn refuse_batches(&mut self, from: usize, why: &str) {
        let taken = self.sockets.split_off(from.min(self.sockets.len()));
        for batch in taken
            .iter()
            .filter(|b| matches!(b.origin, SocketOrigin::Answer(_)))
        {
            self.log(refused(&batch.origin, why));
        }
    }

    /// Address the replies the mapper of `p`, a resource's open socket,
    /// staged as it answered one frame: they go to that socket alone, in
    /// the order the mapper made them, once this frame's commit stands. A
    /// reply on any other socket refuses the frame and leaves them staged,
    /// for the caller to journal.
    pub(super) fn stage_replies(&mut self, p: &PendingReq) -> Result<(), String> {
        let staged = self.store.staged_sockets();
        if staged.is_empty() {
            return Ok(());
        }
        let own = match p.target {
            Target::Resource(i) => Some(i),
            Target::Mutation(_) => None,
        };
        if staged.iter().any(|(i, _)| Some(*i) != own) {
            return Err("a mapper sends only on its own socket".into());
        }
        if !(p.stream.and_then(|s| s.socket)).is_some_and(|s| s.ready_state == 1) {
            return Err("InvalidStateError: the socket is not open".into());
        }
        let name = self.target_name(p.target);
        let ops = (self.store.take_sockets().into_iter())
            .map(|(_, op)| (p.ticket, name.clone(), op))
            .collect();
        let origin = SocketOrigin::Mapper(name);
        self.sockets.push(SocketSend { origin, ops });
        Ok(())
    }

    /// The socket operations of the commits that stood since the last
    /// take, one batch per mutation, mapper call or answer turn, in the
    /// order they were made; an answer turn's whether or not its commit
    /// stood. A host takes them after it has let go of the tickets the
    /// runner no longer holds, so a socket the same commit released refuses
    /// its batch.
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
            Err(why) => return self.log(refused(&batch.origin, &why)),
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

/// The journal's line for a batch that will not be sent, and why.
pub(super) fn refused(origin: &SocketOrigin, why: &str) -> String {
    let (verb, name) = origin.words();
    exact_num::text!(
        "{} {} failed: {}; socket frames were refused",
        verb,
        name,
        why
    )
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
