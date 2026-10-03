//! Float text as core writes it — `{}`, `{:?}`, `{:e}` and `{:.N}`, byte for
//! byte — without core's float printer (Grisu's cached powers and Dragon's
//! tables).
//!
//! Core prints the fewest digits that lie in a value's rounding interval
//! (Dragon's test on core's interval: bounds included for an even mantissa,
//! the lower gap halved at every power of two), and of two such, the closer,
//! a tie rounding up. Here an integer below 2^(p+1) prints as itself; any
//! other value whose shortest form has at most 27 decimal places runs
//! Dragon's test at those places in `u128`; the rest — huge or tiny — run
//! Dragon itself on this crate's big integers. `{:.N}` rounds the exact value
//! half to even, as core does. The tests hold every text to core's.

use crate::{Big, Format, F32, F64};
use std::cmp::Ordering;
use std::fmt::{self, Write};

/// An `f64` as `{}` writes it: `format!("{}", Shortest(v)) == format!("{v}")`.
/// Width, fill and sign flags are not read.
#[derive(Debug, Clone, Copy)]
pub struct Shortest(pub f64);

/// An `f32` as `{}` writes it.
#[derive(Debug, Clone, Copy)]
pub struct Shortest32(pub f32);

/// An `f64` as `{:?}` writes it: decimal with a fraction digit, or
/// exponential below 1e-4 and from 1e16. Flags other than `#` are not read.
#[derive(Clone, Copy)]
pub struct ShortestDebug(pub f64);

/// An `f64` as `{:e}` writes it.
#[derive(Debug, Clone, Copy)]
pub struct Exponent(pub f64);

/// An `f64` as `{:.N}` writes it, `N` the second field: the exact value
/// rounded half to even at `N` places.
#[derive(Debug, Clone, Copy)]
pub struct Fixed(pub f64, pub usize);

impl fmt::Display for Shortest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.write(f)
    }
}

impl fmt::Display for Shortest32 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.write(f)
    }
}

impl Shortest {
    /// The `{}` text into any writer: a `String` takes it without
    /// `core::fmt`'s machinery ([`crate::Piece`]).
    pub(crate) fn write(&self, out: &mut dyn Write) -> fmt::Result {
        write(self.0.to_bits(), &F64, Layout::Decimal, out)
    }
}

impl Shortest32 {
    pub(crate) fn write(&self, out: &mut dyn Write) -> fmt::Result {
        write(u64::from(self.0.to_bits()), &F32, Layout::Decimal, out)
    }
}

impl fmt::Debug for ShortestDebug {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let abs = self.0.abs();
        let layout = if abs != 0.0 && !(1e-4..1e16).contains(&abs) {
            Layout::Exponential
        } else {
            Layout::Point
        };
        write(self.0.to_bits(), &F64, layout, f)
    }
}

impl fmt::Display for Exponent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write(self.0.to_bits(), &F64, Layout::Exponential, f)
    }
}

impl fmt::Display for Fixed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Fixed(v, frac) = *self;
        let Some(d) = sign(v.to_bits(), &F64, f)? else {
            return Ok(());
        };
        let digits = match d {
            Some(d) => rounded(&d, frac),
            None => String::new(),
        };
        if digits.is_empty() {
            return zero(frac, f);
        }
        if digits.len() > frac {
            let (int, fraction) = digits.split_at(digits.len() - frac);
            f.write_str(int)?;
            if frac > 0 {
                f.write_char('.')?;
                f.write_str(fraction)?;
            }
            Ok(())
        } else {
            f.write_str("0.")?;
            zeros(frac - digits.len(), f)?;
            f.write_str(&digits)
        }
    }
}

enum Layout {
    /// `ddd.ddd`, shortest.
    Decimal,
    /// `ddd.ddd` with at least one fraction digit.
    Point,
    /// `d.ddde±x`, shortest.
    Exponential,
}

/// A finite nonzero value as core's `integer_decode` gives it: `mant × 2^exp`.
struct Decoded {
    mant: u64,
    exp: i32,
    subnormal: bool,
}

