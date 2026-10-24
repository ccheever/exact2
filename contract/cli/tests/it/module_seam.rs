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
    { "send": "stop", "options": { "track": "handle<track>" }, "answers": "Reading" },
    { "send": "save", "options": { "track": "handle<track>",
        "path": { "type": "path", "access": "write", "for": "recorder" } }, "answers": "Reading" },
    { "source": "level", "live": true, "poll": "interval",
      "options": { "track": "handle<track>", "interval": "option<number>" }, "answers": "Reading" }
  ]
}"#;

const APP: &str = r#"use open, stop, save, sensorStatus, level, SensorStatus, Reading from "@fixture/sensor"

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
  action keep(to: string)
    send opened = save(track=track, path=to)
  view
    column
      text status.state
      text meter.id
"#;

/// `@fixture/listener`, a recognizer's shape (LLP 1133.012 D12): two
/// permissions, the second the microphone's, asked only by the calls that
/// name a line they cover; a `"*"` line every call needs; an app origin only
/// for a call on a track; and a word the agent prefers for the
/// implementation's agent path.
const LISTENER: &str = r#"{
  "kind": "interface",
  "name": "@fixture/listener",
  "version": "1.0.0",
  "grants": ["device.speech-recognition"],
  "substitute": "implementation",
  "permission": [
    { "name": "speech", "status": "listenerStatus", "field": "state", "denied": "NotAllowedError",
      "grants": ["device.speech-recognition"] },
    { "name": "microphone", "status": "microphoneStatus", "field": "state", "denied": "NotAllowedError",
      "grants": ["device.microphone"] }
  ],
  "agent": { "heard": "string" },
  "shapes": {
    "ListenerStatus": { "available": "bool", "state": "enum<granted|prompt|denied|unknown>" },
    "Heard": { "id": "string" }
  },
  "names": [
    { "source": "listenerStatus", "live": true, "answers": "ListenerStatus",
      "substitute": { "default": { "available": true } } },
    { "send": "listen", "request": true, "foreground": true,
      "grants": { "*": "device.speech-recognition", "track": "device.microphone" },
      "origin": { "whenAny": ["track"], "is": "app" },
      "options": { "track": "option<string>", "file": "option<string>" },
      "answers": "Heard" },
    { "source": "peek", "live": true, "answers": "Heard" }
  ]
}"#;

const LISTENING: &str = r#"use listen, listenerStatus, peek, ListenerStatus, Heard from "@fixture/listener"

component App
  resource status = listenerStatus() as shape ListenerStatus
  resource seen = peek() as shape Heard
  mutation heard as shape Heard
  action live
    send heard = listen(track="t")
  action file
    send heard = listen(file="f")
  view
    column
      text status.state
      text seen.id
"#;

/// An installed interface, the app that uses it, and the app's grant lines.
struct Fixture {
    package: &'static str,
    interface: &'static str,
    app: &'static str,
    grants: &'static str,
}

const SENSOR: Fixture = Fixture {
    package: "@fixture/sensor",
    interface: INTERFACE,
    app: APP,
    grants: "device.microphone purpose.microphone\nfs.write app:/data/takes\n",
};

const LISTENER_APP: Fixture = Fixture {
    package: "@fixture/listener",
    interface: LISTENER,
    app: LISTENING,
    grants: "device.microphone purpose.microphone\ndevice.speech-recognition purpose.transcribe\n",
};

