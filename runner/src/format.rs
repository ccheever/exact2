//! The linked capability `format`: `formatDate` and `formatNumber`.
//!
//! @ref LLP 1054.000.003 D2 (dates), D3 (compact numbers), D7 (bounded
//! input), D8 (linked by use, the router's way)
//!
//! Nothing outside [`crate::RunnerLinks`] names this module: `ALL` fills
//! `format` with [`formatting`], and the web registers it from
//! `exact-web-capabilities` only for a plan that calls one of these, so an
//! app that doesn't carries none of this code. `formatTime` is the core's
//! (`stdlib.rs`), as `formatClockTime` was. The strings are `Intl`'s in
//! `en-US`; the oracle table is `runner/tests/it/format.rs`.

use crate::stdlib::{push_number, wall_ms};
use exact_plan::{Stdlib, Value};

/// A `format` entry's value; `None` when `f` is not one, or its arguments
/// don't fit it (a style the compiler would have refused).
pub fn formatting(f: Stdlib, args: &[Value]) -> Option<Value> {
    let num = |i: usize| args.get(i).and_then(Value::as_number);
    let style = |i: usize| args.get(i).and_then(Value::as_str);
    Some(match f {
        Stdlib::FormatDate => {
            let month_year = match style(2)? {
                "medium" => false,
                "month-year" => true,
                _ => return None,
            };
            match wall_ms(num(0)?, num(1)?) {
                Some(wall) => date(wall, month_year),
                None => Value::str(""),
            }
        }
        Stdlib::FormatNumber => match style(1)? {
            "compact" => compact(num(0)?),
            _ => return None,
        },
        _ => return None,
    })
}

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// `Sep 26, 2026` (`{ dateStyle: "medium" }`) or `September 2026`
/// (`{ month: "long", year: "numeric" }`) of a wall time in range. `en-US`'s
/// short months are the first three letters of the long ones.
fn date(wall: f64, month_year: bool) -> Value {
    let (year, month, day) = civil((wall / 86_400_000.0).floor() as i64);
    let name = MONTHS[month as usize - 1];
    let mut out = String::with_capacity(16);
    if month_year {
        out.push_str(name);
    } else {
        out.push_str(&name[..3]);
        out.push(' ');
        push_number(f64::from(day), &mut out);
        out.push(',');
    }
    out.push(' ');
    push_number(year as f64, &mut out);
    Value::str(&out)
}

/// The proleptic Gregorian (year, month, day) of `days` since 1970-01-01:
/// Hinnant's `civil_from_days`, integers only.
fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(month <= 2), month, day)
}

/// `Intl.NumberFormat("en-US", { notation: "compact", roundingMode: "trunc",
/// signDisplay: "negative" })`, from the value's shortest decimal digits as
/// ICU reads them: `1.2K`, `999K`, `0.29`, `-1.2K`, `0` for `-0`, `10,000T`.
///
/// - The suffix is chosen from the unrounded magnitude: K from 10³, M from
///   10⁶, B from 10⁹, T from 10¹² and for everything above.
/// - Compact's default precision keeps every integer digit and, below 10,
///   two significant digits; truncation never carries, so the suffix stands.
/// - Grouping is `min2`: only an integer part of five digits or more.
/// - A non-finite value is `""` (Intl prints `NaN` and `∞`; D7).
fn compact(n: f64) -> Value {
    if !n.is_finite() {
        return Value::str("");
    }
    if n == 0.0 {
        return Value::str("0");
    }
    // Rust's `{}` of an f64: its shortest round-trip digits, never exponential.
    let mut text = String::new();
    exact_num::push_text!(&mut text, "{}", exact_num::Shortest(n.abs()));
    let (int, frac) = text.split_once('.').unwrap_or((&text, ""));
    let int = if int == "0" { "" } else { int };
    let scale = if int.len() >= 4 {
        ((int.len() - 1) / 3).min(4)
    } else {
        0
    };
    // The point moves 3 × scale places left.
    let cut = int.len() - 3 * scale;
    let (int, moved) = int.split_at(cut);
    let frac = [moved, frac].concat();
    let kept = match int.len() {
        0 => {
            // Below 1: two significant digits.
            let first = frac.bytes().position(|b| b != b'0').unwrap_or(frac.len());
            &frac[..(first + 2).min(frac.len())]
        }
        1 => &frac[..1.min(frac.len())],
        _ => "",
    };
    let kept = kept.trim_end_matches('0');
    let mut out = String::with_capacity(text.len() + 4);
    if n < 0.0 {
        out.push('-');
    }
    if int.is_empty() {
        out.push('0');
    } else if int.len() >= 5 {
        for (i, digit) in int.char_indices() {
            if i > 0 && (int.len() - i) % 3 == 0 {
                out.push(',');
            }
            out.push(digit);
        }
    } else {
        out.push_str(int);
    }
    if !kept.is_empty() {
        out.push('.');
        out.push_str(kept);
    }
    if scale > 0 {
        out.push(['K', 'M', 'B', 'T'][scale - 1]);
    }
    Value::str(&out)
}
