//! `state.midi` (LLP 1125 D5): the ports `exactMIDI()` answers, the inputs
//! the mounted listeners name, how many messages wait, the last 64 messages
//! queued (or the whole record when the request asks `"midi":"all"`, what
//! `expect midi` reads) with their states, the last 64 messages the inputs
//! delivered, and clock out's ticks. Only for a plan that keeps a MIDI queue
//! (`crate::uses::keeps_midi`); the JS target's midi.js writes the same.

use crate::agent::{field_str, num, quote};
use crate::midi::{hex, Port};
use crate::{DataSource, Runner};
use std::fmt::Write as _;

/// Messages `state` shows unless asked for all.
const SHOWN: usize = 64;

fn ports(list: &[Port], s: &mut String) {
    s.push('[');
    for (i, p) in list.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str("{\"id\":");
        quote(&p.id, s);
        s.push_str(",\"name\":");
        quote(&p.name, s);
        s.push_str(",\"manufacturer\":");
        quote(&p.manufacturer, s);
        let _ = write!(s, ",\"state\":\"{}\"}}", p.state());
    }
    s.push(']');
}

/// `,"midi":{…}`.
pub(super) fn state<D: DataSource>(runner: &Runner<D>, request: &str, s: &mut String) {
    if !crate::uses::keeps_midi(runner.plan()) {
        return;
    }
    let m = runner.midi();
    let now = runner.now_ms();
    let p = m.ports();
    let _ = write!(
        s,
        ",\"midi\":{{\"state\":\"{}\",\"inputs\":",
        p.access.word()
    );
    ports(&p.inputs, s);
    s.push_str(",\"outputs\":");
    ports(&p.outputs, s);
    s.push_str(",\"open\":[");
    for (i, input) in runner.midi_inputs().iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        quote(input, s);
    }
    let _ = write!(s, "],\"queued\":{},\"sent\":[", m.waiting(now));
    let all = field_str(request, "midi").as_deref() == Some("all");
    let count = m.sent().count();
    let skip = if all { 0 } else { count.saturating_sub(SHOWN) };
    for (i, e) in m.sent().skip(skip).enumerate() {
        if i > 0 {
            s.push(',');
        }
        let _ = write!(s, "{{\"id\":{},\"output\":", e.id);
        quote(&e.output, s);
        let _ = write!(
            s,
            ",\"data\":\"{}\",\"at\":{},\"state\":\"{}\",\"by\":",
            hex(&e.data),
            num(e.at),
            e.state(now)
        );
        match e.by {
            Some(by) => quote(by, s),
            None => s.push_str("null"),
        }
        let _ = write!(s, ",\"seq\":{}}}", e.seq);
    }
    let _ = write!(s, "],\"recorded\":{},\"received\":[", m.recorded());
    let (received, heard) = m.received();
    let received: Vec<_> = received.collect();
    let skip = received.len().saturating_sub(SHOWN);
    for (i, r) in received.iter().skip(skip).enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str("{\"input\":");
        quote(&r.input, s);
        let _ = write!(
            s,
            ",\"data\":\"{}\",\"timeStamp\":{},\"beat\":{},\"at\":{}}}",
            hex(&r.data),
            num(r.time_stamp),
            num(r.beat),
            num(r.at)
        );
    }
    let (output, ticks) = m.clock();
    let _ = write!(s, "],\"heard\":{heard},\"clock\":{{\"output\":");
    match output {
        Some(o) => quote(o, s),
        None => s.push_str("null"),
    }
    let _ = write!(s, ",\"ticks\":{ticks}}}}}");
}
