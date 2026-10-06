//! The linked capability `format`: `formatDate`, `formatNumber`, `toFixed`
//! and `formatDecimal` (LLP 1102 §3.2, decided (c)).
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
            let style = match style(2)? {
                "medium" => Style::Medium,
                "month-year" => Style::MonthYear,
                "iso" => Style::Iso,
                _ => return None,
            };
            match wall_ms(num(0)?, num(1)?) {
                Some(wall) => date(wall, style),
                None => Value::str(""),
            }
        }
        Stdlib::FormatNumber => match style(1)? {
            "compact" => compact(num(0)?),
            _ => return None,
        },
        Stdlib::ToFixed => to_fixed(num(0)?, digits(num(1)?, 100)?),
        Stdlib::FormatDecimal => format_decimal(num(0)?, digits(num(1)?, 20)?),
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

/// A `formatDate` style.
#[derive(Clone, Copy, PartialEq)]
enum Style {
    Medium,
    MonthYear,
    Iso,
}

/// `Sep 26, 2026` (`{ dateStyle: "medium" }`), `September 2026`
/// (`{ month: "long", year: "numeric" }`) or `2026-09-26` (LLP 1102 §3.4: the
/// date part of `toISOString`) of a wall time in range. `en-US`'s short months are the first three letters of
/// the long ones.
fn date(wall: f64, style: Style) -> Value {
    let (year, month, day) = civil((wall / 86_400_000.0).floor() as i64);
    let mut out = String::with_capacity(16);
    if style == Style::Iso {
        // `wall_ms` admits years 1–9999 only (D7), so four digits always.
        let pad = |out: &mut String, n: f64, width: usize| {
            let mut digits = String::new();
            push_number(n, &mut digits);
            out.extend(std::iter::repeat_n('0', width.saturating_sub(digits.len())));
            out.push_str(&digits);
        };
        pad(&mut out, year as f64, 4);
        out.push('-');
        pad(&mut out, f64::from(month), 2);
        out.push('-');
        pad(&mut out, f64::from(day), 2);
        return Value::str(&out);
    }
    let name = MONTHS[month as usize - 1];
    if style == Style::MonthYear {
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

/// A digits argument the compiler admitted (a whole-number literal up to
/// `max`); `None` for any other, as for a style it would have refused.
fn digits(d: f64, max: u32) -> Option<u32> {
    (d.fract() == 0.0 && (0.0..=f64::from(max)).contains(&d)).then_some(d as u32)
}

/// JavaScript's `Number.prototype.toFixed(digits)` (ECMA-262 §21.1.3.3), on
/// the exact binary value: the integer `n` nearest `|x| × 10^digits`, the
/// larger of two (a tie away from zero), so `1.005` at 2 is `1.00`; the
/// sign of any `x < 0` (`-0.001` is `-0.00`, `-0` is `0.00`); `|x| ≥ 10^21`
/// as `toString` prints it. A non-finite `x` is `""` (D7), where JavaScript
/// prints `NaN` or `Infinity`.
fn to_fixed(x: f64, digits: u32) -> Value {
    if !x.is_finite() {
        return Value::str("");
    }
    let mut out = String::new();
    if x.abs() >= 1e21 {
        push_number(x, &mut out);
        return Value::str(&out);
    }
    // |x| = m × 2^e exactly.
    let bits = x.to_bits();
    let (exp, frac) = ((bits >> 52) & 0x7ff, bits & ((1 << 52) - 1));
    let (m, e) = if exp == 0 {
        (frac, -1074)
    } else {
        (frac | 1 << 52, exp as i32 - 1075)
    };
    let mut n = Big::from(m);
    for _ in 0..digits {
        n.mul_add(10, 0);
    }
    if e >= 0 {
        n.shl(e as u32);
    } else {
        // Half an ulp of the result's last place, then the floor: ties up.
        let mut half = Big::from(1);
        half.shl(e.unsigned_abs() - 1);
        n.add(&half);
        n.shr(e.unsigned_abs());
    }
    if x < 0.0 {
        out.push('-');
    }
    push_places(&n.decimal(), digits, &mut out);
    Value::str(&out)
}

/// `formatDecimal(units, digits)` (LLP 1102 §3.2): the integer `units` of a
/// smallest unit as a decimal with `digits` places, exactly; `-0` is `0`;
/// `""` for a count that is not an integer or not finite.
fn format_decimal(units: f64, digits: u32) -> Value {
    if !units.is_finite() || units.fract() != 0.0 {
        return Value::str("");
    }
    let bits = units.to_bits();
    let (exp, frac) = ((bits >> 52) & 0x7ff, bits & ((1 << 52) - 1));
    let mut n = Big::from(if exp == 0 { frac } else { frac | 1 << 52 });
    let e = exp as i32 - 1075;
    // An integer's bits below its unit are zero, so the shift is exact.
    if e >= 0 {
        n.shl(e as u32);
    } else if exp != 0 {
        n.shr(e.unsigned_abs());
    }
    let mut out = String::new();
    if units < 0.0 {
        out.push('-');
    }
    push_places(&n.decimal(), digits, &mut out);
    Value::str(&out)
}

/// The digits of an integer `n` read as `n / 10^places`: at least one digit
/// before the point, `places` after it, no point for none.
fn push_places(digits: &str, places: u32, out: &mut String) {
    let places = places as usize;
    let pad = (places + 1).saturating_sub(digits.len());
    let padded: String = std::iter::repeat_n('0', pad)
        .chain(digits.chars())
        .collect();
    let (int, frac) = padded.split_at(padded.len() - places);
    out.push_str(int);
    if places > 0 {
        out.push('.');
        out.push_str(frac);
    }
}

/// A natural number in base 2^32, least significant limb first: enough for
/// `toFixed`'s `m × 10^100 / 2^1074` and `formatDecimal`'s `m × 2^971`.
struct Big(Vec<u32>);

impl Big {
    fn from(n: u64) -> Big {
        Big(vec![n as u32, (n >> 32) as u32])
    }

    fn mul_add(&mut self, k: u32, add: u32) {
        let mut carry = u64::from(add);
        for limb in &mut self.0 {
            let t = u64::from(*limb) * u64::from(k) + carry;
            *limb = t as u32;
            carry = t >> 32;
        }
        if carry > 0 {
            self.0.push(carry as u32);
        }
    }

    fn add(&mut self, other: &Big) {
        if self.0.len() < other.0.len() {
            self.0.resize(other.0.len(), 0);
        }
        let mut carry = 0u64;
        for (i, limb) in self.0.iter_mut().enumerate() {
            let t = u64::from(*limb) + u64::from(other.0.get(i).copied().unwrap_or(0)) + carry;
            *limb = t as u32;
            carry = t >> 32;
        }
        if carry > 0 {
            self.0.push(carry as u32);
        }
    }

    fn shl(&mut self, bits: u32) {
        let (words, bits) = ((bits / 32) as usize, bits % 32);
        let mut out = vec![0u32; words];
        let mut carry = 0u32;
        for &limb in &self.0 {
            out.push(if bits == 0 {
                limb
            } else {
                limb << bits | carry
            });
            carry = if bits == 0 { 0 } else { limb >> (32 - bits) };
        }
        out.push(carry);
        self.0 = out;
    }

    /// The floor of `self / 2^bits`.
    fn shr(&mut self, bits: u32) {
        let (words, bits) = ((bits / 32) as usize, bits % 32);
        let src = self.0.get(words..).unwrap_or(&[]);
        self.0 = (0..src.len())
            .map(|i| {
                let hi = src.get(i + 1).copied().unwrap_or(0);
                if bits == 0 {
                    src[i]
                } else {
                    src[i] >> bits | hi << (32 - bits)
                }
            })
            .collect();
    }

    /// Decimal digits, no leading zeros (`"0"` for zero).
    fn decimal(mut self) -> String {
        let mut chunks = Vec::new();
        while self.0.iter().any(|&l| l != 0) {
            let mut rem = 0u64;
            for limb in self.0.iter_mut().rev() {
                let t = rem << 32 | u64::from(*limb);
                *limb = (t / 1_000_000_000) as u32;
                rem = t % 1_000_000_000;
            }
            chunks.push(rem as u32);
        }
        let mut out = String::new();
        for (i, chunk) in chunks.iter().rev().enumerate() {
            let mut text = String::new();
            push_number(f64::from(*chunk), &mut text);
            if i > 0 {
                out.extend(std::iter::repeat_n('0', 9 - text.len()));
            }
            out.push_str(&text);
        }
        if out.is_empty() {
            out.push('0');
        }
        out
    }
}
