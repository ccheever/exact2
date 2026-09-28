//! The roster's implementations — once, here, deterministic.
//!
//! @ref LLP 1004 D4 (formatting is a roster entry, added by fixture)
//!
//! Every entry the plan format's `stdlib` table names has exactly one body
//! here, in the router conversion module, or in the linked `format` module
//! (LLP 1054.000.003 D8). Formatting is `en-US` at a fixed UTC offset the
//! call names, by design: a deterministic string is what the corpus and the
//! agent compare, and the web's `Intl` is its oracle.

use exact_plan::{Plan, Stdlib, Value};
use std::rc::Rc;

/// Why a standard function could not produce its value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallError {
    /// The arguments do not fit the function.
    TypeMismatch,
    /// Interpolation would exceed the VM's string budget.
    StringTooLong,
}

/// Call `f` with `args` (already arity-checked).
pub fn call(
    f: Stdlib,
    args: &[Value],
    now_ms: f64,
    plan: &Plan,
    router: Option<&dyn crate::runner::Routing>,
    format: crate::runner::FormatLink,
) -> Result<Value, CallError> {
    if f == Stdlib::T {
        // The compiler proved the key and placeholder names. Check the
        // expanded byte length before allocating the translated string.
        let text = args
            .first()
            .and_then(Value::as_str)
            .zip(args.get(1).and_then(Value::as_str))
            .and_then(|(locale, key)| plan.localized(locale, key))
            .ok_or(CallError::TypeMismatch)?;
        let Some(Value::List(pairs)) = args.get(2) else {
            return Err(CallError::TypeMismatch);
        };
        return exact_plan::strings::fill(
            text,
            |name| {
                pairs
                    .chunks_exact(2)
                    .find(|pair| pair[0].as_str() == Some(name))
                    .and_then(|pair| pair[1].as_str())
            },
            crate::vm::MAX_STRING,
        )
        .map(|s| Value::Str(Rc::from(s)))
        .ok_or(CallError::StringTooLong);
    }
    call_value(f, args, now_ms, plan, router, format).ok_or(CallError::TypeMismatch)
}

fn call_value(
    f: Stdlib,
    args: &[Value],
    now_ms: f64,
    plan: &Plan,
    router: Option<&dyn crate::runner::Routing>,
    format: crate::runner::FormatLink,
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
        Stdlib::Trim => match args.first()? {
            Value::Str(s) => {
                let trimmed = s.trim_matches(is_js_space);
                if trimmed.len() == s.len() {
                    Value::Str(Rc::clone(s))
                } else {
                    Value::str(trimmed)
                }
            }
            _ => return None,
        },
        Stdlib::Now => Value::Number(now_ms),
        Stdlib::FormatTime => match args.get(2)?.as_str()? {
            "short" => format_time(num(0)?, num(1)?),
            _ => return None,
        },
        // @ref LLP 1054.000.003 D8 — the linked capability, or a trap.
        Stdlib::FormatDate | Stdlib::FormatNumber => return format?(f, args),
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
            Value::Number(n) => assembled(|s| push_number(*n, s)),
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
        // @ref LLP 1017.003 D5 — opcodes with a callback body, never a
        // call; `join` is the VM's, which bounds the string it makes.
        Stdlib::Map | Stdlib::Filter | Stdlib::Join => return None,
        Stdlib::Floor => Value::Number(num(0)?.floor()),
        Stdlib::Max => Value::Number(num(0)?.max(num(1)?)),
        Stdlib::Min => Value::Number(num(0)?.min(num(1)?)),
        Stdlib::T => unreachable!("interpolation is bounded by call"),
    })
}

/// Why `join` refused.
#[derive(Debug, PartialEq)]
pub enum JoinError {
    /// Not a list and a string, or an item that is not a string, number or bool.
    Type,
    /// The result would pass the limit.
    TooLong,
}

