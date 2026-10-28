//! `state.pending`: the requests in flight, by the clock's count (LLP 1016
//! D5), then those the server holds (LLP 1016.000 D9), which no clock waits
//! on, marked `held`, then the answers waiting for their own timers, marked
//! `timer` with when it is due (LLP 1027.000 D1), then held device requests
//! (LLP 1069.007 D3).

use super::quote;
use crate::{DataSource, Runner};
use std::fmt::Write as _;

/// `"pending":[…]`.
pub(super) fn state<D: DataSource>(runner: &Runner<D>, s: &mut String) {
    s.push_str("\"pending\":[");
    let in_flight = runner.in_flight();
    for (i, (name, ticket)) in in_flight.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str("{\"name\":");
        quote(name, s);
        let _ = write!(s, ",\"ticket\":{ticket}}}");
    }
    // Requests the server holds (LLP 1016.000 D9): pending, no clock waits on them.
    let held = runner.held_requests();
    for (i, (name, ticket)) in held.iter().enumerate() {
        if i > 0 || !in_flight.is_empty() {
            s.push(',');
        }
        s.push_str("{\"name\":");
        quote(name, s);
        let _ = write!(s, ",\"ticket\":{ticket},\"held\":true}}");
    }
    // Answers waiting for their own timers: no clock waits on them either.
    let timers = runner.data_timer_waits();
    for (i, (name, ticket, due)) in timers.iter().enumerate() {
        if i > 0 || in_flight.len() + held.len() > 0 {
            s.push(',');
        }
        s.push_str("{\"name\":");
        quote(name, s);
        let _ = write!(
            s,
            ",\"ticket\":{ticket},\"timer\":{}}}",
            crate::agent::num(*due)
        );
    }
    // Held device requests, after the network's (LLP 1069.007 D3): each
    // with its capability and that capability's inspection summary.
    let network = in_flight.len() + held.len() + timers.len();
    for (i, hold) in runner.device_holds().iter().enumerate() {
        if i > 0 || network > 0 {
            s.push(',');
        }
        s.push_str("{\"name\":");
        quote(&hold.name, s);
        let _ = write!(
            s,
            ",\"ticket\":{},\"device\":{{\"capability\":",
            hold.ticket
        );
        quote(&hold.capability, s);
        s.push_str(",\"args\":");
        if hold.args.starts_with('{') && hold.args.ends_with('}') {
            s.push_str(&hold.args);
        } else {
            s.push_str("{}");
        }
        s.push_str("}}");
    }
    s.push(']');
    // Replies landed for held requests, by target: `clock held`'s evidence.
    let replies = runner.held_replies();
    if !replies.is_empty() {
        s.push_str(",\"heldReplies\":{");
        for (i, (name, count)) in replies.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            quote(name, s);
            let _ = write!(s, ":{count}");
        }
        s.push('}');
    }
}

/// `{"held":[…],"heldReplies":{…}}`: the requests the server holds, each
/// `{name, ticket}`, and the replies landed for held requests by target.
pub(super) fn held<D: DataSource>(runner: &Runner<D>) -> String {
    let mut s = String::from("{\"held\":[");
    for (i, (name, ticket)) in runner.held_requests().iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str("{\"name\":");
        quote(name, &mut s);
        let _ = write!(s, ",\"ticket\":{ticket}}}");
    }
    s.push_str("],\"heldReplies\":{");
    for (i, (name, count)) in runner.held_replies().iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        quote(name, &mut s);
        let _ = write!(s, ":{count}");
    }
    s.push_str("}}");
    s
}
