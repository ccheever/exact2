//! The agent API's read operations, once, for every host.
//!
//! @ref LLP 1012 (agent API v1); `rules/NOT-DOING.md` §Agent API
//!
//! `tree`, `state`, and `logs` are answered here from the runner and its
//! kernel — never from a host's mirror of them (a projection that is a
//! parallel reconstruction is the defect exact1's 0495 §4.1 names). The
//! other five operations are the host's: `layout` and `screenshot` read what
//! it renders, `tap` and `type` go through its real input path, `clock`
//! moves its clocks (the runner's through `Runner::advance`; motion's through
//! the host's engine, whose `settle` this module also answers for it).
//!
//! Requests and replies are JSON built and read by hand (no serde anywhere
//! in the runtime): a request is `{"op":"…"}` with at most a couple of flat
//! fields, so [`field_str`] and [`field_num`] are the whole parser.

use crate::runner::{DataSource, Runner};
use exact_kernel::PropValue;
use exact_plan::{EventKind, Plan, TypeKind, TypesId, Value};
use std::fmt::Write as _;

/// Answer one request: `{"op":"tree"}`, `{"op":"state"}`, or
/// `{"op":"logs","since":N}`. Anything else is an `{"error":…}`.
pub fn handle<D: DataSource>(runner: &Runner<D>, request: &str) -> String {
    match field_str(request, "op").as_deref() {
        Some("tree") => tree(runner),
        Some("state") => state(runner),
        Some("logs") => match (after_key(request, "since"), field_num(request, "since")) {
            (None, _) => logs(runner, 0),
            (Some(_), Some(n)) if n >= 0.0 => logs(runner, n as usize),
            (Some(_), _) => error("since must be a non-negative number"),
        },
        Some(other) => error(&format!("unknown op: {other}")),
        None => error("no op"),
    }
}

/// `{"error":"…"}`.
pub fn error(message: &str) -> String {
    let mut s = String::from("{\"error\":");
    quote(message, &mut s);
    s.push('}');
    s
}

/// The tree: every live node in preorder — id, parent, depth, type, props by
/// their schema names, the events it handles, its children — plus the
/// kernel's epoch and incarnation (the consistency token: nothing moves
/// between two calls unless the agent moved it).
pub fn tree<D: DataSource>(runner: &Runner<D>) -> String {
    let kernel = runner.kernel();
    let mut s = String::new();
    let _ = write!(
        s,
        "{{\"epoch\":{},\"incarnation\":{},\"clock\":{},\"roots\":",
        kernel.epoch(),
        kernel.incarnation(),
        num(runner.now_ms())
    );
    ids(&runner.roots(), &mut s);
    s.push_str(",\"nodes\":[");
    let rows = kernel.rows(None).unwrap_or_default();
    let mut first = true;
    for row in &rows {
        let Some(node) = kernel.node(row.id) else {
            continue;
        };
        if !first {
            s.push(',');
        }
        first = false;
        let _ = write!(s, "{{\"id\":{},\"parent\":", node.id);
        match node.parent {
            Some(p) => {
                let _ = write!(s, "{p}");
            }
            None => s.push_str("null"),
        }
        let _ = write!(s, ",\"depth\":{},\"type\":", row.depth);
        quote(node.node_type.name(), &mut s);
        s.push_str(",\"props\":{");
        for (i, (id, value)) in node.props.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            quote(id.name(), &mut s);
            s.push(':');
            match value {
                PropValue::Str(t) => quote(t, &mut s),
                PropValue::Bool(b) => s.push_str(if *b { "true" } else { "false" }),
                PropValue::Int(i) => {
                    let _ = write!(s, "{i}");
                }
                PropValue::Float(f) => s.push_str(&num(*f)),
            }
        }
        s.push_str("},\"handlers\":[");
        for (i, e) in runner.handlers_of(node.id).into_iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            quote(
                match e {
                    EventKind::Press => "press",
                    EventKind::Change => "change",
                    EventKind::Hover => "hover",
                    EventKind::Focus => "focus",
                    EventKind::Blur => "blur",
                    EventKind::Key => "key",
                },
                &mut s,
            );
        }
        s.push_str("],\"children\":");
        ids(&node.children(), &mut s);
        s.push('}');
    }
    s.push_str("]}");
    s
}

