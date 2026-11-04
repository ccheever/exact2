//! Decode a module reply and classify its next step.
use super::*;

impl Module {
    /// Decode once, retaining metadata for async dispatch and the typed answer
    /// for settlement. Captured replies retain their JSON restoration path.
    pub(super) fn decode_reply(
        sig: &Sig,
        engine: &mut Engine,
        source: &str,
        text: &str,
    ) -> Result<exact_js_value::Reply, DataError> {
        // Captured large strings still use path restoration into JSON. Ordinary
        // answers decode directly to Value, without a second full value tree.
        let captured = engine.has_reply_strings();
        let decoded = if captured {
            serde_json::from_str(text)
                .map(|fields| exact_js_value::Reply {
                    fields,
                    value: Ok(Value::Unit),
                })
                .map_err(|e| e.to_string())
        } else {
            exact_js_value::reply_reusing(text, &sig.result, &sig.rows).map_err(|e| e.to_string())
        };
        decoded.map_err(|e| {
            engine.clear_reply();
            DataError::Unavailable(format!(
                "`{source}` answered something other than JSON: {e}"
            ))
        })
    }

    pub(super) fn step(
        sig: &Sig,
        engine: &mut Engine,
        source: &str,
        decoded: exact_js_value::Reply,
        baking: bool,
    ) -> Step {
        let exact_js_value::Reply {
            fields: mut reply,
            mut value,
        } = decoded;
        if engine.has_reply_strings() {
            if let Err(error) = engine.restore_reply(&mut reply) {
                return Step::Done(Err(DataError::Failed(
                    FailureCode::Shape,
                    format!("`{source}` answered outside its shape: {error}"),
                    String::new(),
                )));
            }
            value = from_json(reply.get("value").unwrap_or(&Json::Null), &sig.result);
        }
        let num = |k: &str| reply.get(k).and_then(Json::as_u64);
        match num("tag") {
            Some(0) => Step::Done(value.map_err(|e| {
                let why = format!("`{source}` answered outside its shape: {e}");
                DataError::Failed(FailureCode::Shape, why, String::new())
            })),
            Some(1) => match (num("call"), num("ticket")) {
                (Some(call), Some(0)) if reply.get("secret").and_then(Json::as_str).is_some() => {
                    Step::Pending {
                        call,
                        ticket: secrets::TICKET,
                        due: None,
                    }
                }
                (Some(call), Some(0)) if reply.get("yielded") == Some(&Json::Bool(true)) => {
                    Step::Pending {
                        call,
                        ticket: YIELDED,
                        due: None,
                    }
                }
                (Some(call), Some(0)) if reply.get("waiting") == Some(&Json::Bool(true)) => {
                    Step::Pending {
                        call,
                        ticket: WAITING,
                        due: num("timer"),
                    }
                }
                (Some(call), Some(0)) if num("timer").is_some() => Step::Pending {
                    call,
                    ticket: TIMER,
                    due: num("timer"),
                },
                (Some(call), Some(ticket)) => Step::Pending {
                    call,
                    ticket,
                    due: num("timer"),
                },
                _ => Step::Done(Err(DataError::Unavailable(format!(
                    "`{source}` is pending on no ticket"
                )))),
            },
            Some(2) => {
                let message = reply
                    .get("message")
                    .and_then(Json::as_str)
                    .unwrap_or("")
                    .to_string();
                Step::Done(Err(match reply.get("kind").and_then(Json::as_str) {
                    Some("UnknownSource") => DataError::UnknownSource(message),
                    Some("BadArguments") => DataError::BadArguments(message),
                    Some("Unavailable")
                        if baking && reply.get("code").and_then(Json::as_str) == Some("bake") =>
                    {
                        DataError::DeferredAtBake(message)
                    }
                    // What it let through, by class (LLP 1109 D3; prelude.js `failureCode`).
                    _ => FailureCode::seam_error(
                        reply.get("failure").and_then(Json::as_str),
                        message,
                        reply.get("reason").and_then(Json::as_str),
                    ),
                }))
            }
            _ => Step::Done(Err(DataError::Unavailable(format!(
                "`{source}` answered with no tag (ABI {ABI} expects 0, 1, 2, or 3)"
            )))),
        }
    }
}
