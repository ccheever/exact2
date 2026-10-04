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
    geometry: Option<&crate::geometry::GeometryEnv<'_>>,
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
        .map(|s| Value::str(&s))
        .ok_or(CallError::StringTooLong);
    }
    if let Some(v) = text(f, args)? {
        return Ok(v);
    }
    let v = call_value(f, args, now_ms, plan, router, format, geometry)
        .ok_or(CallError::TypeMismatch)?;
    // An encoding is a string the expression builds, bounded like the rest
    // (LLP 1090 D6), checked after the route segment's empty and dot refusal.
    if matches!(f, Stdlib::EncodeURIComponent | Stdlib::EncodeRouteSegment)
        && v.as_str().is_some_and(|s| s.len() > crate::vm::MAX_STRING)
    {
        return Err(CallError::StringTooLong);
    }
    Ok(v)
}

/// The string functions that build a string (LLP 1088 D2), each bounded by
/// `MAX_STRING` alone, as one call counted as its whole output; `None` for
/// every other entry.
fn text(f: Stdlib, args: &[Value]) -> Result<Option<Value>, CallError> {
    use crate::{strings, vm::MAX_STRING};
    let s = |i: usize| {
        args.get(i)
            .and_then(Value::as_str)
            .ok_or(CallError::TypeMismatch)
    };
    let n = |i: usize| {
        args.get(i)
            .and_then(Value::as_number)
            .ok_or(CallError::TypeMismatch)
    };
    Ok(Some(match f {
        Stdlib::Slice => Value::str(&strings::slice(s(0)?, n(1)?, n(2)?)),
        Stdlib::ReplaceAll => Value::str(
            &strings::replace_all(s(0)?, s(1)?, s(2)?, MAX_STRING)
                .map_err(|_| CallError::StringTooLong)?,
        ),
        // The case tables through `text-transform`'s link, which a web
        // artifact holds only when its plan uses them (LLP 1047 D6 refuses
        // one that does not at boot); unlinked, a trap.
        Stdlib::ToLowerCase => {
            let lower = exact_kernel::linked_lowercase().ok_or(CallError::TypeMismatch)?;
            let out = lower(s(0)?, MAX_STRING).ok_or(CallError::StringTooLong)?;
            if out == s(0)? {
                args[0].clone()
            } else {
                Value::str(&out)
            }
        }
        _ => return Ok(None),
    }))
}