/// The state: the clock and every slot, derive, and resource by the name the
/// plan declares, as typed JSON (records carry their field names).
pub fn state<D: DataSource>(runner: &Runner<D>) -> String {
    let plan = runner.plan();
    let mut s = String::new();
    let _ = write!(s, "{{\"clock\":{},\"slots\":{{", num(runner.now_ms()));
    for (i, row) in plan.slots.iter().enumerate() {
        let name = plan.str(row.name);
        if i > 0 {
            s.push(',');
        }
        quote(name, &mut s);
        s.push(':');
        match runner.slot(name) {
            Some(v) => typed_json(plan, row.ty, v, &mut s),
            None => s.push_str("null"),
        }
    }
    s.push_str("},\"derives\":{");
    for (i, row) in plan.derives.iter().enumerate() {
        let name = plan.str(row.name);
        if i > 0 {
            s.push(',');
        }
        quote(name, &mut s);
        s.push(':');
        match runner.derive(name) {
            Some(v) => typed_json(plan, row.ty, v, &mut s),
            None => s.push_str("null"),
        }
    }
    s.push_str("},\"resources\":{");
    for (i, row) in plan.resources.iter().enumerate() {
        let name = plan.str(row.name);
        if i > 0 {
            s.push(',');
        }
        quote(name, &mut s);
        s.push(':');
        match runner.resource(name) {
            Some(v) => typed_json(plan, row.ty, v, &mut s),
            None => s.push_str("null"),
        }
    }
    s.push_str("},\"pending\":[");
    for (i, (name, ticket)) in runner.pending().iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str("{\"name\":");
        quote(name, &mut s);
        let _ = write!(s, ",\"ticket\":{ticket}}}");
    }
    s.push_str("]}");
    s
}

/// Standard base64 (with padding) — a request or reply body in a batch.
pub fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = ((chunk[0] as u32) << 16)
            | ((chunk.get(1).copied().unwrap_or(0) as u32) << 8)
            | chunk.get(2).copied().unwrap_or(0) as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            T[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

/// The journal from `since`: `{"next":N,"from":M,"lines":[…]}`. `next` is
/// what to pass to read only what is new; `from` is the index of the first
/// line returned — above `since` when the ring has dropped older lines (the
/// oldest go first, silently: `from - since` of them are gone), and never
/// beyond `next`.
pub fn logs<D: DataSource>(runner: &Runner<D>, since: usize) -> String {
    let start = runner.journal_start();
    let lines: Vec<&str> = runner.journal().collect();
    let from = since.clamp(start, start + lines.len());
    let mut s = String::new();
    let _ = write!(
        s,
        "{{\"next\":{},\"from\":{from},\"lines\":[",
        start + lines.len()
    );
    for (i, line) in lines.iter().skip(from - start).enumerate() {
        if i > 0 {
            s.push(',');
        }
        quote(line, &mut s);
    }
    s.push_str("]}");
    s
}

/// A plan value as JSON under its declared type: numbers, strings, booleans;
/// `null` for unit and `none`; lists; records as objects keyed by field
/// name. A value that does not match its type (never, past the runner's
/// conformance checks) falls back to the positional form.
pub fn typed_json(plan: &Plan, ty: TypesId, v: &Value, out: &mut String) {
    let row = plan.type_(ty);
    match (row.kind, v) {
        (_, Value::Number(n)) => out.push_str(&num(*n)),
        (_, Value::Bool(b)) => out.push_str(if *b { "true" } else { "false" }),
        (_, Value::Str(s)) => quote(s, out),
        (_, Value::Unit) | (_, Value::Option(None)) => out.push_str("null"),
        (TypeKind::Option, Value::Option(Some(inner))) => match row.elem {
            Some(elem) => typed_json(plan, elem, inner, out),
            None => untyped_json(inner, out),
        },
        (_, Value::Option(Some(inner))) => untyped_json(inner, out),
        (TypeKind::List, Value::List(items)) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                match row.elem {
                    Some(elem) => typed_json(plan, elem, item, out),
                    None => untyped_json(item, out),
                }
            }
            out.push(']');
        }
        (TypeKind::Record, Value::Record(fields)) if row.fields.len as usize == fields.len() => {
            out.push('{');
            for (i, (f, value)) in row.fields.iter().zip(fields.iter()).enumerate() {
                if i > 0 {
                    out.push(',');
                }
                let field = plan.field(f);
                quote(plan.str(field.name), out);
                out.push(':');
                typed_json(plan, field.ty, value, out);
            }
            out.push('}');
        }
        (_, Value::List(_)) | (_, Value::Record(_)) => untyped_json(v, out),
    }
}

