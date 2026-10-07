//! OpenAI-compatible chat completions, streamed: OpenAI, OpenRouter and
//! Ollama all speak it.

use super::{post_sse, Ask, Endpoint};
use crate::sse::{Event, OpenAi};
use crate::state::{Msg, Part, Role};
use crate::tools::definitions;
use serde_json::{json, Value as Json};

/// The conversation as chat `messages`: a system message first, an
/// assistant's tool calls as `tool_calls`, each result its own `tool`
/// message.
pub fn messages(system: &str, history: &[Msg]) -> Vec<Json> {
    let mut out = vec![json!({"role":"system","content":system})];
    for msg in history {
        let text = msg.text();
        match msg.role {
            Role::User => {
                for p in &msg.parts {
                    if let Part::ToolResult { id, content, .. } = p {
                        out.push(json!({"role":"tool","tool_call_id":id,"content":content}));
                    }
                }
                if !text.is_empty() {
                    out.push(json!({"role":"user","content":text}));
                }
            }
            Role::Assistant => {
                let calls: Vec<Json> = msg
                    .parts
                    .iter()
                    .filter_map(|p| match p {
                        Part::ToolUse { id, name, input } => Some(json!({
                            "id": id, "type": "function",
                            "function": {"name": name, "arguments": input.to_string()}
                        })),
                        _ => None,
                    })
                    .collect();
                let mut m = json!({"role":"assistant","content":text});
                if !calls.is_empty() {
                    m["tool_calls"] = Json::Array(calls);
                }
                out.push(m);
            }
        }
    }
    out
}

/// Stream one reply.
pub fn stream(
    endpoint: &Endpoint,
    ask: &Ask<'_>,
    sink: &mut dyn FnMut(Event),
) -> Result<(), String> {
    let tools: Vec<Json> = definitions()
        .into_iter()
        .map(|(name, description, schema)| {
            json!({"type":"function","function":{"name":name,"description":description,"parameters":schema}})
        })
        .collect();
    let mut body = json!({
        "model": endpoint.model,
        "stream": true,
        "stream_options": {"include_usage": true},
        "tools": tools,
        "messages": messages(ask.system, ask.history),
    });
    if let (Some(body), Some(extra)) = (body.as_object_mut(), endpoint.extra.as_object()) {
        body.extend(extra.clone());
    }
    let mut headers: Vec<(&str, String)> = Vec::new();
    if let Some(key) = &endpoint.key {
        headers.push(("authorization", format!("Bearer {key}")));
    }
    headers.extend(endpoint.headers.iter().map(|(k, v)| (*k, v.clone())));
    let mut decoder = OpenAi::default();
    let mut failed = None;
    let url = format!("{}/chat/completions", endpoint.base.trim_end_matches('/'));
    post_sse(&url, &headers, &body, ask.cancel, &mut |sse| {
        for event in decoder.event(&sse) {
            if let Event::Error(e) = &event {
                failed = Some(e.clone());
                return false;
            }
            sink(event);
        }
        true
    })?;
    if let Some(e) = failed {
        return Err(e);
    }
    for event in decoder.end() {
        sink(event);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_calls_and_results_take_chat_form() {
        let history = vec![
            Msg::user("hi"),
            Msg {
                role: Role::Assistant,
                parts: vec![Part::ToolUse {
                    id: "c1".into(),
                    name: "list_dir".into(),
                    input: json!({"path":"."}),
                }],
            },
            Msg {
                role: Role::User,
                parts: vec![Part::ToolResult {
                    id: "c1".into(),
                    content: "a/".into(),
                    is_error: false,
                }],
            },
        ];
        let m = messages("sys", &history);
        assert_eq!(m.len(), 4);
        assert_eq!(
            m[2]["tool_calls"][0]["function"]["arguments"],
            "{\"path\":\".\"}"
        );
        assert_eq!(m[3]["role"], "tool");
    }
}
