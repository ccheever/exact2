//! The runner's half of `@exact/language-model`'s seam (LLP 1133.016 D15
//! items 1-3; §10's drives 7 and 8), with the implementation played as the
//! host's native slot and the data module's `tools` by a Rust stand-in:
//! a handler runs only for a handle whose call listed it, under the `tool`
//! origin, which each module call it makes carries; a send let go is
//! cancelled at the implementation; a poll ends with its answer.

use exact_runner::{
    Answer, CallOrigin, DataError, Dispatch, Outcome, Reply, Request, Response, Runner, Store,
    Value, Work,
};
use serde_json::{json, Value as Json};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};

const APP: &str = r#"use createLanguageModel, prompt, languageModelResponse, LanguageModelTool, LanguageModelInfo, LanguageModelResult, LanguageModelResponse from "@exact/language-model"

shape NoteQuery
  query: string

component App
  state response = ""
  state noted = 0
  mutation made as shape LanguageModelInfo
  mutation said as shape LanguageModelResult
  derive session = match made { case some(m) => m.id, case none => "" }
  resource reply = languageModelResponse(response=response, interval=50) as shape LanguageModelResponse else empty()
  action create
    send made = createLanguageModel(tools=[LanguageModelTool(name="findNotes", description="Finds notes", inputSchema="NoteQuery")])
  action ask
    send said = prompt(session=session, input="hi")
  action read(id: string)
    response = id
  tool note writes closed "Count a note." = note
  action note
    noted = noted + 1
  view
    column
      text session
      text reply.state
"#;

