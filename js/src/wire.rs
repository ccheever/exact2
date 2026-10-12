//! The JSON both sides of the host door speak: a fetch as the prelude
//! records it, a reply as `__exact_fulfill` and `__exact_message` take it,
//! and the Canvas 2D recorder's text and image questions (ops 9 and 10).
use exact_runner::{Outcome, Redirect, Request};
use serde_json::{json, Value as Json};

pub(crate) fn request_from_json(text: &str) -> Result<Request, String> {
    let j: Json = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let field = |k: &str| j.get(k).and_then(Json::as_str).map(str::to_string);
    let mut headers = Vec::new();
    if let Some(list) = j.get("headers").and_then(Json::as_array) {
        for pair in list {
            match (
                pair.get(0).and_then(Json::as_str),
                pair.get(1).and_then(Json::as_str),
            ) {
                (Some(k), Some(v)) => headers.push((k.to_string(), v.to_string())),
                _ => return Err("a header that is not a name and a value".into()),
            }
        }
    }
    Ok(Request {
        http: match j.get("max_response_bytes") {
            None => exact_runner::HttpScheduling::Ordered,
            Some(value) => exact_runner::HttpScheduling::Independent {
                max_response_bytes: value
                    .as_u64()
                    .filter(|n| (1..=64 << 20).contains(n))
                    .ok_or("invalid independent HTTP response ceiling")?
                    as u32,
            },
        },
        continuation: None,
        storage: None,
        surface: None,
        grants: None,
        method: field("method").ok_or("no method")?,
        url: field("url").ok_or("no url")?,
        headers,
        // A BufferSource body travels as base64 beside the text one.
        body: match field("body_base64") {
            Some(b64) => exact_runner::agent::unbase64(&b64).ok_or("a body that is not base64")?,
            None => field("body").unwrap_or_default().into_bytes(),
        },
        stream: j.get("stream").and_then(Json::as_bool).unwrap_or(false),
        timeout_ms: match j.get("timeout_ms") {
            None | Some(Json::Null) => None,
            Some(value) => Some(
                value
                    .as_u64()
                    .filter(|n| (1..=u64::from(exact_runner::MAX_TIMEOUT_MS)).contains(n))
                    .ok_or("a request timeout must be 1 to 3600000 ms")? as u32,
            ),
        },
        // `exactBodyFrom`: the path alone; the host reads the file when it
        // runs the request, so the bytes never cross the module.
        body_from: match j.get("body_from") {
            None | Some(Json::Null) => None,
            Some(Json::String(path)) => Some(path.clone()),
            Some(_) => return Err("exactBodyFrom must be an app:/ path".into()),
        },
        redirect: Redirect::parse(j.get("redirect").and_then(Json::as_str))?,
    })
}

pub(crate) fn outcome_to_json(outcome: &Outcome) -> Json {
    match outcome {
        Outcome::Storage(_) => {
            json!({"failed":{"kind":"Unsupported","message":"storage result supplied to a fetch continuation"}})
        }
        Outcome::Surface(_) => {
            json!({"failed":{"kind":"Unsupported","message":"surface result supplied to a fetch continuation"}})
        }
        Outcome::Response(r) => json!({
            "response": {
                "status": r.status,
                "headers": r.headers.iter().map(|(k, v)| json!([k, v])).collect::<Vec<_>>(),
                "body": String::from_utf8_lossy(&r.body),
                "bodyBase64": base64(&r.body),
            }
        }),
        Outcome::Failed { kind, message } => json!({
            "failed": { "kind": format!("{kind:?}"), "message": message }
        }),
        Outcome::Message(m) => json!({
            "message": { "event": m.event, "id": m.id, "data": m.data, "coalesced": m.coalesced }
        }),
    }
}

fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk.len();
        let v = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        out.push(T[(v >> 18) as usize & 63] as char);
        out.push(T[(v >> 12) as usize & 63] as char);
        out.push(if n > 1 {
            T[(v >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if n > 2 {
            T[v as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

/// `measureText`'s run from the TypeScript recorder, measured by the
/// canvas's text engine on this thread (LLP 1056 D8): the eleven raw metrics
/// as a JSON array.
pub(crate) fn canvas_measure(
    env: Option<&exact_runner::exact_canvas::Env>,
    json: &str,
) -> Result<String, String> {
    use exact_runner::exact_canvas::font::{Estimate, Font, TextEngine, TextRun};
    let v: serde_json::Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let n = |x: &serde_json::Value| x.as_f64().unwrap_or(0.0);
    let f = &v["font"];
    let font = Font {
        size: n(&f[0]),
        weight: n(&f[1]) as u16,
        style: n(&f[2]) as u8,
        stretch: n(&f[3]),
        caps: n(&f[4]) as u8,
        families: v["families"]
            .as_str()
            .unwrap_or("")
            .split(',')
            .map(str::to_string)
            .collect(),
    };
    let run = TextRun {
        font: &font,
        text: v["text"].as_str().unwrap_or(""),
        rtl: v["rtl"].as_bool().unwrap_or(false),
        letter_spacing: n(&v["ls"]),
        word_spacing: n(&v["ws"]),
        kerning: n(&v["kerning"]) as u8,
    };
    let raw = match env.and_then(|e| e.text.as_ref()) {
        Some(engine) => engine.measure(&run),
        None => Estimate.measure(&run),
    };
    serde_json::to_string(&raw.to_array()).map_err(|e| e.to_string())
}

/// An image handle's natural size for the TypeScript recorder, or nothing
/// while it is not decoded (which asks the host for it).
pub(crate) fn canvas_image(
    env: Option<&exact_runner::exact_canvas::Env>,
    json: &str,
) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    let env = env?;
    let (w, h) =
        exact_runner::exact_canvas::images_in(&env.images).size(env.canvas, v["src"].as_str()?)?;
    Some(format!("[{w},{h}]"))
}
