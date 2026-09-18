//! One shortest-round-trip conversion for diagnostics and JSON. Decimal expansion
//! keeps the previous JSON notation (and four-place agent rounding) intact.
use std::fmt::{self, Write};

/// Display a shortest float in decimal notation, without a redundant `.0`.
pub struct Float<T>(pub T);
impl<T: ryu::Float + Copy + Into<f64>> fmt::Display for Float<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        shortest(f, self.0, false)
    }
}

pub(crate) fn shortest<T: ryu::Float + Copy + Into<f64>>(
    out: &mut impl Write,
    n: T,
    debug: bool,
) -> fmt::Result {
    let mut buffer = ryu::Buffer::new();
    let s = buffer.format(n);
    let mut tie = [0u8; 24];
    let s = preserve_tie(s, n.into(), &mut tie);
    // Rust Debug switches notation at 1e-4 / 1e16; ryu's display thresholds
    // differ. Normalize notation without changing the shortest significand.
    let unsigned = s.strip_prefix('-').unwrap_or(s);
    if debug && !s.contains('e') && unsigned.bytes().any(|b| (b'1'..=b'9').contains(&b)) {
        let point = unsigned.find('.').unwrap_or(unsigned.len());
        let first = unsigned
            .bytes()
            .position(|b| (b'1'..=b'9').contains(&b))
            .unwrap();
        let exponent = point as i32 - first as i32 - i32::from(first < point);
        if !(-4..16).contains(&exponent) {
            if s.starts_with('-') {
                out.write_char('-')?;
            }
            let digits = unsigned[first..]
                .trim_end_matches('0')
                .trim_end_matches('.');
            let mut chars = digits.chars().filter(|c| *c != '.');
            out.write_char(chars.next().unwrap())?;
            if let Some(c) = chars.next() {
                out.write_char('.')?;
                out.write_char(c)?;
                for c in chars {
                    out.write_char(c)?;
                }
            }
            return write!(out, "e{exponent}");
        }
    }
    if let Some((mantissa, exponent)) = s.split_once('e') {
        let exponent: i32 = exponent.parse().expect("ryu exponent");
        if debug && !(-4..16).contains(&exponent) {
            return out.write_str(s);
        }
        let (sign, mantissa) = mantissa
            .strip_prefix('-')
            .map_or(("", mantissa), |s| ("-", s));
        out.write_str(sign)?;
        let mut digits = [0u8; 24];
        let mut len = 0;
        for b in mantissa.bytes().filter(|b| *b != b'.') {
            digits[len] = b;
            len += 1;
        }
        let point = 1 + exponent;
        if point <= 0 {
            out.write_str("0.")?;
            for _ in 0..-point {
                out.write_char('0')?;
            }
        }
        for (i, &digit) in digits[..len].iter().enumerate() {
            if i != 0 && i as i32 == point {
                out.write_char('.')?;
            }
            out.write_char(digit as char)?;
        }
        for _ in len as i32..point {
            out.write_char('0')?;
        }
        if debug && point >= len as i32 {
            out.write_str(".0")?;
        }
        Ok(())
    } else {
        out.write_str(if debug {
            s
        } else {
            s.strip_suffix(".0").unwrap_or(s)
        })
    }
}

// std's shortest formatter chooses the upper magnitude at an exactly equidistant
// decimal tie; Ryu chooses an even last digit. Both round-trip, but pins keep std's
// spelling. Compare the decimal midpoint and IEEE value as exact odd*2^exponent
// rationals, never through a rounded decimal parse. Only short powers of five can
// match a binary significand, so u128 is sufficient even for f64 extremes.
fn preserve_tie<'a>(s: &'a str, n: f64, out: &'a mut [u8; 24]) -> &'a str {
    if !n.is_finite() || n == 0.0 {
        return s;
    }
    let (mantissa, exponent) = s.split_once('e').unwrap_or((s, "0"));
    let mut exponent: i32 = exponent.parse().unwrap();
    let mut digits = 0u64;
    let mut fractional = false;
    let mut last = 0;
    for (i, b) in mantissa.bytes().enumerate() {
        if b == b'.' {
            fractional = true;
        }
        if b.is_ascii_digit() {
            digits = digits * 10 + u64::from(b - b'0');
            if fractional {
                exponent -= 1;
            }
            if b != b'0' {
                last = i;
            }
        }
    }
    while digits.is_multiple_of(10) {
        digits /= 10;
        exponent += 1;
    }
    if !digits.is_multiple_of(2) || !(-24..=23).contains(&exponent) {
        return s;
    }
    let mut odd = u128::from(digits) * 2 + 1;
    let five = 5u128.pow(exponent.unsigned_abs());
    if exponent < 0 {
        if !odd.is_multiple_of(five) {
            return s;
        }
        odd /= five;
    } else {
        odd *= five;
    }
    let bits = n.to_bits();
    let biased = ((bits >> 52) & 2047) as i32;
    let mut binary = bits & ((1 << 52) - 1);
    let mut power = if biased == 0 {
        -1074
    } else {
        biased - 1023 - 52
    };
    if biased != 0 {
        binary |= 1 << 52;
    }
    let zeros = binary.trailing_zeros();
    binary >>= zeros;
    power += zeros as i32;
    if odd != u128::from(binary) || exponent - 1 != power {
        return s;
    }
    out[..s.len()].copy_from_slice(s.as_bytes());
    out[last] += 1; // The lower tied digit was even, so this never carries.
    std::str::from_utf8(&out[..s.len()]).unwrap()
}