fn call_value(
    f: Stdlib,
    args: &[Value],
    now_ms: f64,
    plan: &Plan,
    router: Option<&dyn crate::runner::Routing>,
    format: crate::runner::FormatLink,
    geometry: Option<&crate::geometry::GeometryEnv<'_>>,
) -> Option<Value> {
    let num = |i: usize| args.get(i).and_then(Value::as_number);
    Some(match f {
        // @ref LLP 1051.000 D1/D2 — an action's reads, through the linked
        // geometry. The compiler admits them nowhere else, and a host refuses
        // a plan that reads geometry it doesn't link (LLP 1047 D6).
        Stdlib::Frame | Stdlib::Measure => return geometry?.read(plan, f, args),
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
        // The web's `String.prototype.includes`, `startsWith` and `endsWith`:
        // literal, case-sensitive, and the empty needle matches. A substring
        // of well-formed text is the same in UTF-8 and UTF-16, so Rust's
        // searches answer as JavaScript's do.
        Stdlib::Includes => Value::Bool(args.first()?.as_str()?.contains(args.get(1)?.as_str()?)),
        Stdlib::StartsWith => {
            Value::Bool(args.first()?.as_str()?.starts_with(args.get(1)?.as_str()?))
        }
        Stdlib::EndsWith => Value::Bool(args.first()?.as_str()?.ends_with(args.get(1)?.as_str()?)),
        Stdlib::Trim => {
            let v = args.first()?;
            let s = v.as_str()?;
            let trimmed = s.trim_matches(is_js_space);
            if trimmed.len() == s.len() {
                v.clone()
            } else {
                Value::str(trimmed)
            }
        }
        Stdlib::Now => Value::Number(now_ms),
        Stdlib::FormatTime => match args.get(2)?.as_str()? {
            "short" => format_time(num(0)?, num(1)?),
            _ => return None,
        },
        // @ref LLP 1054.000.003 D8 — the linked capability, or a trap.
        Stdlib::FormatDate | Stdlib::FormatNumber => return format?(f, args),
        Stdlib::Length => Value::Number(match args.first()? {
            Value::List(items) => items.len() as f64,
            // The web's String.length (and `maxlength`): UTF-16 code units.
            v => v.as_str()?.encode_utf16().count() as f64,
        }),
        Stdlib::IsEmpty => Value::Bool(match args.first()? {
            Value::List(items) => items.is_empty(),
            v => v.as_str()?.is_empty(),
        }),
        Stdlib::ToString => match args.first()? {
            Value::Number(n) => assembled(|s| push_number(*n, s)),
            Value::Bool(b) => Value::str(if *b { "true" } else { "false" }),
            v if v.is_str() => v.clone(),
            _ => return None,
        },
        Stdlib::First => match args.first()? {
            Value::List(items) => items
                .first()
                .cloned()
                .map_or(Value::Option(None), Value::some),
            _ => return None,
        },
        // `Array.prototype.at` (LLP 1006 §3): the index as ToIntegerOrInfinity
        // makes it (NaN is 0, fractions truncate toward zero), a negative one
        // counted from the end, and `none` where JavaScript answers undefined.
        Stdlib::At => match (args.first()?, args.get(1)?) {
            (Value::List(items), Value::Number(i)) => {
                let i = if i.is_nan() { 0.0 } else { i.trunc() };
                let len = items.len() as f64;
                let at = if i < 0.0 { len + i } else { i };
                if (0.0..len).contains(&at) {
                    Value::some(items[at as usize].clone())
                } else {
                    Value::Option(None)
                }
            }
            _ => return None,
        },
        // @ref LLP 1017.003 D5 — opcodes with a callback body, never a
        // call; `join` is the VM's, which bounds the string it makes.
        Stdlib::Map | Stdlib::Filter | Stdlib::Join => return None,
        Stdlib::Floor => Value::Number(num(0)?.floor()),
        Stdlib::Max => Value::Number(num(0)?.max(num(1)?)),
        Stdlib::Min => Value::Number(num(0)?.min(num(1)?)),
        Stdlib::T => unreachable!("interpolation is bounded by call"),
        Stdlib::Slice | Stdlib::ReplaceAll | Stdlib::ToLowerCase => {
            unreachable!("strings are bounded by call")
        }
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
    let [Value::List(items), separator] = args else {
        return Err(JoinError::Type);
    };
    let separator = separator.as_str().ok_or(JoinError::Type)?;
    if let [only] = &items[..] {
        if only.is_str() {
            return Ok(only.clone());
        }
    }
    let mut result = Ok(());
    let v = assembled(|out| {
        for (i, item) in items.iter().enumerate() {
            if i > 0 {
                out.push_str(separator);
            }
            match item {
                v if v.is_str() => out.push_str(v.as_str().unwrap_or_default()),
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
            v if v.is_str() => v.as_str().unwrap_or_default().to_string(),
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

/// [`format_number`], appended to `out`: JavaScript's `String(n)`
/// ([`exact_num::push_js`]).
pub fn push_number(n: f64, out: &mut String) {
    exact_num::push_js(n, out);
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
                // Rust's shortest text, but for a tie between two shortest
                // forms, where JavaScript takes the even one: as short, and
                // it reads back as `n`.
                let (ours, rust) = (format_number(n), exact_num::Shortest(n).to_string());
                if ours != rust {
                    assert_eq!(ours.len(), rust.len(), "{n:e}");
                    assert_eq!(ours.parse::<f64>(), Ok(n), "{n:e}");
                }
            }
        }
    }

    #[test]
    fn a_tie_between_two_shortest_forms_takes_the_even_one_as_javascript_does() {
        // Each is exactly halfway between two 17-digit decimals that both
        // read back as it; JavaScript (and the Lean semantics) print the even.
        for (bits, js) in [
            (0x4314_e17f_1d0f_1d8d_u64, "1469358899709795.2"),
            (0x42bc_bf5b_965b_6990, "31608200911721.562"),
            (0xc2e6_a575_0d63_7d24, "-199199113812969.12"),
        ] {
            assert_eq!(format_number(f64::from_bits(bits)), js);
        }
        assert_eq!(format_number(0.1 + 0.2), "0.30000000000000004");
        assert_eq!(
            format_number(123456789012345680000.0),
            "123456789012345680000"
        );
    }

    #[test]
    fn non_finite_numbers_print_as_javascript_prints_them() {
        assert_eq!(format_number(f64::INFINITY), "Infinity");
        assert_eq!(format_number(f64::NEG_INFINITY), "-Infinity");
        assert_eq!(format_number(f64::NAN), "NaN");
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
                call(
                    Stdlib::Length,
                    &[Value::str(text)],
                    0.0,
                    &plan,
                    None,
                    None,
                    None
                ),
                Ok(Value::Number(expected)),
                "{text}"
            );
        }
    }
    /// An encoding one byte past `MAX_STRING` traps, after the route
    /// segment's own refusal of `""` (LLP 1090 D6).
    #[test]
    fn the_encoders_trap_one_byte_past_the_longest_string() {
        let plan = exact_plan::builder::PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1)
            .finish()
            .unwrap();
        let encode = |f, text: &str| call(f, &[Value::str(text)], 0.0, &plan, None, None, None);
        let max = crate::vm::MAX_STRING;
        for f in [Stdlib::EncodeURIComponent, Stdlib::EncodeRouteSegment] {
            // A space encodes to `%20`: three bytes.
            let fits = "a".repeat(max - 3) + " ";
            assert!(encode(f, &fits).is_ok_and(|v| v.as_str().map(str::len) == Some(max)));
            let over = "a".repeat(max - 2) + " ";
            assert_eq!(encode(f, &over), Err(CallError::StringTooLong));
        }
        assert_eq!(
            encode(Stdlib::EncodeRouteSegment, ""),
            Err(CallError::TypeMismatch)
        );
    }

    #[test]
    fn at_answers_what_javascript_answers() {
        let plan = exact_plan::builder::PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1)
            .finish()
            .unwrap();
        let xs = Value::list(vec![Value::str("a"), Value::str("b"), Value::str("c")]);
        let at = |i: f64| {
            call(
                Stdlib::At,
                &[xs.clone(), Value::Number(i)],
                0.0,
                &plan,
                None,
                None,
                None,
            )
        };
        // `["a","b","c"].at(i)` in Bun, undefined as `none`.
        for (i, expected) in [
            (0.0, Some("a")),
            (2.0, Some("c")),
            (3.0, None),
            (-1.0, Some("c")),
            (-3.0, Some("a")),
            (-4.0, None),
            (1.9, Some("b")),
            (-0.5, Some("a")),
            (-1.5, Some("c")),
            (f64::NAN, Some("a")),
            (f64::INFINITY, None),
            (f64::NEG_INFINITY, None),
        ] {
            let want = expected.map_or(Value::Option(None), |s| Value::some(Value::str(s)));
            assert_eq!(at(i), Ok(want), "at({i})");
        }
        let empty = Value::list(vec![]);
        assert_eq!(
            call(
                Stdlib::At,
                &[empty, Value::Number(0.0)],
                0.0,
                &plan,
                None,
                None,
                None
            ),
            Ok(Value::Option(None))
        );
    }
    #[test]
    fn includes_starts_with_and_ends_with_answer_as_javascript_does() {
        let plan = exact_plan::builder::PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1)
            .finish()
            .unwrap();
        let search = |f: Stdlib, args: &[Value]| call(f, args, 0.0, &plan, None, None, None);
        let strs = |a: &str, b: &str| [Value::str(a), Value::str(b)];
        // (haystack, needle, includes, startsWith, endsWith), from Bun.
        for (s, t, inc, start, end) in [
            ("", "", true, true, true),
            ("21", "", true, true, true),
            ("21", "1", true, false, true),
            ("12", "1", true, true, false),
            ("12", "12", true, true, true),
            ("1", "12", false, false, false),
            ("todo.md", ".md", true, false, true),
            ("build log", "o", true, false, false),
            ("build log", "O", false, false, false),
            (" heading2 ", " heading2 ", true, true, true),
            ("😀é", "é", true, false, true),
            ("😀é", "😀", true, true, false),
            ("😀é", "\u{301}", false, false, false),
        ] {
            let args = strs(s, t);
            assert_eq!(
                search(Stdlib::Includes, &args),
                Ok(Value::Bool(inc)),
                "{s:?}.includes({t:?})"
            );
            assert_eq!(
                search(Stdlib::StartsWith, &args),
                Ok(Value::Bool(start)),
                "{s:?}.startsWith({t:?})"
            );
            assert_eq!(
                search(Stdlib::EndsWith, &args),
                Ok(Value::Bool(end)),
                "{s:?}.endsWith({t:?})"
            );
        }
        // Text longer than the inline form, so both text forms are searched.
        let long = strs("a sentence longer than fourteen bytes", "fourteen bytes");
        assert_eq!(search(Stdlib::Includes, &long), Ok(Value::Bool(true)));
        assert_eq!(search(Stdlib::EndsWith, &long), Ok(Value::Bool(true)));
        assert_eq!(search(Stdlib::StartsWith, &long), Ok(Value::Bool(false)));
        // Anything but two strings is a type mismatch, never a coercion.
        for f in [Stdlib::Includes, Stdlib::StartsWith, Stdlib::EndsWith] {
            assert_eq!(
                search(f, &[Value::Number(12.0), Value::str("1")]),
                Err(CallError::TypeMismatch)
            );
            assert_eq!(
                search(f, &[Value::str("12"), Value::Number(1.0)]),
                Err(CallError::TypeMismatch)
            );
            assert_eq!(
                search(f, &[Value::list(vec![Value::str("12")]), Value::str("12")]),
                Err(CallError::TypeMismatch)
            );
            assert_eq!(search(f, &[Value::str("12")]), Err(CallError::TypeMismatch));
        }
    }

    #[test]
    fn trim_strips_what_javascript_strips() {
        let plan = exact_plan::builder::PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1)
            .finish()
            .unwrap();
        let trim = |text: &str| {
            call(
                Stdlib::Trim,
                &[Value::str(text)],
                0.0,
                &plan,
                None,
                None,
                None,
            )
        };
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
        let s = Value::str("kept text longer than fourteen bytes");
        let Ok(out) = call(
            Stdlib::Trim,
            std::slice::from_ref(&s),
            0.0,
            &plan,
            None,
            None,
            None,
        ) else {
            panic!("trim answers a string");
        };
        assert!(Value::same_str(&s, &out));
    }
}
