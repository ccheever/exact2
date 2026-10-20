//! An OpenRouter turn over real HTTP against a local server: the request's
//! attribution headers and usage accounting, keepalive comments, reasoning
//! that stays hidden, a tool call with broken JSON arguments, a mid-stream
//! error, and an error that echoes the key back, which must not show.

use exact_plan::Value;
use exact_runner::DataSource;
use harness_data::shapes::{Ack, ContractValue, Entry, Session};
use harness_data::{Harness, Options};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const KEY: &str = "sk-or-v1-TESTONLY-feedfacecafebeef";

const FIRST: &str = concat!(
    ": OPENROUTER PROCESSING\n\n",
    "data: {\"id\":\"gen-1\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"\",\"reasoning\":\"hmm\"},\"finish_reason\":null}]}\n\n",
    ": OPENROUTER PROCESSING\n\n",
    "data: {\"id\":\"gen-1\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Running it.\"},\"finish_reason\":null}]}\n\n",
    "data: {\"id\":\"gen-1\",\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_9\",\"type\":\"function\",\"function\":{\"name\":\"bash\",\"arguments\":\"{\\\"command\\\": ls\"}}]},\"finish_reason\":null}]}\n\n",
    "data: {\"id\":\"gen-1\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\n",
    "data: {\"id\":\"gen-1\",\"choices\":[],\"usage\":{\"prompt_tokens\":50,\"completion_tokens\":9,\"cost\":0.0001}}\n\n",
    "data: [DONE]\n\n",
);

const SECOND: &str = concat!(
    "data: {\"id\":\"gen-2\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Sorry, \"},\"finish_reason\":null}]}\n\n",
    ": OPENROUTER PROCESSING\n\n",
    "data: {\"id\":\"gen-2\",\"error\":{\"code\":502,\"message\":\"Upstream error\",\"metadata\":{\"provider_name\":\"Anthropic\",\"raw\":\"overloaded\"}},\"choices\":[{\"index\":0,\"delta\":{\"content\":\"\"},\"finish_reason\":\"error\"}]}\n\n",
);

/// What the server saw: each request's header lines and body.
type Seen = Arc<Mutex<Vec<(String, String)>>>;

fn serve(listener: TcpListener, seen: Seen) {
    for (n, stream) in listener.incoming().enumerate() {
        let Ok(mut stream) = stream else { return };
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let (mut length, mut head, mut auth) = (0, String::new(), String::new());
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            if line.trim().is_empty() {
                break;
            }
            let lower = line.to_ascii_lowercase();
            if let Some(v) = lower.strip_prefix("content-length:") {
                length = v.trim().parse().unwrap();
            }
            if lower.starts_with("authorization:") {
                auth = line.trim().to_string();
            }
            head.push_str(&line);
        }
        let mut body = vec![0; length];
        reader.read_exact(&mut body).unwrap();
        seen.lock()
            .unwrap()
            .push((head, String::from_utf8(body).unwrap()));
        let reply = match n {
            0 => FIRST.to_string(),
            1 => SECOND.to_string(),
            _ => {
                // A server that echoes the request's credentials back.
                let body = format!(
                    r#"{{"error":{{"message":"invalid credentials: {auth}","code":401}}}}"#
                );
                let _ = stream.write_all(
                    format!("HTTP/1.1 401 Unauthorized\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len()).as_bytes(),
                );
                continue;
            }
        };
        let _ = stream.write_all(
            b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\n",
        );
        for part in reply.split_inclusive("\n\n") {
            let _ = stream.write_all(part.as_bytes());
            let _ = stream.flush();
            std::thread::sleep(Duration::from_millis(3));
        }
    }
}

fn session(h: &mut Harness) -> Session {
    Session::from_value(&h.query("session", &[]).unwrap()).expect("a Session")
}

fn ack(h: &mut Harness, source: &str, args: &[&str]) -> bool {
    let args: Vec<Value> = args.iter().map(|a| Value::str(a)).collect();
    Ack::from_value(&h.query(source, &args).unwrap())
        .expect("an Ack")
        .ok
}

