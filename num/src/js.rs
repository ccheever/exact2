//! A number as JavaScript prints it: `Number.prototype.toString()`
//! (ECMA-262 Number::toString), which the runner's `toString`, template
//! interpolation and the compiler's constant folding all print.
//!
//! The digits are the shortest that read back as the value, as [`Shortest`]
//! finds them. Of two equally near shortest forms ECMA-262 takes the one
//! whose last digit is even, where core's printer may take the other
//! (`1469358899709795.25` prints `…795.2` in JavaScript, `…795.3` in Rust);
//! that choice is made here, exactly. Found by the Lean semantics'
//! differential run (semantics/README.md).

use crate::{parse_f64, Exponent, Fixed};

/// [`push_js`] into a new string.
pub fn js(n: f64) -> String {
    let mut out = String::new();
    push_js(n, &mut out);
    out
}

/// `n` as JavaScript's `String(n)` prints it, appended to `out`.
pub fn push_js(n: f64, out: &mut String) {
    if n.is_nan() {
        out.push_str("NaN");
        return;
    }
    if n == 0.0 {
        out.push('0');
        return;
    }
    if n.is_infinite() {
        out.push_str(if n > 0.0 { "Infinity" } else { "-Infinity" });
        return;
    }
    // The common case without the formatter: an app's numbers are shown on
    // boot. Fewer than 16 digits cannot be one of a tie.
    if (1e-6..1e21).contains(&n.abs()) {
        let start = out.len();
        crate::push_text!(out, "{}", crate::Shortest(n));
        if out[start..].bytes().filter(u8::is_ascii_digit).count() < 16 {
            return;
        }
        out.truncate(start);
    }
    let scientific = Exponent(n.abs()).to_string();
    let (mantissa, exponent) = scientific.split_once('e').expect("scientific notation");
    let exponent: i32 = exponent.parse().expect("decimal exponent");
    let mut digits: String = mantissa.chars().filter(char::is_ascii_digit).collect();
    if digits.len() >= 16 {
        if let Some(even) = even_of_tie(n.abs(), &digits, exponent) {
            digits = even;
        }
    }
    if n < 0.0 {
        out.push('-');
    }
    layout(&digits, exponent, out);
}

/// ECMA-262's layout of `digits × 10^(exponent − len + 1)`.
fn layout(digits: &str, exponent: i32, out: &mut String) {
    let k = digits.len() as i32;
    let point = exponent + 1;
    if k <= point && point <= 21 {
        out.push_str(digits);
        out.extend(std::iter::repeat_n('0', (point - k) as usize));
    } else if 0 < point && point <= 21 {
        out.push_str(&digits[..point as usize]);
        out.push('.');
        out.push_str(&digits[point as usize..]);
    } else if -6 < point && point <= 0 {
        out.push_str("0.");
        out.extend(std::iter::repeat_n('0', (-point) as usize));
        out.push_str(digits);
    } else {
        out.push_str(&digits[..1]);
        if k > 1 {
            out.push('.');
            out.push_str(&digits[1..]);
        }
        out.push('e');
        out.push(if exponent < 0 { '-' } else { '+' });
        out.push_str(&exponent.unsigned_abs().to_string());
    }
}

/// `digits` plus `delta` (±1) at its last place, when the length holds.
fn step(digits: &str, up: bool) -> Option<String> {
    let mut d: Vec<u8> = digits.bytes().collect();
    let mut i = d.len();
    loop {
        if i == 0 {
            return None;
        }
        i -= 1;
        match (up, d[i]) {
            (true, b'9') => d[i] = b'0',
            (true, c) => {
                d[i] = c + 1;
                break;
            }
            (false, b'0') => d[i] = b'9',
            (false, c) => {
                d[i] = c - 1;
                break;
            }
        }
    }
    (d[0] != b'0').then(|| String::from_utf8(d).expect("ascii"))
}

/// The exact decimal value of a positive double as (significant digits
/// without trailing zeros, exponent of the first).
fn exact(v: f64) -> (String, i32) {
    // 1074 fractional digits hold every double exactly.
    let text = Fixed(v, 1100).to_string();
    let (int, frac) = text.split_once('.').unwrap_or((&text, ""));
    let int = int.trim_start_matches('0');
    if int.is_empty() {
        let lead = frac.len() - frac.trim_start_matches('0').len();
        let digits = frac.trim_start_matches('0').trim_end_matches('0');
        (digits.to_string(), -(lead as i32) - 1)
    } else {
        let all = format!("{int}{frac}");
        (all.trim_end_matches('0').to_string(), int.len() as i32 - 1)
    }
}

/// The even shortest form, when `digits` (odd) and a neighbour of the same
/// length both read back as `v` and `v` lies exactly halfway between them.
fn even_of_tie(v: f64, digits: &str, exponent: i32) -> Option<String> {
    if digits.as_bytes()[digits.len() - 1].is_multiple_of(2) {
        return None;
    }
    for up in [false, true] {
        let Some(other) = step(digits, up) else {
            continue;
        };
        let text = format!("{}.{}e{exponent}", &other[..1], &other[1..]);
        if parse_f64(&text) != Ok(v) {
            continue;
        }
        // The midpoint: the lower of the two followed by a 5.
        let lower = if up { digits } else { &other };
        let mid = format!("{lower}5");
        if exact(v) == (mid, exponent) {
            return Some(other);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::js;

    #[test]
    fn prints_as_javascript_does() {
        for (n, text) in [
            (0.0, "0"),
            (-0.0, "0"),
            (f64::NAN, "NaN"),
            (f64::INFINITY, "Infinity"),
            (f64::NEG_INFINITY, "-Infinity"),
            (0.1 + 0.2, "0.30000000000000004"),
            (1e21, "1e+21"),
            (1e20, "100000000000000000000"),
            (1e-7, "1e-7"),
            (0.000001, "0.000001"),
            (5e-324, "5e-324"),
            (-2.5, "-2.5"),
            (123456789012345680000.0, "123456789012345680000"),
            (f64::MAX, "1.7976931348623157e+308"),
        ] {
            assert_eq!(js(n), text, "{n:e}");
        }
    }

    #[test]
    fn a_tie_takes_the_even_shortest_form() {
        for (bits, text) in [
            (0x4314_e17f_1d0f_1d8d_u64, "1469358899709795.2"),
            (0x42bc_bf5b_965b_6990, "31608200911721.562"),
            (0xc2e6_a575_0d63_7d24, "-199199113812969.12"),
        ] {
            assert_eq!(js(f64::from_bits(bits)), text);
        }
    }
}
