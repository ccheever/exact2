//! The linked capability `format`: `formatDate`, `formatNumber`, `toFixed`
//! and `formatDecimal` (LLP 1102 §3.2, decided (c)).
//!
//! @ref LLP 1054.000.003 D2 (dates), D3 (compact numbers), D7 (bounded
//! input), D8 (linked by use, the router's way); LLP 1116 D8 (decimal,
//! currency and percent numbers)
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
            other => styled(num(0)?, other, style(2).unwrap_or(""))?,
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

/// The ISO 4217 codes `formatNumber(n, "currency", code)` takes (LLP 1116
/// D8), each with its `en-US` symbol and fraction digits, as Bun 1.4.2's
/// `Intl.NumberFormat("en-US", { style: "currency", currency })` (ICU 78.1)
/// prints them. A symbol that ends in a letter is set apart from the number
/// by a no-break space (`CHF 5.00`), as ICU's currency spacing does.
const CURRENCIES: [(&str, &str, u32); 24] = [
    ("USD", "$", 2),
    ("EUR", "€", 2),
    ("GBP", "£", 2),
    ("JPY", "¥", 0),
    ("CNY", "CN¥", 2),
    ("INR", "₹", 2),
    ("CAD", "CA$", 2),
    ("AUD", "A$", 2),
    ("NZD", "NZ$", 2),
    ("HKD", "HK$", 2),
    ("SGD", "SGD", 2),
    ("CHF", "CHF", 2),
    ("SEK", "SEK", 2),
    ("NOK", "NOK", 2),
    ("DKK", "DKK", 2),
    ("PLN", "PLN", 2),
    ("MXN", "MX$", 2),
    ("BRL", "R$", 2),
    ("KRW", "₩", 0),
    ("ZAR", "ZAR", 2),
    ("TWD", "NT$", 2),
    ("ILS", "₪", 2),
    ("PHP", "₱", 2),
    ("VND", "₫", 0),
];

/// `formatNumber(n, "decimal" | "currency" | "percent")` (LLP 1116 D8):
/// `Intl.NumberFormat("en-US")` with `{}`, `{ style: "currency", currency }`
/// or `{ style: "percent" }`, from the value's shortest decimal digits as
/// ICU reads them ([`js_digits`]: `1.005` is `$1.01`, where `toFixed` says
/// `1.00`):
///
/// - `decimal` keeps at most three fraction digits, `currency` exactly the
///   code's, `percent` none after moving the point two places right;
/// - the cut rounds half away from zero (`halfExpand`), and may carry into
///   a new integer digit (`999.9995` is `1,000`);
/// - the integer part is grouped by threes from four digits;
/// - a negative value, `-0` and one that rounds to zero included, is signed
///   before the symbol: `-0`, `-$5.00`, `-0%`;
/// - a non-finite value is `""`, as every format's is (D7).
///
/// `None` for a style or code the compiler would have refused.
fn styled(n: f64, style: &str, code: &str) -> Option<Value> {
    let (symbol, min, max, shift) = match style {
        "decimal" => ("", 0, 3, 0),
        "percent" => ("", 0, 0, 2),
        "currency" => {
            let &(_, symbol, digits) = CURRENCIES.iter().find(|c| c.0 == code)?;
            (symbol, digits as usize, digits as usize, 0)
        }
        _ => return None,
    };
    if !n.is_finite() {
        return Some(Value::str(""));
    }
    let (int, mut frac) = js_digits(n.abs());
    let cut = shift + max;
    let up = frac.get(cut).is_some_and(|&d| d >= b'5');
    frac.resize(frac.len().max(cut), b'0');
    // The kept digits, the point `max` places from the end.
    let mut digits: Vec<u8> = int.into_iter().chain(frac[..cut].iter().copied()).collect();
    if up {
        match digits.iter().rposition(|&d| d != b'9') {
            Some(i) => {
                digits[i] += 1;
                digits[i + 1..].fill(b'0');
            }
            None => {
                digits.fill(b'0');
                digits.insert(0, b'1');
            }
        }
    }
    let (int, frac) = digits.split_at(digits.len() - max);
    let int = &int[int.iter().position(|&d| d != b'0').unwrap_or(int.len() - 1)..];
    let kept = frac.iter().rposition(|&d| d != b'0').map_or(0, |i| i + 1);
    let frac = &frac[..kept.max(min)];
    let mut out = String::with_capacity(int.len() * 4 / 3 + frac.len() + 8);
    if n.is_sign_negative() {
        out.push('-');
    }
    out.push_str(symbol);
    if symbol.ends_with(|c: char| c.is_ascii_alphabetic()) {
        out.push('\u{a0}');
    }
    for (i, &d) in int.iter().enumerate() {
        if i > 0 && (int.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(char::from(d));
    }
    if !frac.is_empty() {
        out.push('.');
        out.extend(frac.iter().map(|&d| char::from(d)));
    }
    if style == "percent" {
        out.push('%');
    }
    Some(Value::str(&out))
}

/// The integer and fraction digits of a finite `x ≥ 0` as JavaScript's
/// `String(x)` chooses them (the shortest that read back as `x`, of two
/// equally near the even, as ICU's are), written out without an exponent:
/// `1e21` is `1` and 21 zeros, `5e-324` is `0.` and 323 zeros and a `5`.
fn js_digits(x: f64) -> (Vec<u8>, Vec<u8>) {
    let mut text = String::new();
    exact_num::push_js(x, &mut text);
    let (mantissa, exponent) = match text.split_once('e') {
        Some((m, e)) => (m, e.parse::<i32>().unwrap_or(0)),
        None => (text.as_str(), 0),
    };
    let (int, frac) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    let mut digits: Vec<u8> = int.bytes().chain(frac.bytes()).collect();
    // Where the point falls in `digits`, which may be past either end.
    let point = int.len() as i32 + exponent;
    if point <= 0 {
        let mut frac = vec![b'0'; point.unsigned_abs() as usize];
        frac.append(&mut digits);
        return (vec![b'0'], frac);
    }
    let point = point as usize;
    digits.resize(digits.len().max(point), b'0');
    let frac = digits.split_off(point);
    (digits, frac)
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
