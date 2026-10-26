//! `@exact/health` admission in the runner (LLP 1133.026 D19 item 1, §11):
//! a permission per data type, chosen by a call's argument; one ticket for
//! a request's list, answered whole or typed; `unknown` stored on an iOS
//! target with the person's answer kept as facts, `denied` on an Android
//! one; and the origin rule. The interface is its own substitute, so the
//! implementation is played by the test as the host's native slot, over
//! `packages/health/fixture/health.json`, as the Apple and Android
//! implementations answer under the agent.

use exact_runner::{CallOrigin, Dispatch, Outcome, Reply, Response, Runner, Work};
use serde_json::{json, Value as Json};
#[cfg(unix)]
use std::os::unix::fs::symlink as symlink_dir;
#[cfg(windows)]
use std::os::windows::fs::symlink_dir;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// 2026-10-01, midnight UTC.
const OCT1: f64 = 1_790_812_800_000.0;
const DAY: f64 = 86_400_000.0;

const APP: &str = r#"use healthStatus, requestHealthAuthorization, healthStatistics, healthWorkouts, HealthStatus, HealthStatistics, HealthWorkoutPage from "@exact/health"

component App
  resource status = healthStatus(read=["steps", "workouts"]) as shape HealthStatus
  resource days = healthStatistics(type="steps", start=1790812800000, end=1792022400000, interval="P1D") as shape HealthStatistics else empty()
  resource walks = healthWorkouts(start=1790812800000, end=1792022400000) as shape HealthWorkoutPage else empty()
  mutation asked as shape HealthStatus
  action ask
    send asked = requestHealthAuthorization(read=["steps", "workouts"])
  view
    column
      text status.state
"#;

struct App(PathBuf);
impl App {
    fn new(name: &str, source: &str) -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("exact-health-{name}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        let packages = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages");
        std::fs::create_dir_all(path.join("node_modules/@exact")).unwrap();
        let to = packages.join("health").canonicalize().unwrap();
        symlink_dir(to, path.join("node_modules/@exact/health")).unwrap();
        std::fs::write(path.join("app.contract"), source).unwrap();
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
        "device.health-read purpose.health\n"
    }
}

/// What the implementation was told: each permission's state and facts.
#[derive(Default)]
struct Told {
    permissions: serde_json::Map<String, Json>,
    facts: serde_json::Map<String, Json>,
    calls: Vec<Json>,
}

/// The fixture implementation (§11): it answers from the fixture store,
/// applying the states and facts it was told, as the platform answers this
/// app. A type the person did not share answers what HealthKit answers for
/// it, the app's own samples: none.
struct Fixture {
    store: Json,
    told: Mutex<Told>,
}

impl Fixture {
    fn new() -> Self {
        let path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/health/fixture/health.json");
        let store = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        Self {
            store,
            told: Mutex::default(),
        }
    }