/// Existing fixed-place audio journal spelling, without core's float formatter.
/// f32's 24 significant bits make the small decimal scaling exact in f64.
pub(crate) struct Fixed(pub f32, pub u32);
impl fmt::Display for Fixed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let n = self.0;
        if !n.is_finite() {
            return write!(f, "{}", Float(n));
        }
        if n.is_sign_negative() {
            f.write_char('-')?;
        }
        let n = n.abs() as f64;
        let places = self.1;
        assert!(places <= 3);
        let scale = 10u64.pow(places);
        let (whole, fraction) = if n >= 8_388_608.0 {
            (n as u128, 0)
        } else {
            let scaled = (n * scale as f64).round_ties_even() as u64;
            ((scaled / scale) as u128, scaled % scale)
        };
        write!(f, "{whole}")?;
        if places != 0 {
            write!(f, ".{fraction:0width$}", width = places as usize)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn float_bits_round_trip_and_fixed_journal_matches() {
        let mut bits = 1u64;
        for _ in 0..100_000 {
            bits = bits.wrapping_mul(6364136223846793005).wrapping_add(1);
            let n = f32::from_bits(bits as u32);
            assert_eq!(Float(n).to_string(), format!("{n}"));
            if n.is_finite() {
                let s = Float(n).to_string();
                assert_eq!(s.parse::<f32>().unwrap().to_bits(), n.to_bits());
                let mut debug = String::new();
                shortest(&mut debug, n, true).unwrap();
                assert_eq!(debug, format!("{n:?}"));
                assert_eq!(Fixed(n, 2).to_string(), format!("{n:.2}"));
                assert_eq!(Fixed(n, 3).to_string(), format!("{n:.3}"));
            }
            let n = f64::from_bits(bits);
            assert_eq!(Float(n).to_string(), format!("{n}"));
            if n.is_finite() {
                assert_eq!(
                    Float(n).to_string().parse::<f64>().unwrap().to_bits(),
                    n.to_bits()
                );
                let mut debug = String::new();
                shortest(&mut debug, n, true).unwrap();
                assert_eq!(debug, format!("{n:?}"));
            }
        }
        for n in [-0.0, 0.0, f64::MIN_POSITIVE, f64::MAX, f64::from_bits(1)] {
            assert_eq!(
                Float(n).to_string().parse::<f64>().unwrap().to_bits(),
                n.to_bits()
            );
        }
    }
    #[test]
    fn std_notation_boundaries_specials_ties_and_rounded_agent_numbers() {
        macro_rules! compare {
            ($ty:ty) => {
                for n in [
                    0.0,
                    -0.0,
                    <$ty>::NAN,
                    -<$ty>::NAN,
                    <$ty>::INFINITY,
                    <$ty>::NEG_INFINITY,
                    <$ty>::from_bits(1),
                    -<$ty>::from_bits(1),
                    <$ty>::MIN_POSITIVE,
                    <$ty>::MAX,
                    1e-5,
                    1e-4,
                    1e15,
                    1e16,
                    1e17,
                    1.23445,
                    -1.23445,
                    0.00005,
                    -0.00005,
                    1.03125,
                ] {
                    assert_eq!(Float(n).to_string(), format!("{n}"));
                    let mut debug = String::new();
                    shortest(&mut debug, n, true).unwrap();
                    assert_eq!(debug, format!("{n:?}"));
                    let mut encoder = crate::json::Encoder::rounded();
                    crate::Data::write(&n, &mut encoder);
                    let expected = if !n.is_finite() {
                        "null".into()
                    } else {
                        format!("{}", crate::json::rounded(n as f64))
                    };
                    if n.is_finite() {
                        assert_eq!(encoder.finish().unwrap(), expected);
                    } else {
                        assert_eq!(
                            encoder.finish().unwrap_err().to_string(),
                            "non-finite number is not JSON"
                        );
                    }
                    assert_eq!(
                        crate::values::value_json(&crate::Value::Number(n as f64), true),
                        expected
                    );
                }
            };
        }
        compare!(f32);
        compare!(f64);
        // Display stays decimal; Debug alone uses this exponent window.
        assert_eq!(Float(1e-5).to_string(), "0.00001");
        assert_eq!(format!("{:?}", 1e-5), "1e-5");
    }
}
