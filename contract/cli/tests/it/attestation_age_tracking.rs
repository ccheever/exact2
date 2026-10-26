//! The age range's and tracking consent's admission in the runner (LLP
//! 1133.028 §7), through this checkout's `@exact/age-range` and
//! `@exact/tracking-consent` installed beside the app, each implementation
//! played by the test as the host's native slot under the agent: the sheet
//! a request holds and what its answer tells the implementation, the decline
//! a region's status rules out, the origin rule, and the targets tracking
//! consent does not exist on.

use exact_runner::{CallOrigin, Dispatch, Outcome, Reply, Response, Runner, Work};
use serde_json::{json, Value as Json};
#[cfg(unix)]
use std::os::unix::fs::symlink as symlink_dir;
#[cfg(windows)]
use std::os::windows::fs::symlink_dir;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const APP: &str = r#"use requestAgeRange, ageRangeStatus, AgeRange, AgeRangeStatus from "@exact/age-range"
use requestTrackingAuthorization, trackingAuthorizationStatus, TrackingStatus from "@exact/tracking-consent"

component App
  resource status = ageRangeStatus() as shape AgeRangeStatus
  resource tracking = trackingAuthorizationStatus() as shape TrackingStatus
  mutation age as shape AgeRange
  mutation asked as shape TrackingStatus
  action ask
    send age = requestAgeRange(ageGates=[13, 18])
  action track
    send asked = requestTrackingAuthorization()
  view
    column
      text tracking.state
"#;

const GRANTS: &str = "device.age-range\ndevice.tracking purpose.ads\n";

