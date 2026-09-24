//! Number text as std reads and writes it, without std's tables.
//!
//! @ref LLP 1047 §6 (the core diet)
//!
//! `str::parse::<f64>()` links `core`'s decimal-to-float machinery, whose
//! Eisel–Lemire table alone is 10 KB of incompressible wasm data. A correctly
//! rounded parse has exactly one answer, so this one — Clinger's exact fast
//! path, an exact `u128` path for up to 19 digits and exponents to ±27 (what
//! `JSON.stringify` writes), then exact big-integer arithmetic — returns
//! std's bits for every input.
//! The grammar, the exponent's saturation and the error messages are std's
//! too. The tests hold it to std, bit for bit.
//!
//! Writing is the same trade: core's float printer (Grisu with Dragon
//! behind it, about 23 KB of wasm) prints one text per value and format, so
//! [`Shortest`], [`Shortest32`], [`ShortestDebug`], [`Exponent`] and
//! [`Fixed`] print exactly core's `{}`, `{:?}`, `{:e}` and `{:.N}`.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::cmp::Ordering;
use std::fmt;

mod text;
pub use text::{Exponent, Fixed, Shortest, Shortest32, ShortestDebug};

#[cfg(test)]
mod tests;
#[cfg(test)]
mod text_tests;

/// Why text is not a float: std's `core::num::ParseFloatError`, with the same
/// `Display` and `Debug` text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseFloatError {
    kind: FloatErrorKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum FloatErrorKind {
    Empty,
    Invalid,
}

impl fmt::Display for ParseFloatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(match self.kind {
            FloatErrorKind::Empty => "cannot parse float from empty string",
            FloatErrorKind::Invalid => "invalid float literal",
        })
    }
}

impl std::error::Error for ParseFloatError {}

/// `s` as an `f64`: exactly `s.parse::<f64>()`.
pub fn parse_f64(s: &str) -> Result<f64, ParseFloatError> {
    parse(s.as_bytes(), &F64).map(f64::from_bits)
}

/// `s` as an `f32`: exactly `s.parse::<f32>()`.
pub fn parse_f32(s: &str) -> Result<f32, ParseFloatError> {
    parse(s.as_bytes(), &F32).map(|bits| f32::from_bits(bits as u32))
}

/// An IEEE 754 binary format, as the one code path both widths share.
struct Format {
    /// Explicit mantissa bits.
    p: u32,
    /// Exponent of the last mantissa bit of the smallest subnormal.
    min_e2: i32,
    /// Exponent of the last mantissa bit of the largest finite value.
    max_e2: i32,
    /// Below `10^(min_k)` a value rounds to zero; at `10^(max_k)` and
    /// above it is infinite (`k` is the decimal exponent past the first
    /// significant digit).
    min_k: i64,
    max_k: i64,
    sign: u64,
    inf: u64,
    nan: u64,
    /// Clinger's exact fast path: a mantissa and a power of ten both
    /// exactly representable make one correctly rounded operation.
    fast_mantissa: u64,
    fast_exp: i64,
}

const F64: Format = Format {
    p: 52,
    min_e2: -1074,
    max_e2: 971,
    min_k: -324,
    max_k: 309,
    sign: 1 << 63,
    inf: 0x7ff0_0000_0000_0000,
    nan: 0x7ff8_0000_0000_0000,
    fast_mantissa: 1 << 53,
    fast_exp: 22,
};

const F32: Format = Format {
    p: 23,
    min_e2: -149,
    max_e2: 104,
    min_k: -46,
    max_k: 39,
    sign: 1 << 31,
    inf: 0x7f80_0000,
    nan: 0x7fc0_0000,
    fast_mantissa: 1 << 24,
    fast_exp: 10,
};

/// More significant digits than a halfway point between two adjacent
/// doubles can have (767); the rest only decide a tie (std keeps 768).
const MAX_DIGITS: usize = 800;

