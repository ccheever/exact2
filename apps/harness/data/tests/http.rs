//! A turn over real HTTP against a local OpenAI-compatible server (the
//! Ollama route, whose address `OLLAMA_HOST` names): the request carries
//! the tools and, the second time, the tool's result; the streamed reply's
//! text and tool call drive the transcript. No network beyond loopback.

use exact_plan::Value;
use exact_runner::DataSource;
use harness_data::shapes::{Ack, ContractValue, Session};
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

fn session(h: &mut Harness) -> Session {
    Session::from_value(&h.query("session", &[]).unwrap()).expect("a Session")
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
    let ack = Ack::from_value(&h.query("setModel", &[Value::str("ollama:fake")]).unwrap());
    assert_eq!(ack, Some(Ack { ok: true }));
    h.query("submit", &[Value::str("look around")]).unwrap();
    let start = Instant::now();
    let s = loop {
        let s = session(&mut h);
        if !s.busy {
            break s;
        }
        assert!(start.elapsed() < Duration::from_secs(20), "never idle");
        std::thread::sleep(Duration::from_millis(5));
    };
    let kinds: Vec<&str> = s.entries.iter().map(|e| e.kind.as_str()).collect();
    assert_eq!(kinds, ["banner", "user", "assistant", "tool", "assistant"]);
    assert_eq!(s.entries[3].title, "List(.)");
    assert_eq!(s.entries[3].status, "ok");
    assert_eq!(s.entries[2].model, "Ollama fake");
    // Tokens: 100 + 180 in, 20 + 5 out; the gauge reads the last request.
    assert_eq!(s.tokens_in, 280.0);
    assert_eq!(s.tokens_out, 25.0);
    assert_eq!(s.context_pct, 0.09);

    // A refused request is an error entry, and the session is idle again.
    h.query("submit", &[Value::str("again")]).unwrap();
    let start = Instant::now();
    loop {
        let s = session(&mut h);
        let last = s.entries.last().unwrap();
        if last.kind == "error" {
            assert_eq!(last.blocks[0].runs[0].text, "HTTP 401: bad key");
            assert!(!s.busy);
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
