//! A keyed row's identity: its key's canonical text, and the journal line
//! for a key that repeats.
use super::*;

/// A row's identity: its key's canonical text, or for the `dup`th repeat
/// of that key, `d{dup}:` before it — never a canonical text, which starts
/// `s:`, `n:` or `b:`.
pub(super) fn ident(key: &Value, dup: u32) -> Option<String> {
    key_text(key).map(|text| disambiguate(text, dup))
}

pub(super) fn disambiguate(text: String, dup: u32) -> String {
    if dup == 0 {
        text
    } else {
        format!("d{dup}:{text}")
    }
}

/// The journal line for a repeated key's row.
pub(super) fn repeated(region: RegionsId, key: &Value, ident: &str) -> String {
    let mut shown = String::new();
    crate::agent::untyped_json(key, &mut shown);
    format!(
        "list {}: key {shown} repeats; this row is {ident}",
        region.0
    )
}

/// One canonical key text: strings, finite numbers (`-0` is `0`, matching the
/// VM's equality), bools. NaN is not a key.
pub(crate) fn key_text(v: &Value) -> Option<String> {
    let mut text = String::new();
    key_text_into(v, &mut text).then_some(text)
}

/// [`key_text`] appended to `out`; whether `v` is a key.
pub(super) fn key_text_into(v: &Value, out: &mut String) -> bool {
    match v {
        v @ exact_plan::str_value!() => exact_num::push_text!(out, "s:{}", v.text()),
        Value::Number(n) if n.is_finite() => exact_num::push_text!(
            out,
            "n:{}",
            exact_num::Shortest(if *n == 0.0 { 0.0 } else { *n })
        ),
        Value::Bool(b) => exact_num::push_text!(out, "b:{}", b),
        _ => return false,
    }
    true
}
