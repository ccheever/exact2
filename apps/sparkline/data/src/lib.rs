//! Sparkline's data (LLP 1055): forty coins, each with a 48-point random walk
//! turned into the chart's `points` in a 96 × 32 box (the minimum at the
//! bottom, the maximum at the top) and the last point, where the pulse sits.
//! `coins(version)` moves every third coin's series on by `version` steps, so
//! a tick redraws some charts in place.

#![deny(missing_docs)]

use exact_plan::Value;
use exact_runner::{DataError, DataSource};

/// The app's data source.
#[derive(Default, Clone, Copy)]
pub struct Spark;

const NAMES: [&str; 8] = [
    "Voltaris", "Nimbex", "Quorra", "Helion", "Brisk", "Tessel", "Orvane", "Kalypt",
];
const COLORS: [&str; 6] = [
    "#f59e0b", "#6366f1", "#10b981", "#ef4444", "#0ea5e9", "#a855f7",
];

/// A deterministic generator (a 64-bit LCG), so every host draws the same rows.
fn next(state: &mut u64) -> f64 {
    *state = state
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    ((*state >> 11) as f64) / ((1u64 << 53) as f64)
}

/// One coin's series: 48 + `shift` steps of a walk, keeping the last 48.
fn series(index: u64, shift: u64) -> Vec<f64> {
    let mut state = index.wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ 49975;
    let mut price = 100.0 + 50.0 * next(&mut state);
    let mut out = Vec::with_capacity(48 + shift as usize);
    for _ in 0..48 + shift {
        price *= 1.0 + (next(&mut state) - 0.5) * 0.06;
        out.push(price);
    }
    out.split_off(shift as usize)
}

/// A number as `points` text: at most two decimals, no trailing zeros.
fn short(v: f64) -> String {
    let s = format!("{:.2}", (v * 100.0).round() / 100.0);
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn coin(index: u64, version: u64) -> Value {
    let shift = if index.is_multiple_of(3) { version } else { 0 };
    let s = series(index, shift);
    let (lo, hi) = s
        .iter()
        .fold((f64::MAX, f64::MIN), |(a, b), v| (a.min(*v), b.max(*v)));
    let span = (hi - lo).max(1e-9);
    let at = |i: usize| (i as f64 * 96.0 / 47.0, 32.0 - (s[i] - lo) / span * 32.0);
    let points: Vec<String> = (0..s.len())
        .map(|i| {
            let (x, y) = at(i);
            format!("{},{}", short(x), short(y))
        })
        .collect();
    let (lx, ly) = at(s.len() - 1);
    let change = (s[47] / s[0] - 1.0) * 100.0;
    let name = format!("{}{}", NAMES[(index % 8) as usize], index / 8);
    // Fields in the order `shape Coin` declares them.
    Value::record(vec![
        Value::str(&format!("c{index}")),
        Value::str(&name),
        Value::str(&name[..3].to_uppercase()),
        Value::str(&name[..1]),
        Value::str(COLORS[(index % 6) as usize]),
        Value::str(&format!("${:.2}", s[47])),
        Value::str(&format!(
            "{}{:.2}%",
            if change >= 0.0 { "+" } else { "" },
            change
        )),
        Value::Bool(change >= 0.0),
        Value::str(&points.join(" ")),
        Value::Number(lx),
        Value::Number(ly),
    ])
}

impl DataSource for Spark {
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        match source {
            "coins" => {
                let version = match args.first() {
                    Some(Value::Number(n)) => *n as u64,
                    _ => 0,
                };
                Ok(Value::list((0..40).map(|i| coin(i, version)).collect()))
            }
            _ => Err(DataError::UnknownSource(source.into())),
        }
    }

    fn app_id(&self) -> &str {
        "com.exact.sparkline"
    }
}
