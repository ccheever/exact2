//! The turn envelope (LLP 1027.002 D3): what a module instance produced
//! against its own copy of the store, carried back as an `Outcome` for the
//! runner to commit inside its transaction. Values cross; nothing else does.

use exact_plan::Value;
use exact_runner::{Answer, DataError, Outcome, Request, Response, Store};
use serde_json::{json, Value as Json};

/// The header that marks a response as a turn's reply, not a host's.
pub const HEADER: (&str, &str) = ("exact-turn", "1");

fn unavailable(message: impl Into<String>) -> DataError {
    DataError::Unavailable(message.into())
}

/// A module's scoped snapshot of the committed store: its own `secret.keep`
/// names, never the runner's kept answers (LLP 1027.002 D3, change 1).
pub fn snapshot(store: &Store, grants: &str) -> Vec<(String, String)> {
    let granted = Store::new(grants, []).granted().to_vec();
    store
        .snapshot()
        .into_iter()
        .filter(|(name, _)| !name.starts_with(Store::KEPT) && granted.iter().any(|g| g == name))
        .collect()
}

/// One turn's reply: the writes it made to `local`, in order; how many
/// device observations it made; its `console` lines; and the answer — a
/// value, the next request (a yield at storage or `fetch`), or an error.
pub fn encode(result: Result<Answer, DataError>, local: &mut Store, logs: Vec<String>) -> Outcome {
    let writes: Vec<Json> = local
        .take_writes()
        .into_iter()
        .map(|w| json!([w.name, w.value]))
        .collect();
    let mut body = json!({"turn": 1, "reads": local.reads(), "writes": writes, "logs": logs});
    match result {
        Ok(Answer::Now(value)) => body["value"] = Json::String(base64(&value.to_bytes())),
        Ok(Answer::Later(request)) => body["request"] = request_json(&request),
        Err(error) => body["error"] = error_json(&error),
    }
    Outcome::Response(Response {
        status: 200,
        headers: vec![(HEADER.0.into(), HEADER.1.into())],
        body: body.to_string().into_bytes(),
    })
}

/// Whether an outcome is a turn's reply.
pub fn is_turn(outcome: &Outcome) -> bool {
    matches!(outcome, Outcome::Response(r) if r.headers.iter().any(|(k, v)| k == HEADER.0 && v == HEADER.1))
}

/// Commit a turn's reply into the live store, inside the runner's
/// transaction: its observations, its writes in order, then the answer. The
/// writes stand even when the answer is an error, as the browser's do; the
/// runner rolls its transaction back. `logs` receives the turn's `console`.
pub fn apply(
    outcome: Outcome,
    store: &mut Store,
    logs: &mut Vec<String>,
) -> Result<Answer, DataError> {
    let body = match outcome {
        Outcome::Response(r)
            if r.headers
                .iter()
                .any(|(k, v)| k == HEADER.0 && v == HEADER.1) =>
        {
            r.body
        }
        Outcome::Failed { message, .. } => return Err(unavailable(message)),
        _ => return Err(unavailable("a turn's reply was expected")),
    };
    let json: Json =
        serde_json::from_slice(&body).map_err(|e| unavailable(format!("turn reply: {e}")))?;
    for line in json["logs"].as_array().into_iter().flatten() {
        if let Some(line) = line.as_str() {
            logs.push(line.to_string());
        }
    }
    for _ in 0..json["reads"].as_u64().unwrap_or(0) {
        store.observe_external_read();
    }
    for write in json["writes"].as_array().into_iter().flatten() {
        let name = write[0]
            .as_str()
            .ok_or_else(|| unavailable("turn reply: an invalid write"))?;
        match &write[1] {
            Json::String(value) => store.set(name, value)?,
            Json::Null => store.forget(name)?,
            _ => return Err(unavailable("turn reply: an invalid store value")),
        }
    }
    if let Some(error) = json.get("error") {
        return Err(error_from(error));
    }
    if let Some(request) = json.get("request") {
        return request_from(request).map(Answer::Later);
    }
    let bytes = json["value"]
        .as_str()
        .and_then(unbase64)
        .ok_or_else(|| unavailable("turn reply: no answer"))?;
    Value::from_bytes(&bytes)
        .map(Answer::Now)
        .map_err(|e| unavailable(format!("turn reply: {e:?}")))
}

fn error_json(error: &DataError) -> Json {
    let (kind, message) = match error {
        DataError::UnknownSource(m) => ("UnknownSource", m),
        DataError::BadArguments(m) => ("BadArguments", m),
        DataError::Unavailable(m) => ("Unavailable", m),
    };
    json!({"kind": kind, "message": message})
}

fn error_from(error: &Json) -> DataError {
    let message = error["message"].as_str().unwrap_or("").to_string();
    match error["kind"].as_str() {
        Some("UnknownSource") => DataError::UnknownSource(message),
        Some("BadArguments") => DataError::BadArguments(message),
        _ => DataError::Unavailable(message),
    }
}

fn request_json(request: &Request) -> Json {
    json!({
        "continuation": request.continuation,
        "storage": request.storage.as_deref().map(base64),
        "grants": request.grants,
        "method": request.method,
        "url": request.url,
        "headers": request.headers,
        "body": base64(&request.body),
    })
}

