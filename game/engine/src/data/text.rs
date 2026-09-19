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
    if !debug && !s.contains('e') {
        return out.write_str(s.strip_suffix(".0").unwrap_or(s));
    }
    notation(out, s, debug)
}

fn notation(out: &mut impl Write, s: &str, debug: bool) -> fmt::Result {
    // Rust Debug switches notation at 1e-4 / 1e16; ryu's display thresholds differ.
    let unsigned = s.strip_prefix('-').unwrap_or(s);
    if debug && !s.contains('e') {
        if let Some(first) = unsigned.bytes().position(|b| (b'1'..=b'9').contains(&b)) {
            let point = unsigned.find('.').unwrap_or(unsigned.len());
            let exponent = point as i32 - first as i32 - i32::from(first < point);
            if !(-4..16).contains(&exponent) {
                if s.starts_with('-') {
                    out.write_char('-')?;
                }
                let digits = unsigned[first..]
                    .trim_end_matches('0')
                    .trim_end_matches('.');
                out.write_str(&digits[..1])?;
                let (whole, fraction) = digits[1..].split_once('.').unwrap_or((&digits[1..], ""));
                if !whole.is_empty() || !fraction.is_empty() {
                    out.write_char('.')?;
                    out.write_str(whole)?;
                    out.write_str(fraction)?;
                }
                return write!(out, "e{exponent}");
            }
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
        let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
        let point = 1 + exponent;
        if point <= 0 {
            out.write_str("0.")?;
            zeros(out, -point as usize)?;
            out.write_str(whole)?;
            out.write_str(fraction)
        } else {
            out.write_str(whole)?;
            let prefix = (point as usize - 1).min(fraction.len());
            out.write_str(&fraction[..prefix])?;
            if prefix < fraction.len() {
                out.write_char('.')?;
                out.write_str(&fraction[prefix..])?;
            } else {
                zeros(out, point as usize - 1 - prefix)?;
                if debug {
                    out.write_str(".0")?;
                }
            }
            Ok(())
        }
    } else {
        out.write_str(if debug {
            s
        } else {
            s.strip_suffix(".0").unwrap_or(s)
        })
    }
}
fn zeros(out: &mut impl Write, count: usize) -> fmt::Result {
    const ZEROS: &str = "0000000000000000000000000000000000000000000000000000000000000000";
    for _ in 0..count / ZEROS.len() {
        out.write_str(ZEROS)?;
    }
    out.write_str(&ZEROS[..count % ZEROS.len()])
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
            if n.is_finite() {
                let s = Float(n).to_string();
                assert_eq!(s.parse::<f32>().unwrap().to_bits(), n.to_bits());
                let mut debug = String::new();
                shortest(&mut debug, n, true).unwrap();
                assert_eq!(debug.parse::<f32>().unwrap().to_bits(), n.to_bits());
                assert_eq!(Fixed(n, 2).to_string(), format!("{n:.2}"));
                assert_eq!(Fixed(n, 3).to_string(), format!("{n:.3}"));
            }
            let n = f64::from_bits(bits);
            if n.is_finite() {
                assert_eq!(
                    Float(n).to_string().parse::<f64>().unwrap().to_bits(),
                    n.to_bits()
                );
                let mut debug = String::new();
                shortest(&mut debug, n, true).unwrap();
                assert_eq!(debug.parse::<f64>().unwrap().to_bits(), n.to_bits());
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
    fn notation_boundaries_specials_and_rounded_agent_numbers() {
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
        for (n, expected) in [
            (0., "0.0"),
            (-0., "-0.0"),
            (1e-5, "1e-5"),
            (1e-4, "0.0001"),
            (1e16, "1e16"),
        ] {
            let mut out = String::new();
            shortest(&mut out, n, true).unwrap();
            assert_eq!(out, expected);
        }
        // Both decimal endings round back to this f32; Ryu's even ending wins.
        let tied = f32::from_bits(0x4980_0002); // exactly 1,048,576.25
        assert_eq!(Float(tied).to_string(), "1048576.2");
    }
}
