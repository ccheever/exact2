//! A turn over real HTTP against a local OpenAI-compatible server (the
//! Ollama route, whose address `OLLAMA_HOST` names): the request carries
//! the tools and, the second time, the tool's result; the streamed reply's
//! text and tool call drive the transcript. No network beyond loopback.

use exact_plan::Value;
use exact_runner::DataSource;
use harness_data::{Harness, Options};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const FIRST: &[&str] = &[
    r#"{"choices":[{"index":0,"delta":{"role":"assistant","content":"Looking "},"finish_reason":null}]}"#,
    r#"{"choices":[{"index":0,"delta":{"content":"around."},"finish_reason":null}]}"#,
    r#"{"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"call_1","type":"function","function":{"name":"list_dir","arguments":"{\"pa"}}]},"finish_reason":null}]}"#,
    r#"{"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"th\":\".\"}"}}]},"finish_reason":null}]}"#,
    r#"{"choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}"#,
    r#"{"choices":[],"usage":{"prompt_tokens":100,"completion_tokens":20}}"#,
    "[DONE]",
];

const SECOND: &[&str] = &[
    r#"{"choices":[{"index":0,"delta":{"content":"All **done**."},"finish_reason":"stop"}]}"#,
    r#"{"choices":[],"usage":{"prompt_tokens":180,"completion_tokens":5}}"#,
    "[DONE]",
];

fn serve(listener: TcpListener, bodies: Arc<Mutex<Vec<String>>>) {
    for (n, stream) in listener.incoming().enumerate() {
        let Ok(mut stream) = stream else { return };
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut length = 0;
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            if line.trim().is_empty() {
                break;
            }
            if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                length = v.trim().parse().unwrap();
            }
        }
        let mut body = vec![0; length];
        reader.read_exact(&mut body).unwrap();
        bodies
            .lock()
            .unwrap()
            .push(String::from_utf8(body).unwrap());
        if n == 2 {
            let body = r#"{"error":{"message":"bad key","type":"auth"}}"#;
            let _ = stream.write_all(
                format!(
                    "HTTP/1.1 401 Unauthorized\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                )
                .as_bytes(),
            );
            continue;
        }
        let events = if n == 0 { FIRST } else { SECOND };
        let _ = stream.write_all(
            b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\n",
        );
        for e in events {
            let _ = stream.write_all(format!("data: {e}\n\n").as_bytes());
            let _ = stream.flush();
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}

fn record(v: &Value) -> &[Value] {
    match v {
        Value::Record(f) => f,
        other => panic!("{other:?}"),
    }
}

fn list(v: &Value) -> Vec<Value> {
    match v {
        Value::List(items) => items.iter().cloned().collect(),
        other => panic!("{other:?}"),
    }
}

#[test]
fn an_openai_compatible_stream_drives_a_turn() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let bodies = Arc::new(Mutex::new(Vec::new()));
    let seen = bodies.clone();
    std::thread::spawn(move || serve(listener, seen));
    std::env::set_var("OLLAMA_HOST", format!("127.0.0.1:{port}"));
    std::env::set_var("EXACT_HARNESS_OLLAMA_MODEL", "fake");

    let mut h = Harness::with_options(Options::offline(None));
    let ack = h.query("setModel", &[Value::str("ollama:fake")]).unwrap();
    assert_eq!(record(&ack)[0], Value::Bool(true));
    h.query("submit", &[Value::str("look around")]).unwrap();
    let start = Instant::now();
    let session = loop {
        let s = h.query("session", &[]).unwrap();
        if record(&s)[4] == Value::Bool(false) {
            break s;
        }
        assert!(start.elapsed() < Duration::from_secs(20), "never idle");
        std::thread::sleep(Duration::from_millis(5));
    };
    let fields = record(&session);
    let kinds: Vec<String> = list(&fields[9])
        .iter()
        .map(|e| record(e)[1].as_str().unwrap().to_string())
        .collect();
    assert_eq!(kinds, ["banner", "user", "assistant", "tool", "assistant"]);
    let tool = list(&fields[9])[3].clone();
    assert_eq!(record(&tool)[3], Value::str("List(.)"));
    assert_eq!(record(&tool)[4], Value::str("ok"));
    // Tokens: 100 + 180 in, 20 + 5 out; the gauge reads the last request.
    assert_eq!(fields[6], Value::Number(280.0));
    assert_eq!(fields[7], Value::Number(25.0));
    assert_eq!(fields[8], Value::Number(0.09));

    // A refused request is an error entry, and the session is idle again.
    h.query("submit", &[Value::str("again")]).unwrap();
    let start = Instant::now();
    loop {
        let s = h.query("session", &[]).unwrap();
        let entries = list(&record(&s)[9]);
        let last = record(entries.last().unwrap());
        if last[1] == Value::str("error") {
            let block = record(&list(&last[5])[0]).to_vec();
            let run = record(&list(&block[4])[0]).to_vec();
            assert_eq!(run[0], Value::str("HTTP 401: bad key"));
            assert_eq!(record(&s)[4], Value::Bool(false));
            break;
        }
        assert!(start.elapsed() < Duration::from_secs(20), "no error entry");
        std::thread::sleep(Duration::from_millis(5));
    }

    let bodies = bodies.lock().unwrap();
    assert_eq!(bodies.len(), 3);
    let first: serde_json::Value = serde_json::from_str(&bodies[0]).unwrap();
    assert_eq!(first["model"], "fake");
    assert_eq!(first["stream"], true);
    assert_eq!(first["tools"].as_array().unwrap().len(), 6);
    let second: serde_json::Value = serde_json::from_str(&bodies[1]).unwrap();
    let messages = second["messages"].as_array().unwrap();
    assert_eq!(messages[2]["tool_calls"][0]["id"], "call_1");
    assert_eq!(messages[3]["role"], "tool");
    assert_eq!(messages[3]["tool_call_id"], "call_1");
    assert!(messages[3]["content"]
        .as_str()
        .unwrap()
        .contains("Cargo.toml"));
}
