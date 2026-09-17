use crate::Stats;
use std::fmt::Write;

pub(crate) struct Stamp {
    #[cfg(not(target_arch = "wasm32"))]
    time: std::time::Instant,
    #[cfg(target_arch = "wasm32")]
    time: f64,
}
impl Stamp {
    pub fn now() -> Self {
        Self {
            #[cfg(not(target_arch = "wasm32"))]
            time: std::time::Instant::now(),
            #[cfg(target_arch = "wasm32")]
            time: performance_now(),
        }
    }
    pub fn elapsed(&self) -> f64 {
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
    web_sys::window()
        .and_then(|w| w.performance())
        .expect("WorldSurface requires window.performance")
        .now()
}
pub(crate) struct Ring {
    values: [f64; 240],
    len: usize,
    next: usize,
}
impl Default for Ring {
    fn default() -> Self {
        Self {
            values: [0.0; 240],
            len: 0,
            next: 0,
        }
    }
}
impl Ring {
    pub fn push(&mut self, value: f64) {
        if !value.is_finite() || value < 0.0 {
            return;
        }
        self.values[self.next] = value;
        self.next = (self.next + 1) % 240;
        self.len = (self.len + 1).min(240);
    }
    fn json(&self, out: &mut String) {
        let mut values = self.values;
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
            "{{\"p50\":{},\"p95\":{},\"p99\":{},\"max\":{}}}",
            p(50),
            p(95),
            p(99),
            p(100)
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
}
impl Perf {
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
        assert_eq!(json, r#"{"p50":179,"p95":287,"p99":297,"max":299}"#);
    }
}
