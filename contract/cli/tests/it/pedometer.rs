//! `@exact/pedometer` and `@exact/altimeter` under the declared substitute
//! (LLP 1133.026 §11): one `motion-fitness` decision across the two
//! interfaces on an iOS target, the altimeter's permission absent on an
//! Android one, a count's track and its `since`, a hidden stretch, and the
//! origin rule. The packages are the repository's own, installed beside the
//! app as `exact add` would.

use exact_runner::{CallOrigin, Dispatch, Runner, Work};
use serde_json::{json, Value as Json};
#[cfg(unix)]
use std::os::unix::fs::symlink as symlink_dir;
#[cfg(windows)]
use std::os::windows::fs::symlink_dir;
use std::path::{Path, PathBuf};

const APP: &str = r#"use pedometer, pedometerStatus, requestPedometerPermission, Pedometer, PedometerStatus from "@exact/pedometer"
use altimeter, altimeterStatus, requestAltimeterPermission, Altimeter, AltimeterStatus from "@exact/altimeter"

component App
  state walking = false
  state start = 0
  resource steps = pedometerStatus() as shape PedometerStatus
  resource height = altimeterStatus() as shape AltimeterStatus
  resource count = pedometer(start=start) as shape Pedometer else empty()
  resource climb = altimeter(interval=2000) as shape Altimeter else empty()
  mutation askedSteps as shape PedometerStatus
  mutation askedHeight as shape AltimeterStatus
  action walk
    send askedSteps = requestPedometerPermission()
    send askedHeight = requestAltimeterPermission()
  action from(at: number)
    start = at
  view
    column
      text steps.state
      text height.state
"#;

struct App(PathBuf);
impl App {
    fn new(name: &str, source: &str) -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("exact-pedometer-{name}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        let packages = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages");
        std::fs::create_dir_all(path.join("node_modules/@exact")).unwrap();
        for package in ["pedometer", "altimeter"] {
            let to = packages.join(package).canonicalize().unwrap();
            symlink_dir(to, path.join("node_modules/@exact").join(package)).unwrap();
        }
        std::fs::write(path.join("app.contract"), source).unwrap();
        Self(path.canonicalize().unwrap())
    }
}
impl Drop for App {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The app's data grants motion.
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
        "device.motion purpose.motion\n"
    }
}

/// A runner over `app` on `target`, the session clock's zero at `epoch`.
fn boot(app: &App, target: &str) -> Runner<Granted> {
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
    r.set_module_target(target);
    r
}

/// Every request dispatched (a held one parked), released work run, each
/// outcome fulfilled, until nothing is left.
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
            if let Dispatch::Run(Work::Now(work)) = dispatch {
                let _ = r.fulfill(out.ticket, work());
            }
        }
        for (token, dispatch) in released {
            let ticket = parked.iter().find(|(t, _)| *t == token).expect("parked").1;
            if let Dispatch::Run(Work::Now(work)) = dispatch {
                let _ = r.fulfill(ticket, work());
            }
        }
    }
}