/// A plan value as JSON with no type to hand: records positional.
pub fn untyped_json(v: &Value, out: &mut String) {
    match v {
        Value::Number(n) => out.push_str(&num(*n)),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Str(s) => quote(s, out),
        Value::Unit | Value::Option(None) => out.push_str("null"),
        Value::Option(Some(inner)) => untyped_json(inner, out),
        Value::List(items) | Value::Record(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                untyped_json(item, out);
            }
            out.push(']');
        }
    }
}

/// A finite number as JSON; anything else is `null`.
pub fn num(n: f64) -> String {
    if n.is_finite() {
        if n == n.trunc() && n.abs() < 1e15 {
            format!("{}", n as i64)
        } else {
            format!("{n}")
        }
    } else {
        "null".to_string()
    }
}

/// A JSON string.
pub fn quote(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

fn ids(ids: &[u32], out: &mut String) {
    out.push('[');
    for (i, id) in ids.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let _ = write!(out, "{id}");
    }
    out.push(']');
}

/// The string value of a top-level `"key":"…"` field in a JSON object, with
/// JSON's escapes decoded (surrogate pairs included). `None` when the key is
/// absent at the top level, the value is not a string, or the string never
/// ends.
pub fn field_str(json: &str, key: &str) -> Option<String> {
    let rest = after_key(json, key)?;
    let rest = rest.strip_prefix('"')?;
    let mut out = String::new();
    let mut chars = rest.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => return Some(out),
            '\\' => match chars.next()? {
                'n' => out.push('\n'),
                't' => out.push('\t'),
                'r' => out.push('\r'),
                'b' => out.push('\u{8}'),
                'f' => out.push('\u{c}'),
                'u' => {
                    let mut unit = hex4(&mut chars)?;
                    if (0xD800..0xDC00).contains(&unit) {
                        // A high surrogate: the low one must follow as `\uXXXX`.
                        if chars.next()? != '\\' || chars.next()? != 'u' {
                            return None;
                        }
                        let low = hex4(&mut chars)?;
                        if !(0xDC00..0xE000).contains(&low) {
                            return None;
                        }
                        unit = 0x10000 + ((unit - 0xD800) << 10) + (low - 0xDC00);
                    }
                    out.push(char::from_u32(unit)?);
                }
                other => out.push(other),
            },
            c => out.push(c),
        }
    }
    None
}

fn hex4(chars: &mut std::str::Chars<'_>) -> Option<u32> {
    let hex: String = chars.by_ref().take(4).collect();
    if hex.len() != 4 {
        return None;
    }
    u32::from_str_radix(&hex, 16).ok()
}

/// The numeric value of a top-level `"key":N` field.
pub fn field_num(json: &str, key: &str) -> Option<f64> {
    let rest = after_key(json, key)?;
    let end = rest
        .find(|c: char| !(c.is_ascii_digit() || matches!(c, '-' | '+' | '.' | 'e' | 'E')))
        .unwrap_or(rest.len());
    rest[..end].parse().ok()
}