/// The app, with the repository's package installed beside it.
struct App(PathBuf);
impl App {
    fn new(name: &str) -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "exact-language-model-seam-{name}-{}-{n}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/language-model");
        let to = path.join("node_modules/@exact/language-model");
        for file in [
            "exact.json",
            "package.json",
            "android/exact.json",
            "apple/exact.json",
            "web/exact.json",
        ] {
            std::fs::create_dir_all(to.join(file).parent().unwrap()).unwrap();
            std::fs::copy(from.join(file), to.join(file)).unwrap();
        }
        std::fs::write(path.join("app.contract"), APP).unwrap();
        Self(path.canonicalize().unwrap())
    }
}
impl Drop for App {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The data module's `tools`, as the bake's TypeScript source runs them:
/// `findNotes` answers its notes, or, asked for `"create"`, awaits a
/// `createLanguageModel` of its own and answers what it was told.
struct Tools;
impl exact_runner::DataSource for Tools {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
    fn answer(&mut self, _: &mut Store, source: &str, args: &[Value]) -> Result<Answer, DataError> {
        let args: Json = serde_json::from_str(args[0].as_str().unwrap()).unwrap();
        match (source, args["query"].as_str()) {
            // A prompt of its own, which the host holds.
            ("tool:findNotes", Some("hold")) => {
                let body = json!({ "extension": "language-model", "send": "prompt", "args": {} });
                Ok(Answer::Later(Request::native(
                    body.to_string().into_bytes(),
                )))
            }
            ("tool:findNotes", Some("create")) => {
                let body = json!({ "extension": "language-model", "send": "createLanguageModel", "args": {} });
                Ok(Answer::Later(Request::native(
                    body.to_string().into_bytes(),
                )))
            }
            // A sleep on a timer of its own (LLP 1027.000 D1), due at 500 ms.
            ("tool:findNotes", Some("sleep")) => {
                Ok(Answer::Later(Request::continuation(9).due(500)))
            }
            ("tool:findNotes", _) => Ok(Answer::Now(Value::from(r#"[{"title":"Lifetimes"}]"#))),
            _ => Err(DataError::UnknownSource(format!(
                "the data module's `tools` has no `{source}`"
            ))),
        }
    }
    fn parse(
        &mut self,
        _: &mut Store,
        _: &str,
        _: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        let Outcome::Response(Response { body, .. }) = outcome else {
            panic!("an answer");
        };
        Ok(Answer::Now(Value::from(String::from_utf8(body).unwrap())))
    }
}

type Calls = Arc<Mutex<Vec<Json>>>;
type Held = Arc<Mutex<Vec<Reply>>>;

/// A runner over the app, its implementation bound as in production: a
/// create answers `s1` unless a tool made it (the download rule's
/// refusal); a prompt is held until the test answers it; a response reads
/// `generating` until `r1` has ended.
fn boot(name: &str) -> (App, Runner<Tools>, Calls, Held) {
    let app = App::new(name);
    let (plan, _) = contract::compile_path_all(&app.0.join("app.contract"), false)
        .unwrap_or_else(|e| panic!("{e:?}"));
    let mut r = Runner::boot(
        plan,
        Tools,
        exact_kernel::Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let (calls, held): (Calls, Held) = Default::default();
    let (seen, holding) = (calls.clone(), held.clone());
    r.native_slot().host(Some(Arc::new(move |body: Vec<u8>, reply: Reply| {
        let v: Json = serde_json::from_slice(&body).unwrap();
        seen.lock().unwrap().push(v.clone());
        let answer = match (v["send"].as_str(), v["source"].as_str()) {
            (Some("createLanguageModel"), _) if v["origin"] == "tool" => {
                json!({ "error": "NotAllowedError", "message": "person-required (tool origin)" })
            }
            (Some("createLanguageModel"), _) => {
                json!({ "id": "s1", "contextWindow": 4096, "tools": ["findNotes"], "overflowed": false })
            }
            (Some("prompt"), _) => return holding.lock().unwrap().push(reply),
            (_, Some("languageModelResponse")) => {
                let state = if v["args"]["response"] == "ended" { "complete" } else { "generating" };
                json!({ "id": v["args"]["response"], "session": "s1", "state": state, "text": "", "toolCalls": [], "overflowed": false })
            }
            _ => json!({}),
        };
        reply.send(response(&answer));
    })));
    r.bind_extensions(&["language-model"], &[]);
    (app, r, calls, held)
}

fn response(answer: &Json) -> Outcome {
    Outcome::Response(Response {
        status: 200,
        headers: Vec::new(),
        body: answer.to_string().into_bytes(),
    })
}

/// The host's part: each request run, each outcome fulfilled, until
/// nothing is left to do; a held one's reply waits in `waiting`.
fn pump(r: &mut Runner<Tools>, waiting: &mut Vec<(u64, Receiver<Outcome>)>) {
    for _ in 0..16 {
        let outs = r.take_requests();
        let announced = r.apply_announced().0;
        let mut landed = Vec::new();
        waiting.retain(|(ticket, rx)| match rx.try_recv() {
            Ok(outcome) => {
                landed.push((*ticket, outcome));
                false
            }
            Err(_) => true,
        });
        if outs.is_empty() && announced.is_empty() && landed.is_empty() {
            return;
        }
        for (ticket, outcome) in landed {
            let _ = r.fulfill(ticket, outcome);
        }
        for out in outs {
            let outcome = match r.native_work(&out.request) {
                Dispatch::Run(Work::Now(work)) => work(),
                Dispatch::Run(Work::Later(work)) => {
                    let (tx, rx) = channel();
                    work(Reply::new(move |o| {
                        let _ = tx.send(o);
                    }));
                    match rx.try_recv() {
                        Ok(outcome) => outcome,
                        Err(_) => {
                            waiting.push((out.ticket, rx));
                            continue;
                        }
                    }
                }
                _ => continue,
            };
            let _ = r.fulfill(out.ticket, outcome);
        }
    }
}

/// The implementation's call back, its answer.
fn invoke(r: &mut Runner<Tools>, waiting: &mut Vec<(u64, Receiver<Outcome>)>, body: Json) -> Json {
    let (tx, rx) = channel();
    r.invoke_module(
        &body.to_string(),
        Box::new(move |answer| tx.send(answer).unwrap()),
    );
    pump(r, waiting);
    serde_json::from_str(&rx.try_recv().expect("answered")).unwrap()
}

fn logs(r: &Runner<Tools>) -> String {
    exact_runner::agent::handle(r, r#"{"op":"logs"}"#)
}

fn sent<'a>(calls: &'a [Json], name: &str) -> Vec<&'a Json> {
    calls.iter().filter(|c| c["send"] == name).collect()
}

/// D15 item 1: a handler runs only for a handle whose call listed it, as
/// `tool`, its value the implementation's as JSON text.
#[test]
fn a_handler_runs_only_for_a_handle_whose_call_listed_it() {
    let (_app, mut r, _calls, _held) = boot("listed");
    let mut waiting = Vec::new();
    r.act("create", Vec::new()).unwrap();
    pump(&mut r, &mut waiting);
    let call = |handle: &str, handler: &str| json!({ "extension": "language-model", "handle": handle, "handler": handler, "args": { "query": "rust" } });
    let found = invoke(&mut r, &mut waiting, call("s1", "findNotes"));
    assert_eq!(
        found,
        json!({ "text": r#"[{"title":"Lifetimes"}]"# }),
        "{}",
        logs(&r)
    );
    assert!(
        logs(&r).contains("language-model: findNotes for s1: run, tool:model:s1"),
        "{}",
        logs(&r)
    );
    for (handle, handler) in [("s1", "deleteNotes"), ("s2", "findNotes")] {
        let refused = invoke(&mut r, &mut waiting, call(handle, handler));
        assert_eq!(
            refused["error"], "NotAllowedError",
            "{handle} {handler}: {refused}"
        );
    }
    assert!(
        logs(&r).contains("refused, no call made for it lists the handler"),
        "{}",
        logs(&r)
    );
}

/// Drive 8: a handler that calls `createLanguageModel` makes it as `tool`,
/// which the implementation's download rule refuses; the refusal is the
/// handler's answer to the model.
#[test]
fn a_handlers_module_call_carries_the_tool_origin() {
    let (_app, mut r, calls, _held) = boot("origin");
    let mut waiting = Vec::new();
    r.act("create", Vec::new()).unwrap();
    pump(&mut r, &mut waiting);
    let body = json!({ "extension": "language-model", "handle": "s1", "handler": "findNotes", "args": { "query": "create" } });
    let answer = invoke(&mut r, &mut waiting, body);
    let made = sent(&calls.lock().unwrap(), "createLanguageModel")
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(made.len(), 2, "{made:?}");
    assert_eq!(made[0].get("origin"), None, "the person's create");
    assert_eq!(made[1]["origin"], "tool", "the handler's");
    assert!(
        answer["text"].as_str().unwrap().contains("person-required"),
        "{answer}"
    );
}

/// Drive 7: a tool invocation of the app that would start a download is
/// told it comes from a tool, and the implementation refuses it.
#[test]
fn a_tool_invocations_create_is_told_its_origin() {
    let (_app, mut r, calls, _held) = boot("drive7");
    let mut waiting = Vec::new();
    r.act_from(CallOrigin::Tool, "create", Vec::new()).unwrap();
    pump(&mut r, &mut waiting);
    let calls = calls.lock().unwrap();
    assert_eq!(sent(&calls, "createLanguageModel")[0]["origin"], "tool");
    assert_eq!(
        format!("{:?}", r.slot("made")),
        "Some(Option(None))",
        "no session is made"
    );
}

/// D15 item 2: a send whose mutation is sent again before its reply is
/// cancelled at the implementation by its number.
#[test]
fn a_send_let_go_is_cancelled_at_its_implementation() {
    let (_app, mut r, calls, held) = boot("cancel");
    let mut waiting = Vec::new();
    r.act("create", Vec::new()).unwrap();
    pump(&mut r, &mut waiting);
    r.act("ask", Vec::new()).unwrap();
    pump(&mut r, &mut waiting);
    r.act("ask", Vec::new()).unwrap();
    pump(&mut r, &mut waiting);
    let calls = calls.lock().unwrap().clone();
    let prompts = sent(&calls, "prompt");
    assert_eq!(prompts.len(), 2, "{calls:?}");
    let first = prompts[0]["request"].as_u64().expect("a send's number");
    assert_ne!(prompts[1]["request"].as_u64(), Some(first));
    let cancels: Vec<&Json> = calls.iter().filter(|c| c.get("cancel").is_some()).collect();
    assert_eq!(
        cancels,
        vec![&json!({ "extension": "language-model", "cancel": first })],
        "{}",
        logs(&r)
    );
    assert!(
        logs(&r).contains(&format!(
            "cancel language-model.prompt request {first}: its call was let go"
        )),
        "{}",
        logs(&r)
    );
    // The second is answered: nothing more is cancelled.
    let reply = held.lock().unwrap().pop().unwrap();
    reply.send(response(&json!({ "text": "Hi.", "overflowed": false })));
    pump(&mut r, &mut waiting);
    let after = calls_with_cancel(&r);
    assert_eq!(after, 1, "{}", logs(&r));
}

/// LLP 1133.023 D7: a model's handler runs as `tool` but under no
/// invocation, so a system's call of a tool while it is in flight neither
/// waits for its request nor ends it.
#[test]
fn a_handler_in_flight_is_not_a_tool_calls_work() {
    let (_app, mut r, _calls, held) = boot("handler-call");
    let mut waiting = Vec::new();
    r.act("create", Vec::new()).unwrap();
    pump(&mut r, &mut waiting);
    let (tx, rx) = channel();
    let body = json!({ "extension": "language-model", "handle": "s1", "handler": "findNotes", "args": { "query": "hold" } });
    r.invoke_module(
        &body.to_string(),
        Box::new(move |answer| tx.send(answer).unwrap()),
    );
    pump(&mut r, &mut waiting);
    assert_eq!(
        held.lock().unwrap().len(),
        1,
        "the handler's prompt is held: {}",
        logs(&r)
    );
    let how = exact_runner::CallHow {
        door: "agent".into(),
        ..Default::default()
    };
    r.call_tool("note", "{}", how).unwrap();
    r.advance(r.now_ms()).unwrap();
    pump(&mut r, &mut waiting);
    assert!(logs(&r).contains("tool note ok applied"), "{}", logs(&r));
    assert!(rx.try_recv().is_err(), "the handler is still in flight");
    let reply = held.lock().unwrap().pop().unwrap();
    reply.send(response(&json!({ "text": "Hi.", "overflowed": false })));
    pump(&mut r, &mut waiting);
    assert!(rx.try_recv().is_ok(), "the handler answers: {}", logs(&r));
}

fn calls_with_cancel(r: &Runner<Tools>) -> usize {
    logs(r).matches("its call was let go").count()
}

/// D15 item 3: a polled source whose answer leaves `while` (a response no
/// longer generating) is not asked again by the clock.
#[test]
fn a_poll_ends_with_its_answer() {
    let (_app, mut r, calls, _held) = boot("while");
    let mut waiting = Vec::new();
    let reads = |calls: &Calls| {
        (calls.lock().unwrap().iter())
            .filter(|c| c["source"] == "languageModelResponse")
            .count()
    };
    r.act("read", vec![Value::from("r1")]).unwrap();
    pump(&mut r, &mut waiting);
    let before = reads(&calls);
    r.advance(200.0).unwrap();
    pump(&mut r, &mut waiting);
    assert!(
        reads(&calls) > before,
        "a generating response is polled: {}",
        logs(&r)
    );
    r.act("read", vec![Value::from("ended")]).unwrap();
    pump(&mut r, &mut waiting);
    let ended = reads(&calls);
    r.advance(400.0).unwrap();
    pump(&mut r, &mut waiting);
    assert_eq!(
        reads(&calls),
        ended,
        "a completed response is not: {}",
        logs(&r)
    );
}

/// A handler sleeping on a timer of its own is listed in `state.pending`,
/// marked `timer` with when it is due, and is no I/O a clock waits on.
#[test]
fn a_handler_sleeping_on_its_own_timer_is_pending_on_a_timer() {
    let (_app, mut r, _calls, _held) = boot("sleeping");
    let mut waiting = Vec::new();
    r.act("create", Vec::new()).unwrap();
    pump(&mut r, &mut waiting);
    let call = json!({ "extension": "language-model", "handle": "s1", "handler": "findNotes", "args": { "query": "sleep" } });
    r.invoke_module(&call.to_string(), Box::new(|_| {}));
    assert!(r.take_requests().is_empty(), "kept back until it is due");
    let state = exact_runner::agent::handle(&r, r#"{"op":"state"}"#);
    let state: Json = serde_json::from_str(&state).unwrap();
    let sleeping: Vec<&Json> = (state["pending"].as_array().unwrap().iter())
        .filter(|p| p["timer"] == 500.0)
        .collect();
    assert_eq!(sleeping.len(), 1, "{}", state["pending"]);
    assert!(!r.has_pending(), "no clock waits on it");
}
