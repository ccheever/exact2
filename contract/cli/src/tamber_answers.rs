//! The design server's small answers: a diagnostic where an author finds
//! it, a refusal, a tuple's modes, why `uses` has no token.

use serde_json::{json, Value};

/// Whether `name` is one of this server's temporary files in `.exact`.
pub(super) fn temp_name(name: &str) -> bool {
    name.strip_prefix("tamber-")
        .and_then(|n| n.strip_suffix(".tmp"))
        .is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
}

/// Why `uses` has no token `name`: an icon (a param's too) is read through
/// `describe`, `resolve` and `explain` (D10).
pub(super) fn no_token(folding: &exact_tamber::Folding, name: &str) -> Value {
    match folding.icons.iter().any(|i| i.name == name) {
        true => {
            json!({"message": format!("`{name}` is an icon: `describe`, `resolve` and `explain` read an icon; `uses` takes a token")})
        }
        false => json!({"message": format!("no token `{name}`")}),
    }
}

pub(super) fn diagnostic(file: &str, d: &exact_tamber::Diagnostic) -> Value {
    json!({"file": file, "line": d.line, "col": d.col, "id": d.id, "message": d.message})
}

/// [`diagnostic`], one of a param's (D15) in its declaring file, at its
/// default, every name in full.
pub(super) fn located(
    file: &str,
    d: &exact_tamber::Diagnostic,
    app: &contract::tamber_params::AppParams,
) -> Value {
    let mut d = d.clone();
    let declaring = contract::tamber_params::relocate(&mut d, app);
    diagnostic(declaring.as_deref().unwrap_or(file), &d)
}

pub(super) fn refused(file: &str, all: Vec<exact_tamber::Diagnostic>) -> Value {
    json!({"message": "the design is refused", "diagnostics": all.iter().map(|d| diagnostic(file, d)).collect::<Vec<_>>()})
}

/// Tuple `tuple`'s modes, `{axis: value}`, the first axis most significant.
pub(super) fn modes_of(axes: &[exact_tamber::Axis], tuple: usize) -> Value {
    let mut rest = tuple;
    let mut out = vec![(String::new(), String::new()); axes.len()];
    for (i, a) in axes.iter().enumerate().rev() {
        out[i] = (
            a.name.value.clone(),
            a.values[rest % a.values.len()].clone(),
        );
        rest /= a.values.len();
    }
    Value::Object(out.into_iter().map(|(a, v)| (a, json!(v))).collect())
}