    fn answer(&self, body: &Json) -> Option<Json> {
        let mut told = self.told.lock().unwrap();
        if body["preferred"].is_object() && body["args"].is_null() {
            told.permissions = body["permissions"].as_object().cloned().unwrap_or_default();
            told.facts = (body["preferred"]["facts"].as_object())
                .cloned()
                .unwrap_or_default();
            return None;
        }
        told.calls.push(body.clone());
        let args = &body["args"];
        let name = body["source"].as_str().or(body["send"].as_str())?;
        let state = |t: &str| {
            (told.permissions.get(&format!("health.{t}")))
                .and_then(Json::as_str)
                .unwrap_or("prompt")
                .to_owned()
        };
        let fact = |t: &str, f: &str| told.facts.get(&format!("health.{t}")).map(|x| x[f].clone());
        let shared = |t: &str| fact(t, "shared") != Some(json!(false));
        let since = |t: &str| fact(t, "since").filter(Json::is_number);
        Some(match name {
            "healthStatus" | "requestHealthAuthorization" => {
                let types: Vec<Json> = (args["read"].as_array().into_iter().flatten())
                    .filter_map(Json::as_str)
                    .map(|t| {
                        let read = state(t);
                        json!({ "type": t, "read": read, "since": since(t), "complete": read == "granted" })
                    })
                    .collect();
                let reads: Vec<&str> = types.iter().filter_map(|t| t["read"].as_str()).collect();
                let summary = if reads.contains(&"prompt") {
                    "prompt"
                } else if reads.contains(&"unknown") {
                    "unknown"
                } else if reads.iter().all(|r| *r == "denied") {
                    "denied"
                } else {
                    "granted"
                };
                json!({ "available": true, "state": summary, "types": types })
            }
            "healthStatistics" => {
                let t = args["type"].as_str().unwrap_or("steps");
                let (start, end) = (args["start"].as_f64()?, args["end"].as_f64()?);
                let from = since(t).and_then(|s| s.as_f64());
                let hours = &self.store["hours"][t];
                let mut buckets = Vec::new();
                let mut at = start;
                while at < end {
                    let day = date(at);
                    let seen = shared(t) && from.is_none_or(|s| at + DAY > s);
                    let value = (hours[&day].as_array())
                        .filter(|_| seen)
                        .map(|h| h.iter().filter_map(Json::as_f64).sum::<f64>());
                    buckets.push(json!({ "start": at, "end": at + DAY, "value": value }));
                    at += DAY;
                }
                json!({ "type": t, "unit": self.store["units"][t], "since": from,
                        "complete": state(t) == "granted", "buckets": buckets })
            }
            "healthWorkouts" => {
                let (start, end) = (args["start"].as_f64()?, args["end"].as_f64()?);
                let workouts: Vec<Json> = (self.store["workouts"].as_array().into_iter().flatten())
                    .filter(|_| shared("workouts"))
                    .filter(|w| (start..end).contains(&w["start"].as_f64().unwrap_or(0.0)))
                    .cloned()
                    .collect();
                json!({ "workouts": workouts, "next": null, "since": since("workouts"),
                        "complete": state("workouts") == "granted" })
            }
            _ => json!({ "error": "NotSupportedError" }),
        })
    }
}

