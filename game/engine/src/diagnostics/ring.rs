use crate::data::text::Float;
use std::fmt::Write;
#[derive(Default)]
#[doc(hidden)]
pub struct Ring {
    #[doc(hidden)]
    pub values: Vec<f64>,
    #[doc(hidden)]
    pub len: usize,
    #[doc(hidden)]
    pub next: usize,
    #[doc(hidden)]
    pub count: u64,
    #[doc(hidden)]
    pub sum: f64,
    #[doc(hidden)]
    pub max: f64,
}
impl Ring {
    #[doc(hidden)]
    pub fn arm(&mut self, reset: bool) {
        if reset {
            *self = Self::default();
        }
        self.values.resize(16384, 0.);
    }
    #[doc(hidden)]
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
    #[doc(hidden)]
    pub fn json(&self, out: &mut String) {
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
