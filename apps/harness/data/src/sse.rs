//! Server-sent events, and the two streaming dialects read from them:
//! Anthropic's Messages events and OpenAI's chat-completion chunks. Pure:
//! lines in, [`Event`]s out, so a recorded stream tests them.

use serde_json::Value as Json;
use std::collections::BTreeMap;

/// One dispatched server-sent event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sse {
    pub event: String,
    pub data: String,
}

/// The line-level SSE parser: `event:` and `data:` fields, dispatched at a
/// blank line; comments and other fields are ignored.
#[derive(Default)]
pub struct Parser {
    event: String,
    data: Vec<String>,
}

impl Parser {
    /// Feed one line (without its terminator); an event when it completes one.
    pub fn line(&mut self, line: &str) -> Option<Sse> {
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line.is_empty() {
            return self.flush();
        }
        if line.starts_with(':') {
            return None;
        }
        let (field, value) = match line.split_once(':') {
            Some((f, v)) => (f, v.strip_prefix(' ').unwrap_or(v)),
            None => (line, ""),
        };
        match field {
            "event" => self.event = value.to_string(),
            "data" => self.data.push(value.to_string()),
            _ => {}
        }
        None
    }

    /// The event in progress at the end of the stream, if any.
    pub fn flush(&mut self) -> Option<Sse> {
        if self.data.is_empty() {
            self.event.clear();
            return None;
        }
        let sse = Sse {
            event: std::mem::take(&mut self.event),
            data: self.data.join("\n"),
        };
        self.data.clear();
        Some(sse)
    }
}

/// What a provider's stream says, provider-neutral.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// Assistant text to append.
    Text(String),
    /// A complete tool call.
    ToolCall {
        id: String,
        name: String,
        input: Json,
    },
    /// Token counts so far for this request (each a running total, or
    /// `None` when unknown).
    Usage {
        input: Option<f64>,
        output: Option<f64>,
    },
    /// The stream failed.
    Error(String),
    /// The response is complete.
    Done,
}

fn number(v: &Json) -> Option<f64> {
    v.as_f64()
}

fn input_json(text: &str) -> Json {
    if text.trim().is_empty() {
        return Json::Object(Default::default());
    }
    serde_json::from_str(text)
        .unwrap_or_else(|e| serde_json::json!({ "_unparsed": text, "_error": e.to_string() }))
}

/// Anthropic's Messages stream.
#[derive(Default)]
pub struct Anthropic {
    /// Tool-use blocks being streamed, by content index: id, name, JSON.
    tools: BTreeMap<u64, (String, String, String)>,
}

impl Anthropic {
    /// Decode one event.
    pub fn event(&mut self, sse: &Sse) -> Vec<Event> {
        let Ok(v) = serde_json::from_str::<Json>(&sse.data) else {
            return vec![Event::Error(format!("bad event data: {}", sse.data))];
        };
        let kind = v["type"].as_str().unwrap_or(sse.event.as_str());
        match kind {
            "message_start" => {
                let u = &v["message"]["usage"];
                let input = [
                    "input_tokens",
                    "cache_creation_input_tokens",
                    "cache_read_input_tokens",
                ]
                .iter()
                .filter_map(|k| number(&u[*k]))
                .sum::<f64>();
                vec![Event::Usage {
                    input: Some(input),
                    output: number(&u["output_tokens"]),
                }]
            }
            "content_block_start" => {
                let block = &v["content_block"];
                let index = v["index"].as_u64().unwrap_or(0);
                match block["type"].as_str() {
                    Some("tool_use") => {
                        self.tools.insert(
                            index,
                            (
                                block["id"].as_str().unwrap_or("").to_string(),
                                block["name"].as_str().unwrap_or("").to_string(),
                                String::new(),
                            ),
                        );
                        vec![]
                    }
                    Some("text") => match block["text"].as_str() {
                        Some(t) if !t.is_empty() => vec![Event::Text(t.to_string())],
                        _ => vec![],
                    },
                    _ => vec![],
                }
            }
            "content_block_delta" => {
                let d = &v["delta"];
                let index = v["index"].as_u64().unwrap_or(0);
                match d["type"].as_str() {
                    Some("text_delta") => {
                        vec![Event::Text(d["text"].as_str().unwrap_or("").to_string())]
                    }
                    Some("input_json_delta") => {
                        if let Some(tool) = self.tools.get_mut(&index) {
                            tool.2.push_str(d["partial_json"].as_str().unwrap_or(""));
                        }
                        vec![]
                    }
                    _ => vec![],
                }
            }
            "content_block_stop" => {
                let index = v["index"].as_u64().unwrap_or(0);
                match self.tools.remove(&index) {
                    Some((id, name, json)) => vec![Event::ToolCall {
                        id,
                        name,
                        input: input_json(&json),
                    }],
                    None => vec![],
                }
            }
            "message_delta" => vec![Event::Usage {
                input: None,
                output: number(&v["usage"]["output_tokens"]),
            }],
            "message_stop" => vec![Event::Done],
            "error" => vec![Event::Error(format!(
                "{}: {}",
                v["error"]["type"].as_str().unwrap_or("error"),
                v["error"]["message"].as_str().unwrap_or("")
            ))],
            _ => vec![],
        }
    }
}