/// `ms` since the epoch as its UTC date, `2026-10-12`.
fn date(ms: f64) -> String {
    let days = (ms / DAY).floor() as i64;
    // Civil from days (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

/// A runner over `source` on `target`, the implementation bound under the
/// agent and played by the fixture.
fn boot(name: &str, source: &str, target: &str) -> (App, Runner<Granted>, Arc<Fixture>) {
    let app = App::new(name, source);
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
    let fixture = Arc::new(Fixture::new());
    let plays = fixture.clone();
    r.native_slot()
        .host(Some(Arc::new(move |body: Vec<u8>, reply: Reply| {
            let v: Json = serde_json::from_slice(&body).unwrap();
            if let Some(answer) = plays.answer(&v) {
                reply.send(Outcome::Response(Response {
                    status: 200,
                    headers: Vec::new(),
                    body: answer.to_string().into_bytes(),
                }));
            }
        })));
    r.set_module_target(target);
    r.bind_extensions(&[], &["health"]);
    (app, r, fixture)
}

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

fn logs(r: &Runner<Granted>) -> String {
    exact_runner::agent::handle(r, r#"{"op":"logs"}"#)
}

fn answer(r: &Runner<Granted>, name: &str) -> Json {
    let state = exact_runner::agent::handle(r, r#"{"op":"state"}"#);
    let state: Json = serde_json::from_str(&state).unwrap();
    state["resources"][name].clone()
}

fn extensions(r: &Runner<Granted>) -> Json {
    serde_json::from_str(&r.extensions_json().unwrap()).unwrap()
}

/// The one ticket held: its number and the permissions it asks.
fn held(r: &Runner<Granted>) -> (u64, Json) {
    let holds = r.device_holds();
    assert_eq!(holds.len(), 1, "{holds:?}\n{}", logs(r));
    let args: Json = serde_json::from_str(&holds[0].args).unwrap();
    (holds[0].ticket, args["permissions"].clone())
}

fn answer_hold(r: &mut Runner<Granted>, op: Json) {
    exact_runner::agent::answer(r, &op.to_string()).unwrap();
}

/// The value of `days`' bucket starting at `date`.
fn day(r: &Runner<Granted>, date: f64) -> Option<f64> {
    let days = answer(r, "days");
    let buckets = days["buckets"].as_array().cloned().unwrap_or_default();
    (buckets.iter())
        .find(|b| b["start"].as_f64() == Some(date))
        .unwrap_or_else(|| panic!("no bucket at {date}: {days}"))["value"]
        .as_f64()
}

/// D6, D19 item 1, §11: while a type is `prompt` its read fails naming the
/// request; the request's list is one ticket; on an iOS target a typed
/// partial answer stores `unknown` for both, and the person's answer is
/// kept as facts, which the implementation applies: steps read the
/// fixture's days, workouts nothing, and neither says it was denied.
#[test]
fn ios_stores_unknown_and_keeps_the_answer_as_facts() {
    let (_app, mut r, fixture) = boot("ios", APP, "ios");
    let mut parked = Vec::new();
    pump(&mut r, &mut parked);
    assert_eq!(answer(&r, "status")["state"], "prompt", "{}", logs(&r));
    let refused =
        "days failed: health.healthStatistics: NotAllowedError: requestHealthAuthorization";
    assert!(logs(&r).contains(refused), "{}", logs(&r));

    r.act("ask", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    let (ticket, asked) = held(&r);
    assert_eq!(asked, json!(["health.steps", "health.workouts"]));
    answer_hold(
        &mut r,
        json!({ "op": "type", "ticket": ticket, "text": "steps" }),
    );
    pump(&mut r, &mut parked);
    assert!(r.device_holds().is_empty(), "{}", logs(&r));

    let status = answer(&r, "status");
    assert_eq!(status["state"], "unknown", "{status}\n{}", logs(&r));
    let reads: Vec<&Json> = (status["types"].as_array().unwrap().iter())
        .map(|t| &t["read"])
        .collect();
    assert_eq!(reads, [&json!("unknown"), &json!("unknown")]);
    assert_eq!(status["types"][0]["complete"], false);
    let facts = &extensions(&r)["health"]["facts"];
    assert_eq!(facts["health.workouts"]["shared"], false, "{facts}");

    // 2026-10-12 sums to 8,412; 2026-10-04 has no samples; 2026-10-05 zero.
    assert_eq!(day(&r, OCT1 + 11.0 * DAY), Some(8412.0), "{}", logs(&r));
    assert_eq!(day(&r, OCT1 + 3.0 * DAY), None);
    assert_eq!(day(&r, OCT1 + 4.0 * DAY), Some(0.0));
    let walks = answer(&r, "walks");
    assert_eq!(walks["workouts"], json!([]), "admitted, and empty: {walks}");
    let sent = fixture.told.lock().unwrap().calls.len();
    assert!(sent >= 3, "{sent}");

    // `denied` is not an iOS word; `shared=` is.
    let denied = json!({ "op": "prefer", "permission": { "health.steps": "denied" } });
    let (_, reply) = r.extension_agent(&denied.to_string()).unwrap();
    assert!(reply.unwrap_err().contains("shared=false"));
    let window = json!({ "op": "prefer", "permission": { "health.steps": "since=2026-10-10" } });
    r.extension_agent(&window.to_string()).unwrap().1.unwrap();
    pump(&mut r, &mut parked);
    assert_eq!(day(&r, OCT1 + 8.0 * DAY), None, "{}", logs(&r));
    assert_eq!(day(&r, OCT1 + 11.0 * DAY), Some(8412.0));
}

/// D19 item 1: `steps@<date>` grants with a window; `allow` grants every
/// permission the ticket asks.
#[test]
fn a_typed_window_and_a_whole_allow() {
    let (_app, mut r, _) = boot("window", APP, "ios");
    let mut parked = Vec::new();
    pump(&mut r, &mut parked);
    r.act("ask", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    let (ticket, _) = held(&r);
    answer_hold(
        &mut r,
        json!({ "op": "type", "ticket": ticket, "text": "steps@2026-10-10,workouts" }),
    );
    pump(&mut r, &mut parked);
    let status = answer(&r, "status");
    assert_eq!(
        status["types"][0]["since"].as_f64(),
        Some(OCT1 + 9.0 * DAY),
        "{status}"
    );
    assert_eq!(day(&r, OCT1 + 8.0 * DAY), None);
    assert_eq!(day(&r, OCT1 + 11.0 * DAY), Some(8412.0));
    let walks = answer(&r, "walks")["workouts"].as_array().unwrap().len();
    assert_eq!(walks, 4, "w5 to w8 begin in the range");

    let (_app, mut r, _) = boot("allow", APP, "ios");
    pump(&mut r, &mut parked);
    r.act("ask", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    let (ticket, _) = held(&r);
    answer_hold(
        &mut r,
        json!({ "op": "tap", "ticket": ticket, "choice": "allow" }),
    );
    pump(&mut r, &mut parked);
    let facts = &extensions(&r)["health"]["facts"];
    assert_eq!(facts["health.steps"]["shared"], true, "{facts}");
    assert_eq!(facts["health.workouts"]["shared"], true);
}

/// D5, D19 item 1: on an Android target the states are readable: a typed
/// partial answer grants steps and denies workouts, whose read then fails;
/// `history` is a permission there and nowhere else.
#[test]
fn android_denies_what_the_person_declined() {
    let src = APP.replace(
        r#"requestHealthAuthorization(read=["steps", "workouts"])"#,
        r#"requestHealthAuthorization(read=["steps", "workouts"], android.readHealthDataHistory=true)"#,
    );
    let (_app, mut r, _) = boot("android", &src, "android");
    let mut parked = Vec::new();
    pump(&mut r, &mut parked);
    r.act("ask", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    let (ticket, asked) = held(&r);
    assert_eq!(
        asked,
        json!(["health.steps", "health.workouts", "health.history"])
    );
    answer_hold(
        &mut r,
        json!({ "op": "type", "ticket": ticket, "text": "steps" }),
    );
    pump(&mut r, &mut parked);
    let status = answer(&r, "status");
    assert_eq!(status["state"], "granted", "{status}");
    assert_eq!(status["types"][1]["read"], "denied");
    assert_eq!(day(&r, OCT1 + 11.0 * DAY), Some(8412.0));
    let refused = "walks failed: health.healthWorkouts: NotAllowedError";
    assert!(logs(&r).contains(refused), "{}", logs(&r));

    // On iOS the same request asks no history.
    let (_app, mut r, _) = boot("ios-history", &src, "ios");
    pump(&mut r, &mut parked);
    r.act("ask", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    assert_eq!(held(&r).1, json!(["health.steps", "health.workouts"]));
}

/// D7: a tool's read and request fail `person-required` in every state;
/// the status answers every caller.
#[test]
fn a_tool_never_reads_health() {
    let src = APP.replace(
        "  action ask\n",
        "  action peek\n    send asked = requestHealthAuthorization(read=[\"steps\"])\n  action ask\n",
    );
    let (_app, mut r, fixture) = boot("tool", &src, "ios");
    let mut parked = Vec::new();
    pump(&mut r, &mut parked);
    let granted = json!({ "op": "prefer", "permission": { "health": "granted" } });
    r.extension_agent(&granted.to_string()).unwrap().1.unwrap();
    pump(&mut r, &mut parked);
    assert_eq!(answer(&r, "status")["state"], "unknown", "{}", logs(&r));
    r.act_from(CallOrigin::Tool, "peek", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    assert!(logs(&r).contains("person-required"), "{}", logs(&r));
    let asked = (fixture.told.lock().unwrap().calls.iter())
        .filter(|c| c["send"] == "requestHealthAuthorization")
        .count();
    assert_eq!(asked, 0);
}

/// D1, D19 item 4: `read` is a literal list; a computed one is refused at
/// bake.
#[test]
fn a_computed_read_list_is_refused_at_bake() {
    let computed = APP.replace(
        r#"healthStatus(read=["steps", "workouts"])"#,
        r#"healthStatus(read=(true ? ["steps"] : ["workouts"]))"#,
    );
    let app = App::new("computed", &computed);
    let Err(refused) = contract::compile_path_all(&app.0.join("app.contract"), false) else {
        panic!("a computed read list is refused");
    };
    assert!(
        format!("{refused:?}").contains("contract-module-literal"),
        "{refused:?}"
    );
}

/// D4, D19 item 4: Android's `READ_*` permissions are those of the types
/// the app's literal lists name, and history's where it sets the flag.
#[test]
fn the_read_list_derives_its_android_permissions() {
    let app = App::new("derive", APP);
    let (plan, _) = contract::compile_path_all(&app.0.join("app.contract"), false).unwrap();
    let android = |text: &str| -> Vec<String> {
        (contract::native::derived(&plan, text, "android").into_iter())
            .map(|(_, p)| p)
            .collect()
    };
    let (steps, exercise) = (
        "android.permission.health.READ_STEPS",
        "android.permission.health.READ_EXERCISE",
    );
    let mut read = android(APP);
    read.sort();
    assert_eq!(read, [exercise, steps]);
    let history = APP.replace(
        r#"requestHealthAuthorization(read=["steps", "workouts"])"#,
        r#"requestHealthAuthorization(read=["steps", "workouts"], android.readHealthDataHistory=true)"#,
    );
    let all = android(&history);
    assert!(all.contains(&"android.permission.health.READ_HEALTH_DATA_HISTORY".to_owned()));
    assert!(contract::native::derived(&plan, APP, "ios").is_empty());
}
