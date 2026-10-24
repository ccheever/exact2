//! A presence secret's calls from Rust data code (LLP 1133.010 D5, D14):
//! `store.has` and `store.locked` are the [`exact_runner::Store`]'s own; the
//! calls that wait on the host's backend are a request a source returns as
//! `Answer::Later` from `answer` and reads in `parse`, as the module calls
//! are. The reply carries an unlocked value to that parse alone.

use crate::modules::ModuleError;
use exact_runner::{Outcome, Request};
use serde_json::{json, Value as Json};

fn request(op: &str, name: &str, reason: &str) -> Request {
    Request::native(
        json!({ "presence": op, "name": name, "reason": reason })
            .to_string()
            .into_bytes(),
    )
}

/// `store.unlock(name, { reason })`'s request: the person's check when the
/// name is locked; `reason` is a strings key.
pub fn unlock(name: &str, reason: &str) -> Request {
    request("unlock", name, reason)
}

/// `store.protect(name)`'s request: an `optional` name moved to locked.
pub fn protect(name: &str) -> Request {
    request("protect", name, "")
}

/// `store.unprotect(name, { reason })`'s request: the check, then the name
/// moved to plain.
pub fn unprotect(name: &str, reason: &str) -> Request {
    request("unprotect", name, reason)
}

/// A presence call's reply: `unlock`'s value (`None` when nothing is kept),
/// `None` for `protect` and `unprotect`; else D1's code and message.
pub fn reply(outcome: Outcome) -> Result<Option<String>, ModuleError> {
    let Outcome::Response(response) = outcome else {
        return Err(ModuleError::new("UnknownError", outcome.summary()));
    };
    let v: Json = serde_json::from_slice(&response.body)
        .map_err(|e| ModuleError::new("UnknownError", e.to_string()))?;
    if let Some(code) = v["error"].as_str() {
        let message = v["message"].as_str().unwrap_or(code);
        return Err(ModuleError::new(code, message));
    }
    Ok(v["value"].as_str().map(str::to_owned))
}