fn idle(h: &mut Harness) -> Session {
    let start = Instant::now();
    loop {
        let s = session(h);
        if !s.busy {
            return s;
        }
        assert!(start.elapsed() < Duration::from_secs(20), "never idle");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn texts(entry: &Entry) -> String {
    let mut out = String::new();
    for block in &entry.blocks {
        for run in &block.runs {
            out.push_str(&run.text);
        }
        for line in &block.lines {
            for run in &line.runs {
                out.push_str(&run.text);
            }
            out.push('\n');
        }
    }
    out
}

#[test]
fn an_openrouter_turn_end_to_end() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}/api/v1", listener.local_addr().unwrap());
    let seen: Seen = Arc::default();
    let server = seen.clone();
    std::thread::spawn(move || serve(listener, server));
    let dir = std::env::temp_dir().join(format!("exact-harness-or-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut h = Harness::with_options(Options {
        config_dir: Some(dir.clone()),
        openrouter_base: base,
        network: false,
    });
    assert!(ack(&mut h, "setModel", &["mock"]));
    assert!(ack(&mut h, "setKey", &["openrouter", KEY]));

    assert!(ack(&mut h, "submit", &["go"]));
    let s = idle(&mut h);
    let entries = &s.entries;
    let kinds: Vec<&str> = entries.iter().map(|e| e.kind.as_str()).collect();
    assert_eq!(
        kinds,
        ["banner", "user", "assistant", "tool", "assistant", "error"]
    );
    assert_eq!(texts(&entries[2]), "Running it.");
    assert_eq!(entries[3].status, "error");
    assert!(texts(&entries[3]).starts_with("invalid JSON arguments"));
    assert_eq!(texts(&entries[4]), "Sorry,");
    assert_eq!(texts(&entries[5]), "Upstream error (Anthropic): overloaded");
    // The broken call never asked for approval.
    assert_eq!(s.approval.id, "");

    assert!(ack(&mut h, "submit", &["again"]));
    let s = idle(&mut h);
    let last = texts(s.entries.last().unwrap());
    assert!(last.starts_with("HTTP 401: invalid credentials"), "{last}");
    assert!(last.contains("[redacted]"), "{last}");
    let shown = format!("{s:?}");
    assert!(!shown.contains(KEY) && !shown.contains("feedfacecafebeef"));

    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 3);
    let (head, body) = &seen[0];
    let head = head.to_ascii_lowercase();
    assert!(head.contains(&format!(
        "authorization: bearer {}",
        KEY.to_ascii_lowercase()
    )));
    assert!(head.contains("http-referer: https://github.com/ccheever/exact2"));
    assert!(head.contains("x-title: exact harness"));
    let body: serde_json::Value = serde_json::from_str(body).unwrap();
    assert_eq!(body["model"], "anthropic/claude-sonnet-4.5");
    assert_eq!(body["usage"]["include"], true);
    assert_eq!(body["stream_options"]["include_usage"], true);
    assert_eq!(body["tools"][0]["type"], "function");
    let second: serde_json::Value = serde_json::from_str(&seen[1].1).unwrap();
    let tool_msg = second["messages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["role"] == "tool")
        .unwrap()
        .clone();
    assert_eq!(tool_msg["tool_call_id"], "call_9");
    assert!(tool_msg["content"]
        .as_str()
        .unwrap()
        .starts_with("invalid JSON arguments"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_interrupt_stops_a_stalled_stream_at_once() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}/api/v1", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { return };
            let _ = stream.write_all(
                b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\n\
                  data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Start\"},\"finish_reason\":null}]}\n\n",
            );
            let _ = stream.flush();
            // Then nothing, for longer than the test runs.
            std::thread::sleep(Duration::from_secs(30));
        }
    });
    let mut h = Harness::with_options(Options {
        config_dir: None,
        openrouter_base: base,
        network: false,
    });
    assert!(ack(&mut h, "setKey", &["openrouter", KEY]));
    assert!(ack(&mut h, "setModel", &["openrouter:openrouter/auto"]));
    assert!(ack(&mut h, "submit", &["go"]));
    let start = Instant::now();
    loop {
        if session(&mut h).phase == "streaming" {
            break;
        }
        assert!(start.elapsed() < Duration::from_secs(10), "never streamed");
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(ack(&mut h, "interrupt", &[]));
    let s = session(&mut h);
    assert!(!s.busy);
    assert_eq!(texts(s.entries.last().unwrap()), "Start[interrupted]");
    // The work stopped, not only the entry: the body reader's thread ends
    // within a read slice though the server never sends another byte.
    let stopped = Instant::now();
    while harness_data::providers::HTTP_READERS.load(std::sync::atomic::Ordering::SeqCst) > 0 {
        assert!(
            stopped.elapsed() < Duration::from_secs(1),
            "the reader outlived its cancel"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    eprintln!("reader gone {:?} after the interrupt", stopped.elapsed());
    // A new turn starts at once, though the old reader is still blocked.
    assert!(ack(&mut h, "submit", &["again"]));
    assert!(ack(&mut h, "interrupt", &[]));
}
