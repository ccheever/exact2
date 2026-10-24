//! Stream timing, for LLP 1133.003's throughput gate (§9): four stamps per
//! message of a byte stream or a socket — the transport's receipt, the
//! mapper's entry, the decode call reaching the host, and the commit
//! standing — read by `perf streams` (LLP 1079). Nothing is stamped until
//! a development build's agent starts it (`perf streams start`); production
//! trust refuses that, as it refuses `perf`.
//!
//! The stamps are process-wide, keyed by stream ticket: the executor's
//! reader thread stamps a receipt when it queues a new message, the runner
//! stamps entry and commit around the message's settlement, and the
//! module's decode door stamps between them. A process with two runners
//! measures both into one table, whose tickets may collide; the gate runs
//! one. The web JS target keeps its own (`host/web-js/ts-stream.js`).
use std::collections::{BTreeMap, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Instant;

static ON: AtomicBool = AtomicBool::new(false);
static TABLE: Mutex<Option<Table>> = Mutex::new(None);

/// The most samples a stream keeps (ten minutes of 60 a second).
const SAMPLES: usize = 36_000;

#[derive(Default)]
struct Stream {
    /// Receipts of the messages queued and not yet entered, oldest first.
    queued: VecDeque<Instant>,
    messages: u64,
    commits: u64,
    refused: u64,
    decodes: u64,
    decoded_bytes: u64,
    /// Receipt to entry, to decode, to commit; entry to commit, in µs.
    to_entry: Vec<u32>,
    to_decode: Vec<u32>,
    to_commit: Vec<u32>,
    busy_us: u64,
}

struct Table {
    started: Instant,
    streams: BTreeMap<u64, Stream>,
    /// The message being settled: its ticket, receipt and entry.
    current: Option<(u64, Option<Instant>, Instant)>,
}

fn micros(from: Instant, to: Instant) -> u32 {
    to.saturating_duration_since(from)
        .as_micros()
        .min(u128::from(u32::MAX)) as u32
}

fn push(samples: &mut Vec<u32>, value: u32) {
    if samples.len() < SAMPLES {
        samples.push(value);
    }
}

fn with(f: impl FnOnce(&mut Table)) {
    if !ON.load(Ordering::Relaxed) {
        return;
    }
    if let Some(table) = TABLE.lock().unwrap().as_mut() {
        f(table);
    }
}

/// Measuring starts, or starts again, from now. False where there is no
/// clock to read (wasm: the web hosts keep their own).
pub fn start() -> bool {
    if cfg!(target_arch = "wasm32") {
        return false;
    }
    *TABLE.lock().unwrap() = Some(Table {
        started: Instant::now(),
        streams: BTreeMap::new(),
        current: None,
    });
    ON.store(true, Ordering::Relaxed);
    true
}

/// The executor queued a new message for stream `ticket`.
pub fn received(ticket: u64) {
    with(|t| {
        t.streams
            .entry(ticket)
            .or_default()
            .queued
            .push_back(Instant::now())
    });
}

/// The runner begins settling a message of stream `ticket`.
pub fn entered(ticket: u64) {
    with(|t| {
        let now = Instant::now();
        let s = t.streams.entry(ticket).or_default();
        s.messages += 1;
        let receipt = s.queued.pop_front();
        if let Some(receipt) = receipt {
            push(&mut s.to_entry, micros(receipt, now));
        }
        t.current = Some((ticket, receipt, now));
    });
}

/// The message being settled handed `bytes` to the host's decoder.
pub fn decoded(bytes: usize) {
    with(|t| {
        let Some((ticket, receipt, _)) = t.current else {
            return;
        };
        let s = t.streams.entry(ticket).or_default();
        s.decodes += 1;
        s.decoded_bytes += bytes as u64;
        if let Some(receipt) = receipt {
            push(&mut s.to_decode, micros(receipt, Instant::now()));
        }
    });
}

/// The settlement of the current message ended; `stood` when its commit did.
pub fn committed(stood: bool) {
    with(|t| {
        let Some((ticket, receipt, entry)) = t.current.take() else {
            return;
        };
        let now = Instant::now();
        let s = t.streams.entry(ticket).or_default();
        s.busy_us += u64::from(micros(entry, now));
        if !stood {
            s.refused += 1;
            return;
        }
        s.commits += 1;
        if let Some(receipt) = receipt {
            push(&mut s.to_commit, micros(receipt, now));
        }
    });
}

/// Percentiles of `samples` in milliseconds: p50, p95, p99 and the most.
fn percentiles(samples: &[u32], out: &mut String) {
    use std::fmt::Write as _;
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    let at = |p: f64| {
        sorted
            .get(((sorted.len() as f64 * p).ceil() as usize).saturating_sub(1))
            .map_or(0.0, |us| f64::from(*us) / 1000.0)
    };
    let _ = write!(
        out,
        "{{\"n\":{},\"p50\":{:.3},\"p95\":{:.3},\"p99\":{:.3},\"max\":{:.3}}}",
        sorted.len(),
        at(0.5),
        at(0.95),
        at(0.99),
        at(1.0)
    );
}

/// `"streamTiming":{…}`'s body: the seconds measured and, per ticket, the
/// counts, rates, busy share and the three latencies from receipt, with
/// `name` naming each ticket's resource where the runner still holds it.
pub fn report(name: &dyn Fn(u64) -> Option<String>) -> String {
    use std::fmt::Write as _;
    let table = TABLE.lock().unwrap();
    let Some(table) = table.as_ref() else {
        return "{\"measuring\":false}".into();
    };
    let seconds = table.started.elapsed().as_secs_f64().max(1e-9);
    let mut out = format!("{{\"measuring\":true,\"seconds\":{seconds:.3},\"streams\":[");
    for (i, (ticket, s)) in table.streams.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let _ = write!(out, "{{\"ticket\":{ticket}");
        if let Some(name) = name(*ticket) {
            let _ = write!(out, ",\"name\":{}", serde_json::Value::String(name));
        }
        let _ = write!(
            out,
            ",\"messages\":{},\"commits\":{},\"refused\":{},\"decodes\":{},\"decodedBytes\":{},\
             \"messageRate\":{:.2},\"commitRate\":{:.2},\"decodeRate\":{:.2},\"busy\":{:.4},\"toEntry\":",
            s.messages,
            s.commits,
            s.refused,
            s.decodes,
            s.decoded_bytes,
            s.messages as f64 / seconds,
            s.commits as f64 / seconds,
            s.decodes as f64 / seconds,
            s.busy_us as f64 / 1e6 / seconds,
        );
        percentiles(&s.to_entry, &mut out);
        out.push_str(",\"toDecode\":");
        percentiles(&s.to_decode, &mut out);
        out.push_str(",\"toCommit\":");
        percentiles(&s.to_commit, &mut out);
        out.push('}');
    }
    out.push_str("]}");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// No other test's stream has this ticket: the table is the process's.
    const TICKET: u64 = u64::MAX - 7;

    #[test]
    fn a_message_is_stamped_from_receipt_through_decode_to_commit() {
        start();
        received(TICKET);
        received(TICKET);
        entered(TICKET);
        decoded(100);
        decoded(50);
        committed(true);
        entered(TICKET);
        committed(false);
        let report = report(&|t| (t == TICKET).then(|| "screen".to_string()));
        let v: serde_json::Value = serde_json::from_str(&report).unwrap();
        let s = (v["streams"].as_array().unwrap().iter())
            .find(|s| s["ticket"].as_u64() == Some(TICKET))
            .unwrap();
        assert_eq!(s["name"], "screen");
        assert_eq!(
            (
                s["messages"].as_u64(),
                s["commits"].as_u64(),
                s["refused"].as_u64()
            ),
            (Some(2), Some(1), Some(1))
        );
        assert_eq!(
            (s["decodes"].as_u64(), s["decodedBytes"].as_u64()),
            (Some(2), Some(150))
        );
        assert_eq!(
            (
                s["toEntry"]["n"].as_u64(),
                s["toDecode"]["n"].as_u64(),
                s["toCommit"]["n"].as_u64()
            ),
            (Some(2), Some(2), Some(1))
        );
    }
}