fn request_from(json: &Json) -> Result<Request, DataError> {
    let text = |key: &str| json[key].as_str().map(str::to_string);
    let storage = match &json["storage"] {
        Json::Null => None,
        Json::String(s) => {
            Some(unbase64(s).ok_or_else(|| unavailable("turn reply: an invalid storage payload"))?)
        }
        _ => return Err(unavailable("turn reply: an invalid storage payload")),
    };
    let headers = json["headers"]
        .as_array()
        .map(|list| {
            list.iter()
                .map(|pair| match (pair[0].as_str(), pair[1].as_str()) {
                    (Some(k), Some(v)) => Ok((k.to_string(), v.to_string())),
                    _ => Err(unavailable("turn reply: an invalid header")),
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?
        .unwrap_or_default();
    Ok(Request {
        continuation: json["continuation"].as_u64(),
        storage,
        grants: text("grants"),
        method: text("method").unwrap_or_default(),
        url: text("url").unwrap_or_default(),
        headers,
        body: json["body"].as_str().and_then(unbase64).unwrap_or_default(),
    })
}

const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Standard base64 with padding, as the runner's agent module spells it.
pub fn base64(bytes: &[u8]) -> String {
    exact_runner::agent::base64(bytes)
}

/// The inverse of [`base64`]; `None` for anything that is not base64.
pub fn unbase64(text: &str) -> Option<Vec<u8>> {
    let text = text.trim_end_matches('=');
    let mut out = Vec::with_capacity(text.len() * 3 / 4);
    let mut acc = 0u32;
    let mut bits = 0;
    for b in text.bytes() {
        let v = TABLE.iter().position(|t| *t == b)? as u32;
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_turn_round_trips_its_value_writes_reads_and_yields() {
        let mut local = Store::new("secret.keep a", [("a".to_string(), "1".to_string())]);
        let _ = local.get("a");
        local.set("a", "2").unwrap();
        let value = Value::record(vec![Value::str("x"), Value::Number(2.0)]);
        let outcome = encode(
            Ok(Answer::Now(value.clone())),
            &mut local,
            vec!["hi".into()],
        );
        assert!(is_turn(&outcome));
        let mut live = Store::new("secret.keep a\nsecret.keep b", []);
        let mut logs = Vec::new();
        match apply(outcome, &mut live, &mut logs).unwrap() {
            Answer::Now(v) => assert_eq!(v.to_bytes(), value.to_bytes()),
            _ => panic!("a value"),
        }
        assert_eq!(live.get("a"), Some("2"));
        assert_eq!(live.reads(), 2);
        assert_eq!(logs, vec!["hi".to_string()]);

        let mut request = Request::post_json("https://x.test", "{}");
        request.grants = Some("net.fetch https://x.test".into());
        let outcome = encode(
            Ok(Answer::Later(request.clone())),
            &mut Store::default(),
            vec![],
        );
        match apply(outcome, &mut live, &mut logs).unwrap() {
            Answer::Later(r) => assert_eq!(r, request),
            _ => panic!("a yield"),
        }
        let storage = Request::storage(vec![0, 255, 7]);
        let outcome = encode(
            Ok(Answer::Later(storage.clone())),
            &mut Store::default(),
            vec![],
        );
        match apply(outcome, &mut live, &mut logs).unwrap() {
            Answer::Later(r) => assert_eq!(r, storage),
            _ => panic!("a storage yield"),
        }
    }

    #[test]
    fn an_error_keeps_its_kind_and_its_writes_stand_for_the_runner_to_roll_back() {
        let mut local = Store::new("secret.keep a", []);
        local.set("a", "written").unwrap();
        let outcome = encode(Err(DataError::BadArguments("n".into())), &mut local, vec![]);
        let mut live = Store::new("secret.keep a", []);
        let mut logs = Vec::new();
        assert_eq!(
            apply(outcome, &mut live, &mut logs).unwrap_err(),
            DataError::BadArguments("n".into())
        );
        assert_eq!(live.get("a"), Some("written"));
        assert_eq!(
            apply(
                Outcome::Failed {
                    kind: exact_runner::FailureKind::Aborted,
                    message: "the owner ended without a reply".into()
                },
                &mut live,
                &mut logs
            )
            .unwrap_err(),
            DataError::Unavailable("the owner ended without a reply".into())
        );
    }

    #[test]
    fn a_snapshot_is_scoped_and_never_carries_kept_answers() {
        let store = Store::new(
            "secret.keep a\nsecret.keep b",
            [
                ("a".to_string(), "1".to_string()),
                ("b".to_string(), "2".to_string()),
                ("exact.kept.x".to_string(), "k".to_string()),
            ],
        );
        assert_eq!(
            snapshot(&store, "secret.keep b\nnet.fetch https://x"),
            vec![("b".to_string(), "2".to_string())]
        );
        assert_eq!(unbase64(&base64(b"hello, turn")).unwrap(), b"hello, turn");
        assert_eq!(unbase64("!!"), None);
    }
}
