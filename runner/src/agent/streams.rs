//! `state.streams` (LLP 1016.000 D5): the open streams, pending until their
//! first message, then listed with what they delivered and what coalesced,
//! the grant line that admitted each, a two-way socket's state (D7), and
//! a local listener's (LLP 1133.005 D12).

use super::quote;
use crate::{DataSource, Runner};
use std::fmt::Write as _;

/// `"streams":[…]`.
pub(super) fn state<D: DataSource>(runner: &Runner<D>, s: &mut String) {
    s.push_str("\"streams\":[");
    for (i, (name, ticket, count)) in runner.streams().iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str("{\"name\":");
        quote(name, s);
        let _ = write!(
            s,
            ",\"ticket\":{ticket},\"messages\":{},\"coalesced\":{}",
            count.messages, count.coalesced
        );
        if let Some(grant) = runner.stream_grant(*ticket) {
            s.push_str(",\"grant\":");
            quote(grant, s);
        }
        // A two-way socket's state, as the web JS target lists it.
        if let Some(socket) = count.socket {
            let _ = write!(
                s,
                ",\"readyState\":{},\"bufferedAmount\":{},\"sent\":{}",
                socket.ready_state,
                runner.socket_buffered(*ticket, socket.buffered),
                socket.sent
            );
        }
        // A listener's address and state, and its open connections with
        // the lines each received and sent (LLP 1133.005 D12).
        if let Some(listener) = &count.listener {
            s.push_str(",\"address\":");
            quote(&listener.address, s);
            let state = if listener.listening {
                "listening"
            } else if listener.waiting {
                "waiting"
            } else {
                "binding"
            };
            let _ = write!(s, ",\"listener\":\"{state}\",\"connections\":[");
            for (i, c) in listener.connections.iter().enumerate() {
                let comma = if i > 0 { "," } else { "" };
                let _ = write!(
                    s,
                    "{comma}{{\"id\":{},\"received\":{},\"sent\":{}}}",
                    c.id, c.received, c.sent
                );
            }
            s.push(']');
        }
        s.push('}');
    }
    s.push(']');
}
