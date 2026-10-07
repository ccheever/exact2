//! Exact's browser-compatible base64 normalization. Ibex owns the algorithm;
//! this keeps the native data-source surface aligned with browsers whose
//! built-in Hermes base64 functions are more permissive.
use ibex2::stdlib::base64;
use serde_json::{json, Value};

pub(crate) fn call(op: &str, input: &str) -> Result<String, String> {
    let args: Value = serde_json::from_str(input).map_err(|error| error.to_string())?;
    let text = args[0].as_str().ok_or("expected string")?;
    let result = match op {
        "btoa" => json!(base64::btoa(text).map_err(|error| error.to_string())?),
        "atob" => json!(base64::atob(text).map_err(|error| error.to_string())?),
        _ => return Err(format!("unknown pure operation {op}")),
    };
    Ok(result.to_string())
}
