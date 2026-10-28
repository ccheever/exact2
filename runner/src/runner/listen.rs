//! A resource's local listener (LLP 1133.005): its standing on its address
//! (D2), its journal lines (D12) and the host's view of its connections.
//! Its events and sends are a two-way socket's (`sockets.rs`), with each
//! send addressed to a connection.
use super::super::*;
use crate::listen::{ListenView, Standing, LISTEN_STATUS};

impl<D: DataSource> Runner<D> {
    /// Read each listener's address and open connections from the host
    /// with `reader`, for `state.streams` (D12).
    pub fn set_listen_reader(&mut self, reader: crate::listen::ListenReader) {
        self.socket_reader.listen = Some(reader);
    }

    /// Listener `ticket`'s address and open connections, from the host.
    pub fn listen_view(&self, ticket: u64) -> Option<ListenView> {
        (self.socket_reader.listen.as_ref()).and_then(|read| read(ticket))
    }

    /// Where the listener of stream `ticket` stands, if it is one.
    pub fn listen_standing(&self, ticket: u64) -> Option<Standing> {
        let p = self.pending.iter().find(|p| p.ticket == ticket)?;
        p.stream?.socket?.listen
    }

    /// The journal's lines for one message of the listener at `pos`, and
    /// whether it was only its standing (D2), which no mapper sees: the
    /// runner records it and the message commits nothing. Its data is a
    /// [`Standing`]'s word (`holding` is listening again), or `took over`
    /// after a `listening` that replaced a live socket; its id is the
    /// address. False for any other stream.
    pub(in crate::runner) fn listen_event(&mut self, pos: usize, m: &crate::Message) -> bool {
        let target = self.pending[pos].target;
        let socket = self.pending[pos].stream.and_then(|s| s.socket);
        if socket.and_then(|s| s.listen).is_none() {
            return false;
        }
        let name = self.target_name(target);
        if m.event != LISTEN_STATUS {
            let line = match m.event.as_str() {
                "listening" => exact_num::text!("listen {} {} bound", name, m.data),
                "connection" => exact_num::text!("listen {}: connection {} opened", name, m.id),
                "end" => {
                    exact_num::text!("listen {}: connection {} ended ({})", name, m.id, m.data)
                }
                _ => return false,
            };
            self.log(line);
            return false;
        }
        if m.data == "took over" {
            self.log(exact_num::text!("listen {} {}: took over", name, m.id));
            return true;
        }
        let standing = match m.data.as_str() {
            "waiting" => Standing::Waiting,
            "displaced" => Standing::Displaced,
            "holding" => Standing::Listening,
            _ => return true,
        };
        let line = match standing {
            Standing::Waiting => exact_num::text!("listen {} {}: waiting for it", name, m.id),
            Standing::Displaced => exact_num::text!("listen {} {}: taken over", name, m.id),
            _ => exact_num::text!("listen {} {}: held again", name, m.id),
        };
        self.log(line);
        let count = self.pending[pos].stream.as_mut();
        if let Some(s) = count.and_then(|s| s.socket.as_mut()) {
            // A waiting listener has not bound, so it stays pending.
            if s.ready_state == 1 || standing == Standing::Waiting {
                s.listen = Some(standing);
            }
        }
        self.sync_pending_flags();
        true
    }
}

impl PendingReq {
    /// No clock waits on it: held (LLP 1016.000 D9), or a listener waiting
    /// for an address another listener of this process holds (D2).
    pub(in crate::runner) fn quiet(&self) -> bool {
        self.held
            || (self.stream)
                .and_then(|s| s.socket?.listen)
                .is_some_and(|l| l == Standing::Waiting)
    }
}

/// The journal's line for one listener send that the host accepted:
/// `listen send: N bytes (<resource>, connection 3)`, never the line's text.
pub(super) fn sent(name: &str, connection: u64, op: &crate::SocketOp) -> String {
    match op {
        crate::SocketOp::End { .. } => {
            exact_num::text!("listen end ({}, connection {})", name, connection)
        }
        _ => exact_num::text!(
            "listen send: {} bytes ({}, connection {})",
            op.size(),
            name,
            connection
        ),
    }
}
