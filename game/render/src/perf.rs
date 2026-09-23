use crate::Stats;
use exact_game::data::text::Float;
use std::fmt::Write;
#[cfg(test)]
thread_local! { pub(crate) static CLOCK_READS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }

pub(crate) struct Stamp {
    #[cfg(not(target_arch = "wasm32"))]
    time: std::time::Instant,
    #[cfg(target_arch = "wasm32")]
    time: f64,
}
impl Stamp {
    pub fn now() -> Self {
        #[cfg(test)]
        CLOCK_READS.with(|n| n.set(n.get() + 1));
        Self {
            #[cfg(not(target_arch = "wasm32"))]
            time: std::time::Instant::now(),
            #[cfg(target_arch = "wasm32")]
            time: performance_now(),
        }
    }
    pub fn wall_ms(&self) -> f64 {
        #[cfg(target_arch = "wasm32")]
        {
            performance_now()
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.time.elapsed().as_secs_f64() * 1000.0
        }
    }
    pub fn elapsed(&self) -> f64 {
        #[cfg(test)]
        CLOCK_READS.with(|n| n.set(n.get() + 1));
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.time.elapsed().as_secs_f64() * 1000.0
        }
        #[cfg(target_arch = "wasm32")]
        {
            performance_now() - self.time
        }
    }
}
#[cfg(target_arch = "wasm32")]
fn performance_now() -> f64 {
    thread_local! {
        static PERFORMANCE: web_sys::Performance = web_sys::window()
            .and_then(|w| w.performance()).expect("WorldSurface requires window.performance");
    }
    PERFORMANCE.with(web_sys::Performance::now)
}
#[derive(Default)]
pub(crate) struct Ring {
    values: Vec<f64>,
    len: usize,
    next: usize,
    count: u64,
    sum: f64,
    max: f64,
}
impl Ring {
    pub(crate) fn arm(&mut self, reset: bool) {
        if reset {
            *self = Self::default();
        }
        self.values.resize(16384, 0.);
    }
    pub fn push(&mut self, value: f64) {
        if !value.is_finite() || value < 0.0 {
            return;
        }
        self.count += 1;
        self.sum += value;
        self.max = self.max.max(value);
        if self.values.is_empty() {
            return;
        }
        self.values[self.next] = value;
        self.next = (self.next + 1) % self.values.len();
        self.len = (self.len + 1).min(self.values.len());
    }
    pub(crate) fn json(&self, out: &mut String) {
        let mut values = self.values[..self.len].to_vec();
        values[..self.len].sort_unstable_by(f64::total_cmp);
        let p = |percent: usize| {
            if self.len == 0 {
                0.0
            } else {
                values[(self.len * percent).div_ceil(100) - 1]
            }
        };
        write!(
            out,
            "{{\"p50\":{},\"p95\":{},\"p99\":{},\"max\":{},\"count\":{},\"mean\":{}}}",
            Float(p(50)),
            Float(p(95)),
            Float(p(99)),
            Float(self.max),
            self.count,
            Float(self.sum / self.count.max(1) as f64)
        )
        .unwrap();
    }
}
#[derive(Default)]
pub(crate) struct Perf {
    frame: Ring,
    pub tick: Ring,
    pub feed: Ring,
    pub encode: Ring,
    pub ticks: Ring,
    last_live: Option<f64>,
    pub stats: Stats,
    pub pixels: (u32, u32),
}
impl Perf {
    pub fn armed(&self) -> bool {
        !self.frame.values.is_empty()
    }
    pub fn arm(&mut self) {
        for r in [
            &mut self.frame,
            &mut self.tick,
            &mut self.feed,
            &mut self.encode,
            &mut self.ticks,
        ] {
            r.values.resize(16384, 0.0);
        }
    }
    pub fn reset(&mut self) {
        for r in [
            &mut self.frame,
            &mut self.tick,
            &mut self.feed,
            &mut self.encode,
            &mut self.ticks,
        ] {
            r.values.resize(16384, 0.0);
            r.count = 0;
            r.sum = 0.0;
            r.max = 0.0;
            r.len = 0;
            r.next = 0;
        }
        self.last_live = None;
    }
    pub fn frame(&mut self, now: f64, seekable: bool) {
        if seekable {
            self.last_live = None;
            return;
        }
        if let Some(last) = self.last_live {
            self.frame.push(now - last);
        }
        self.last_live = Some(now);
    }
    pub fn append(&self, out: &mut String) {
        out.push_str(",\"perf\":{\"wallClock\":true");
        write!(out, ",\"armed\":{}", self.armed()).unwrap();
        write!(out, ",\"pixels\":[{},{}]", self.pixels.0, self.pixels.1).unwrap();
        for (name, ring) in [
            ("frameMs", &self.frame),
            ("tickMs", &self.tick),
            ("feedMs", &self.feed),
            ("encodeMs", &self.encode),
            ("ticksPerFrame", &self.ticks),
        ] {
            write!(out, ",\"{name}\":").unwrap();
            ring.json(out);
        }
        write!(
            out,
            ",\"draws\":{},\"instances\":{},\"triangles\":{}}}",
            self.stats.draws, self.stats.instances, self.stats.triangles
        )
        .unwrap();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cadence_excludes_seeks_and_keeps_dropped_intervals() {
        let mut p = Perf::default();
        assert!(!p.armed());
        p.tick.push(3.0);
        assert_eq!(p.tick.count, 1);
        assert_eq!(p.tick.values.capacity(), 0);
        p.reset();
        assert!(p.armed());
        for (now, seek) in [
            (0., false),
            (8., false),
            (24., false),
            (1000., true),
            (2000., false),
            (2008., false),
        ] {
            p.frame(now, seek);
        }
        assert_eq!(&p.frame.values[..p.frame.len], &[8., 16., 8.]);
        for i in 0..300 {
            p.tick.push(i as f64);
        }
        let mut json = String::new();
        p.tick.json(&mut json);
        assert_eq!(
            json,
            r#"{"p50":149,"p95":284,"p99":296,"max":299,"count":300,"mean":149.5}"#
        );
    }
}
