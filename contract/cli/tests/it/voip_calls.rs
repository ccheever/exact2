//! `@exact/calls`' declared substitute (LLP 1133.033 D21), through this
//! checkout's package installed beside the app: a call is listed in
//! `state.calls` as the app places it, moves as the app reports it, and the
//! person's acts on the system's screen are `tap @ref <choice>`, each raising
//! its event with a person's origin. Ended calls stay listed until the list
//! next changes; a live id is refused; a tool may end a call and may not
//! place one.

use exact_runner::{agent, CallOrigin, Runner};
#[cfg(unix)]
use std::os::unix::fs::symlink as symlink_dir;
#[cfg(windows)]
use std::os::windows::fs::symlink_dir;
use std::path::{Path, PathBuf};

const APP: &str = r#"use startCall, reportIncomingCall, reportCallState, setCallMuted, endCall, calls, callsStatus, callanswer, callhangup, callhold, callmute, Call, CallEvent, CallsStatus from "@exact/calls"

component App
  state heard = ""
  resource list = calls() as shape Call[]
  resource status = callsStatus() as shape CallsStatus
  action place
    startCall(id="c1", handle="test-call", displayName="Test call")
  action ring
    reportIncomingCall(id="c2", handle="+15555550100", handleType="phoneNumber")
  action connect
    reportCallState(id="c1", state="connected")
  action unmute
    setCallMuted(id="c1", muted=false)
  action finish
    endCall(id="c1", reason="remote")
  action answered(call: Call)
    heard = "answer " + call.id + " " + call.state
  action hungUp(call: Call)
    heard = "hangup " + call.id
  action held(call: Call, e: CallEvent)
    heard = "hold " + call.state
  action muted(call: Call, e: CallEvent)
    heard = "mute " + call.id
  view
    column callanswer=answered callhangup=hungUp callhold=held callmute=muted
      text testId="heard" heard
      text testId="count" `${list.length}`
"#;

struct App(PathBuf);
impl App {
    fn new(name: &str) -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("exact-voip-{name}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        let packages = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages");
        std::fs::create_dir_all(path.join("node_modules/@exact")).unwrap();
        let to = packages.join("calls").canonicalize().unwrap();
        symlink_dir(to, path.join("node_modules/@exact/calls")).unwrap();
        std::fs::write(path.join("app.contract"), APP).unwrap();
        Self(path.canonicalize().unwrap())
    }
}
impl Drop for App {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

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
        "device.microphone purpose.microphone\ndevice.calls purpose.calls\n"
    }
}