/// OpenAI's chat-completion chunks (and every compatible server's).
#[derive(Default)]
pub struct OpenAi {
    /// Tool calls being streamed, by index: id, name, argument JSON.
    tools: BTreeMap<u64, (String, String, String)>,
    done: bool,
}

impl OpenAi {
    fn finish(&mut self) -> Vec<Event> {
        let mut out: Vec<Event> = std::mem::take(&mut self.tools)
            .into_values()
            .map(|(id, name, args)| Event::ToolCall {
                id,
                name,
                input: input_json(&args),
            })
            .collect();
        if !self.done {
            self.done = true;
            out.push(Event::Done);
        }
        out
    }

    /// Decode one event; `[DONE]` finishes the stream.
    pub fn event(&mut self, sse: &Sse) -> Vec<Event> {
        if sse.data.trim() == "[DONE]" {
            return self.finish();
        }
        let Ok(v) = serde_json::from_str::<Json>(&sse.data) else {
            return vec![Event::Error(format!("bad chunk: {}", sse.data))];
        };
        if let Some(e) = v.get("error").filter(|e| !e.is_null()) {
            let message = e["message"].as_str().map(str::to_string);
            return vec![Event::Error(message.unwrap_or_else(|| e.to_string()))];
        }
        let mut out = Vec::new();
        if let Some(u) = v.get("usage").filter(|u| u.is_object()) {
            out.push(Event::Usage {
                input: number(&u["prompt_tokens"]),
                output: number(&u["completion_tokens"]),
            });
        }
        let Some(choice) = v["choices"].get(0) else {
            return out;
        };
        let d = &choice["delta"];
        if let Some(t) = d["content"].as_str().filter(|t| !t.is_empty()) {
            out.push(Event::Text(t.to_string()));
        }
        if let Some(calls) = d["tool_calls"].as_array() {
            for (n, call) in calls.iter().enumerate() {
                let index = call["index"].as_u64().unwrap_or(n as u64);
                let tool = self.tools.entry(index).or_default();
                if let Some(id) = call["id"].as_str().filter(|s| !s.is_empty()) {
                    tool.0 = id.to_string();
                }
                if let Some(name) = call["function"]["name"].as_str() {
                    tool.1.push_str(name);
                }
                if let Some(args) = call["function"]["arguments"].as_str() {
                    tool.2.push_str(args);
                }
            }
        }
        if choice["finish_reason"].as_str().is_some() {
            // Tool calls are complete; usage may still follow before [DONE].
            out.extend(
                std::mem::take(&mut self.tools)
                    .into_values()
                    .map(|(id, name, args)| Event::ToolCall {
                        id,
                        name,
                        input: input_json(&args),
                    }),
            );
        }
        out
    }

