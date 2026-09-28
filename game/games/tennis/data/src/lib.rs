//! The `jev` resource. Contract calls `jev(hud.ask)` whenever the world
//! publishes a new numbered question; this source turns it into a Jev
//! evaluation request and turns the reply into the normalized plan the world
//! reads from its live `plan` argument. The world never sees HTTP.
//!
//! Where the request goes: a native host with `AI_GATEWAY_API_KEY` in its
//! environment calls the Vercel AI Gateway directly (the key rides only in the
//! request header, read at request time). Everything else, and every web
//! build, posts to the dev proxy on 127.0.0.1:47913 (`jev-proxy.mjs`), which
//! holds the key; a browser bundle never contains one.
//! @ref game/diaries/003-tennis.md "Where the HTTP call lives"
use exact_runner::{Answer, DataError, DataSource, Outcome, Request, Store, Value};
use serde_json::{json, Value as Json};

pub const GATEWAY: &str = "https://ai-gateway.vercel.sh";
pub const PROXY: &str = "http://127.0.0.1:47913";
const PATH: &str = "/v1/evaluate";
const MODEL: &str = "typesafe-ai/jev";
const AGGRESSION: [&str; 5] = ["very safe", "safe", "balanced", "aggressive", "all out"];

#[derive(Default)]
pub struct Jev;

/// Origins are what grants compare, so both endpoints are fixed here.
const GRANTS: &str = "net.fetch https://ai-gateway.vercel.sh\nnet.fetch http://127.0.0.1:47913";

#[cfg(not(target_arch = "wasm32"))]
fn endpoint() -> (&'static str, Option<String>) {
    match std::env::var("AI_GATEWAY_API_KEY") {
        Ok(key) if !key.trim().is_empty() => (GATEWAY, Some(key.trim().to_string())),
        _ => (PROXY, None),
    }
}
#[cfg(target_arch = "wasm32")]
fn endpoint() -> (&'static str, Option<String>) {
    (PROXY, None)
}

/// The questions Jev answers. Serves and rally shots share keys so the world
/// reads one shape.
pub fn questions(serve: bool) -> Json {
    let aggression = json!({
        "type": "score",
        "instructions": "How much risk this shot takes: pace, and how close to the lines it aims.",
        "criteria": AGGRESSION,
    });
    if serve {
        return json!({
            "shot": {"type": "choice", "instructions": "Choose Jev's serve.", "criteria": {
                "flat": "a fast flat serve: high risk, high reward",
                "kick": "a high-bouncing topspin kick serve: the safest",
                "slice": "a slice serve that curves away from the receiver"}},
            "target": {"type": "choice", "instructions": "Where in the service box to aim.", "criteria": {
                "wide": "wide, pulling the receiver off the court",
                "body": "into the receiver's body",
                "t": "down the middle, at the T"}},
            "aggression": aggression,
            "approach_net": {"type": "boolean", "instructions": "Serve and volley: rush the net after this serve?"},
        });
    }
    json!({
        "shot": {"type": "choice", "instructions": "Choose Jev's next shot. Attack a weak side or an opponent out of position; defend when stretched.", "criteria": {
            "drive": "a heavy topspin drive with safe net clearance",
            "flat": "a flat, fast drive that skids through the court",
            "slice": "a low backspin slice that stays low and slows the rally",
            "drop_shot": "a soft short ball just over the net, good against a deep opponent",
            "lob": "a high topspin lob over an opponent at the net"}},
        "target": {"type": "choice", "instructions": "Where to aim, relative to the opponent's position and strokes.", "criteria": {
            "forehand": "the opponent's forehand side",
            "backhand": "the opponent's backhand side",
            "body": "straight at the opponent's body",
            "open_court": "the side of the court the opponent is not covering"}},
        "aggression": aggression,
        "approach_net": {"type": "boolean", "instructions": "Should Jev follow this shot in to the net to volley the next ball?"},
    })
}

/// The evaluation request for a published question, or None when there is none.
pub fn request(ask: &str) -> Option<(u64, Json)> {
    let ask: Json = serde_json::from_str(ask).ok()?;
    let id = ask["id"].as_u64()?;
    let serve = ask["kind"] == "serve";
    Some((
        id,
        json!({"model": MODEL, "state": ask["state"], "questions": questions(serve)}),
    ))
}

fn failure(id: u64, error: &str) -> Value {
    Value::str(&json!({"id": id, "ok": false, "error": error}).to_string())
}

fn choice(answer: &Json) -> Json {
    let p: serde_json::Map<String, Json> = answer["probabilities"]
        .as_object()
        .map(|m| {
            m.iter()
                .filter_map(|(k, v)| Some((k.clone(), json!(v.as_f64()?))))
                .collect()
        })
        .unwrap_or_default();
    json!({
        "choice": answer["choice"].as_str().unwrap_or_default(),
        "confidence": answer["confidence"].as_f64().unwrap_or(0.0),
        "p": p,
    })
}

/// Jev's reply as the world's `brain::Answer`, every probability kept.
pub fn normalize(id: u64, reply: &Json) -> Value {
    let a = &reply["answers"];
    if !a["shot"].is_object() || !a["target"].is_object() {
        return failure(id, "reply without answers");
    }
    let mut aggression = choice(&a["aggression"]);
    aggression["score"] = json!(a["aggression"]["score"].as_f64().unwrap_or(2.0));
    aggression["levels"] = json!(AGGRESSION.len());
    if let Some(m) = aggression.as_object_mut() {
        m.remove("choice");
        m.remove("confidence");
    }
    Value::str(
        &json!({
            "id": id,
            "ok": true,
            "error": "",
            "shot": choice(&a["shot"]),
            "target": choice(&a["target"]),
            "aggression": aggression,
            "approach": a["approach_net"]["probability"].as_f64().unwrap_or(0.0),
        })
        .to_string(),
    )
}