struct App(PathBuf);
impl App {
    fn new(name: &str, fixture: &Fixture) -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("exact-seam-{name}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        let package = path.join("node_modules").join(fixture.package);
        std::fs::create_dir_all(&package).unwrap();
        let manifest = json!({ "name": fixture.package, "version": "1.0.0", "exports": { ".": "./exact.json" } });
        std::fs::write(package.join("package.json"), manifest.to_string()).unwrap();
        std::fs::write(package.join("exact.json"), fixture.interface).unwrap();
        std::fs::write(path.join("app.contract"), fixture.app).unwrap();
        Self(path.canonicalize().unwrap())
    }
}
impl Drop for App {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The app's grant lines: the sensor's grant the microphone, not the camera.
struct Granted(&'static str);
impl exact_runner::DataSource for Granted {
    fn query(
        &mut self,
        source: &str,
        _: &[exact_runner::Value],
    ) -> Result<exact_runner::Value, exact_runner::DataError> {
        Err(exact_runner::DataError::UnknownSource(source.into()))
    }
    fn grants(&self) -> &str {
        self.0
    }
}

type Calls = Arc<Mutex<Vec<Json>>>;

fn boot(name: &str) -> (App, Runner<Granted>, Calls) {
    boot_with(name, &SENSOR, SENSOR.grants)
}

/// A runner over the fixture, its native slot answered as an implementation
/// would: `open` a reading of a fresh track, `level` its track's reading,
/// `listen` and `peek` a recognition's id.
fn boot_with(name: &str, fixture: &Fixture, grants: &'static str) -> (App, Runner<Granted>, Calls) {
    let app = App::new(name, fixture);
    let (plan, _) = contract::compile_path_all(&app.0.join("app.contract"), false)
        .unwrap_or_else(|e| panic!("{e:?}"));
    let r = Runner::boot(
        plan,
        Granted(grants),
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
                (Some("sensorStatus" | "listenerStatus"), _) => {
                    json!({ "available": true, "state": "granted" })
                }
                (Some("peek"), _) | (_, Some("listen")) => json!({ "id": "l1" }),
                (Some("level"), _) if v["args"]["track"] == "gone" => {
                    json!({ "error": "NotFoundError" })
                }
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

/// LLP 1133.009 D10 item 4: a path option is admitted against the app's
/// `fs.write` lines before the implementation hears of the call; a path
/// outside them, or one that is not a plain `app:/` file, is refused.
#[test]
fn a_path_to_write_is_admitted_against_the_app_s_fs_write_lines() {
    let (_app, mut r, calls) = boot("paths");
    r.bind_extensions(&["sensor"], &[]);
    let mut parked = Vec::new();
    pump(&mut r, &mut parked);
    let keep = |r: &mut Runner<Granted>, parked: &mut Vec<(u64, u64)>, to: &str| {
        r.act("keep", vec![exact_runner::Value::str(to)]).unwrap();
        pump(r, parked);
    };
    keep(&mut r, &mut parked, "app:/data/takes/take-1");
    assert_eq!(sent(&calls, "save"), 1, "{}", logs(&r));
    let saved = (calls.lock().unwrap().iter())
        .rev()
        .find(|c| c["send"] == "save")
        .cloned();
    assert_eq!(saved.unwrap()["args"]["path"], "app:/data/takes/take-1");
    for outside in ["app:/data/other/take", "app:/data/takes/../x", "/tmp/take"] {
        keep(&mut r, &mut parked, outside);
        let refused =
            format!("sensor.save: refused, grant-missing (path={outside} needs fs.write)");
        assert!(logs(&r).contains(&refused), "{outside}: {}", logs(&r));
    }
    assert_eq!(sent(&calls, "save"), 1, "no refused path reaches it");
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

/// LLP 1133.009 D10 item 12: a reader asked again with new arguments hands
/// its implementation the new group before the old one is released, and
/// only the old group is released; the whole name is released when its last
/// reader goes.
#[test]
fn an_argument_group_with_no_reader_is_released_after_its_successor_is_asked() {
    let (_app, mut r, calls) = boot("groups");
    r.bind_extensions(&["sensor"], &[]);
    let mut parked = Vec::new();
    pump(&mut r, &mut parked);
    assert!(show(&r, "meter").contains("\"a\""), "{}", show(&r, "meter"));
    r.act("move", vec![exact_runner::Value::str("b")]).unwrap();
    pump(&mut r, &mut parked);
    assert!(show(&r, "meter").contains("\"b\""), "{}", show(&r, "meter"));
    let calls = calls.lock().unwrap().clone();
    let at = |want: &Json| calls.iter().position(|c| c == want);
    let asked_b = at(
        &json!({ "extension": "sensor", "source": "level", "args": { "track": "b", "interval": 50.0 } }),
    );
    let released_a = at(
        &json!({ "extension": "sensor", "release": "level", "args": { "track": "a", "interval": 50.0 } }),
    );
    assert!(asked_b.is_some() && released_a.is_some(), "{calls:?}");
    assert!(asked_b < released_a, "the new group first: {calls:?}");
    assert!(
        !calls
            .iter()
            .any(|c| c["release"] == "level" && c["args"]["track"] == "b"),
        "{calls:?}"
    );
    assert!(
        !calls
            .iter()
            .any(|c| c == &json!({ "extension": "sensor", "release": "level" })),
        "a reader is left: {calls:?}"
    );
}

/// LLP 1133.009 D6, D10 item 10: a reply carrying handles is provisional
/// until delivered; the reply to a send superseded before it landed is
/// dropped, and its implementation told to dispose of its handles, while
/// the delivered one's stay the session's.
#[test]
fn a_dropped_reply_s_handles_are_disposed_of() {
    let (_app, mut r, calls) = boot("handles");
    r.bind_extensions(&["sensor"], &[]);
    let mut parked = Vec::new();
    pump(&mut r, &mut parked);
    r.act("start", Vec::new()).unwrap();
    r.act("start", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    assert_eq!(sent(&calls, "open"), 2);
    let disposed: Vec<Json> = (calls.lock().unwrap().iter())
        .filter(|c| c.get("dispose").is_some())
        .cloned()
        .collect();
    assert_eq!(
        disposed,
        [json!({ "extension": "sensor", "dispose": ["t1"] })],
        "{}",
        logs(&r)
    );
    assert!(logs(&r).contains("dispose sensor t1: its reply was dropped"));
    assert!(
        show(&r, "opened").contains("t1"),
        "the second reply is delivered"
    );
}

/// LLP 1133.009 D10 item 7: a polled source is asked again once after an
/// advance that crossed its period, however many periods it crossed; never
/// before its period, never while its last ask is in flight, and its poll
/// is no timer that would stop the agent's jump. The agent's `polls` says
/// when its ask is in flight.
#[test]
fn a_polled_source_is_asked_once_after_an_advance_that_crossed_its_period() {
    let (_app, mut r, calls) = boot("poll");
    r.bind_extensions(&["sensor"], &[]);
    let mut parked = Vec::new();
    pump(&mut r, &mut parked);
    let levels = |calls: &Calls| {
        (calls.lock().unwrap().iter())
            .filter(|c| c["source"] == "level")
            .count()
    };
    assert_eq!(levels(&calls), 1);
    assert_eq!(
        r.timer_due_ms(),
        None,
        "a poll is no timer under the agent's clock"
    );
    r.advance(30.0).unwrap();
    pump(&mut r, &mut parked);
    assert_eq!(levels(&calls), 1, "its period is 50 ms");
    r.advance(3000.0).unwrap();
    assert!(r.polls_asking());
    assert_eq!(
        exact_runner::agent::handle(&r, r#"{"op":"polls"}"#),
        r#"{"asking":true}"#
    );
    // A second advance while the ask is in flight asks nothing more.
    r.advance(3100.0).unwrap();
    pump(&mut r, &mut parked);
    assert_eq!(levels(&calls), 2, "{}", logs(&r));
    assert!(!r.polls_asking());
    assert_eq!(
        logs(&r).matches("poll sensor.level").count(),
        1,
        "{}",
        logs(&r)
    );

    // A reader whose answer is an error (a track the session does not hold)
    // is not polled; asking anew with a held one polls again.
    r.act("move", vec![exact_runner::Value::str("gone")])
        .unwrap();
    pump(&mut r, &mut parked);
    let asked = levels(&calls);
    r.advance(6000.0).unwrap();
    pump(&mut r, &mut parked);
    assert_eq!(levels(&calls), asked, "an error is not polled");
    r.act("move", vec![exact_runner::Value::str("b")]).unwrap();
    pump(&mut r, &mut parked);
    r.advance(6100.0).unwrap();
    pump(&mut r, &mut parked);
    assert_eq!(levels(&calls), asked + 2, "{}", logs(&r));

    // A reader that names no handle (`track=""`) is asked once and not polled.
    r.act("move", vec![exact_runner::Value::str("")]).unwrap();
    pump(&mut r, &mut parked);
    r.advance(9000.0).unwrap();
    pump(&mut r, &mut parked);
    assert_eq!(levels(&calls), asked + 3, "{}", logs(&r));
}

fn prefer(r: &mut Runner<Granted>, request: &str) -> Result<String, String> {
    r.extension_agent(request).expect("a prefer").1
}

/// LLP 1133.012 D12 item 5, §10 drive 4: under the agent, a tool may start a
/// recognition of a file while its permission is granted, never one on the
/// microphone; while `prompt` a tool raises no ticket, and the app's call on
/// a track waits on the speech permission's.
#[test]
fn a_tool_may_recognise_a_file_but_never_the_microphone() {
    let (_app, mut r, calls) = boot_with("tools", &LISTENER_APP, LISTENER_APP.grants);
    r.bind_extensions(&[], &["listener"]);
    let mut parked = Vec::new();
    pump(&mut r, &mut parked);
    let both = r#"{"op":"prefer","permission":{"speech":"granted","microphone":"granted"}}"#;
    prefer(&mut r, both).unwrap();
    r.act_from(CallOrigin::Tool, "file", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    assert_eq!(sent(&calls, "listen"), 1, "{}", logs(&r));
    r.act_from(CallOrigin::Tool, "live", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    assert_eq!(sent(&calls, "listen"), 1);
    assert!(
        logs(&r).contains("listener.listen: refused, person-required (tool origin)"),
        "{}",
        logs(&r)
    );

    prefer(
        &mut r,
        r#"{"op":"prefer","permission":{"speech":"prompt"}}"#,
    )
    .unwrap();
    r.act_from(CallOrigin::Tool, "file", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    assert!(r.device_holds().is_empty(), "a tool raises no ticket");
    assert_eq!(sent(&calls, "listen"), 1);
    r.act("live", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    assert_eq!(r.device_holds().len(), 1, "{}", logs(&r));
    assert_eq!(sent(&calls, "listen"), 1, "held under the ticket");
}

/// LLP 1133.012 D12 item 3: a `"*"` grant line holds every call of the name,
/// whatever its arguments.
#[test]
fn a_star_grant_line_holds_every_call() {
    let (_app, mut r, calls) = boot_with("star", &LISTENER_APP, "device.microphone purpose.m\n");
    r.bind_extensions(&["listener"], &[]);
    let mut parked = Vec::new();
    pump(&mut r, &mut parked);
    r.act("file", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    assert_eq!(sent(&calls, "listen"), 0);
    assert!(
        logs(&r).contains("grant-missing (*= needs device.speech-recognition)"),
        "{}",
        logs(&r)
    );
}

/// LLP 1133.012 §9: a word the interface declares for its implementation's
/// agent path is preferred by its type and told to the implementation; a
/// name neither a source nor a word is refused. The permission is asked only
/// by the names whose lines it covers: while it is denied, a source with no
/// line still answers and the status reads `denied`.
#[test]
fn an_agent_word_is_preferred_by_its_type() {
    let (_app, mut r, calls) = boot_with("words", &LISTENER_APP, LISTENER_APP.grants);
    r.bind_extensions(&[], &["listener"]);
    let mut parked = Vec::new();
    pump(&mut r, &mut parked);
    let wrong = prefer(
        &mut r,
        r#"{"op":"prefer","extension":{"listener.heard":5}}"#,
    );
    assert!(wrong.unwrap_err().contains("`listener.heard` is a string"));
    let unknown = prefer(
        &mut r,
        r#"{"op":"prefer","extension":{"listener.said":"x"}}"#,
    );
    assert!(unknown.unwrap_err().contains("no source `listener.said`"));
    prefer(
        &mut r,
        r#"{"op":"prefer","extension":{"listener.heard":"pick up milk"}}"#,
    )
    .unwrap();
    let granted = r#"{"op":"prefer","permission":{"speech":"granted","microphone":"granted"}}"#;
    prefer(&mut r, granted).unwrap();
    r.act("file", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    let told = (calls.lock().unwrap().iter()).any(|c| c["preferred"]["heard"] == "pick up milk");
    assert!(told, "{:?}", calls.lock().unwrap());
    assert_eq!(sent(&calls, "listen"), 1, "{}", logs(&r));

    prefer(
        &mut r,
        r#"{"op":"prefer","permission":{"speech":"denied"}}"#,
    )
    .unwrap();
    pump(&mut r, &mut parked);
    assert!(
        show(&r, "status").contains("\"denied\""),
        "{}",
        show(&r, "status")
    );
    assert!(show(&r, "seen").contains("l1"), "{}", show(&r, "seen"));
    r.act("file", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    assert_eq!(sent(&calls, "listen"), 1, "refused while denied");
}