    /// The end of the body: whatever was still open.
    pub fn end(&mut self) -> Vec<Event> {
        self.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed(text: &str) -> Vec<Sse> {
        let mut p = Parser::default();
        let mut out: Vec<Sse> = text.lines().filter_map(|l| p.line(l)).collect();
        out.extend(p.flush());
        out
    }

    const ANTHROPIC: &str = r#"event: message_start
data: {"type":"message_start","message":{"id":"msg_1","type":"message","role":"assistant","content":[],"model":"claude-sonnet-5-5","stop_reason":null,"usage":{"input_tokens":412,"cache_read_input_tokens":8,"output_tokens":1}}}

event: content_block_start
data: {"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}

event: ping
data: {"type": "ping"}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Let me "}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"look."}}

event: content_block_stop
data: {"type":"content_block_stop","index":0}

event: content_block_start
data: {"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"toolu_01","name":"bash","input":{}}}

event: content_block_delta
data: {"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":""}}

event: content_block_delta
data: {"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{\"command\": \"ls"}}

event: content_block_delta
data: {"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":" -la\"}"}}

event: content_block_stop
data: {"type":"content_block_stop","index":1}

event: message_delta
data: {"type":"message_delta","delta":{"stop_reason":"tool_use","stop_sequence":null},"usage":{"output_tokens":57}}

event: message_stop
data: {"type":"message_stop"}
"#;

    #[test]
    fn anthropic_stream_with_a_tool_call() {
        let mut a = Anthropic::default();
        let events: Vec<Event> = feed(ANTHROPIC).iter().flat_map(|e| a.event(e)).collect();
        assert_eq!(
            events,
            vec![
                Event::Usage {
                    input: Some(420.0),
                    output: Some(1.0)
                },
                Event::Text("Let me ".into()),
                Event::Text("look.".into()),
                Event::ToolCall {
                    id: "toolu_01".into(),
                    name: "bash".into(),
                    input: serde_json::json!({"command": "ls -la"})
                },
                Event::Usage {
                    input: None,
                    output: Some(57.0)
                },
                Event::Done,
            ]
        );
    }

    #[test]
    fn anthropic_error_event() {
        let mut a = Anthropic::default();
        let sse = feed("event: error\ndata: {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"Overloaded\"}}\n");
        assert_eq!(
            a.event(&sse[0]),
            vec![Event::Error("overloaded_error: Overloaded".into())]
        );
    }

    const OPENAI: &str = r#"data: {"id":"c1","object":"chat.completion.chunk","choices":[{"index":0,"delta":{"role":"assistant","content":""},"finish_reason":null}]}

data: {"id":"c1","choices":[{"index":0,"delta":{"content":"Checking"},"finish_reason":null}]}

data: {"id":"c1","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"call_a","type":"function","function":{"name":"read_file","arguments":""}}]},"finish_reason":null}]}

data: {"id":"c1","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"path\":"}}]},"finish_reason":null}]}

data: {"id":"c1","choices":[{"index":0,"delta":{"tool_calls":[{"index":1,"id":"call_b","type":"function","function":{"name":"list_dir","arguments":"{\"path\":\".\"}"}}]},"finish_reason":null}]}

data: {"id":"c1","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"\"Cargo.toml\"}"}}]},"finish_reason":null}]}

data: {"id":"c1","choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}

data: {"id":"c1","choices":[],"usage":{"prompt_tokens":120,"completion_tokens":33,"total_tokens":153}}

data: [DONE]
"#;

    #[test]
    fn openai_stream_with_interleaved_tool_calls() {
        let mut o = OpenAi::default();
        let events: Vec<Event> = feed(OPENAI).iter().flat_map(|e| o.event(e)).collect();
        assert_eq!(
            events,
            vec![
                Event::Text("Checking".into()),
                Event::ToolCall {
                    id: "call_a".into(),
                    name: "read_file".into(),
                    input: serde_json::json!({"path": "Cargo.toml"})
                },
                Event::ToolCall {
                    id: "call_b".into(),
                    name: "list_dir".into(),
                    input: serde_json::json!({"path": "."})
                },
                Event::Usage {
                    input: Some(120.0),
                    output: Some(33.0)
                },
                Event::Done,
            ]
        );
        assert_eq!(o.end(), vec![]);
    }

    #[test]
    fn sse_fields_comments_and_multiline_data() {
        let events = feed(": keepalive\nevent: x\ndata: a\ndata:b\r\n\ndata: tail");
        assert_eq!(
            events,
            vec![
                Sse {
                    event: "x".into(),
                    data: "a\nb".into()
                },
                Sse {
                    event: "".into(),
                    data: "tail".into()
                }
            ]
        );
    }
}