/// `join(list, separator)` (LLP 1017.003 D4): the web's `Array.prototype.join`
/// over strings, numbers and bools, each printed as `toString` prints it, in
/// at most `limit` bytes.
pub fn join(args: &[Value], limit: usize) -> Result<Value, JoinError> {
    let [Value::List(items), Value::Str(separator)] = args else {
        return Err(JoinError::Type);
    };
    if let [Value::Str(only)] = &items[..] {
        return Ok(Value::Str(Rc::clone(only)));
    }
    let mut result = Ok(());
    let v = assembled(|out| {
        for (i, item) in items.iter().enumerate() {
            if i > 0 {
                out.push_str(separator);
            }
            match item {
                Value::Str(s) => out.push_str(s),
                Value::Number(n) => push_number(*n, out),
                Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
                _ => {
                    result = Err(JoinError::Type);
                    return;
                }
            }
            if out.len() > limit {
                result = Err(JoinError::TooLong);
                return;
            }
        }
    });
    result.map(|()| v)
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
    let mut out = String::new();
    push_number(n, &mut out);
    out
}

/// [`format_number`], appended to `out`.
pub fn push_number(n: f64, out: &mut String) {
    if n == 0.0 {
        out.push('0');
    } else if n.is_finite() && (n.abs() >= 1e21 || n.abs() < 1e-6) {
        let scientific = exact_num::Exponent(n).to_string();
        let (mantissa, exponent) = scientific.split_once('e').expect("scientific notation");
        let exponent: i32 = exponent.parse().expect("decimal exponent");
        out.push_str(&format!("{mantissa}e{exponent:+}"));
    } else {
        // Written without the formatter: an app's numbers are shown on boot.
        exact_num::push_text!(out, "{}", exact_num::Shortest(n));
    }
}

thread_local! {
    /// Text a value is assembled in before it is copied, once, into its own
    /// shared string: a string value costs one allocation, not two.
    static SCRATCH: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}

/// A string value made by `build` in the scratch text.
pub fn assembled(build: impl FnOnce(&mut String)) -> Value {
    SCRATCH.with(|s| match s.try_borrow_mut() {
        Ok(mut s) => {
            s.clear();
            build(&mut s);
            let v = Value::str(&s);
            // A long one does not keep its capacity.
            if s.capacity() > 4096 {
                *s = String::new();
            }
            v
        }
        Err(_) => {
            let mut s = String::new();
            build(&mut s);
            Value::str(&s)
        }
    })
}

/// ECMA-262's WhiteSpace and LineTerminator: what `String.prototype.trim`
/// strips. Not Rust's `White_Space`, which adds U+0085 and drops U+FEFF.
/// @ref LLP 1054.000.005 D1
fn is_js_space(c: char) -> bool {
    matches!(
        c,
        '\u{9}'..='\u{d}'
            | ' '
            | '\u{a0}'
            | '\u{1680}'
            | '\u{2000}'..='\u{200a}'
            | '\u{2028}'
            | '\u{2029}'
            | '\u{202f}'
            | '\u{205f}'
            | '\u{3000}'
            | '\u{feff}'
    )
}

/// The first and last wall times a date or time is formatted at:
/// 0001-01-01T00:00 and 9999-12-31T23:59:59.999 (LLP 1054.000.003 D7).
const FIRST_WALL_MS: f64 = -62_135_596_800_000.0;
const LAST_WALL_MS: f64 = 253_402_300_799_999.0;

/// The wall time of `epoch_ms` at `utc_offset` minutes east, in the one
/// order LLP 1054.000.003 D7 names:
/// 1. a non-finite argument, or an offset past ±18 h, is invalid;
/// 2. the instant is clipped first, as ECMA-262's TimeClip does (truncated
///    toward zero, so `-0.5` is the epoch), as `new Date(epochMs)` is;
/// 3. then shifted by `utc_offset × 60,000` ms, as a fixed-offset
///    `timeZone` shifts it;
/// 4. and the shifted wall time must fall in years 1–9999.
///
/// There is no zero sentinel: `0` is 1970-01-01T00:00Z, as it is to `Intl`.
/// `None` is invalid, which every entry prints as `""`.
pub(crate) fn wall_ms(epoch_ms: f64, utc_offset: f64) -> Option<f64> {
    if !epoch_ms.is_finite() || !utc_offset.is_finite() || utc_offset.abs() > 1080.0 {
        return None;
    }
    let wall = epoch_ms.trunc() + utc_offset * 60_000.0;
    (FIRST_WALL_MS..=LAST_WALL_MS)
        .contains(&wall)
        .then_some(wall)
}