struct App(PathBuf);
impl App {
    fn new(name: &str) -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("exact-aat-{name}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        let packages = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages");
        std::fs::create_dir_all(path.join("node_modules/@exact")).unwrap();
        for package in ["age-range", "tracking-consent"] {
            let to = packages.join(package).canonicalize().unwrap();
            symlink_dir(to, path.join("node_modules/@exact").join(package)).unwrap();
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

struct Grants;
impl exact_runner::DataSource for Grants {
    fn query(
        &mut self,
        source: &str,
        _: &[exact_runner::Value],
    ) -> Result<exact_runner::Value, exact_runner::DataError> {
        Err(exact_runner::DataError::UnknownSource(source.into()))
    }
    fn grants(&self) -> &str {
        GRANTS
    }
}

type Calls = Arc<Mutex<Vec<Json>>>;

/// A runner over the app on `target`, `bound` implementations bound under
/// the agent and played by the native slot.
fn boot(name: &str, target: &str, bound: &[&str]) -> (App, Runner<Grants>, Calls) {
    let app = App::new(name);
    let (plan, _) = contract::compile_path_all(&app.0.join("app.contract"), false)
        .unwrap_or_else(|e| panic!("{e:?}"));
    let mut r = Runner::boot(
        plan,
        Grants,
        exact_kernel::Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    r.set_module_target(target);
    let calls: Calls = Arc::default();
    let seen = calls.clone();
    r.native_slot()
        .host(Some(Arc::new(move |body: Vec<u8>, reply: Reply| {
            let v: Json = serde_json::from_slice(&body).unwrap();
            seen.lock().unwrap().push(v.clone());
            let answer = match (v["send"].as_str(), v["source"].as_str()) {
                (Some("requestAgeRange"), _) => json!({ "status": "sharing", "lowerBound": 18, "upperBound": null, "declaration": "self", "apple": null, "android": null }),
                (_, Some("ageRangeStatus")) => json!({ "available": true, "regulated": false, "apple": null }),
                _ => json!({ "available": true, "status": "authorized", "state": "granted" }),
            };
            reply.send(Outcome::Response(Response {
                status: 200,
                headers: Vec::new(),
                body: answer.to_string().into_bytes(),
            }));
        })));
    r.bind_extensions(&[], bound);
    (app, r, calls)
}

fn pump(r: &mut Runner<Grants>, parked: &mut Vec<(u64, u64)>) {
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

fn run(r: &mut Runner<Grants>, ticket: u64, dispatch: Dispatch) {
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

fn sent(calls: &Calls, name: &str) -> Vec<Json> {
    (calls.lock().unwrap().iter())
        .filter(|c| c["send"] == name)
        .cloned()
        .collect()
}

fn logs(r: &Runner<Grants>) -> String {
    exact_runner::agent::handle(r, r#"{"op":"logs"}"#)
}

/// The one ticket held: its capability, choices and arguments.
fn held(r: &Runner<Grants>) -> (u64, String, Vec<String>, Json) {
    let holds = r.device_holds();
    assert_eq!(holds.len(), 1, "{}", logs(r));
    let hold = holds[0].clone();
    let args: Json = serde_json::from_str(&hold.args).unwrap();
    (hold.ticket, hold.capability, hold.choices, args)
}

fn answer(
    r: &mut Runner<Grants>,
    parked: &mut Vec<(u64, u64)>,
    ticket: u64,
    op: &str,
    word: &str,
) -> Result<(), String> {
    let key = if op == "tap" { "choice" } else { "text" };
    let line = json!({ "op": op, "ticket": ticket, key: word }).to_string();
    let (said, _) = exact_runner::agent::answer(r, &line).expect("a module's ticket");
    pump(r, parked);
    match said.contains("\"error\"") {
        true => Err(said),
        false => Ok(()),
    }
}

/// Every request holds an `age-range` sheet naming the gates, the target and
/// the origin; a typed age reaches the implementation as the call's
/// `preferred`; a tool's request fails `person-required` before any sheet.
#[test]
fn an_age_request_holds_its_sheet_and_a_tool_cannot_ask() {
    let (_app, mut r, calls) = boot("ask", "ios", &["age-range"]);
    let mut parked = Vec::new();
    pump(&mut r, &mut parked);
    r.act("ask", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    let (ticket, capability, choices, args) = held(&r);
    assert_eq!(capability, "age-range");
    assert_eq!(choices, ["decline", "unverified", "unavailable"]);
    assert_eq!(
        (
            &args["options"]["ageGates"],
            &args["target"],
            &args["origin"]
        ),
        (&json!([13.0, 18.0]), &json!("ios"), &json!("app"))
    );
    assert!(
        sent(&calls, "requestAgeRange").is_empty(),
        "held before the implementation"
    );
    answer(&mut r, &mut parked, ticket, "type", "30").unwrap();
    let asked = sent(&calls, "requestAgeRange");
    assert_eq!(asked.len(), 1, "{}", logs(&r));
    assert_eq!(asked[0]["preferred"], json!({ "typed": ["30"] }));

    r.act_from(CallOrigin::Tool, "ask", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    assert!(r.device_holds().is_empty(), "{}", logs(&r));
    assert_eq!(sent(&calls, "requestAgeRange").len(), 1);
    assert!(
        logs(&r).contains("age-range.requestAgeRange: refused, person-required (tool origin)"),
        "{}",
        logs(&r)
    );
}

/// Where the status says the region requires the signal, Apple's sheet has
/// no decline: the ticket does not take it, and the agent can answer the
/// sheet otherwise.
#[test]
fn a_required_signal_leaves_no_decline() {
    let (_app, mut r, _) = boot("required", "ios", &["age-range"]);
    let mut parked = Vec::new();
    pump(&mut r, &mut parked);
    let required = r#"{"op":"prefer","extension":{"age-range.ageRangeStatus":{"apple":{"requiredRegulatoryFeatures":["declaredAgeRangeRequired"]}}}}"#;
    r.extension_agent(required).unwrap().1.unwrap();
    r.act("ask", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    let (ticket, _, choices, _) = held(&r);
    assert_eq!(choices, ["unverified", "unavailable"]);
    let refused = answer(&mut r, &mut parked, ticket, "tap", "decline").unwrap_err();
    assert!(refused.contains("not decline"), "{refused}");
    answer(&mut r, &mut parked, ticket, "type", "15").unwrap();
    assert!(r.device_holds().is_empty());
}

/// On iOS each request holds a `tracking` sheet answered by tap alone, which
/// reaches the implementation; on every other target the request fails
/// `NotSupportedError` before any sheet, and the status, answered by the
/// declared defaults, reads `denied` (D13, D17).
#[test]
fn tracking_is_ios_and_tvos_alone() {
    let (_app, mut r, calls) = boot("track", "ios", &["tracking-consent"]);
    let mut parked = Vec::new();
    pump(&mut r, &mut parked);
    r.act("track", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    let (ticket, capability, choices, _) = held(&r);
    assert_eq!(
        (capability.as_str(), choices),
        (
            "tracking",
            vec!["allow".to_owned(), "deny".into(), "dismiss".into()]
        )
    );
    let typed = answer(&mut r, &mut parked, ticket, "type", "yes").unwrap_err();
    assert!(typed.contains("tap"), "{typed}");
    answer(&mut r, &mut parked, ticket, "tap", "allow").unwrap();
    let asked = sent(&calls, "requestTrackingAuthorization");
    assert_eq!(asked.len(), 1, "{}", logs(&r));
    assert_eq!(asked[0]["preferred"], json!({ "tap": "allow" }));

    for target in ["web", "macos", "android", "linux", "windows"] {
        let (_app, mut r, calls) = boot("absent", target, &[]);
        let mut parked = Vec::new();
        pump(&mut r, &mut parked);
        r.act("track", Vec::new()).unwrap();
        pump(&mut r, &mut parked);
        assert!(r.device_holds().is_empty(), "{target}: {}", logs(&r));
        assert!(calls.lock().unwrap().is_empty(), "{target}");
        let said = format!("{:?}", r.resource("tracking"));
        // available, status, state: false, denied, denied.
        assert!(
            said.contains(r#"[Bool(false), Str("denied"), Str("denied")]"#),
            "{target}: {said}"
        );
        let failed = format!("{:?}", r.resource("asked"));
        assert!(!failed.contains("authorized"), "{target}: {failed}");
    }
}
