//! Anthropic's Messages API, streamed.

use super::{post_sse, Ask};
use crate::sse::{Anthropic, Event};
use crate::state::{Msg, Part, Role};
use crate::tools::definitions;
use serde_json::{json, Value as Json};

/// The conversation as Messages API `messages`: consecutive messages of
/// one role merged, tool results as `tool_result` blocks in a user turn.
pub fn messages(history: &[Msg]) -> Vec<Json> {
    let mut out: Vec<(Role, Vec<Json>)> = Vec::new();
    for msg in history {
        let blocks: Vec<Json> = msg
            .parts
            .iter()
            .filter_map(|p| match p {
                Part::Text(t) if t.is_empty() => None,
                Part::Text(t) => Some(json!({"type":"text","text":t})),
                Part::ToolUse { id, name, input } => {
                    Some(json!({"type":"tool_use","id":id,"name":name,"input":input}))
                }
                Part::ToolResult {
                    id,
                    content,
                    is_error,
                } => Some(json!({"type":"tool_result","tool_use_id":id,"content":content,"is_error":is_error})),
            })
            .collect();
        if blocks.is_empty() {
            continue;
        }
        match out.last_mut() {
            Some((role, existing)) if *role == msg.role => existing.extend(blocks),
            _ => out.push((msg.role, blocks)),
        }
    }
    out.into_iter()
        .map(|(role, content)| {
            json!({"role": if role == Role::User {"user"} else {"assistant"}, "content": content})
        })
        .collect()
}

/// Stream one reply.
pub fn stream(
    key: &str,
    model: &str,
    ask: &Ask<'_>,
    sink: &mut dyn FnMut(Event),
) -> Result<(), String> {
    let tools: Vec<Json> = definitions()
        .into_iter()
        .map(|(name, description, schema)| {
            json!({"name":name,"description":description,"input_schema":schema})
        })
        .collect();
    let body = json!({
        "model": model,
        "max_tokens": 8192,
        "stream": true,
        "system": ask.system,
        "tools": tools,
        "messages": messages(ask.history),
    });
    let headers = [
        ("x-api-key", key.to_string()),
        ("anthropic-version", "2023-06-01".to_string()),
    ];
    let mut decoder = Anthropic::default();
    let mut failed = None;
    post_sse(
        "https://api.anthropic.com/v1/messages",
        &headers,
        &body,
        ask.cancel,
        &mut |sse| {
            for event in decoder.event(&sse) {
                if let Event::Error(e) = &event {
                    failed = Some(e.clone());
                    return false;
                }
                sink(event);
            }
            true
        },
    )?;
    match failed {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_results_follow_their_calls_in_one_user_turn() {
        let history = vec![
            Msg::user("hi"),
            Msg {
                role: Role::Assistant,
                parts: vec![
                    Part::Text("ok".into()),
                    Part::ToolUse {
                        id: "t1".into(),
                        name: "bash".into(),
                        input: json!({"command":"ls"}),
                    },
                ],
            },
            Msg {
                role: Role::User,
                parts: vec![Part::ToolResult {
                    id: "t1".into(),
                    content: "a".into(),
                    is_error: false,
                }],
            },
            Msg::user("next"),
        ];
        let m = messages(&history);
        assert_eq!(m.len(), 3);
        assert_eq!(m[2]["content"][0]["type"], "tool_result");
        assert_eq!(m[2]["content"][1]["text"], "next");
    }
}
