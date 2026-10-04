//! The journal's lines (what the agent API's `logs` reads), written piece by
//! piece: boot and a first press journal, and `exact_num::text!` keeps
//! `core::fmt` off those paths (LLP 1047 §10). Each line is the `format!`
//! text it was, byte for byte; the tests hold them to it.

use crate::request::Request;
use exact_num::text;

/// A line as the journal keeps it: stamped with the clock.
pub(super) fn stamped(now_ms: f64, line: &str) -> String {
    text!("t={} {}", crate::agent::num(now_ms), line)
}

pub(super) fn boot(carried: bool, nodes: usize, epoch: u64) -> String {
    let carried = if carried { " (carried)" } else { "" };
    text!("boot{}: {} nodes, epoch {}", carried, nodes, epoch)
}

/// A commit that stood: `what` it was and what it changed.
pub(super) fn committed(
    what: &str,
    epoch: u64,
    created: usize,
    destroyed: usize,
    touched: usize,
) -> String {
    text!(
        "{} → epoch {} (+{} −{} ~{})",
        what,
        epoch,
        created,
        destroyed,
        touched
    )
}

pub(super) fn query(resource: &str, source: &str) -> String {
    text!("query {}: {}", resource, source)
}

/// A source not ready at boot: the bake's answer shows until it is ready
/// and answers again (LLP 1048.003 D6; feed F24). The web JS target says
/// the same (`rt.js`).
pub(super) fn build_time(resource: &str) -> String {
    text!(
        "{} shows its build-time answer until its source answers",
        resource
    )
}

/// That ask answered: the line says whether the first frame was right.
pub(super) fn revalidated(resource: &str, same: bool) -> String {
    let what = if same {
        "equal to its build-time answer"
    } else {
        "replaces its build-time answer"
    };
    text!("{} answered: {}", resource, what)
}

pub(super) fn advanced(fired: usize, epoch: u64) -> String {
    let plural = if fired == 1 { "" } else { "s" };
    text!("advance → {} timer{} fired, epoch {}", fired, plural, epoch)
}

/// A store write, by name only (never the value).
pub(super) fn store_write(name: &str, kept: bool) -> String {
    text!("{} {}", if kept { "store" } else { "forget" }, name)
}

pub(super) fn kept(ticket: u64, name: &str) -> String {
    text!(
        "keep request {} ({}): the same request for newer arguments",
        ticket,
        name
    )
}

pub(super) fn forgot(ticket: u64, name: &str) -> String {
    text!("forget request {} ({})", ticket, name)
}

/// Sends that waited and were never asked (LLP 1092 D4).
pub(super) fn forgot_waiting(n: usize, name: &str) -> String {
    text!(
        "forgot {} waiting send{} ({})",
        n,
        if n == 1 { "" } else { "s" },
        name
    )
}

pub(super) fn enqueued(ticket: u64, name: &str, request: &Request) -> String {
    match request.continuation {
        Some(token) => text!(
            "continuation {} ({}): executor token {}",
            ticket,
            name,
            token
        ),
        None if request.storage.is_some() => text!("storage {} ({})", ticket, name),
        None => text!(
            "request {} ({}): {} {}",
            ticket,
            name,
            request.method,
            request.url
        ),
    }
}

pub(super) fn fulfilling(ticket: u64, name: &str, summary: &str) -> String {
    text!("fulfil {} ({}) [{}]", ticket, name, summary)
}

pub(super) fn dropped(ticket: u64, summary: &str) -> String {
    text!(
        "reply {} dropped: no such request in flight [{}]",
        ticket,
        summary
    )
}

pub(super) fn one_more(name: &str) -> String {
    text!("{}: the reply asks for one more request", name)
}

pub(super) fn seeded(taken: usize, answers: usize) -> String {
    text!("checkpoint: {} of {} answers taken", taken, answers)
}

/// An event delivered to a view, before its action is known.
pub(super) fn event(kind: &str, view: u32) -> String {
    text!("{} view {}", kind, view)
}

/// The action an event ran, appended to its line.
pub(super) fn ran(what: &mut String, action: &str) {
    exact_num::push_text!(what, " ({})", action);
}

pub(super) fn failed_now(name: &str, why: &str) -> String {
    text!("resource {} failed: {}", name, why)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NUMBERS: [u64; 6] = [0, 1, 9, 10, 4_294_967_295, u64::MAX];

    #[test]
    fn each_line_is_the_text_format_wrote() {
        for now in [0.0, 16.0, 1234.5, -0.0, 1e15, 1e16, 0.1 + 0.2, f64::NAN] {
            let n = crate::agent::num(now);
            assert_eq!(
                stamped(now, "boot: 3 nodes"),
                format!("t={n} boot: 3 nodes")
            );
        }
        for n in NUMBERS {
            let u = n as usize;
            for carried in [false, true] {
                let c = if carried { " (carried)" } else { "" };
                assert_eq!(
                    boot(carried, u, n),
                    format!("boot{c}: {u} nodes, epoch {n}")
                );
            }
            assert_eq!(
                committed("press view 7 (select)", n, u, 2, 30),
                format!("press view 7 (select) → epoch {n} (+{u} −2 ~30)")
            );
            for fired in [0, 1, 2, u] {
                let s = if fired == 1 { "" } else { "s" };
                assert_eq!(
                    advanced(fired, n),
                    format!("advance → {fired} timer{s} fired, epoch {n}")
                );
            }
            assert_eq!(forgot(n, "feed"), format!("forget request {n} (feed)"));
            assert_eq!(
                fulfilling(n, "feed", "HTTP 200, 5 bytes"),
                format!("fulfil {n} (feed) [HTTP 200, 5 bytes]")
            );
            assert_eq!(
                dropped(n, "storage, 0 bytes"),
                format!("reply {n} dropped: no such request in flight [storage, 0 bytes]")
            );
            assert_eq!(seeded(u, 4), format!("checkpoint: {u} of 4 answers taken"));
            assert_eq!(
                event("hover in", n as u32),
                format!("hover in view {}", n as u32)
            );
            let mut get = Request::get("https://api.example/x?y=1");
            get.method = "POST".into();
            assert_eq!(
                enqueued(n, "feed", &get),
                format!("request {n} (feed): POST https://api.example/x?y=1")
            );
            assert_eq!(
                enqueued(n, "s", &Request::storage(vec![1])),
                format!("storage {n} (s)")
            );
            assert_eq!(
                enqueued(n, "m", &Request::continuation(n / 3)),
                format!("continuation {n} (m): executor token {}", n / 3)
            );
        }
        assert_eq!(query("feed", "articles"), "query feed: articles");
        assert_eq!(store_write("jwt", true), "store jwt");
        assert_eq!(store_write("jwt", false), "forget jwt");
        assert_eq!(
            one_more("feed"),
            "feed: the reply asks for one more request"
        );
        let mut what = event("press", 12);
        ran(&mut what, "select");
        assert_eq!(what, format!("press view 12 ({})", "select"));
    }
}
