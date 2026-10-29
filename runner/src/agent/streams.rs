//! `state.streams` (LLP 1016.000 D5): the open streams, pending until their
//! first message, then listed with what they delivered and what coalesced,
//! the grant line that admitted each, a two-way socket's state (D7), and a
//! local listener's (LLP 1133.005 D12).

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
        // A local listener's address and connections (LLP 1133.005 D12).
        if let Some(standing) = count.socket.and_then(|s| s.listen) {
            listener(runner, *ticket, standing, s);
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

/// A listener's `"address"`, `"listen"` (where it stands: `waiting for
/// <address>` while another listener of this process holds it) and its
/// open `"connections"`, each with the lines received and sent.
fn listener<D: DataSource>(
    runner: &Runner<D>,
    ticket: u64,
    standing: crate::listen::Standing,
    s: &mut String,
) {
    let view = runner.listen_view(ticket).unwrap_or_default();
    s.push_str(",\"address\":");
    quote(&view.address, s);
    s.push_str(",\"listen\":");
    match standing {
        crate::listen::Standing::Waiting => quote(&format!("waiting for {}", view.address), s),
        other => quote(other.name(), s),
    }
    s.push_str(",\"connections\":[");
    for (i, c) in view.connections.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        let _ = write!(
            s,
            "{{\"id\":{},\"received\":{},\"sent\":{}}}",
            c.id, c.received, c.sent
        );
    }
    s.push(']');
}
