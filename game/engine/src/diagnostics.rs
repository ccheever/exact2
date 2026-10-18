//! Host-owned measurement; gameplay receives only opaque span guards.
use crate::data::text::Float;
#[doc(hidden)]
pub use ring::Ring;
mod ring;
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
        *self = Self {
            clock: Some(clock),
            rows: Vec::with_capacity(256),
            open: Vec::with_capacity(256),
            ..Self::default()
        };
        self.row("tick.other", Location::caller());
    }
    fn row(&mut self, name: &'static str, at: &'static Location<'static>) -> usize {
        if let Some(i) = self.rows.iter().position(|r| r.name == name) {
            return i;
        }
        if self.rows.len() == 256 {
            return 0;
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
    pub(crate) fn begin(&mut self) {
        if let Some(clock) = self.clock {
            for row in &mut self.rows {
                row.value = 0.;
            }
            self.open.clear();
            self.active = true;
            self.last = clock();
        }
    }
    pub(crate) fn end(&mut self) {
        if let Some(clock) = self.clock {
            self.charge(clock());
            for row in &mut self.rows {
                row.samples.push(row.value);
            }
            self.open.clear();
            self.active = false;
            self.ticks += 1;
        }
    }
    pub(crate) fn json(&self, since: Option<u64>) -> String {
        let since = since.unwrap_or(0);
        if since > self.ticks || self.ticks - since > 16384 && since != 0 {
            return "{\"error\":\"perf world: span window expired or counters went back\"}".into();
        }
        let mut out = format!(
            "{{\"armed\":{},\"spanTicks\":{},\"spans\":[",
            self.clock.is_some(),
            self.ticks
        );
        for (i, row) in self.rows.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            let count = (self.ticks - since.max(row.born)).min(row.samples.len as u64) as usize;
            let mut values: Vec<_> = (0..count)
                .map(|i| row.samples.values[(row.samples.next + 16384 - count + i) % 16384])
                .collect();
            let sum: f64 = values.iter().sum();
            values.sort_unstable_by(f64::total_cmp);
            let p = |n: usize| {
                if count == 0 {
                    0.
                } else {
                    values[(count * n).div_ceil(100) - 1]
                }
            };
            write!(out, "{{\"name\":{},\"at\":{},\"p50\":{},\"p95\":{},\"max\":{},\"perTick\":{},\"count\":{count}}}",
                crate::values::quote(row.name), crate::values::quote(&format!("{}:{}", row.at.file(), row.at.line())),
                Float(p(50)), Float(p(95)), Float(values.last().copied().unwrap_or(0.)), Float(sum/count.max(1) as f64)).unwrap();
        }
        out.push_str("]}");
        out
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
        let Some(clock) = s.clock.filter(|_| s.active && s.open.len() < 256) else {
            return Self { owner: None, id: 0 };
        };
        s.charge(clock());
        let row = s.row(name, at);
        s.next += 1;
        let id = s.next;
        s.open.push(Open { id, row });
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
            s.open.remove(i);
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
