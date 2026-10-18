//! Host-owned measurement; gameplay receives only opaque span guards.
use crate::data::text::Float;
#[doc(hidden)]
pub use ring::Ring;
mod ring;
mod trace;
use std::{cell::RefCell, fmt::Write, panic::Location, rc::Rc};

struct Row {
    name: &'static str,
    at: &'static Location<'static>,
    value: f64,
    samples: Ring,
    born: u64,
}
struct Open {
    id: u64,
    row: usize,
    start: f64,
}
#[derive(Default)]
pub(crate) struct Spans {
    clock: Option<fn() -> f64>,
    rows: Vec<Row>,
    open: Vec<Open>,
    last: f64,
    next: u64,
    ticks: u64,
    active: bool,
    trace: trace::Trace,
    tick_start: f64,
    tick: u64,
    total: Ring,
}
impl Spans {
    fn charge(&mut self, now: f64) {
        let row = self.open.last().map_or(0, |s| s.row);
        self.rows[row].value += (now - self.last).max(0.);
        self.last = now;
    }
    pub(crate) fn arm(&mut self, clock: fn() -> f64, reset: bool) {
        if self.clock.is_some() && !reset {
            return;
        }
        let next = self.next;
        let mut total = Ring::default();
        total.arm(false);
        *self = Self {
            total,
            next,
            clock: Some(clock),
            rows: Vec::with_capacity(256),
            open: Vec::with_capacity(256),
            trace: trace::Trace::armed(),
            ..Self::default()
        };
        self.row("tick.other", Location::caller());
    }
    fn row(&mut self, name: &'static str, at: &'static Location<'static>) -> usize {
        if let Some(i) = self.rows.iter().position(|r| r.name == name) {
            return i;
        }
        let mut samples = Ring::default();
        samples.arm(false);
        self.rows.push(Row {
            name,
            at,
            value: 0.,
            samples,
            born: self.ticks,
        });
        self.rows.len() - 1
    }
    pub(crate) fn begin(&mut self, tick: u64) {
        if let Some(clock) = self.clock {
            for row in &mut self.rows {
                row.value = 0.;
            }
            self.open.clear();
            self.active = true;
            self.last = clock();
            self.tick_start = self.last;
            self.tick = tick;
        }
    }
    pub(crate) fn end(&mut self) {
        if let Some(clock) = self.clock {
            self.charge(clock());
            self.total.push((self.last - self.tick_start).max(0.));
            self.trace
                .record("tick", "", self.tick_start, self.last, self.tick);
            for row in &mut self.rows {
                row.samples.push(row.value);
            }
            self.open.clear();
            self.active = false;
            self.ticks += 1;
        }
    }
    pub(crate) fn frame(&mut self) -> u64 {
        if let Some(clock) = self.clock {
            self.trace.frame(clock());
        }
        self.trace.frames
    }
    pub(crate) fn trace_json(&self) -> String {
        self.trace.json()
    }
    pub(crate) fn json(&self, since: Option<u64>) -> String {
        let window = since.is_some();
        let since = since.unwrap_or(0);
        if since > self.ticks || window && self.ticks - since > 16384 {
            return "{\"error\":\"perf world: span window expired or counters went back\"}".into();
        }
        let first = if window {
            since
        } else {
            self.ticks.saturating_sub(16384)
        };
        let count = (self.ticks - first) as usize;
        let total = distribution(&self.total, count, 0);
        let mut out = format!(
            "{{\"armed\":{},\"spanTicks\":{},\"tickMs\":{},\"spans\":[",
            self.clock.is_some(),
            self.ticks,
            crate::json::to_string(&total).unwrap()
        );
        for (i, row) in self.rows.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            let present = (self.ticks - first.max(row.born)).min(row.samples.len as u64) as usize;
            let d = distribution(&row.samples, present, count - present);
            write!(out, "{{\"name\":{},\"at\":{},\"p50\":{},\"p95\":{},\"max\":{},\"perTick\":{},\"count\":{count}}}",
                crate::values::quote(row.name), crate::values::quote(&format!("{}:{}", row.at.file(), row.at.line())),
                Float(d.p50), Float(d.p95), Float(d.max), Float(d.mean)).unwrap();
        }
        out.push_str("]}");
        out
    }
}
#[derive(Default, crate::Data)]
struct Distribution {
    p50: f64,
    p95: f64,
    max: f64,
    mean: f64,
    count: u32,
}
fn distribution(ring: &Ring, count: usize, zeros: usize) -> Distribution {
    let mut values = vec![0.; zeros];
    values.extend((0..count).map(|i| ring.values[(ring.next + 16384 - count + i) % 16384]));
    let sum: f64 = values.iter().sum();
    values.sort_unstable_by(f64::total_cmp);
    let n = values.len();
    let p = |percent: usize| {
        if n == 0 {
            0.
        } else {
            values[(n * percent).div_ceil(100) - 1]
        }
    };
    Distribution {
        p50: p(50),
        p95: p(95),
        max: values.last().copied().unwrap_or(0.),
        mean: sum / n.max(1) as f64,
        count: n as u32,
    }
}
/// Opaque measurement lifetime. It exposes no clock or elapsed value to gameplay.
pub struct Span {
    owner: Option<Rc<RefCell<Spans>>>,
    id: u64,
}
impl Span {
    pub(crate) fn open(
        owner: &Rc<RefCell<Spans>>,
        name: &'static str,
        at: &'static Location<'static>,
    ) -> Self {
        let mut s = owner.borrow_mut();
        let Some(clock) = s.clock.filter(|_| s.active) else {
            return Self { owner: None, id: 0 };
        };
        s.charge(clock());
        let row = s.row(name, at);
        s.next += 1;
        let id = s.next;
        let start = s.last;
        s.open.push(Open { id, row, start });
        Self {
            owner: Some(owner.clone()),
            id,
        }
    }
}
impl Drop for Span {
    fn drop(&mut self) {
        let Some(owner) = &self.owner else {
            return;
        };
        let mut s = owner.borrow_mut();
        if let Some(i) = s.open.iter().position(|o| o.id == self.id) {
            let clock = s.clock.unwrap();
            s.charge(clock());
            let open = s.open.remove(i);
            let row = &s.rows[open.row];
            let (name, at, end, tick) = (row.name, row.at, s.last, s.tick);
            s.trace.record(name, at.file(), open.start, end, tick);
        }
    }
}
#[cfg(all(test, not(target_arch = "wasm32")))]
pub(crate) fn clock() -> f64 {
    thread_local! { static ORIGIN: std::time::Instant = std::time::Instant::now(); }
    ORIGIN.with(|t| t.elapsed().as_secs_f64() * 1000.)
}

#[cfg(test)]
mod tests;
