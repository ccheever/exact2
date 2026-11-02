//! Crypto list data: the 5,000 coins of `data/coins.json` (the copy of
//! `bench/crypto-list/data/coins.json` that `../prepare.sh` writes; bundled into
//! the binary), the launch config, and the live ticks (SPEC.md "Live ticks" and
//! "Decisions").
//!
//! - `market()` answers every coin (baked into the plan at build);
//! - `config()` answers `{ live, freeze }` from the launch environment
//!   (`BENCH_LIVE`, `BENCH_SCENARIO=rest`, `BENCH_FREEZE`);
//! - `tick()` advances tick `k`: 40 coins get `price *= 1 + δ`, their series
//!   shifts, `change24h += δ·100`, and their price flashes; last tick's flashes
//!   are switched off (the view fades them). Answers the market.
//!
//! This crate knows nothing about the chart: a row carries the series, and the
//! Contract decides how to draw it (a GPU canvas here; inline SVG in ../exact-crypto-svg, whose rows hold the series as numbers instead).
//!
//! Record sharing (as in the heavy bench): every answer is a fresh list whose
//! items are the previous answer's `Rc` records for every coin that did not
//! change, so the runner's identity checks hold for them.
#![forbid(unsafe_code)]


use exact_plan::Value;
use exact_runner::{DataError, DataSource};

/// The benchmark's data, as the build found it.
pub const JSON: &[u8] = include_bytes!("../coins.json");

#[derive(serde::Deserialize)]
struct Doc {
    coins: Vec<Raw>,
}

#[derive(serde::Deserialize)]
struct Raw {
    id: String,
    name: String,
    ticker: String,
    color: String,
    price: f64,
    #[serde(rename = "change24h")]
    change: f64,
    series: Vec<f64>,
}

/// `$61,234.56` at or above $1; four significant digits below (`$0.0009697`).
pub fn format_price(p: f64) -> String {
    if p >= 1.0 {
        let fixed = format!("{p:.2}");
        let (int, frac) = fixed.split_once('.').unwrap_or((&fixed, "00"));
        let mut grouped = String::new();
        for (i, ch) in int.chars().enumerate() {
            if i > 0 && (int.len() - i) % 3 == 0 {
                grouped.push(',');
            }
            grouped.push(ch);
        }
        format!("${grouped}.{frac}")
    } else {
        let decimals = (3 - p.log10().floor() as i32).max(0) as usize;
        format!("${p:.decimals$}")
    }
}

/// `+3.42%` / `-3.42%`.
pub fn format_change(c: f64) -> String {
    format!("{}{:.2}%", if c >= 0.0 { "+" } else { "-" }, c.abs())
}

/// One coin's live state.
struct Coin {
    raw: Raw,
    /// Flash sequence (the flash node's key) and direction.
    flash_seq: u32,
    flash_up: bool,
    flash_on: bool,
}

impl Coin {
    /// One row. Field order is `shape Coin` in app.contract.
    fn value(&self) -> Value {
        let r = &self.raw;
        let flash = if self.flash_seq == 0 {
            Value::list(vec![])
        } else {
            // Field order is `shape Flash`.
            Value::list(vec![Value::record(vec![
                Value::str(&self.flash_seq.to_string()),
                Value::Bool(self.flash_up),
                Value::Bool(self.flash_on),
            ])])
        };
        Value::record(vec![
            Value::str(&r.id),
            Value::str(&r.name),
            Value::str(&r.ticker),
            Value::str(&r.ticker[..1]),
            Value::str(&r.color),
            Value::str(&format_price(r.price)),
            Value::str(&format_change(r.change)),
            Value::Bool(r.change >= 0.0),
            Value::list(r.series.iter().map(|v| Value::Number(*v)).collect()),
            flash,
        ])
    }
}

struct Market {
    coins: Vec<Coin>,
    rows: Vec<Value>,
    /// Coins whose flash is on (switched off by the next tick).
    flashing: Vec<usize>,
    ticks: usize,
}

impl Market {
    fn load(doc: Doc) -> Market {
        let coins: Vec<Coin> =
            doc.coins.into_iter().map(|raw| Coin { raw, flash_seq: 0, flash_up: true, flash_on: false }).collect();
        let rows = coins.iter().map(Coin::value).collect();
        Market { coins, rows, flashing: Vec::new(), ticks: 0 }
    }

