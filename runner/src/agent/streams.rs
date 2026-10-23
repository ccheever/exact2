//! `state.streams` (LLP 1016.000 D5): the open streams, pending until their
//! first message, then listed with what they delivered and what coalesced,
//! the grant line that admitted each, and a two-way socket's state (D7).

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
        // A long poll waiting to ask again (LLP 1016.000 D8).
        if let Some(retry) = runner.retrying(*ticket) {
            let _ = write!(
                s,
                ",\"retrying\":{{\"attempt\":{},\"inMs\":{},\"error\":",
                retry.attempt, retry.in_ms
            );
            quote(&retry.error, s);
            s.push('}');
        }
        s.push('}');
    }
    s.push(']');
}