fn boot(app: &App) -> Runner<Granted> {
    let (plan, _) = contract::compile_path_all(&app.0.join("app.contract"), false)
        .unwrap_or_else(|e| panic!("{e:?}"));
    let mut r = Runner::boot(
        plan,
        Granted,
        exact_kernel::Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    r.listen(std::sync::Arc::new(|| {}));
    r
}

fn state(r: &Runner<Granted>) -> serde_json::Value {
    serde_json::from_str(&agent::handle(r, r#"{"op":"state"}"#)).unwrap()
}

fn logs(r: &Runner<Granted>) -> String {
    agent::handle(r, r#"{"op":"logs"}"#)
}

fn settle(r: &mut Runner<Granted>) {
    r.apply_announced();
    let at = r.now_ms();
    r.advance(at).unwrap();
}

/// `tap @ref <choice>`, as the agent sends it.
fn tap(r: &mut Runner<Granted>, reference: u64, choice: &str) -> Result<String, String> {
    let request = format!(r#"{{"op":"tap","ticket":{reference},"choice":"{choice}"}}"#);
    let reply = agent::answer(r, &request).expect("an agent op").0;
    let parsed: serde_json::Value = serde_json::from_str(&reply).unwrap_or_default();
    match parsed["error"].as_str() {
        Some(error) => Err(error.to_owned()),
        None => Ok(reply),
    }
}

fn slot(r: &Runner<Granted>, name: &str) -> String {
    format!("{:?}", r.slot(name))
}

#[test]
fn a_call_is_listed_reported_and_acted_on_by_the_person() {
    let app = App::new("call");
    let mut r = boot(&app);
    r.act("place", Vec::new()).unwrap();
    settle(&mut r);
    let call = state(&r)["calls"][0].clone();
    assert_eq!(
        (
            call["id"].as_str(),
            call["state"].as_str(),
            call["direction"].as_str()
        ),
        (Some("c1"), Some("dialing"), Some("outgoing")),
        "{call}"
    );
    assert!(
        call["startedAt"].is_number() && call["audio"] == false,
        "{call}"
    );
    let reference = call["ref"].as_u64().unwrap();
    // A live id is refused, and the list stays as it was.
    r.act("place", Vec::new()).unwrap();
    settle(&mut r);
    assert!(
        logs(&r).contains("startCall: refused: InvalidStateError"),
        "{}",
        logs(&r)
    );
    assert_eq!(state(&r)["calls"].as_array().unwrap().len(), 1);
    // The person cannot answer an outgoing call: refused by name.
    let refused = tap(&mut r, reference, "answer").unwrap_err();
    assert!(
        refused.contains("`answer` is refused: its state is \"dialing\""),
        "{refused}"
    );
    r.act("connect", Vec::new()).unwrap();
    settle(&mut r);
    let call = state(&r)["calls"][0].clone();
    assert_eq!(
        (call["state"].as_str(), call["audio"].as_bool()),
        (Some("connected"), Some(true)),
        "{call}"
    );
    assert!(call["connectedAt"].is_number());
    // The person holds it on the system's screen, then takes it back.
    tap(&mut r, reference, "hold").unwrap();
    settle(&mut r);
    assert!(
        slot(&r, "heard").contains("hold held"),
        "{} {}",
        slot(&r, "heard"),
        logs(&r)
    );
    tap(&mut r, reference, "unhold").unwrap();
    tap(&mut r, reference, "mute").unwrap();
    settle(&mut r);
    assert!(
        slot(&r, "heard").contains("mute c1"),
        "{}",
        slot(&r, "heard")
    );
    assert_eq!(state(&r)["calls"][0]["muted"], true);
    // A tool may not unmute a call (it turns the microphone back on).
    r.act_from(CallOrigin::Tool, "unmute", Vec::new()).unwrap();
    settle(&mut r);
    assert!(logs(&r).contains("person-required"), "{}", logs(&r));
    assert_eq!(state(&r)["calls"][0]["muted"], true);
    tap(&mut r, reference, "hangup").unwrap();
    settle(&mut r);
    assert!(
        slot(&r, "heard").contains("hangup c1"),
        "{}",
        slot(&r, "heard")
    );
    let call = state(&r)["calls"][0].clone();
    assert_eq!(
        (call["state"].as_str(), call["endedReason"].as_str()),
        (Some("ended"), Some("local")),
        "{call}"
    );
    // Ended, it is listed until the list changes, and its id is free again.
    r.act("place", Vec::new()).unwrap();
    settle(&mut r);
    let calls = state(&r)["calls"].clone();
    assert_eq!(calls.as_array().unwrap().len(), 1, "{calls}");
    assert_eq!(calls[0]["state"], "dialing");
    assert_ne!(calls[0]["ref"].as_u64().unwrap(), reference);
}

#[test]
fn an_incoming_call_rings_and_the_person_answers_it() {
    let app = App::new("ring");
    let mut r = boot(&app);
    r.act("ring", Vec::new()).unwrap();
    settle(&mut r);
    let call = state(&r)["calls"][0].clone();
    assert_eq!(
        (call["state"].as_str(), call["direction"].as_str()),
        (Some("ringing"), Some("incoming")),
        "{call}"
    );
    tap(&mut r, call["ref"].as_u64().unwrap(), "answer").unwrap();
    settle(&mut r);
    assert!(
        slot(&r, "heard").contains("answer c2 connected"),
        "{} {}",
        slot(&r, "heard"),
        logs(&r)
    );
    // A tool places nothing: person-required, and no entry.
    let app = App::new("tool");
    let mut r = boot(&app);
    r.act_from(CallOrigin::Tool, "place", Vec::new()).unwrap();
    settle(&mut r);
    assert!(
        logs(&r).contains("startCall: refused, person-required"),
        "{}",
        logs(&r)
    );
    assert!(state(&r)["calls"].as_array().is_none_or(|c| c.is_empty()));
    // A tool may end a call.
    r.act("place", Vec::new()).unwrap();
    r.act_from(CallOrigin::Tool, "finish", Vec::new()).unwrap();
    settle(&mut r);
    let call = state(&r)["calls"][0].clone();
    assert_eq!(
        (call["state"].as_str(), call["endedReason"].as_str()),
        (Some("ended"), Some("remote")),
        "{call} {}",
        logs(&r)
    );
}

/// `@fixture/intercom`, a module outside exact2's packages (LLP 1133.033 §12
/// stage 0): its own keyed list with the forms calls uses (`with`, `stamp`,
/// `refuse`, `ends`, `tap.choices` with `when`, `set` and an event) drives as
/// `@exact/calls` does.
const INTERCOM: &str = r#"{
  "kind": "interface",
  "name": "@fixture/intercom",
  "version": "1.0.0",
  "shapes": {
    "Line": { "id": "string", "peer": "string", "state": "enum<open|paused|closed>", "openedAt": "option<number>" },
    "LineEvent": { "paused": "option<bool>" }
  },
  "names": [
    {
      "send": "openLine",
      "permission": "none",
      "options": { "id": "string", "peer": "string" },
      "answers": null,
      "substitute": {
        "list": {
          "state": "lines",
          "key": "id",
          "refuse": "InvalidStateError",
          "with": { "state": "open" },
          "stamp": "openedAt",
          "ends": { "state": ["closed"] },
          "tap": {
            "choices": {
              "pause": { "when": { "state": ["open"] }, "set": { "state": "paused" }, "event": "linepause", "args": [{ "id": "id", "peer": "peer", "state": "state", "openedAt": "openedAt" }, { "paused": true }] },
              "close": { "when": { "state": ["open", "paused"] }, "set": { "state": "closed" }, "event": "lineclose", "args": [{ "id": "id", "peer": "peer", "state": "state", "openedAt": "openedAt" }] }
            }
          }
        }
      }
    },
    {
      "send": "closeLine",
      "permission": "none",
      "options": { "id": "string" },
      "answers": null,
      "substitute": { "update": { "state": "lines", "key": "id", "when": { "state": ["open", "paused"] }, "set": { "state": "closed" }, "else": "InvalidStateError" } }
    },
    { "source": "lines", "live": true, "answers": "Line[]", "substitute": { "listed": { "state": "lines", "fields": ["id", "peer", "state", "openedAt"] } } },
    { "event": "linepause", "on": "root", "raised": "activation", "args": ["Line", "LineEvent"], "hold": 4 },
    { "event": "lineclose", "on": "root", "raised": "activation", "args": ["Line"], "hold": 4 }
  ]
}"#;

const INTERCOM_APP: &str = r#"use openLine, closeLine, lines, linepause, lineclose, Line, LineEvent from "@fixture/intercom"

component App
  state heard = ""
  resource list = lines() as shape Line[]
  action open
    openLine(id="l1", peer="door")
  action close
    closeLine(id="l1")
  action paused(line: Line, e: LineEvent)
    heard = "pause " + line.id + " " + line.state
  action closed(line: Line)
    heard = "close " + line.id
  view
    column linepause=paused lineclose=closed
      text testId="heard" heard
"#;

#[test]
fn a_third_party_list_drives_as_calls_does() {
    let app = App::new("intercom");
    let package = app.0.join("node_modules/@fixture/intercom");
    std::fs::create_dir_all(&package).unwrap();
    std::fs::write(
        package.join("package.json"),
        r#"{ "name": "@fixture/intercom", "version": "1.0.0", "exports": { ".": "./exact.json" } }"#,
    )
    .unwrap();
    std::fs::write(package.join("exact.json"), INTERCOM).unwrap();
    std::fs::write(app.0.join("app.contract"), INTERCOM_APP).unwrap();
    let mut r = boot(&app);
    r.act("open", Vec::new()).unwrap();
    settle(&mut r);
    let line = state(&r)["lines"][0].clone();
    assert_eq!(
        (line["state"].as_str(), line["openedAt"].is_number()),
        (Some("open"), true),
        "{line}"
    );
    let reference = line["ref"].as_u64().unwrap();
    r.act("open", Vec::new()).unwrap();
    settle(&mut r);
    assert!(
        logs(&r).contains("openLine: refused: InvalidStateError"),
        "{}",
        logs(&r)
    );
    tap(&mut r, reference, "pause").unwrap();
    settle(&mut r);
    assert!(
        slot(&r, "heard").contains("pause l1 paused"),
        "{} {}",
        slot(&r, "heard"),
        logs(&r)
    );
    let refused = tap(&mut r, reference, "pause").unwrap_err();
    assert!(refused.contains("`pause` is refused"), "{refused}");
    tap(&mut r, reference, "close").unwrap();
    settle(&mut r);
    assert!(
        slot(&r, "heard").contains("close l1"),
        "{}",
        slot(&r, "heard")
    );
    // Closed, the app's own close is refused by its `when`.
    r.act("close", Vec::new()).unwrap();
    settle(&mut r);
    assert!(
        logs(&r).contains("closeLine: refused: InvalidStateError"),
        "{}",
        logs(&r)
    );
}

/// Bound, `@exact/calls`' list stays the runner's (LLP 1133.033 §13,
/// "Runner hands it over"): the implementation is handed it at the bind and
/// at each change, never sent the list's sends, and its report of a
/// person's act on the platform's screen takes the declared choice.
#[test]
fn a_bound_implementation_is_handed_the_list_and_reports_a_choice() {
    use exact_runner::{Outcome, Reply, Response};
    use std::sync::{Arc, Mutex};
    let app = App::new("handed");
    let mut r = boot(&app);
    let told: Arc<Mutex<Vec<serde_json::Value>>> = Arc::default();
    let seen = told.clone();
    r.native_slot()
        .host(Some(Arc::new(move |body: Vec<u8>, reply: Reply| {
            let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
            seen.lock().unwrap().push(v);
            reply.send(Outcome::Response(Response {
                status: 200,
                headers: Vec::new(),
                body: br#"{"available":true,"systemUI":true}"#.to_vec(),
            }));
        })));
    r.bind_extensions(&["calls"], &[]);
    let handed = |told: &Arc<Mutex<Vec<serde_json::Value>>>| {
        (told.lock().unwrap().iter())
            .rfind(|v| v["send"] == "exact:listed")
            .cloned()
    };
    let first = handed(&told).expect("handed at the bind");
    assert_eq!(first["args"]["state"], "calls");
    assert_eq!(first["args"]["entries"], serde_json::json!([]));
    r.act("place", Vec::new()).unwrap();
    r.act("connect", Vec::new()).unwrap();
    settle(&mut r);
    let now = handed(&told).unwrap();
    let entry = &now["args"]["entries"][0];
    assert_eq!(
        (entry["id"].as_str(), entry["state"].as_str()),
        (Some("c1"), Some("connected")),
        "{now}"
    );
    assert!(entry["ref"].is_u64(), "{now}");
    assert!(
        !told
            .lock()
            .unwrap()
            .iter()
            .any(|v| v["send"] == "startCall"),
        "the list's sends stay the runner's"
    );
    // The person hangs up on the platform's screen.
    r.native_slot().changed(
        r#"exact-choice:{"extension":"calls","state":"calls","key":"c1","choice":"hangup"}"#,
    );
    settle(&mut r);
    assert!(
        slot(&r, "heard").contains("hangup c1"),
        "{} {}",
        slot(&r, "heard"),
        logs(&r)
    );
    assert_eq!(state(&r)["calls"][0]["endedReason"], "local");
    // A choice for no entry, or one the list does not declare, changes nothing.
    r.native_slot().changed(
        r#"exact-choice:{"extension":"calls","state":"calls","key":"c9","choice":"answer"}"#,
    );
    settle(&mut r);
    assert!(
        logs(&r).contains(r#"calls.calls: `answer` for \"c9\": no such entry"#),
        "{}",
        logs(&r)
    );
}
