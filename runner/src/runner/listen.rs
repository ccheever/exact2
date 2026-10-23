//! A local listener's stream (LLP 1133.005): the runner's record of it for
//! `state.streams` and the journal (D12), the host's notes no mapper sees
//! (D2's waiting, displaced and taken back), and the connections a send may
//! name. Its replies are a two-way socket's (D6): `sockets.rs` addresses
//! them, each to its connection.
use super::*;
use crate::request::{Message, SocketOp};
use exact_num::text;

/// The host's live count of a listener connection's unwritten bytes, by
/// the listener's stream ticket and the connection's id; `None` for one it
/// holds no connection for.
pub type ListenReader = std::sync::Arc<dyn Fn(u64, u64) -> Option<u64> + Send + Sync>;

/// What `state.streams` shows of a listener (D12).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ListenState {
    /// Its address, as the source wrote it.
    pub address: String,
    /// Its `listening` event has landed.
    pub listening: bool,
    /// Another listener of this process holds the address (D2). A waiting
    /// listener is quiescent: no clock waits on it.
    pub waiting: bool,
    /// Its open connections, in the order they connected.
    pub connections: Vec<ConnectionCount>,
}

/// One open connection of a listener.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConnectionCount {
    /// Its id, from its `connection` event.
    pub id: u64,
    /// Lines delivered to the runner.
    pub received: u64,
    /// Lines the host accepted for sending.
    pub sent: u64,
}

/// The host's notes on a listener, delivered as messages no mapper sees.
pub const WAITING: &str = "exact:waiting";
/// A newer listener of this process took the address over.
pub const DISPLACED: &str = "exact:displaced";
/// The listener took the address back.
pub const RESUMED: &str = "exact:resumed";

impl<D: DataSource> Runner<D> {
    /// Read each listener connection's unwritten bytes from the host with
    /// `reader`, at the start of every turn that may send on one.
    pub fn set_listen_reader(&mut self, reader: ListenReader) {
        self.socket_readers.listen = Some(reader);
    }

    /// A host note on the listener at `pos`, journaled and recorded with no
    /// commit: false for any message the mapper answers.
    pub(super) fn listen_note(&mut self, pos: usize, m: &Message) -> bool {
        let note = matches!(m.event.as_str(), WAITING | DISPLACED | RESUMED);
        let name = self.target_name(self.pending[pos].target);
        let Some(listener) = (self.pending[pos].stream.as_mut())
            .and_then(|s| s.listener.as_mut())
            .filter(|_| note)
        else {
            return false;
        };
        let line = match m.event.as_str() {
            WAITING => {
                listener.waiting = true;
                text!("listen {}: waiting for {}", name, listener.address)
            }
            DISPLACED => text!(
                "listen {}: a newer listener took {} over",
                name,
                listener.address
            ),
            _ => text!("listen {}: took {} back", name, listener.address),
        };
        self.log(line);
        true
    }

    /// Whether `p` is a listener waiting for its address: not in flight.
    pub(super) fn listen_waiting(p: &PendingReq) -> bool {
        (p.stream.as_ref())
            .and_then(|s| s.listener.as_ref())
            .is_some_and(|l| l.waiting)
    }

    /// Each listener connection's unwritten bytes for this turn, by
    /// resource: the host's count with the batches it has not taken.
    pub(super) fn snapshot_listeners(&mut self) {
        let mut snapshot = Vec::new();
        for p in &self.pending {
            let (Target::Resource(i), Some(listener)) = (
                p.target,
                p.stream.as_ref().and_then(|s| s.listener.as_ref()),
            ) else {
                continue;
            };
            let reader = self.socket_readers.listen.as_ref();
            for c in &listener.connections {
                let host = reader.and_then(|read| read(p.ticket, c.id)).unwrap_or(0);
                let held = (self.sockets.iter().flat_map(|b| &b.ops))
                    .filter(|(t, _, op)| *t == p.ticket && op.connection() == Some(c.id))
                    .map(|(.., op)| op.size() as u64);
                snapshot.push((i, c.id, host + held.sum::<u64>()));
            }
        }
        self.store.set_listen_buffered(snapshot);
    }

    /// The lines the host accepted on listener `ticket`, counted on their
    /// connections.
    pub(super) fn listen_sent(&mut self, ticket: u64, op: &SocketOp) {
        let Some(id) = op
            .connection()
            .filter(|_| matches!(op, SocketOp::Line { .. }))
        else {
            return;
        };
        let listener = (self.pending.iter_mut().find(|p| p.ticket == ticket))
            .and_then(|p| p.stream.as_mut()?.listener.as_mut());
        if let Some(c) = listener.and_then(|l| l.connections.iter_mut().find(|c| c.id == id)) {
            c.sent += 1;
        }
    }
}

/// Whether `p` may be sent on now: a two-way socket that is open, or a
/// listener that is listening, for an operation on one of its open
/// connections.
pub(super) fn sendable(p: &PendingReq, op: &SocketOp) -> bool {
    let Some(stream) = p.stream.as_ref() else {
        return false;
    };
    match (op.connection(), &stream.listener, stream.socket) {
        (Some(id), Some(l), _) => l.listening && l.connections.iter().any(|c| c.id == id),
        (None, None, Some(socket)) => socket.ready_state == 1,
        _ => false,
    }
}

/// A delivered event's effect on a listener's record, and its journal line.
pub(super) fn listen_message(count: &mut StreamCount, name: &str, m: &Message) -> Option<String> {
    let listener = count.listener.as_mut()?;
    let id = m.id.parse::<u64>().unwrap_or(0);
    match m.event.as_str() {
        "listening" => {
            listener.listening = true;
            listener.waiting = false;
            let how = if m.data.contains("\"tookOver\":true") {
                "took it over"
            } else {
                "bound"
            };
            Some(text!("listen {}: {} {}", name, listener.address, how))
        }
        "connection" => {
            listener.connections.push(ConnectionCount {
                id,
                ..ConnectionCount::default()
            });
            Some(text!("listen {}: connection {} opened", name, id))
        }
        "message" => {
            if let Some(c) = listener.connections.iter_mut().find(|c| c.id == id) {
                c.received += 1;
            }
            None
        }
        "end" => {
            listener.connections.retain(|c| c.id != id);
            Some(text!(
                "listen {}: connection {} ended ({})",
                name,
                id,
                m.data
            ))
        }
        _ => None,
    }
}