fn parse(s: &[u8], f: &Format) -> Result<u64, ParseFloatError> {
    let invalid = ParseFloatError {
        kind: FloatErrorKind::Invalid,
    };
    let Some(&first) = s.first() else {
        return Err(ParseFloatError {
            kind: FloatErrorKind::Empty,
        });
    };
    let sign = if first == b'-' { f.sign } else { 0 };
    let s = if first == b'-' || first == b'+' {
        &s[1..]
    } else {
        s
    };
    if s.is_empty() {
        return Err(invalid);
    }
    let bits = match decimal(s) {
        Some((int, frac, exp)) => value(int, frac, exp, f),
        None => special(s, f).ok_or(invalid)?,
    };
    Ok(bits | sign)
}

/// `digits [. digits] [(e|E) [+|-] digits]`, at least one mantissa digit and
/// nothing after: the integer digits, the fraction digits and the exponent,
/// which saturates as std's does (it stops growing at 0x10000).
fn decimal(s: &[u8]) -> Option<(&[u8], &[u8], i64)> {
    let digits = |s: &[u8]| s.iter().take_while(|c| c.is_ascii_digit()).count();
    let i = digits(s);
    let (int, mut rest) = s.split_at(i);
    let mut frac: &[u8] = &[];
    if let Some((b'.', after)) = rest.split_first() {
        let j = digits(after);
        (frac, rest) = after.split_at(j);
    }
    if int.is_empty() && frac.is_empty() {
        return None;
    }
    let mut exp = 0i64;
    if let Some((&e, after)) = rest.split_first() {
        if e != b'e' && e != b'E' {
            return None;
        }
        let (negative, after) = match after.split_first() {
            Some((b'-', after)) => (true, after),
            Some((b'+', after)) => (false, after),
            _ => (false, after),
        };
        let j = digits(after);
        if j == 0 || j != after.len() {
            return None;
        }
        for &c in after {
            if exp < 0x10000 {
                exp = 10 * exp + i64::from(c - b'0');
            }
        }
        if negative {
            exp = -exp;
        }
    }
    Some((int, frac, exp))
}

/// `inf`, `infinity` or `nan`, in any case.
fn special(s: &[u8], f: &Format) -> Option<u64> {
    let is = |word: &[u8]| s.len() == word.len() && s.iter().zip(word).all(|(c, w)| c | 0x20 == *w);
    if is(b"inf") || is(b"infinity") {
        Some(f.inf)
    } else if is(b"nan") {
        Some(f.nan)
    } else {
        None
    }
}

/// The correctly rounded bits of `int.frac × 10^exp`, unsigned.
fn value(int: &[u8], frac: &[u8], exp: i64, f: &Format) -> u64 {
    let len = int.len() + frac.len();
    let at = |i: usize| {
        if i < int.len() {
            int[i]
        } else {
            frac[i - int.len()]
        }
    };
    let lead = (0..len).take_while(|&i| at(i) == b'0').count();
    let end = (lead..len)
        .rev()
        .find(|&i| at(i) != b'0')
        .map_or(lead, |i| i + 1);
    let n = end - lead;
    if n == 0 {
        return 0;
    }
    // The significant digits D, read as an integer, times 10^e10.
    let e10 = int.len() as i64 - end as i64 + exp;
    let k = e10 + n as i64;
    if k <= f.min_k {
        return 0;
    }
    if k > f.max_k {
        return f.inf;
    }
    if n <= 19 {
        let d = (lead..end).fold(0u64, |d, i| d * 10 + u64::from(at(i) - b'0'));
        if d <= f.fast_mantissa && e10.abs() <= f.fast_exp {
            return fast(d, e10, f);
        }
        if e10.abs() <= 27 {
            return medium(d, e10, f);
        }
    }
    let kept = n.min(MAX_DIGITS);
    let mut num = Big(vec![0]);
    for i in lead..lead + kept {
        num.mul_add(10, u32::from(at(i) - b'0'));
    }
    exact(num, e10 + (n - kept) as i64, n > kept, f)
}

fn fast(d: u64, e10: i64, f: &Format) -> u64 {
    if f.p == F32.p {
        let mut p = 1f32;
        for _ in 0..e10.abs() {
            p *= 10.0;
        }
        let x = d as f32;
        u64::from(if e10 < 0 { x / p } else { x * p }.to_bits())
    } else {
        let mut p = 1f64;
        for _ in 0..e10.abs() {
            p *= 10.0;
        }
        let x = d as f64;
        if e10 < 0 { x / p } else { x * p }.to_bits()
    }
}