/// Writes NaN or an infinity whole and gives `None`; otherwise writes the
/// sign (`-` for any negative, zero included, as core does) and gives the
/// value, `Some(None)` for zero.
fn sign(bits: u64, f: &Format, out: &mut dyn Write) -> Result<Option<Option<Decoded>>, fmt::Error> {
    let (width, exp_bits) = if f.p == F32.p { (32, 8) } else { (64, 11) };
    let negative = bits >> (width - 1) & 1 == 1;
    let field = (bits >> f.p) & ((1 << exp_bits) - 1);
    let frac = bits & ((1u64 << f.p) - 1);
    let bias = (1 << (exp_bits - 1)) - 1;
    if field == (1 << exp_bits) - 1 {
        out.write_str(match (frac == 0, negative) {
            (false, _) => "NaN",
            (true, false) => "inf",
            (true, true) => "-inf",
        })?;
        return Ok(None);
    }
    if negative {
        out.write_char('-')?;
    }
    Ok(Some(match (field, frac) {
        (0, 0) => None,
        (0, _) => Some(Decoded {
            mant: frac << 1,
            exp: -(bias + f.p as i32),
            subnormal: true,
        }),
        _ => Some(Decoded {
            mant: frac | (1 << f.p),
            exp: field as i32 - bias - f.p as i32,
            subnormal: false,
        }),
    }))
}

fn write(bits: u64, f: &Format, layout: Layout, out: &mut dyn Write) -> fmt::Result {
    let Some(d) = sign(bits, f, out)? else {
        return Ok(());
    };
    let Some(d) = d else {
        return out.write_str(match layout {
            Layout::Decimal => "0",
            Layout::Point => "0.0",
            Layout::Exponential => "0e0",
        });
    };
    let (digits, exp) = shortest(&d, f);
    let mut buf = [0; 20];
    let digits = ascii(digits, &mut buf);
    match layout {
        Layout::Decimal => decimal(digits, exp, out),
        Layout::Point => {
            decimal(digits, exp, out)?;
            if exp >= digits.len() as i32 {
                out.write_str(".0")?;
            }
            Ok(())
        }
        Layout::Exponential => exponential(digits, exp, out),
    }
}

/// `v` in decimal, in the tail of `buf`.
pub(crate) fn ascii(mut v: u64, buf: &mut [u8; 20]) -> &str {
    let mut i = buf.len();
    loop {
        i -= 1;
        buf[i] = b'0' + (v % 10) as u8;
        v /= 10;
        if v == 0 {
            break;
        }
    }
    std::str::from_utf8(&buf[i..]).unwrap_or_default()
}

fn zeros(n: usize, out: &mut dyn Write) -> fmt::Result {
    (0..n).try_for_each(|_| out.write_char('0'))
}

fn zero(frac: usize, out: &mut dyn Write) -> fmt::Result {
    out.write_char('0')?;
    if frac > 0 {
        out.write_char('.')?;
        zeros(frac, out)?;
    }
    Ok(())
}

/// Core's `digits_to_dec_str` with no forced fraction digits:
/// `0.digits × 10^exp`.
fn decimal(digits: &str, exp: i32, out: &mut dyn Write) -> fmt::Result {
    if exp <= 0 {
        out.write_str("0.")?;
        zeros(exp.unsigned_abs() as usize, out)?;
        out.write_str(digits)
    } else if (exp as usize) < digits.len() {
        let (int, fraction) = digits.split_at(exp as usize);
        out.write_str(int)?;
        out.write_char('.')?;
        out.write_str(fraction)
    } else {
        out.write_str(digits)?;
        zeros(exp as usize - digits.len(), out)
    }
}

/// Core's `digits_to_exp_str`, lower case, no forced fraction digits.
fn exponential(digits: &str, exp: i32, out: &mut dyn Write) -> fmt::Result {
    let (first, rest) = digits.split_at(1);
    out.write_str(first)?;
    if !rest.is_empty() {
        out.write_char('.')?;
        out.write_str(rest)?;
    }
    out.write_char('e')?;
    let e = exp - 1;
    if e < 0 {
        out.write_char('-')?;
    }
    let mut buf = [0; 20];
    out.write_str(ascii(u64::from(e.unsigned_abs()), &mut buf))
}

