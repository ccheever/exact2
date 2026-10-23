//! The module seam's general pieces, through an interface no module ships:
//! `@fixture/sensor`, written beside the app as an installed package, whose
//! implementation the test plays as the host's native slot (LLP 1133.002
//! D14; LLP 1133.008 D8; LLP 1133.009 D10).

use exact_runner::{CallOrigin, Dispatch, Outcome, Reply, Response, Runner, Work};
use serde_json::{json, Value as Json};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

const INTERFACE: &str = r#"{
  "kind": "interface",
  "name": "@fixture/sensor",
  "version": "1.0.0",
  "grants": ["device.microphone", "device.camera"],
  "substitute": "implementation",
  "permission": { "name": "microphone", "status": "sensorStatus", "field": "state", "denied": "NotAllowedError" },
  "shapes": {
    "SensorStatus": { "available": "bool", "state": "enum<granted|prompt|denied|unknown>" },
    "Reading": { "id": "string", "level": "number", "muted": "bool", "at": "option<number>" }
  },
  "names": [
    { "source": "sensorStatus", "live": true, "answers": "SensorStatus",
      "substitute": { "default": { "available": true } } },
    { "send": "open", "request": true, "origin": "app", "foreground": true,
      "grants": { "audio": "device.microphone", "video": "device.camera" },
      "options": { "audio": "option<bool>", "video": "option<bool>" },
      "answers": "Reading", "handles": { "id": "track" } },
    { "send": "stop", "options": { "track": "string" }, "answers": "Reading" },
    { "source": "level", "live": true, "poll": "interval",
      "options": { "track": "string", "interval": "option<number>" }, "answers": "Reading" }
  ]
}"#;

const APP: &str = r#"use open, stop, sensorStatus, level, SensorStatus, Reading from "@fixture/sensor"

component App
  state track = "a"
  state shown = true
  resource status = sensorStatus() as shape SensorStatus
  resource meter = level(track=track, interval=50) as shape Reading
  mutation opened as shape Reading
  action start
    send opened = open(audio=true)
  action startVideo
    send opened = open(video=true)
  action move(to: string)
    track = to
  view
    column
      text status.state
      text meter.id
"#;