fn decode(outcome: Outcome) -> Result<Json, String> {
    match outcome {
        Outcome::Response(r) if r.status == 200 => {
            serde_json::from_slice(&r.body).map_err(|_| "malformed reply".into())
        }
        Outcome::Response(r) if r.status == 401 || r.status == 403 => {
            Err(format!("HTTP {}: key refused", r.status))
        }
        Outcome::Response(r) => Err(format!("HTTP {}", r.status)),
        Outcome::Failed { kind, .. } => Err(format!("no connection ({kind:?})")),
        _ => Err("unexpected reply".into()),
    }
}

impl DataSource for Jev {
    fn grants(&self) -> &str {
        GRANTS
    }

    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        match source {
            "jev" => Ok(Value::str("")),
            other => Err(DataError::UnknownSource(other.into())),
        }
    }

    fn answer(&mut self, _: &mut Store, source: &str, args: &[Value]) -> Result<Answer, DataError> {
        if source != "jev" {
            return Err(DataError::UnknownSource(source.into()));
        }
        let ask = args.first().and_then(Value::as_str).unwrap_or("");
        // No question (and the bake's boot request): answer now, with nothing.
        if ask.is_empty() {
            return Ok(Answer::Now(Value::str("")));
        }
        let Some((_, body)) = request(ask) else {
            return Ok(Answer::Now(failure(0, "malformed question")));
        };
        let (base, key) = endpoint();
        let mut request = Request::post_json(&format!("{base}{PATH}"), &body.to_string())
            .independent_http(1 << 18);
        if let Some(key) = key {
            request = request.header("authorization", &format!("Bearer {key}"));
        }
        Ok(Answer::Later(request))
    }

    fn parse(
        &mut self,
        _: &mut Store,
        source: &str,
        args: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        if source != "jev" {
            return Err(DataError::UnknownSource(source.into()));
        }
        let id = args
            .first()
            .and_then(Value::as_str)
            .and_then(request)
            .map_or(0, |(id, _)| id);
        Ok(Answer::Now(match decode(outcome) {
            Ok(reply) => normalize(id, &reply),
            Err(error) => failure(id, &error),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use exact_runner::Response;

    const ASK: &str = r#"{"id":7,"kind":"rally","state":{"decide":"your next shot"}}"#;
    // A real reply's shape (2026-09-23), trimmed of gateway metadata.
    const REPLY: &str = r#"{"model":"typesafe-ai/jev","answers":{"shot":{"type":"choice","choice":"drive","probabilities":{"drive":0.46,"drop_shot":0.02,"flat":0.43,"slice":0.04,"lob":0.05},"confidence":0.33},"target":{"type":"choice","choice":"backhand","probabilities":{"body":0,"backhand":0.98,"forehand":0.02,"open_court":0},"confidence":0.97},"aggression":{"type":"score","score":2.34,"probabilities":{"0":0.02,"1":0.18,"2":0.28,"3":0.5,"4":0.02},"confidence":0.42},"approach_net":{"type":"boolean","probability":0.44}},"usage":{"inputTokens":653,"outputTokens":142}}"#;

    #[test]
    fn empty_question_answers_now_and_a_question_posts_to_jev() {
        let mut store = Store::default();
        let mut jev = Jev;
        assert_eq!(
            jev.answer(&mut store, "jev", &[Value::str("")]).unwrap(),
            Answer::Now(Value::str(""))
        );
        let Answer::Later(r) = jev.answer(&mut store, "jev", &[Value::str(ASK)]).unwrap() else {
            panic!("expected a request")
        };
        assert_eq!(r.method, "POST");
        assert!(r.url.ends_with("/v1/evaluate"));
        let body: Json = serde_json::from_slice(&r.body).unwrap();
        assert_eq!(body["model"], "typesafe-ai/jev");
        assert_eq!(body["state"]["decide"], "your next shot");
        assert_eq!(body["questions"]["aggression"]["criteria"][4], "all out");
        assert!(r.url.starts_with(PROXY) || r.headers.iter().any(|(k, _)| k == "authorization"));
    }

    #[test]
    fn replies_normalize_and_failures_are_data() {
        let mut store = Store::default();
        let mut jev = Jev;
        let reply = Outcome::Response(Response {
            status: 200,
            headers: vec![],
            body: REPLY.as_bytes().to_vec(),
        });
        let Answer::Now(plan) = jev
            .parse(&mut store, "jev", &[Value::str(ASK)], reply)
            .unwrap()
        else {
            panic!()
        };
        let plan = plan.text();
        let plan: Json = serde_json::from_str(&plan).unwrap();
        assert_eq!(plan["id"], 7);
        assert_eq!(plan["ok"], true);
        assert_eq!(plan["shot"]["p"]["flat"], 0.43);
        assert_eq!(plan["aggression"]["levels"], 5);
        assert_eq!(plan["approach"], 0.44);
        let refused = Outcome::Response(Response {
            status: 401,
            headers: vec![],
            body: vec![],
        });
        let Answer::Now(plan) = jev
            .parse(&mut store, "jev", &[Value::str(ASK)], refused)
            .unwrap()
        else {
            panic!()
        };
        let plan = plan.text();
        assert!(plan.contains("\"ok\":false") && plan.contains("key refused"));
    }
}