/// `d × 10^e10` for `|e10| <= 27`, exactly: `10^e10 = 5^e10 × 2^e10`, and
/// `5^27` fits a `u64`. A product `d × 5^e10` is an exact `u128`; a quotient
/// `d / 5^-e10`, taken with the dividend at the top of a `u128`, keeps at
/// least 65 bits and folds its remainder into the last bit, a sticky bit that
/// breaks exact ties the way the dropped digits would. One `as` conversion
/// (correctly rounded) makes the float; scaling by a power of two is exact,
/// because every result here is a normal float of either width.
fn medium(d: u64, e10: i64, f: &Format) -> u64 {
    let p5 = u128::from(5u64.pow(e10.unsigned_abs() as u32));
    let (n, e2) = if e10 >= 0 {
        (u128::from(d) * p5, e10 as i32)
    } else {
        let s = d.leading_zeros() + 64;
        let n = u128::from(d) << s;
        let q = n / p5;
        (q | u128::from(q * p5 != n), e10 as i32 - s as i32)
    };
    // Halves of a power of two, each normal: two exact multiplications.
    let (a, b) = (e2 / 2, e2 - e2 / 2);
    if f.p == F32.p {
        let two = |e: i32| f32::from_bits(((127 + e) as u32) << 23);
        u64::from((n as f32 * two(a) * two(b)).to_bits())
    } else {
        let two = |e: i32| f64::from_bits(((1023 + e) as u64) << 52);
        (n as f64 * two(a) * two(b)).to_bits()
    }
}

/// `num × 10^e10` (plus a nonzero tail when `truncated`), rounded half to
/// even into the format: `num / den` as an integer quotient of `p + 1` bits
/// and its remainder, found by long division.
fn exact(mut num: Big, e10: i64, truncated: bool, f: &Format) -> u64 {
    let mut den = Big(vec![1]);
    if e10 >= 0 {
        num.mul_pow10(e10 as u32);
    } else {
        den.mul_pow10(-e10 as u32);
    }
    let p = f.p as i32;
    // num / den lies in (2^(nb-mb-1), 2^(nb-mb+1)), so this e2 puts the
    // quotient in [2^p, 2^(p+2)); one step up if it came out a bit long.
    let mut e2 = num.bits() as i32 - den.bits() as i32 - p - 1;
    loop {
        let e = e2.max(f.min_e2);
        let (mut r, mut d) = (num.clone(), den.clone());
        if e >= 0 {
            d.shl(e as u32);
        } else {
            r.shl(-e as u32);
        }
        let mut q = r.div_rem(&d, f.p + 1);
        if e == e2 && q >> (f.p + 1) != 0 {
            e2 += 1;
            continue;
        }
        let mut e = e;
        r.shl(1);
        let half = match r.cmp(&d) {
            Ordering::Equal if truncated => Ordering::Greater,
            c => c,
        };
        if half == Ordering::Greater || (half == Ordering::Equal && q & 1 == 1) {
            q += 1;
            if q >> (f.p + 1) != 0 {
                q >>= 1;
                e += 1;
            }
        }
        if e > f.max_e2 {
            return f.inf;
        }
        return (((e - f.min_e2) as u64) << f.p) + q;
    }
}

/// A natural number, little-endian in base 2^32, with no high zero limbs
/// (zero is one zero limb).
#[derive(Clone)]
struct Big(Vec<u32>);

impl Big {
    fn from_u64(n: u64) -> Big {
        let mut big = Big(vec![n as u32, (n >> 32) as u32]);
        big.trim();
        big
    }

    fn add(&mut self, other: &Big) {
        let mut carry = 0u64;
        for i in 0..self.0.len().max(other.0.len()) {
            if i == self.0.len() {
                self.0.push(0);
            }
            let t = u64::from(self.0[i]) + u64::from(other.0.get(i).copied().unwrap_or(0)) + carry;
            self.0[i] = t as u32;
            carry = t >> 32;
        }
        if carry != 0 {
            self.0.push(carry as u32);
        }
    }

