//! Opt-in live-window observations. @ref LLP 1149.001 Stage 0.
use exact_ink::counters;
use serde_json::{json, Value};
use std::{
    cell::RefCell,
    time::{Duration, Instant},
};

#[derive(Default)]
struct Recording {
    events: Vec<Value>,
    samples: Vec<Value>,
    sampled: Option<Instant>,
    dropped: usize,
}
thread_local! { static RECORDING: RefCell<Recording> = RefCell::default(); }

pub(crate) struct Span(Option<(Instant, &'static str, u8)>);
impl Span {
    pub(crate) fn new(name: &'static str, cause: u8) -> Self {
        Self(counters::enabled().then(|| (Instant::now(), name, cause)))
    }
}
impl Drop for Span {
    fn drop(&mut self) {
        let Some((start, name, cause)) = self.0 else {
            return;
        };
        let duration = start.elapsed().as_secs_f64() * 1e6;
        let start = counters::timestamp(start);
        RECORDING.with_borrow_mut(|r| {
            if r.events.len() < 131_072 {
                r.events
                    .push(json!({"name":name,"ph":"X","ts":start,"dur":duration,
                    "pid":std::process::id(),"tid":1,"args":{"value":cause as u16 + 1}}));
            } else {
                r.dropped += 1;
            }
        });
    }
}

/// Read gauges on existing service turns; no timer or wake is introduced.
pub(crate) fn sample() {
    if !counters::enabled() {
        return;
    }
    RECORDING.with_borrow_mut(|r| {
        let now = Instant::now();
        if r.sampled
            .is_none_or(|last| now.duration_since(last) >= Duration::from_millis(100))
        {
            if r.samples.len() < 10_000 {
                r.samples.push(exact_linux::ink::counter_state());
                r.sampled = Some(now);
            } else {
                r.dropped += 1;
            }
        }
    });
}

pub(crate) fn save() {
    let Ok(path) = std::env::var("EXACT_INK_COUNTER_OUTPUT") else {
        return;
    };
    sample();
    let data = RECORDING.with_borrow(|r| {
        json!({"mainThread":1,"clock":"monotonic microseconds anchored once to Unix time",
        "traceEvents":r.events,"samples":r.samples,"dropped":r.dropped,
        "final":exact_linux::ink::counter_state()})
    });
    let result = serde_json::to_vec(&data)
        .map_err(|e| e.to_string())
        .and_then(|bytes| std::fs::write(path, bytes).map_err(|e| e.to_string()));
    if let Err(error) = result {
        eprintln!("exact-windows: counter export: {error}");
    }
}
