//! `state.streams` (LLP 1016.000 D5): the open streams, pending until their
//! first message, then listed with what they delivered and what coalesced,
//! the grant line that admitted each, and a two-way socket's state (D7).

use super::{error, field_str, quote};
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
        // A child process (LLP 1133.007 D2): what ran, its incarnation,
        // bytes each way, restarts, its exit and whether it drained.
        if let Some(child) = runner.child_state(*ticket) {
            s.push_str(",\"kind\":\"process\",\"program\":");
            quote(&child.program, s);
            s.push_str(",\"ran\":");
            quote(&child.ran, s);
            let pid = child.pid.map_or("null".into(), |p| p.to_string());
            let _ = write!(
                s,
                ",\"pid\":{pid},\"incarnation\":{},\"restarts\":{},\"bytesOut\":{},\"bytesIn\":{}",
                child.incarnation, child.restarts, child.bytes_out, child.bytes_in
            );
            match &child.exit {
                Some((code, signal)) => {
                    let code = code.map_or("null".into(), |c| c.to_string());
                    let _ = write!(s, ",\"exit\":{{\"code\":{code},\"signal\":");
                    match signal {
                        Some(signal) => quote(signal, s),
                        None => s.push_str("null"),
                    }
                    s.push('}');
                }
                None => s.push_str(",\"exit\":null"),
            }
            let drained = child
                .drained
                .map_or("null", |d| if d { "true" } else { "false" });
            let _ = write!(s, ",\"drained\":{drained}");
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
        s.push('}');
    }
    s.push(']');
}

/// `prefer origin <person|tool|wake|headless>`: the origin later inputs run
/// as (LLP 1133.002 D12). Nothing in the runner makes a tool, wake or
/// headless turn yet (LLP 1111, 1114.001), so this stands in for one: a
/// drive proves LLP 1133.007 D13's refusal of a child's start, write, end
/// or signal from a turn that is not a person's.
pub(super) fn turn<D: DataSource>(runner: &Runner<D>, request: &str) -> String {
    match field_str(request, "origin")
        .as_deref()
        .and_then(crate::TurnOrigin::parse)
    {
        Some(origin) => {
            runner.set_turn_origin(origin);
            format!("{{\"origin\":\"{}\"}}", origin.name())
        }
        None => error("prefer origin: person, tool, wake or headless"),
    }
}