    fn shr(&mut self, n: u32) {
        let (limbs, bits) = ((n / 32) as usize, n % 32);
        if limbs >= self.0.len() {
            *self = Big(vec![0]);
            return;
        }
        self.0.drain(..limbs);
        if bits != 0 {
            let mut carry = 0;
            for x in self.0.iter_mut().rev() {
                let next = *x << (32 - bits);
                *x = (*x >> bits) | carry;
                carry = next;
            }
        }
        self.trim();
    }

    /// Decimal digits, most significant first.
    fn to_decimal(&self) -> String {
        let mut limbs = self.0.clone();
        let mut chunks = Vec::new();
        while limbs.len() > 1 || limbs[0] != 0 {
            let mut rem = 0u64;
            for x in limbs.iter_mut().rev() {
                let t = (rem << 32) | u64::from(*x);
                *x = (t / 1_000_000_000) as u32;
                rem = t % 1_000_000_000;
            }
            chunks.push(rem as u32);
            while limbs.len() > 1 && limbs[limbs.len() - 1] == 0 {
                limbs.pop();
            }
        }
        let mut out = String::new();
        for (i, chunk) in chunks.iter().rev().enumerate() {
            let text = chunk.to_string();
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

    fn mul_add(&mut self, m: u32, a: u32) {
        let mut carry = u64::from(a);
        for x in &mut self.0 {
            let t = u64::from(*x) * u64::from(m) + carry;
            *x = t as u32;
            carry = t >> 32;
        }
        if carry != 0 {
            self.0.push(carry as u32);
        }
    }

    fn mul_pow10(&mut self, mut e: u32) {
        while e > 0 {
            let step = e.min(9);
            self.mul_add(10u32.pow(step), 0);
            e -= step;
        }
    }

    fn bits(&self) -> u32 {
        let top = self.0[self.0.len() - 1];
        (self.0.len() as u32 - 1) * 32 + (32 - top.leading_zeros())
    }

    fn shl(&mut self, n: u32) {
        let (limbs, bits) = ((n / 32) as usize, n % 32);
        if bits != 0 {
            let mut carry = 0;
            for x in &mut self.0 {
                let next = *x >> (32 - bits);
                *x = (*x << bits) | carry;
                carry = next;
            }
            if carry != 0 {
                self.0.push(carry);
            }
        }
        if limbs != 0 && self.0 != [0] {
            self.0.splice(0..0, std::iter::repeat_n(0, limbs));
        }
    }

    fn shr1(&mut self) {
        let mut carry = 0;
        for x in self.0.iter_mut().rev() {
            let next = *x << 31;
            *x = (*x >> 1) | carry;
            carry = next;
        }
        self.trim();
    }

    fn trim(&mut self) {
        while self.0.len() > 1 && self.0[self.0.len() - 1] == 0 {
            self.0.pop();
        }
    }

    fn cmp(&self, other: &Big) -> Ordering {
        self.0
            .len()
            .cmp(&other.0.len())
            .then_with(|| self.0.iter().rev().cmp(other.0.iter().rev()))
    }

    /// `self -= other`, for `self >= other`.
    fn sub(&mut self, other: &Big) {
        let mut borrow = 0i64;
        for (i, x) in self.0.iter_mut().enumerate() {
            let t = i64::from(*x) - i64::from(other.0.get(i).copied().unwrap_or(0)) - borrow;
            *x = t as u32;
            borrow = i64::from(t < 0);
        }
        self.trim();
    }

    /// The quotient `self / den`, known to be below `2^(qbits + 1)`, leaving
    /// the remainder in `self`.
    fn div_rem(&mut self, den: &Big, qbits: u32) -> u64 {
        let mut d = den.clone();
        d.shl(qbits);
        let mut q = 0u64;
        for _ in 0..=qbits {
            q <<= 1;
            if self.cmp(&d) != Ordering::Less {
                self.sub(&d);
                q |= 1;
            }
            d.shr1();
        }
        q
    }
}
