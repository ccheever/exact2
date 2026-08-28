//! The roster's implementations — once, here, deterministic.
//!
//! @ref LLP 1004 D4 (formatting is a roster entry, added by fixture)
//!
//! Every entry the plan format's `stdlib` table names has exactly one body
//! in this file. Time formatting is UTC and locale-free by design: the v1 app
//! is a schedule board, and a deterministic string is what the corpus and the
//! agent compare.

use exact_plan::{Stdlib, Value};
use std::rc::Rc;

/// Call `f` with `args` (already arity-checked). `None` on a type mismatch.
pub fn call(f: Stdlib, args: &[Value], now_ms: f64) -> Option<Value> {
    let num = |i: usize| args.get(i).and_then(Value::as_number);
    Some(match f {
        Stdlib::Now => Value::Number(now_ms),
        Stdlib::FormatClockTime => Value::str(&format_clock_time(num(0)?)),
        Stdlib::FormatCountdownMinutes => {
            let minutes = ((num(0)? - num(1)?) / 60_000.0).ceil().max(0.0);
            Value::str(&format!("{}", minutes as i64))
        }
        Stdlib::FormatDistance => {
            let miles = num(0)? / 1609.344;
            Value::str(&if miles < 0.1 {
                "nearby".to_string()
            } else {
                format!("{:.1} mi", (miles * 10.0).round() / 10.0)
            })
        }
        Stdlib::FormatWalk => {
            let minutes = (num(0)? / 80.0).ceil().max(1.0);
            Value::str(&format!("{} min walk", minutes as i64))
        }
        Stdlib::Length => Value::Number(match args.first()? {
            Value::List(items) => items.len() as f64,
            Value::Str(s) => s.chars().count() as f64,
            _ => return None,
        }),
        Stdlib::IsEmpty => Value::Bool(match args.first()? {
            Value::List(items) => items.is_empty(),
            Value::Str(s) => s.is_empty(),
            _ => return None,
        }),
        Stdlib::ToString => match args.first()? {
            Value::Number(n) => Value::str(&format_number(*n)),
            Value::Bool(b) => Value::str(if *b { "true" } else { "false" }),
            Value::Str(s) => Value::Str(Rc::clone(s)),
            _ => return None,
        },
        Stdlib::Floor => Value::Number(num(0)?.floor()),
        Stdlib::Max => Value::Number(num(0)?.max(num(1)?)),
        Stdlib::Min => Value::Number(num(0)?.min(num(1)?)),
    })
}

/// A number the way JavaScript prints it for integers, else with the
/// shortest round-trip representation.
pub fn format_number(n: f64) -> String {
    if n.is_finite() && n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

/// `h:mm AM` from milliseconds since the Unix epoch, UTC.
pub fn format_clock_time(ms: f64) -> String {
    let seconds = (ms / 1000.0).floor() as i64;
    let day_seconds = seconds.rem_euclid(86_400);
    let hours = day_seconds / 3600;
    let minutes = (day_seconds % 3600) / 60;
    let (h12, suffix) = match hours {
        0 => (12, "AM"),
        1..=11 => (hours, "AM"),
        12 => (12, "PM"),
        _ => (hours - 12, "PM"),
    };
    format!("{h12}:{minutes:02} {suffix}")
}
