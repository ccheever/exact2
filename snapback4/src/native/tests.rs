use super::*;
use exact_js::NativeReply;
use exact_runner::{Outcome, Reply};
use serde_json::json;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    mpsc,
};
use std::time::{Duration, Instant};

const VIEWER: &str = "dev:alice";
const SCHEMA: &str = r#"
use identity
table messages:
  author: principal
  body: text <=2000
  at: time
  by byTime: at, id
  public 'messages are public'
  insert <- .author = viewer
  update <- deny
  delete <- .author = viewer
  sync public last 10000 by byTime
query inbox():
  return messages last 50 by byTime
"#;

struct Fixture(PathBuf);
impl Fixture {
    fn module() -> (Self, Module) {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let fixture = Self(std::env::temp_dir().join(format!(
            "exact-snapback4-later-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )));
        let mut module = Module::new("test.snapback4.later", "sqlite.open app:/data").unwrap();
        module
            .configure_storage(
                fixture.0.join("data"),
                fixture.0.join("cache"),
                fixture.0.join("tmp"),
            )
            .unwrap();
        (fixture, module)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn backend() -> Value {
    let compiled = snapback4_lang::compile(&[("schema.q".into(), SCHEMA.into())]);
    assert!(
        compiled.diagnostics.is_empty(),
        "{:?}",
        compiled.diagnostics
    );
    json!({"generation": 1, "schema": compiled.schema,
        "programs": compiled.programs.into_iter().map(|p| p.program).collect::<Vec<_>>()})
}

fn open(backend: Value) -> Value {
    json!({"op": "open", "path": "app:/data/messages.sqlite", "origin": "https://example.test", "viewer": VIEWER, "backend": backend})
}

fn page(count: usize) -> Value {
    let events: Vec<_> = (0..count).map(|i| json!({"table": "messages", "kind": "put", "id": format!("m{i}"),
        "data": {"id": format!("m{i}"), "author": VIEWER, "body": "A message with enough text to exercise row decoding and SQLite persistence.", "at": i}})).collect();
    json!({"op": "apply", "first": true, "page": {"snapshot": true, "events": events,
        "watermark": 1, "more": false, "generation": 1}})
}

fn start(
    handler: &LaterHandler,
    request: Value,
) -> mpsc::Receiver<(std::thread::ThreadId, Outcome)> {
    let (tx, rx) = mpsc::channel();
    handler(
        request,
        NativeReply::new(Reply::new(move |outcome| {
            // A killed host answer may have dropped its receiver; work still releases ownership.
            let _ = tx.send((std::thread::current().id(), outcome));
        })),
    );
    rx
}

fn received(rx: mpsc::Receiver<(std::thread::ThreadId, Outcome)>) -> Value {
    let (_, outcome) = rx.recv_timeout(Duration::from_secs(20)).unwrap();
    let Outcome::Response(response) = outcome else {
        panic!("{outcome:?}")
    };
    assert_eq!(
        response.status,
        200,
        "{}",
        String::from_utf8_lossy(&response.body)
    );
    let answer: Value = serde_json::from_slice(&response.body).unwrap();
    assert!(answer.get("denied").is_none(), "{answer}");
    answer["ok"].clone()
}

#[test]
fn later_owns_apply_off_thread_and_cheap_calls_refuse_then_retry() {
    let (_fixture, mut module) = Fixture::module();
    let handler = module.later().unwrap();
    received(start(&handler, open(backend())));
    let owner = std::thread::current().id();
    // A cheap call actually dispatches on its caller, without a worker hop.
    module.state.lock().unwrap().before_call = Some(Box::new(move |request| {
        assert_eq!(request["op"], "sync_state");
        assert_eq!(std::thread::current().id(), owner);
    }));
    assert!(module
        .call(&json!({"op": "sync_state", "capture": false}))
        .unwrap()
        .get("ok")
        .is_some());

    let (entered, entering) = mpsc::channel();
    let (release, resume) = mpsc::channel();
    module.state.lock().unwrap().before_call = Some(Box::new(move |request| {
        assert_eq!(request["op"], "apply");
        assert_ne!(std::thread::current().id(), owner);
        entered.send(()).unwrap();
        resume.recv_timeout(Duration::from_secs(20)).unwrap();
    }));
    let applying = start(&handler, page(1000));
    entering.recv_timeout(Duration::from_secs(20)).unwrap();
    let status = json!({"op": "sync_state", "capture": false});
    let busy = module.call(&status).unwrap();
    assert_eq!(busy["denied"]["code"], "E_BUSY");
    assert_eq!(busy["denied"]["retryable"], true);
    let retry = start(&handler, status);
    assert!(matches!(retry.try_recv(), Err(mpsc::TryRecvError::Empty)));
    release.send(()).unwrap();
    received(applying);
    assert_eq!(received(retry)["acquired"], true);
    let rows = module
        .call(&json!({"op": "query", "name": "inbox", "viewer": VIEWER}))
        .unwrap();
    assert_eq!(rows["ok"]["data"].as_array().unwrap().len(), 50);
}

#[test]
fn a_killed_apply_reply_still_releases_ownership_and_allows_cancellation() {
    let (_fixture, mut module) = Fixture::module();
    let handler = module.later().unwrap();
    received(start(&handler, open(backend())));
    // Open a client over the kept store, then start an owned round.
    received(start(&handler, json!({"op": "close"})));
    let mut opening = open(Value::Null);
    opening.as_object_mut().unwrap().remove("backend");
    received(start(&handler, opening));
    let step = received(start(&handler, json!({"op": "sync", "round": "killed"})));
    assert!(step["fetch"].is_object(), "{step}");

    let (entered, entering) = mpsc::channel();
    let (release, resume) = mpsc::channel();
    module.state.lock().unwrap().before_call = Some(Box::new(move |request| {
        assert_eq!(request["op"], "apply");
        entered.send(()).unwrap();
        resume.recv_timeout(Duration::from_secs(20)).unwrap();
    }));
    let applying = start(&handler, page(1000));
    entering.recv_timeout(Duration::from_secs(20)).unwrap();
    drop(applying);
    let cancel = json!({"op": "cancel", "round": "killed"});
    assert_eq!(module.call(&cancel).unwrap()["denied"]["code"], "E_BUSY");
    let cancelled = start(&handler, cancel);
    release.send(()).unwrap();
    assert_eq!(received(cancelled), true);
    let next = received(start(&handler, json!({"op": "sync", "round": "live"})));
    assert!(next["fetch"].is_object(), "{next}");
    assert_eq!(
        module
            .call(&json!({"op": "cancel", "round": "killed"}))
            .unwrap()["ok"],
        false
    );
    assert_eq!(
        module
            .call(&json!({"op": "cancel", "round": "live"}))
            .unwrap()["ok"],
        true
    );
}

#[test]
fn measure_apply_thousand_message_rows() {
    for sample in 1..=3 {
        let (_fixture, mut module) = Fixture::module();
        let handler = module.later().unwrap();
        received(start(&handler, open(backend())));
        let request = page(1000);
        let started = Instant::now();
        received(start(&handler, request));
        eprintln!("native apply sample {sample}: 1000 messages in {:.3} ms (including worker dispatch and reply)", started.elapsed().as_secs_f64() * 1000.0);
        assert_eq!(
            module.call(&json!({"op": "state"})).unwrap()["ok"]["watermark"],
            1
        );
    }
}
