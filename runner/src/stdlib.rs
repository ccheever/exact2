//! The roster's implementations — once, here, deterministic.
//!
//! @ref LLP 1004 D4 (formatting is a roster entry, added by fixture)
//!
//! Every entry the plan format's `stdlib` table names has exactly one body
//! here or in the router conversion module. Time formatting is UTC and locale-free by design: the v1 app
//! is a schedule board, and a deterministic string is what the corpus and the
//! agent compare.

use exact_plan::{Plan, Stdlib, Value};
use std::rc::Rc;

/// Call `f` with `args` (already arity-checked). `None` on a type mismatch.
pub fn call(
    f: Stdlib,
    args: &[Value],
    now_ms: f64,
    plan: &Plan,
    router: Option<&dyn crate::runner::Routing>,
) -> Option<Value> {
    let num = |i: usize| args.get(i).and_then(Value::as_number);
    Some(match f {
        // @ref LLP 1038 D3/D9 — pure verbs and typed reads over the plan shapes.
        Stdlib::Open
        | Stdlib::Push
        | Stdlib::Replace
        | Stdlib::Back
        | Stdlib::Select
        | Stdlib::Go
        | Stdlib::Stack
        | Stdlib::Top
        | Stdlib::Depth
        | Stdlib::Params
        | Stdlib::SearchParam => {
            return router?.call(plan, f, args);
        }
        Stdlib::EncodeURIComponent => {
            Value::str(&exact_route::encode_uri_component(args.first()?.as_str()?))
        }
        Stdlib::EncodeRouteSegment => {
            match exact_route::encode_route_segment(args.first()?.as_str()?) {
                Ok(encoded) => Value::str(&encoded),
                Err(error) => {
                    router?.refuse("path", &error.message);
                    return None;
                }
            }
        }
        Stdlib::Contains => Value::Bool(args.first()?.as_str()?.contains(args.get(1)?.as_str()?)),
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
                format!("{} mi", exact_num::Fixed((miles * 10.0).round() / 10.0, 1))
            })
        }
        Stdlib::FormatWalk => {
            let minutes = (num(0)? / 80.0).ceil().max(1.0);
            Value::str(&format!("{} min walk", minutes as i64))
        }
        Stdlib::Length => Value::Number(match args.first()? {
            Value::List(items) => items.len() as f64,
            // The web's String.length (and `maxlength`): UTF-16 code units.
            Value::Str(s) => s.encode_utf16().count() as f64,
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
        Stdlib::First => match args.first()? {
            Value::List(items) => items
                .first()
                .cloned()
                .map_or(Value::Option(None), Value::some),
            _ => return None,
        },
        Stdlib::Floor => Value::Number(num(0)?.floor()),
        Stdlib::Max => Value::Number(num(0)?.max(num(1)?)),
        Stdlib::Min => Value::Number(num(0)?.min(num(1)?)),
    })
}

/// A native module's props (LLP 1024 D1): `pairs` alternate key and value,
/// keys already in canonical order. Every value is carried as a string (a
/// number as JavaScript prints it); an option's `none` leaves its key out.
/// Escaping is JSON's, deterministic: `"`, `\\` and C0 controls only.
pub fn native_props(pairs: &[Value]) -> Option<String> {
    fn quote(s: &str, out: &mut String) {
        out.push('"');
        for c in s.chars() {
            match c {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
                c => out.push(c),
            }
        }
        out.push('"');
    }
    let mut out = String::from("{");
    for pair in pairs.chunks(2) {
        let [key, value] = pair else { return None };
        let value = match value {
            Value::Option(None) => continue,
            Value::Option(Some(inner)) => inner.as_ref(),
            v => v,
        };
        let text = match value {
            Value::Str(s) => s.to_string(),
            Value::Number(n) => format_number(*n),
            Value::Bool(b) => b.to_string(),
            _ => return None,
        };
        if out.len() > 1 {
            out.push(',');
        }
        quote(key.as_str()?, &mut out);
        out.push(':');
        quote(&text, &mut out);
    }
    out.push('}');
    Some(out)
}

/// JavaScript's decimal/exponent boundaries over Rust's shortest-round-trip printer.
pub fn format_number(n: f64) -> String {
    if n == 0.0 {
        "0".into()
    } else if n.is_finite() && (n.abs() >= 1e21 || n.abs() < 1e-6) {
        let scientific = exact_num::Exponent(n).to_string();
        let (mantissa, exponent) = scientific.split_once('e').expect("scientific notation");
        let exponent: i32 = exponent.parse().expect("decimal exponent");
        format!("{mantissa}e{exponent:+}")
    } else {
        // Written without the formatter: an app's numbers are shown on boot.
        exact_num::text!("{}", exact_num::Shortest(n))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_string_uses_javascript_decimal_and_exponent_boundaries() {
        let plan = exact_plan::builder::PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1)
            .finish()
            .unwrap();
        for (value, expected) in [
            (0.0, "0"),
            (-0.0, "0"),
            (1e-7, "1e-7"),
            (-1e-7, "-1e-7"),
            (1e-6, "0.000001"),
            (-1e-6, "-0.000001"),
            (1e20, "100000000000000000000"),
            (1e21, "1e+21"),
            (-1e21, "-1e+21"),
            (1.234e22, "1.234e+22"),
            (f64::MIN_POSITIVE, "2.2250738585072014e-308"),
        ] {
            assert_eq!(
                call(Stdlib::ToString, &[Value::Number(value)], 0.0, &plan, None),
                Some(Value::str(expected)),
                "{value}"
            );
        }
    }

    #[test]
    fn a_number_in_the_decimal_range_is_shortest_text() {
        let mut state = 0x9e37_79b9_7f4a_7c15u64;
        let mut next = || {
            state ^= state >> 12;
            state ^= state << 25;
            state ^= state >> 27;
            state.wrapping_mul(0x2545_f491_4f6c_dd1d)
        };
        let mut values = vec![1.0, -1.5, 0.1 + 0.2, 1e-6, 123_456.789, 9.999e20];
        values.extend((0..10_000).map(|_| (next() % 10_000_000) as f64 / 1000.0));
        values.extend((0..10_000).map(|_| f64::from_bits(next())));
        for n in values {
            if n != 0.0 && n.is_finite() && (1e-6..1e21).contains(&n.abs()) {
                assert_eq!(format_number(n), exact_num::Shortest(n).to_string());
            }
        }
    }

    #[test]
    fn length_counts_utf16_code_units_as_the_web_does() {
        let plan = exact_plan::builder::PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1)
            .finish()
            .unwrap();
        for (text, expected) in [
            ("", 0.0),
            ("abc", 3.0),
            ("é", 1.0),
            ("😀", 2.0),
            ("a👍🏽", 5.0),
        ] {
            assert_eq!(
                call(Stdlib::Length, &[Value::str(text)], 0.0, &plan, None),
                Some(Value::Number(expected)),
                "{text}"
            );
        }
    }
}