/// Whether a top-level `"key":true` field is set.
pub fn field_bool(json: &str, key: &str) -> bool {
    after_key(json, key).is_some_and(|rest| rest.starts_with("true"))
}

/// What follows `"key":` at the top level of a JSON object — a one-pass scan
/// that steps over string tokens and nested objects and arrays, so a key
/// inside a string value or a nested object never matches. The whole
/// parser: requests are flat.
fn after_key<'a>(json: &'a str, key: &str) -> Option<&'a str> {
    let b = json.as_bytes();
    let mut i = 0;
    let mut depth = 0i32;
    while i < b.len() {
        match b[i] {
            b'"' => {
                let start = i + 1;
                let mut j = start;
                loop {
                    let c = *b.get(j)?;
                    if c == b'\\' {
                        j += 2;
                    } else if c == b'"' {
                        break;
                    } else {
                        j += 1;
                    }
                }
                let token = &json[start..j.min(b.len())];
                i = j + 1;
                if depth == 1 && token == key {
                    let rest = json.get(i..)?.trim_start();
                    if let Some(rest) = rest.strip_prefix(':') {
                        return Some(rest.trim_start());
                    }
                }
            }
            b'{' | b'[' => {
                depth += 1;
                i += 1;
            }
            b'}' | b']' => {
                depth -= 1;
                i += 1;
            }
            _ => i += 1,
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_fields_parse() {
        let r = r#"{"op":"logs","since":12,"text":"a \"b\" \\ c\n","wheel":true}"#;
        assert_eq!(field_str(r, "op").as_deref(), Some("logs"));
        assert_eq!(field_num(r, "since"), Some(12.0));
        assert_eq!(field_str(r, "text").as_deref(), Some("a \"b\" \\ c\n"));
        assert!(field_bool(r, "wheel"));
        assert!(!field_bool(r, "since"));
        assert_eq!(field_str(r, "missing"), None);
        assert_eq!(field_num(r, "op"), None);
    }

    #[test]
    fn keys_inside_values_and_nested_objects_never_match() {
        // A string value that contains `"op":`, a nested object with an `op`,
        // and an array — only the top-level `op` counts.
        let r =
            r#"{"text":"{\"op\":\"logs\"}","meta":{"op":"state"},"list":[{"op":"x"}],"op":"tree"}"#;
        assert_eq!(field_str(r, "op").as_deref(), Some("tree"));
        assert_eq!(field_str(r, "text").as_deref(), Some(r#"{"op":"logs"}"#));
        assert_eq!(field_str(r#"{"meta":{"op":"state"}}"#, "op"), None);
        // A key at the top level whose value is not a string.
        assert_eq!(field_str(r#"{"op":3}"#, "op"), None);
        assert_eq!(field_num(r#"{"since":"3"}"#, "since"), None);
        // Unterminated strings and requests are `None`, never a panic.
        assert_eq!(field_str(r#"{"op":"tre"#, "op"), None);
        assert_eq!(field_str(r#"{"op"#, "op"), None);
        assert_eq!(field_str(r#"{"op\"#, "op"), None);
        // Surrogate pairs decode to one character; a lone surrogate is refused.
        assert_eq!(field_str(r#"{"t":"😀"}"#, "t").as_deref(), Some("😀"));
        assert_eq!(field_str(r#"{"t":"\ud83d"}"#, "t"), None);
    }

    #[test]
    fn numbers_render_as_json() {
        assert_eq!(num(3.0), "3");
        assert_eq!(num(-0.5), "-0.5");
        assert_eq!(num(f64::NAN), "null");
        assert_eq!(num(1e20), "100000000000000000000");
        let mut s = String::new();
        quote("tab\there \"q\" \u{1}", &mut s);
        assert_eq!(s, "\"tab\\there \\\"q\\\" \\u0001\"");
    }
}