fn logs(r: &Runner<Granted>) -> String {
    exact_runner::agent::handle(r, r#"{"op":"logs"}"#)
}

/// Resource `name`'s answer as JSON, as the agent's `state` shows it.
fn answer(r: &Runner<Granted>, name: &str) -> Json {
    let state = exact_runner::agent::handle(r, r#"{"op":"state"}"#);
    let state: Json = serde_json::from_str(&state).unwrap();
    state["resources"][name].clone()
}

fn prefer(r: &mut Runner<Granted>, request: Json) {
    let (_, reply) = r.extension_agent(&request.to_string()).expect("a prefer");
    reply.unwrap_or_else(|e| panic!("{e}"));
}

/// D15, D19 item 2: on an iOS target the two interfaces' requests made
/// together hold one `motion-fitness` ticket, and its allow settles both
/// statuses; the readers then answer the defaults.
#[test]
fn one_decision_settles_both_interfaces_on_ios() {
    let app = App::new("ios", APP);
    let mut r = boot(&app, "ios");
    let mut parked = Vec::new();
    pump(&mut r, &mut parked);
    assert_eq!(answer(&r, "steps")["state"], "prompt", "{}", logs(&r));
    assert_eq!(answer(&r, "height")["state"], "prompt");
    let refused = "count failed: pedometer.pedometer: NotAllowedError: requestPedometerPermission";
    assert!(
        logs(&r).contains(refused),
        "a reader waits for the request: {}",
        logs(&r)
    );

    r.act("walk", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    let holds = r.device_holds().to_vec();
    assert_eq!(
        holds.len(),
        1,
        "one sheet for both: {holds:?}\n{}",
        logs(&r)
    );
    let args: Json = serde_json::from_str(&holds[0].args).unwrap();
    assert_eq!(args["capability"], "motion-fitness");
    let tap = format!(
        r#"{{"op":"tap","ticket":{},"choice":"allow"}}"#,
        holds[0].ticket
    );
    exact_runner::agent::answer(&mut r, &tap).unwrap();
    pump(&mut r, &mut parked);
    assert!(r.device_holds().is_empty(), "{}", logs(&r));
    assert_eq!(answer(&r, "steps")["state"], "granted", "{}", logs(&r));
    assert_eq!(answer(&r, "height")["state"], "granted");
    assert_eq!(answer(&r, "count")["steps"], 0, "{}", logs(&r));
    assert_eq!(answer(&r, "climb")["relativeAltitude"], 0.0);
}

/// D15, D19 item 2: on an Android target the altimeter declares no
/// permission, so its status reads `granted` and its request answers at
/// once, while the pedometer's still asks.
#[test]
fn android_asks_only_the_pedometer() {
    let app = App::new("android", APP);
    let mut r = boot(&app, "android");
    let mut parked = Vec::new();
    pump(&mut r, &mut parked);
    assert_eq!(answer(&r, "height")["state"], "granted", "{}", logs(&r));
    assert_eq!(answer(&r, "steps")["state"], "prompt");
    assert_eq!(answer(&r, "climb")["relativeAltitude"], 0.0, "{}", logs(&r));
    r.act("walk", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    let holds = r.device_holds().to_vec();
    assert_eq!(holds.len(), 1, "{holds:?}");
    let tap = format!(
        r#"{{"op":"tap","ticket":{},"choice":"deny"}}"#,
        holds[0].ticket
    );
    exact_runner::agent::answer(&mut r, &tap).unwrap();
    pump(&mut r, &mut parked);
    assert_eq!(answer(&r, "steps")["state"], "denied", "{}", logs(&r));
    assert_eq!(answer(&r, "height")["state"], "granted");
}

/// §11: a count's track holds each step from its instant; `since` is the
/// reader's `start`, or the group's first ask, and a step's own `since`
/// moves it; while the page is hidden the count still answers the track's
/// value, and the altimeter's batch reads `interrupted`; a tool's call is
/// refused `person-required`.
#[test]
fn a_count_answers_its_track_and_since() {
    let app = App::new("track", APP);
    let mut r = boot(&app, "linux");
    let mut parked = Vec::new();
    pump(&mut r, &mut parked);
    // No permission on Linux: the readers answer at once.
    let first = answer(&r, "count");
    assert_eq!(first["steps"], 0, "{first}\n{}", logs(&r));
    let since = first["since"].as_f64().unwrap();
    assert_eq!(since, first["timestamp"].as_f64().unwrap(), "{first}");

    prefer(
        &mut r,
        json!({ "op": "prefer", "extension": { "pedometer.pedometer":
            [{ "at": 0, "steps": 0 }, { "at": 60000, "steps": 110 }, { "at": 120000, "steps": 230 }] } }),
    );
    r.advance(61_000.0).unwrap();
    pump(&mut r, &mut parked);
    assert_eq!(answer(&r, "count")["steps"], 110, "{}", logs(&r));
    assert_eq!(answer(&r, "count")["since"].as_f64(), Some(since));

    // Hidden: the count holds the track's value at the time; the
    // altimeter's next batch says it was interrupted.
    let page = r.page();
    r.set_page(exact_runner::Page {
        hidden: true,
        ..page
    })
    .unwrap();
    r.advance(122_000.0).unwrap();
    pump(&mut r, &mut parked);
    assert_eq!(answer(&r, "count")["steps"], 230, "{}", logs(&r));
    r.set_page(page).unwrap();
    r.advance(124_000.0).unwrap();
    pump(&mut r, &mut parked);
    assert_eq!(answer(&r, "climb")["interrupted"], true, "{}", logs(&r));

    // A step's own `since` moves it, as a reboot would.
    prefer(
        &mut r,
        json!({ "op": "prefer", "extension": { "pedometer.pedometer":
            [{ "at": 0, "steps": 12, "since": 5 }] } }),
    );
    r.advance(126_000.0).unwrap();
    pump(&mut r, &mut parked);
    let moved = answer(&r, "count");
    assert_eq!(
        (moved["steps"].clone(), moved["since"].clone()),
        (json!(12), json!(5)),
        "{moved}"
    );

    // A reader's `start` is its `since`.
    r.act("from", vec![exact_runner::Value::Number(1000.0)])
        .unwrap();
    prefer(
        &mut r,
        json!({ "op": "prefer", "extension": { "pedometer.pedometer": "clear" } }),
    );
    r.advance(128_000.0).unwrap();
    pump(&mut r, &mut parked);
    let started = answer(&r, "count");
    assert_eq!(
        started["since"].as_f64(),
        Some(1000.0_f64.max(since - 604_800_000.0)),
        "{started}"
    );

    r.act_from(CallOrigin::Tool, "walk", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    assert!(logs(&r).contains("person-required"), "{}", logs(&r));
}

/// D13: a relative altitude's `since` is when its group's zero was taken,
/// and a track gives the height directly.
#[test]
fn an_altitude_counts_from_its_group_s_zero() {
    let app = App::new("height", APP);
    let mut r = boot(&app, "linux");
    let mut parked = Vec::new();
    pump(&mut r, &mut parked);
    let zero = answer(&r, "climb");
    assert!(zero["since"].is_number(), "{zero}\n{}", logs(&r));
    prefer(
        &mut r,
        json!({ "op": "prefer", "extension": { "altimeter.altimeter":
            [{ "at": 0, "relativeAltitude": 0 }, { "at": 1000, "relativeAltitude": 86 }] } }),
    );
    r.advance(4000.0).unwrap();
    pump(&mut r, &mut parked);
    let climbed = answer(&r, "climb");
    assert_eq!(climbed["relativeAltitude"], 86.0, "{climbed}\n{}", logs(&r));
    assert_eq!(climbed["since"], zero["since"]);
    let barometer: Json = serde_json::from_str(&r.extensions_json().unwrap()).unwrap();
    assert!(barometer["altimeter"].is_object(), "{barometer}");
}

/// D15, D19 item 4: an app that uses a pedometer or altimeter name derives
/// `NSMotionUsageDescription` under its motion line, and on Android the
/// pedometer's `ACTIVITY_RECOGNITION`; the altimeter asks nothing there.
#[test]
fn the_names_an_app_uses_derive_its_keys() {
    let app = App::new("derive", APP);
    let (plan, _) = contract::compile_path_all(&app.0.join("app.contract"), false).unwrap();
    let pair = |line: &str, key: &str| vec![(line.to_owned(), key.to_owned())];
    let ios = contract::native::derived(&plan, APP, "ios");
    assert_eq!(ios, pair("device.motion", "NSMotionUsageDescription"));
    let android = |text: &str| contract::native::derived(&plan, text, "android");
    let recognition = "android.permission.ACTIVITY_RECOGNITION";
    assert_eq!(android(APP), pair("device.motion", recognition));
    // The status alone asks nothing: an app that reads it is told the
    // pedometer is there, and derives no permission until it counts.
    let only_height = APP
        .replace(
            "use pedometer, pedometerStatus, requestPedometerPermission, Pedometer,",
            "use pedometerStatus,",
        )
        .replace(
            "  resource count = pedometer(start=start) as shape Pedometer else empty()\n",
            "",
        )
        .replace("    send askedSteps = requestPedometerPermission()\n", "");
    assert!(
        android(&only_height).is_empty(),
        "{:?}",
        android(&only_height)
    );
}