/// `formatTime(epochMs, utcOffset, "short")`: `h:mm AM`, as
/// `Intl.DateTimeFormat("en-US", { timeStyle: "short" })` prints it in a
/// fixed-offset zone, with U+0020 before the day period
/// (@ref LLP 1054.000.003 D1, D7).
pub fn format_time(epoch_ms: f64, utc_offset: f64) -> Value {
    let Some(wall) = wall_ms(epoch_ms, utc_offset) else {
        return Value::str("");
    };
    let minutes = (wall.rem_euclid(86_400_000.0) / 60_000.0).floor() as u32;
    let (hours, minutes) = (minutes / 60, minutes % 60);
    let (h12, suffix) = match hours {
        0 => (12, " AM"),
        1..=11 => (hours, " AM"),
        12 => (12, " PM"),
        _ => (hours - 12, " PM"),
    };
    let mut out = String::with_capacity(8);
    push_number(f64::from(h12), &mut out);
    out.push(':');
    out.push(char::from(b'0' + (minutes / 10) as u8));
    out.push(char::from(b'0' + (minutes % 10) as u8));
    out.push_str(suffix);
    Value::str(&out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_props_carry_strings_drop_none_and_escape_as_json() {
        let pairs = [
            Value::str("a"),
            Value::Number(1.5),
            Value::str("b"),
            Value::Option(None),
            Value::str("c"),
            Value::some(Value::Bool(true)),
            Value::str("d"),
            Value::str("q\"\\\t\u{1}"),
        ];
        assert_eq!(
            native_props(&pairs).unwrap(),
            r#"{"a":"1.5","c":"true","d":"q\"\\\t\u0001"}"#
        );
        assert_eq!(native_props(&[Value::str("x"), Value::list(vec![])]), None);
    }

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
                call(
                    Stdlib::ToString,
                    &[Value::Number(value)],
                    0.0,
                    &plan,
                    None,
                    None
                ),
                Ok(Value::str(expected)),
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
                call(Stdlib::Length, &[Value::str(text)], 0.0, &plan, None, None),
                Ok(Value::Number(expected)),
                "{text}"
            );
        }
    }
    #[test]
    fn trim_strips_what_javascript_strips() {
        let plan = exact_plan::builder::PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1)
            .finish()
            .unwrap();
        let trim = |text: &str| call(Stdlib::Trim, &[Value::str(text)], 0.0, &plan, None, None);
        // Every code point `(c + "x").trim() === "x"` holds for, from Bun.
        let js = "\u{9}\u{a}\u{b}\u{c}\u{d}\u{20}\u{a0}\u{1680}\u{2000}\u{2001}\u{2002}\u{2003}\u{2004}\u{2005}\u{2006}\u{2007}\u{2008}\u{2009}\u{200a}\u{2028}\u{2029}\u{202f}\u{205f}\u{3000}\u{feff}";
        assert_eq!(js.chars().count(), 25);
        for c in js.chars() {
            assert_eq!(
                trim(&format!("{c}x{c}")),
                Ok(Value::str("x")),
                "{:x}",
                c as u32
            );
        }
        assert_eq!(trim(js), Ok(Value::str("")));
        for (text, expected) in [
            ("", ""),
            (" a b ", "a b"),
            ("\u{85}x\u{85}", "\u{85}x\u{85}"),
            ("\u{200b}x\u{180e}", "\u{200b}x\u{180e}"),
            ("\u{feff}😀\u{3000}", "😀"),
        ] {
            assert_eq!(trim(text), Ok(Value::str(expected)), "{text:?}");
        }
        // Nothing to strip: the same string, not a copy.
        let s: Rc<str> = Rc::from("kept");
        let Ok(Value::Str(out)) = call(
            Stdlib::Trim,
            &[Value::Str(Rc::clone(&s))],
            0.0,
            &plan,
            None,
            None,
        ) else {
            panic!("trim answers a string");
        };
        assert!(Rc::ptr_eq(&s, &out));
    }
}