struct App(PathBuf);
impl App {
    fn new(name: &str) -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("exact-seam-{name}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        let package = path.join("node_modules/@fixture/sensor");
        std::fs::create_dir_all(&package).unwrap();
        std::fs::write(
            package.join("package.json"),
            r#"{ "name": "@fixture/sensor", "version": "1.0.0", "exports": { ".": "./exact.json" } }"#,
        )
        .unwrap();
        std::fs::write(package.join("exact.json"), INTERFACE).unwrap();
        std::fs::write(path.join("app.contract"), APP).unwrap();
        Self(path.canonicalize().unwrap())
    }
}
impl Drop for App {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The app's data grants the microphone, not the camera.
struct Granted;
impl exact_runner::DataSource for Granted {
    fn query(
        &mut self,
        source: &str,
        _: &[exact_runner::Value],
    ) -> Result<exact_runner::Value, exact_runner::DataError> {
        Err(exact_runner::DataError::UnknownSource(source.into()))
    }
    fn grants(&self) -> &str {
        "device.microphone purpose.microphone\n"
    }
}

type Calls = Arc<Mutex<Vec<Json>>>;

/// A runner over the fixture, its native slot answered as an implementation
/// would: `open` a reading of a fresh track, `level` its track's reading.
fn boot(name: &str) -> (App, Runner<Granted>, Calls) {
    let app = App::new(name);
    let (plan, _) = contract::compile_path_all(&app.0.join("app.contract"), false)
        .unwrap_or_else(|e| panic!("{e:?}"));
    let r = Runner::boot(
        plan,
        Granted,
        exact_kernel::Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let calls: Calls = Arc::default();
    let seen = calls.clone();
    r.native_slot()
        .host(Some(Arc::new(move |body: Vec<u8>, reply: Reply| {
            let v: Json = serde_json::from_slice(&body).unwrap();
            seen.lock().unwrap().push(v.clone());
            let answer = match (v["source"].as_str(), v["send"].as_str()) {
                (Some("sensorStatus"), _) => json!({ "available": true, "state": "granted" }),
                (Some("level"), _) => {
                    json!({ "id": v["args"]["track"], "level": 0.5, "muted": false })
                }
                (_, Some(_)) => json!({ "id": "t1", "level": 0, "muted": false }),
                _ => json!({}),
            };
            reply.send(Outcome::Response(Response {
                status: 200,
                headers: Vec::new(),
                body: answer.to_string().into_bytes(),
            }));
        })));
    (app, r, calls)
}

/// The host's part: each request dispatched (a held one parked), released
/// work run, each outcome fulfilled, until nothing is left to do.
fn pump(r: &mut Runner<Granted>, parked: &mut Vec<(u64, u64)>) {
    for _ in 0..8 {
        let outs = r.take_requests();
        let released = r.release_work();
        if outs.is_empty() && released.is_empty() && r.apply_announced().0.is_empty() {
            return;
        }
        for out in outs {
            let dispatch = match out.request.continuation {
                Some(token) => {
                    parked.push((token, out.ticket));
                    r.dispatch_work(token)
                }
                None => r.native_work(&out.request),
            };
            run(r, out.ticket, dispatch);
        }
        for (token, dispatch) in released {
            let ticket = parked.iter().find(|(t, _)| *t == token).expect("parked").1;
            run(r, ticket, dispatch);
        }
    }
}

fn run(r: &mut Runner<Granted>, ticket: u64, dispatch: Dispatch) {
    let outcome = match dispatch {
        Dispatch::Run(Work::Now(work)) => work(),
        Dispatch::Run(Work::Later(work)) => {
            let (tx, rx) = std::sync::mpsc::channel();
            work(Reply::new(move |o| tx.send(o).unwrap()));
            rx.recv().unwrap()
        }
        _ => return,
    };
    let _ = r.fulfill(ticket, outcome);
}

fn sent(calls: &Calls, name: &str) -> usize {
    (calls.lock().unwrap().iter())
        .filter(|c| c["send"] == name)
        .count()
}

fn logs(r: &Runner<Granted>) -> String {
    exact_runner::agent::handle(r, r#"{"op":"logs"}"#)
}

fn show(r: &Runner<Granted>, name: &str) -> String {
    format!("{:?}", r.resource(name).or(r.slot(name)))
}

/// A data-code call of `body`, its answer.
fn data_call(r: &mut Runner<Granted>, body: Json) -> Json {
    let request = exact_runner::Request::native(body.to_string().into_bytes());
    let Dispatch::Run(work) = r.native_work(&request) else {
        panic!("a module call is work");
    };
    let outcome = match work {
        Work::Now(work) => work(),
        Work::Later(work) => {
            let (tx, rx) = std::sync::mpsc::channel();
            work(Reply::new(move |o| tx.send(o).unwrap()));
            rx.recv().unwrap()
        }
    };
    let Outcome::Response(Response { body, .. }) = outcome else {
        panic!("an answer");
    };
    serde_json::from_slice(&body).unwrap()
}

/// LLP 1133.002 D14, LLP 1133.009 D10 items 3, 8, 9: a bound implementation
/// answers only what admission lets through, from Contract and from data
/// code alike: an argument whose grant line the app lacks, a capture start
/// while the app is in the background, and one a tool invocation makes are
/// each refused before the implementation hears of it.
#[test]
fn a_bound_call_is_admitted_before_its_implementation_answers() {
    let (_app, mut r, calls) = boot("admitted");
    r.bind_extensions(&["sensor"], &[]);
    let mut parked = Vec::new();
    pump(&mut r, &mut parked);
    r.act("start", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    assert_eq!(sent(&calls, "open"), 1, "{}", logs(&r));
    assert!(show(&r, "opened").contains("t1"), "{}", show(&r, "opened"));

    r.act("startVideo", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    assert_eq!(sent(&calls, "open"), 1, "the camera's line is missing");
    assert!(
        logs(&r).contains("sensor.open: refused, grant-missing (video= needs device.camera)"),
        "{}",
        logs(&r)
    );
    let refused = data_call(
        &mut r,
        json!({ "extension": "sensor", "send": "open", "args": { "video": true } }),
    );
    assert_eq!(refused, json!({ "error": "NotAllowedError" }));

    r.act_from(CallOrigin::Tool, "start", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    assert_eq!(sent(&calls, "open"), 1, "a tool starts no capture");
    assert!(
        logs(&r).contains("sensor.open: refused, person-required (tool origin)"),
        "{}",
        logs(&r)
    );

    let page = r.page();
    r.set_page(exact_runner::Page {
        hidden: true,
        ..page
    })
    .unwrap();
    r.act("start", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    assert_eq!(sent(&calls, "open"), 1, "nothing starts in the background");
    let told = |shown: bool| {
        (calls.lock().unwrap().iter())
            .any(|c| *c == json!({ "extension": "sensor", "foreground": shown }))
    };
    assert!(
        told(false),
        "the implementation is told it left the foreground"
    );
    assert!(
        logs(&r).contains("refused, not in the foreground"),
        "{}",
        logs(&r)
    );
    let refused = data_call(
        &mut r,
        json!({ "extension": "sensor", "send": "open", "args": { "audio": true } }),
    );
    assert_eq!(refused, json!({ "error": "NotAllowedError" }));
    r.set_page(page).unwrap();
    assert!(told(true));
    let answered = data_call(
        &mut r,
        json!({ "extension": "sensor", "send": "open", "args": { "audio": true } }),
    );
    assert_eq!(answered["id"], "t1", "{answered}");
    assert_eq!(sent(&calls, "open"), 2);
}

/// LLP 1133.008 D8 item 9, LLP 1133.009 D10 item 11: under the agent an
/// interface that is its own substitute is answered by its implementation,
/// told so, behind the runner's permission: a request while `prompt` waits
/// on a ticket the agent answers before the call reaches the
/// implementation; a tool's call raises no ticket; the agent's preferred
/// value reaches the implementation and is laid over its answer, checked
/// field by field.
#[test]
fn an_implementation_that_is_its_own_substitute_answers_behind_the_agent_s_ticket() {
    let (_app, mut r, calls) = boot("agent");
    r.bind_extensions(&[], &["sensor"]);
    let mut parked = Vec::new();
    pump(&mut r, &mut parked);
    // The permission is the runner's: its status source is not asked of the implementation.
    assert!(
        show(&r, "status").contains("\"prompt\""),
        "{}",
        show(&r, "status")
    );
    assert!(!calls
        .lock()
        .unwrap()
        .iter()
        .any(|c| c["source"] == "sensorStatus"));
    let state = r.extensions_json().unwrap();
    assert!(state.contains(r#""agent":true"#), "{state}");

    r.act_from(CallOrigin::Tool, "start", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    assert!(r.device_holds().is_empty(), "a tool raises no ticket");
    assert!(
        logs(&r).contains("person-required (tool origin)"),
        "{}",
        logs(&r)
    );

    r.act("start", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    assert_eq!(sent(&calls, "open"), 0, "held under the ticket");
    let hold = r.device_holds()[0].ticket;
    assert!(r.device_holds()[0].args.contains(r#""origin":"app""#));
    let tap = format!(r#"{{"op":"tap","ticket":{hold},"choice":"allow"}}"#);
    exact_runner::agent::answer(&mut r, &tap).unwrap();
    pump(&mut r, &mut parked);
    assert_eq!(sent(&calls, "open"), 1, "{}", logs(&r));
    assert!(show(&r, "opened").contains("t1"), "{}", show(&r, "opened"));
    assert!(show(&r, "status").contains("\"granted\""));

    let wrong = r#"{"op":"prefer","extension":{"sensor.level":{"loud":true}}}"#;
    let reply = r.extension_agent(wrong).unwrap().1.unwrap_err();
    assert!(
        reply.contains("field `loud` is not in the shape"),
        "{reply}"
    );
    let muted = r#"{"op":"prefer","extension":{"sensor.level":{"muted":true}}}"#;
    r.extension_agent(muted).unwrap().1.unwrap();
    pump(&mut r, &mut parked);
    let told = calls
        .lock()
        .unwrap()
        .iter()
        .any(|c| c["preferred"]["level"]["muted"] == true);
    assert!(
        told,
        "the implementation is told: {:?}",
        calls.lock().unwrap()
    );
    let meter = show(&r, "meter");
    assert!(
        meter.contains("Bool(true)") && meter.contains("0.5"),
        "{meter}"
    );
}
