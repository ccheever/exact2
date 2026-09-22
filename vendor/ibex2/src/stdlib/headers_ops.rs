//! Host ops behind the JavaScript `Headers` class.
//!
//! A header list crosses as a **handle** for the same reason a `Response` does
//! (LLP 0059.000 §1.1): it is a list of pairs, and serializing it at the
//! boundary is what the contract forbids. Iteration is therefore
//! count/name-at/value-at over primitives rather than a marshalled array.
//!
//! Rust owns the semantics WPT actually checks: case-folding, name and value
//! validation, value normalization, comma-joining on append, and sorted
//! iteration order.

use crate::boundary::{HostArg, HostError, HostValue};
use crate::stdlib::fetch::{is_valid_name, is_valid_value, Headers};
use crate::task::RuntimeState;

/// Handle any headers op. Returns `None` for ops this module does not own.
pub fn dispatch(
    op: u32,
    args: &[HostArg],
    state: Option<&RuntimeState>,
) -> Option<Result<HostValue, HostError>> {
    if !(40..=51).contains(&op) {
        return None;
    }
    Some(run(op, args, state))
}

fn run(op: u32, args: &[HostArg], state: Option<&RuntimeState>) -> Result<HostValue, HostError> {
    // Validation ops need no registry, so answer them before demanding state.
    let text = |index: usize| -> Result<&str, HostError> {
        args.get(index)
            .and_then(HostArg::as_str)
            .ok_or_else(|| HostError::InvalidArgument(format!("argument {index} must be a string")))
    };
    match op {
        49 => return Ok(HostValue::Bool(is_valid_name(text(0)?))),
        50 => return Ok(HostValue::Bool(is_valid_value(text(0)?))),
        _ => {}
    }

    let state = state.ok_or_else(|| HostError::Failed("no runtime state".into()))?;
    if op == 40 {
        return Ok(HostValue::Number(state.store_headers(Headers::new()) as f64));
    }

    let handle = match args.first() {
        Some(HostArg::Number(n)) => *n as u64,
        _ => {
            return Err(HostError::InvalidArgument(
                "expected a headers handle".into(),
            ))
        }
    };
    let missing = || HostError::Failed("TypeError: unknown headers handle".into());

    match op {
        41 | 42 | 45 => {
            // Resolve arguments before entering the closure: `?` inside one
            // would try to return from the closure, not from here.
            let name = text(1)?.to_string();
            let value = if op == 45 {
                String::new()
            } else {
                text(2)?.to_string()
            };
            state
                .with_headers_mut(handle, |h| match op {
                    41 => h.append(&name, &value),
                    42 => h.set(&name, &value),
                    _ => h.delete(&name),
                })
                .ok_or_else(missing)?;
            Ok(HostValue::Undefined)
        }
        43 | 44 => {
            let name = text(1)?.to_string();
            state
                .with_headers(handle, |h| {
                    if op == 44 {
                        HostValue::Bool(h.has(&name))
                    } else {
                        match h.get(&name) {
                            Some(value) => HostValue::Str(value.to_string()),
                            None => HostValue::Null,
                        }
                    }
                })
                .ok_or_else(missing)
        }
        46 => state
            .with_headers(handle, |h| HostValue::Number(h.len() as f64))
            .ok_or_else(missing),
        47 | 48 => {
            let index = match args.get(1) {
                Some(HostArg::Number(n)) => *n as usize,
                _ => return Err(HostError::InvalidArgument("expected an index".into())),
            };
            let entry = state
                .with_headers(handle, |h| h.sorted_entries().get(index).cloned())
                .ok_or_else(missing)?
                .ok_or_else(|| HostError::Failed("index out of range".into()))?;
            Ok(HostValue::Str(if op == 47 { entry.0 } else { entry.1 }))
        }
        51 => {
            state.drop_headers(handle);
            Ok(HostValue::Undefined)
        }
        other => Err(HostError::InvalidArgument(format!(
            "unknown headers op {other}"
        ))),
    }
}