/// `round_half_even(mant × 2^exp × 10^frac)` in decimal, empty for zero.
fn rounded(d: &Decoded, frac: usize) -> String {
    if d.exp < 0 && frac <= 19 {
        // mant × 10^frac < 2^53 × 2^64 fits a u128.
        let n = u128::from(d.mant) * u128::from(10u64.pow(frac as u32));
        let s = d.exp.unsigned_abs();
        let q = if s >= 120 {
            Some(0)
        } else {
            let (q, rem, half) = (n >> s, n & ((1 << s) - 1), 1u128 << (s - 1));
            let up = rem > half || (rem == half && q & 1 == 1);
            u64::try_from(q + u128::from(up)).ok()
        };
        if let Some(q) = q {
            let mut buf = [0; 20];
            return if q == 0 {
                String::new()
            } else {
                ascii(q, &mut buf).to_owned()
            };
        }
    }
    let mut num = Big::from_u64(d.mant);
    num.mul_pow10(frac as u32);
    let mut q = num.clone();
    if d.exp >= 0 {
        q.shl(d.exp as u32);
    } else {
        let s = d.exp.unsigned_abs();
        q.shr(s);
        // The remainder num - q·2^s against half of 2^s.
        let mut back = q.clone();
        back.shl(s);
        let mut rem = num;
        rem.sub(&back);
        rem.shl(1);
        let mut unit = Big::from_u64(1);
        unit.shl(s);
        let order = rem.cmp(&unit);
        if order == Ordering::Greater || (order == Ordering::Equal && q.0[0] & 1 == 1) {
            q.add(&Big::from_u64(1));
        }
    }
    let digits = q.to_decimal();
    if digits == "0" {
        String::new()
    } else {
        digits
    }
}

/// The shortest digits and `exp`, the value being `0.digits × 10^exp`.
fn shortest(d: &Decoded, f: &Format) -> (u64, i32) {
    // An integer below 2^(p + 1) is its own shortest decimal.
    let integral = d.exp >= 0 || (d.exp > -64 && d.mant & ((1u64 << -d.exp) - 1) == 0);
    if integral {
        let value = if d.exp >= 0 {
            d.mant
                .checked_shl(d.exp as u32)
                .filter(|v| *v >> d.exp == d.mant)
        } else {
            Some(d.mant >> -d.exp)
        };
        if let Some(value) = value.filter(|v| *v < 1 << (f.p + 1)) {
            return trimmed(value, 0);
        }
    } else if let Some(found) = places(d, f) {
        return found;
    }
    dragon(d, f)
}

/// `value × 10^-places` without trailing zeros, and its `exp`.
fn trimmed(mut value: u64, places: i32) -> (u64, i32) {
    let mut len = 0;
    let mut rest = value;
    while rest > 0 {
        rest /= 10;
        len += 1;
    }
    while value.is_multiple_of(10) {
        value /= 10;
    }
    (value, len - places)
}

/// Core's rounding interval: the value is `mant × 2^exp`, the midpoints to
/// its neighbours `(mant - minus) × 2^exp` and `(mant + plus) × 2^exp`, and
/// every normal power of two has the lower gap halved.
fn interval(d: &Decoded, f: &Format) -> (u64, u64, u64, i32) {
    if d.subnormal {
        (d.mant, 1, 1, d.exp)
    } else if d.mant == 1 << f.p {
        (d.mant << 2, 1, 2, d.exp - 2)
    } else {
        (d.mant << 1, 1, 1, d.exp - 1)
    }
}

/// The fewest decimal places, 1 to 27, holding a decimal in the interval,
/// and the digits Dragon keeps there. A non-integer's interval holds no
/// integer, so Dragon's first stop is at one place or more; a decimal at
/// `k` places is one at `k + 1` too, so a binary search finds the first.
fn places(d: &Decoded, f: &Format) -> Option<(u64, i32)> {
    let (mant, minus, plus, exp) = interval(d, f);
    let inclusive = d.mant & 1 == 0;
    let at = |k| stop(mant, minus, plus, exp, inclusive, k);
    // The numbers an app shows mostly stop within a few places: those are
    // tried in order first, one test each instead of the search's six.
    let (mut lo, mut hi) = match (1..=FEW).find(|&k| at(k).is_some()) {
        Some(k) => (k, k),
        None => {
            at(27)?;
            (FEW + 1, 27)
        }
    };
    while lo < hi {
        let mid = (lo + hi) / 2;
        if at(mid).is_some() {
            hi = mid;
        } else {
            lo = mid + 1;
        }
    }
    let digits = u64::try_from(at(lo)?).ok().filter(|&c| c != 0)?;
    Some(trimmed(digits, lo as i32))
}

