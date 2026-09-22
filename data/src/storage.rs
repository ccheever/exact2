//! Portable storage requests; handles stay inside their host (LLP 1027.001 D2).
use exact_runner::{Outcome, Request};
use serde_json::{json, Value};

/// Bounded serialized request/result, including byte representation overhead.
pub const MAX_BYTES: usize = exact_runner::MAX_HOST_WORK_BYTES;
/// Encode a storage operation. Validation occurs before the host performs effects.
pub fn request(op: &str, args: Value) -> Request {
    Request::storage(serde_json::to_vec(&json!({"version":1,"op":op,"args":args})).unwrap())
}
/// Decode the dedicated outcome, preserving the host's failure diagnostic.
pub fn response(outcome: Outcome) -> Result<Value, String> {
    match outcome {
        Outcome::Storage(bytes) if bytes.len() <= MAX_BYTES => {
            let value: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
            if let Some(error) = value.get("error").and_then(Value::as_str) {
                Err(error.into())
            } else {
                Ok(value)
            }
        }
        Outcome::Failed { message, .. } => Err(message),
        _ => Err("expected a bounded storage result".into()),
    }
}
/// Source scopes can only select exact grant declarations already admitted by
/// their containing app. URI/path matching itself remains the host's authority.
pub fn scope<'a>(admitted: &'a str, requested: Option<&'a str>) -> Result<&'a str, String> {
    let Some(requested) = requested else {
        return Ok(admitted);
    };
    let lines: Vec<_> = admitted
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if requested
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .any(|line| !lines.contains(&line))
    {
        return Err("source scope exceeds the app's admitted grants".into());
    }
    Ok(requested)
}