    fn value(&self) -> Value {
        Value::record(vec![Value::list(self.rows.clone())])
    }

    /// Tick `k` (SPEC "Live ticks").
    fn tick(&mut self) {
        let k = self.ticks + 1;
        self.ticks = k;
        let n = self.coins.len();
        let mut changed: Vec<usize> = std::mem::take(&mut self.flashing);
        for &i in &changed {
            self.coins[i].flash_on = false;
        }
        for j in 0..40 {
            let i = (k * 7919 + j * 104_729) % n;
            let delta = (((k * 31 + j * 17) % 201) as f64 - 100.0) / 10_000.0;
            let c = &mut self.coins[i];
            c.raw.price *= 1.0 + delta;
            c.raw.series.remove(0);
            c.raw.series.push(c.raw.price);
            c.raw.change += delta * 100.0;
            c.flash_seq += 1;
            c.flash_up = delta >= 0.0;
            c.flash_on = true;
            self.flashing.push(i);
            changed.push(i);
        }
        changed.sort_unstable();
        changed.dedup();
        for i in changed {
            self.rows[i] = self.coins[i].value();
        }
    }
}

/// The source. The list first shows the plan's baked value; the source's own
/// copy (what a tick edits) is parsed on a background thread started at launch.
pub struct Crypto {
    parsed: Option<std::thread::JoinHandle<Doc>>,
    market: Option<Market>,
}

impl Default for Crypto {
    fn default() -> Self {
        let parsed = std::thread::spawn(|| serde_json::from_slice::<Doc>(JSON).expect("coins.json"));
        Crypto { parsed: Some(parsed), market: None }
    }
}

impl Crypto {
    fn market(&mut self) -> &mut Market {
        if self.market.is_none() {
            let doc = self.parsed.take().expect("parsed once").join().expect("parse thread");
            self.market = Some(Market::load(doc));
        }
        self.market.as_mut().expect("loaded")
    }
}

fn env_is(name: &str, value: &str) -> bool {
    std::env::var(name).is_ok_and(|v| v == value)
}

impl DataSource for Crypto {
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        match (source, args) {
            ("market", []) => Ok(self.market().value()),
            ("config", []) => {
                let freeze = env_is("BENCH_FREEZE", "1");
                let live = !freeze && (env_is("BENCH_LIVE", "1") || env_is("BENCH_SCENARIO", "rest"));
                if live {
                    self.market();
                }
                Ok(Value::record(vec![Value::Bool(live), Value::Bool(freeze)]))
            }
            ("tick", []) => {
                let m = self.market();
                m.tick();
                Ok(m.value())
            }
            ("market" | "config" | "tick", _) => Err(DataError::BadArguments(format!("{source}()"))),
            _ => Err(DataError::UnknownSource(source.into())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats() {
        assert_eq!(format_price(61234.5), "$61,234.50");
        assert_eq!(format_price(1443.3095), "$1,443.31");
        assert_eq!(format_price(450.82944), "$450.83");
        assert_eq!(format_price(0.00096967856), "$0.0009697");
        assert_eq!(format_price(0.12), "$0.1200");
        assert_eq!(format_price(0.00005), "$0.00005000");
        assert_eq!(format_change(3.4249), "+3.42%");
        assert_eq!(format_change(-0.5229), "-0.52%");
    }

    #[test]
    fn ticks_share_records() {
        let mut s = Crypto::default();
        let a = s.query("market", &[]).unwrap();
        let b = s.query("tick", &[]).unwrap();
        let rows = |v: &Value| match v {
            Value::Record(f) => match &f[0] {
                Value::List(l) => l.clone(),
                _ => unreachable!(),
            },
            _ => unreachable!(),
        };
        let (a, b) = (rows(&a), rows(&b));
        assert_eq!(a.len(), 5000);
        let changed = a.iter().zip(b.iter()).filter(|(x, y)| !exact_runner::compare::same(x, y)).count();
        assert_eq!(changed, 40);
        let c = s.query("tick", &[]).unwrap();
        let changed = b.iter().zip(rows(&c).iter()).filter(|(x, y)| !exact_runner::compare::same(x, y)).count();
        assert_eq!(changed, 80, "40 updated + 40 flashes switched off");
    }
}