/// The places [`places`] tries one by one before its binary search.
const FEW: u32 = 3;

/// Dragon's stopping test at `k` decimal places, in `u128`: whether the
/// truncation (`down`) or the decimal above it (`up`) lies in the interval,
/// and which of them Dragon keeps. `None` while neither does.
fn stop(mant: u64, minus: u64, plus: u64, exp: i32, inclusive: bool, k: u32) -> Option<u128> {
    // value × 10^k = mant × 5^k / 2^t, and mant < 2^56, 5^27 < 2^63.
    let t = -(exp + k as i32);
    if t <= 0 {
        return Some(0); // exact at k places: past the first stop
    }
    if t >= 120 {
        return None; // (mant + plus) × 5^k < 2^119: no decimal here
    }
    let p5 = u128::from(5u64.pow(k));
    let n = u128::from(mant) * p5;
    let (q, rem, scale) = (n >> t, n & ((1 << t) - 1), 1u128 << t);
    let (low, high) = (u128::from(minus) * p5, rem + u128::from(plus) * p5);
    let down = if inclusive { rem <= low } else { rem < low };
    let up = if inclusive {
        scale <= high
    } else {
        scale < high
    };
    match (down, up) {
        (false, false) => None,
        (true, false) => Some(q),
        (false, true) => Some(q + 1),
        // The closer; a tie rounds up.
        (true, true) => Some(q + u128::from(2 * rem >= scale)),
    }
}

/// Core's Dragon `format_shortest`, on this crate's big integers.
fn dragon(d: &Decoded, f: &Format) -> (u64, i32) {
    let inclusive = d.mant & 1 == 0;
    let (mant, minus, plus, exp) = interval(d, f);
    let rounding = if inclusive {
        Ordering::Greater
    } else {
        Ordering::Equal
    };
    // `10^(k-1) < high <= 10^(k+1)`: core's `estimate_scaling_factor`.
    let nbits = 64 - (mant + plus - 1).leading_zeros() as i64;
    let mut k = (((nbits + exp as i64) * 1_292_913_986) >> 32) as i32;
    let (mut mant, mut minus, mut plus) = (
        Big::from_u64(mant),
        Big::from_u64(minus),
        Big::from_u64(plus),
    );
    let mut scale = Big::from_u64(1);
    if exp < 0 {
        scale.shl(exp.unsigned_abs());
    } else {
        mant.shl(exp as u32);
        minus.shl(exp as u32);
        plus.shl(exp as u32);
    }
    if k >= 0 {
        scale.mul_pow10(k as u32);
    } else {
        mant.mul_pow10(k.unsigned_abs());
        minus.mul_pow10(k.unsigned_abs());
        plus.mul_pow10(k.unsigned_abs());
    }
    let sum = |a: &Big, b: &Big| {
        let mut s = a.clone();
        s.add(b);
        s
    };
    if scale.cmp(&sum(&mant, &plus)) < rounding {
        k += 1;
    } else {
        mant.mul_add(10, 0);
        minus.mul_add(10, 0);
        plus.mul_add(10, 0);
    }
    let multiples: Vec<(u64, Big)> = [(8, 3), (4, 2), (2, 1), (1, 0)]
        .into_iter()
        .map(|(weight, shift)| {
            let mut multiple = scale.clone();
            multiple.shl(shift);
            (weight, multiple)
        })
        .collect();
    let (mut digits, mut len) = (0u64, 0);
    let (mut down, mut up);
    loop {
        let mut digit = 0;
        for (weight, multiple) in &multiples {
            if mant.cmp(multiple) != Ordering::Less {
                mant.sub(multiple);
                digit += weight;
            }
        }
        digits = digits * 10 + digit;
        len += 1;
        down = mant.cmp(&minus) < rounding;
        up = scale.cmp(&sum(&mant, &plus)) < rounding;
        if down || up {
            break;
        }
        mant.mul_add(10, 0);
        minus.mul_add(10, 0);
        plus.mul_add(10, 0);
    }
    if up
        && (!down || {
            let mut twice = mant.clone();
            twice.shl(1);
            twice.cmp(&scale) != Ordering::Less
        })
    {
        // Core's round_up: all nines become 1 then zeros, one digit longer.
        digits += 1;
        if digits == 10u64.pow(len) {
            k += 1;
        }
    }
    (digits, k)
}
