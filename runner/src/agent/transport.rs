//! `state.transport` (LLP 1125 D5): the transport record as the driver
//! reads it — what `exactTransport()` answers, the sequence that last
//! played, how far its hits are issued, the last 16 segments and how many
//! the record ever held, and the last 64 timed haptics with their states;
//! while a sequence follows a MIDI clock (LLP 1125 D9), what the clock has
//! said (`follow`). Only for a plan that declares a sequence.

use super::{num, quote};

// `state.midi` (LLP 1125 D5–D8), written after the transport's.
#[path = "midi.rs"]
mod midi;
use crate::{DataSource, Runner};
use std::fmt::Write as _;

/// Segments `state` shows.
const SEGMENTS: usize = 16;
/// Haptics `state` shows.
const HAPTICS: usize = 64;

/// `,"transport":{…}`, for a plan that declares a sequence.
/// Then `,"midi":{…}`, for a plan that uses MIDI.
pub(super) fn state<D: DataSource>(runner: &Runner<D>, request: &str, s: &mut String) {
    transport(runner, s);
    midi::state(runner, request, s);
}

fn transport<D: DataSource>(runner: &Runner<D>, s: &mut String) {
    let plan = runner.plan();
    if plan.sequences.is_empty() {
        return;
    }
    let t = runner.transport();
    let a = t.answer();
    let _ = write!(
        s,
        ",\"transport\":{{\"state\":\"{}\",\"reason\":",
        a.state.word()
    );
    quote(a.reason, s);
    s.push_str(",\"sequence\":");
    match t.sequence() {
        Some(i) => quote(plan.str(plan.sequences[i].name), s),
        None => s.push_str("null"),
    }
    let _ = write!(
        s,
        ",\"bpm\":{},\"anchorAt\":{},\"anchorBeat\":{},\"clock\":\"{}\",",
        num(a.bpm),
        num(a.anchor_at),
        num(a.anchor_beat),
        a.clock
    );
    // The clock followed (LLP 1125 D9), while a sequence follows one.
    if let Some(f) = t.follower() {
        s.push_str("\"follow\":{\"input\":");
        quote(&f.input, s);
        let opt = |v: Option<f64>| num(v.unwrap_or(f64::NAN));
        let _ = write!(
            s,
            ",\"ticks\":{},\"window\":{},\"estimate\":{},\"next\":{},\"held\":{}}},",
            f.ticks,
            f.window(),
            opt(f.estimate),
            num(f.next),
            opt(f.held)
        );
    }
    let _ = write!(s, "\"issuedTo\":{},\"segments\":[", num(t.issued_to()));
    let (segments, recorded) = t.segments();
    let skip = segments.len().saturating_sub(SEGMENTS);
    for (i, g) in segments.skip(skip).enumerate() {
        if i > 0 {
            s.push(',');
        }
        let _ = write!(
            s,
            "{{\"at\":{},\"beat\":{},\"bpm\":{}}}",
            num(g.at),
            num(g.beat),
            num(g.bpm)
        );
    }
    let _ = write!(s, "],\"recorded\":{recorded},\"haptics\":[");
    let now = runner.now_ms();
    let haptics = runner.sounds().haptics();
    let skip = haptics.len().saturating_sub(HAPTICS);
    for (i, h) in haptics.skip(skip).enumerate() {
        if i > 0 {
            s.push(',');
        }
        let _ = write!(s, "{{\"id\":{},\"kind\":", h.id);
        quote(&h.kind, s);
        let _ = write!(s, ",\"at\":{},\"state\":\"{}\"}}", num(h.at), h.state(now));
    }
    s.push_str("]}");
}
