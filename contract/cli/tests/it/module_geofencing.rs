//! LLP 1133.021 stage 0 for `@exact/geofencing`, the package copied out of
//! the checkout as an installed package is: a permission that adds to
//! another (`base`), asked one ticket on Apple and two on Android; a send
//! that answers the entry it lists, bounded by `most`; a tap that chooses
//! its event by a field and flips it; an entry that stays whatever its
//! invocation's outcome, keeping an undelivered transition; and a
//! `--wake` entry delivered cold, or at a launch with a view.

use exact_runner::{agent, Runner, Value};
use serde_json::{json, Value as Json};
use std::path::{Path, PathBuf};

struct App(PathBuf);
impl App {
    fn new(name: &str, contract: &str) -> Self {
        Self::with_data(name, contract, "")
    }
    fn with_data(name: &str, contract: &str, data: &str) -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("exact-fence-{name}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        let packages = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../packages");
        for name in ["geofencing", "notifications"] {
            copy(
                &packages.join(name),
                &path.join("node_modules/@exact").join(name),
            );
        }
        std::fs::write(path.join("app.contract"), contract).unwrap();
        if !data.is_empty() {
            std::fs::write(path.join("app.ts"), data).unwrap();
        }
        Self(path.canonicalize().unwrap())
    }
    fn compile(&self) -> Result<exact_plan::Plan, Vec<contract::CompileError>> {
        contract::compile_path_all(&self.0.join("app.contract"), false).map(|(plan, _)| plan)
    }
    fn plan(&self) -> exact_plan::Plan {
        self.compile().unwrap_or_else(|e| panic!("{e:?}"))
    }
}
impl Drop for App {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn copy(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() {
            copy(&entry.path(), &to.join(entry.file_name()));
        } else {
            std::fs::copy(entry.path(), to.join(entry.file_name())).unwrap();
        }
    }
}

const APP: &str = r#"use addGeofence, removeGeofence, geofences, geofencingStatus, geofenceenter, geofenceleave, Geofence, GeofenceEvent, GeofencingStatus from "@exact/geofencing"
use showNotification from "@exact/notifications"

component App
  resource status = geofencingStatus() as shape GeofencingStatus
  resource fences = geofences() as shape Geofence[]
  mutation added as shape Geofence
  state heard = ""
  action entered(e: GeofenceEvent)
    heard = `enter ${e.geofence.name} ${e.timestamp}`
    showNotification(title=`Near ${e.geofence.name}`, body="")
  action left(e: GeofenceEvent)
    heard = `leave ${e.geofence.name}`
  action add
    send added = addGeofence(name="home", latitude=37.4419, longitude=-122.143, radius=200)
  action named(n: string)
    send added = addGeofence(name=n, latitude=37.0, longitude=-122.0, radius=150)
  action remove
    removeGeofence(name="home")
  view
    column geofenceenter=entered geofenceleave=left
      text testId="state" status.state
      text testId="heard" heard
      text testId="count" `${fences.length}`
"#;

/// The app writes its grant lines.
struct Granted;
impl exact_runner::DataSource for Granted {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, exact_runner::DataError> {
        Err(exact_runner::DataError::UnknownSource(source.into()))
    }
    fn grants(&self) -> &str {
        "device.geolocation locationPurpose\ndevice.geolocation-background alwaysPurpose\ndevice.notifications noticePurpose"
    }
    /// `pull` answers later, as a server would: a wake's owned work.
    fn answer(
        &mut self,
        _: &mut exact_runner::Store,
        _: &str,
        _: &[Value],
    ) -> Result<exact_runner::Answer, exact_runner::DataError> {
        Ok(exact_runner::Answer::Later(exact_runner::Request::native(
            b"{}".to_vec(),
        )))
    }
}

/// [`APP`] whose enter handler pulls, a request that does not land.
fn pulling() -> String {
    APP.replace(
        "  state heard = \"\"\n",
        "  state heard = \"\"\n  mutation pulled as shape Pulled\n",
    )
    .replace(
        "    showNotification(",
        "    send pulled = pull(e.geofence.name)\n    showNotification(",
    )
    .replace(
        "\ncomponent App",
        "\nshape Pulled\n  n: number\n\ncomponent App",
    )
}

const PULL: &str = r#"import type { Answer, Sources } from './app.contract.d.ts';
const sources: Sources = { pull: async () => ({ n: 7 }) };
export const answer: Answer = (source, args, store, storage, native) => sources[source](args, store, storage, native);
"#;

fn boot(plan: exact_plan::Plan, target: &str) -> Runner<Granted> {
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

fn logs(r: &Runner<Granted>) -> String {
    agent::handle(r, r#"{"op":"logs"}"#)
}

fn state(r: &Runner<Granted>) -> Json {
    serde_json::from_str(&agent::handle(r, r#"{"op":"state"}"#)).unwrap()
}

fn fences(r: &Runner<Granted>) -> Json {
    state(r)["geofences"].clone()
}

fn slot(r: &Runner<Granted>, name: &str) -> String {
    format!("{:?}", r.resource(name).or_else(|| r.slot(name)))
}

fn by(r: &mut Runner<Granted>, ms: f64) {
    let at = r.now_ms() + ms;
    r.advance(at).unwrap();
}

fn tap(r: &mut Runner<Granted>, ticket: u64, choice: Option<&str>) -> String {
    let choice = choice
        .map(|c| format!(r#","choice":"{c}""#))
        .unwrap_or_default();
    agent::answer(r, &format!(r#"{{"op":"tap","ticket":{ticket}{choice}}}"#))
        .unwrap()
        .0
}

fn prefer(r: &mut Runner<Granted>, name: &str, state: &str) {
    let request = json!({ "op": "prefer", "permission": { name: state } }).to_string();
    let (_, reply) = r.extension_agent(&request).expect("a prefer");
    reply.unwrap_or_else(|e| panic!("{e}"));
}

/// The permission state the runner reads for `name`.
fn permission(r: &Runner<Granted>, name: &str) -> String {
    let journal: Json = serde_json::from_str(&logs(r)).unwrap();
    let line = format!("permission {name}: ");
    (journal["lines"].as_array().into_iter().flatten().rev())
        .filter_map(Json::as_str)
        .find_map(|l| l.split_once(&line).map(|(_, s)| s.trim().to_owned()))
        .unwrap_or_else(|| "prompt".into())
}

thread_local! {
    static PARKED: std::cell::RefCell<Vec<(u64, u64)>> = Default::default();
}

/// The host's part: held module work run, released work run when its
/// ticket is answered, readers asked again.
fn pump(r: &mut Runner<Granted>) {
    for _ in 0..8 {
        let outs = r.take_requests();
        let released = r.release_work();
        let (asked, error) = r.apply_announced();
        assert!(error.is_none(), "{error:?}");
        if outs.is_empty() && released.is_empty() && asked.is_empty() {
            return;
        }
        for out in outs {
            let token = out.request.continuation.expect("held work");
            PARKED.with(|p| p.borrow_mut().push((token, out.ticket)));
            let dispatch = r.dispatch_work(token);
            run(r, out.ticket, dispatch);
        }
        for (token, dispatch) in released {
            let parked = PARKED.with(|p| p.borrow().iter().find(|(t, _)| *t == token).copied());
            run(r, parked.expect("parked").1, dispatch);
        }
    }
}

fn run(r: &mut Runner<Granted>, ticket: u64, dispatch: exact_runner::Dispatch) {
    if let exact_runner::Dispatch::Run(exact_runner::Work::Now(work)) = dispatch {
        r.fulfill(ticket, work()).unwrap();
    }
}

/// The permission tickets held now, oldest first, by what each asks.
fn tickets(r: &Runner<Granted>) -> Vec<(u64, String)> {
    (r.device_holds().iter())
        .filter(|h| h.capability == "permission")
        .map(|h| {
            let args: Json = serde_json::from_str(&h.args).unwrap_or_default();
            (
                h.ticket,
                args["capability"].as_str().unwrap_or_default().to_owned(),
            )
        })
        .collect()
}

#[test]
fn the_events_carry_their_budget_and_a_literal_radius_holds_its_floor() {
    let app = App::new("compile", APP);
    let plan = app.plan();
    let events: Vec<String> = (plan.module_events.iter())
        .map(|e| {
            format!(
                "{}.{} {} {} {}",
                plan.str(e.extension),
                plan.str(e.declared),
                plan.str(e.raised),
                e.hold,
                e.budget
            )
        })
        .collect();
    assert_eq!(
        events,
        [
            "geofencing.geofenceenter wake 16 30000",
            "geofencing.geofenceleave wake 16 30000"
        ]
    );
    let small = App::new("small", &APP.replace("radius=200", "radius=50"));
    let refused = small.compile().unwrap_err();
    assert!(
        refused.iter().any(|e| e.id == "contract-module-bound"),
        "{refused:?}"
    );
}

/// §12 on iOS: one ticket, for background location, whose `allow` grants
/// location too; the call answers the entry it listed, `inside` none.
/// A tap enters, the next leaves, each a wake that flips `inside`.
#[test]
fn an_apple_add_asks_once_and_a_tap_enters_then_leaves() {
    let app = App::new("apple", APP);
    let mut r = boot(app.plan(), "ios");
    r.act("add", Vec::new()).unwrap();
    pump(&mut r);
    let held = tickets(&r);
    assert_eq!(
        held.iter().map(|t| t.1.as_str()).collect::<Vec<_>>(),
        ["geolocation-background"],
        "{}",
        logs(&r)
    );
    tap(&mut r, held[0].0, Some("allow"));
    pump(&mut r);
    assert_eq!(permission(&r, "geolocation"), "granted", "{}", logs(&r));
    let listed = fences(&r);
    assert_eq!(listed[0]["name"], "home", "{listed} {}", logs(&r));
    assert_eq!(listed[0]["inside"], Json::Null);
    let added = slot(&r, "added");
    assert!(
        added.contains("home") && added.contains("200"),
        "{added} {}",
        logs(&r)
    );
    assert!(
        slot(&r, "fences").contains("home"),
        "{}",
        slot(&r, "fences")
    );
    assert!(
        slot(&r, "status").contains("granted"),
        "{}",
        slot(&r, "status")
    );

    let reference = listed[0]["ref"].as_u64().unwrap();
    let reply = tap(&mut r, reference, None);
    assert!(reply.contains(r#""event":"geofenceenter""#), "{reply}");
    by(&mut r, 0.0);
    pump(&mut r);
    assert!(slot(&r, "heard").contains("enter home"), "{}", logs(&r));
    assert!(r.page().hidden || logs(&r).contains("wake: completed"));
    by(&mut r, 0.0);
    assert!(logs(&r).contains("wake: completed"), "{}", logs(&r));
    assert_eq!(fences(&r)[0]["inside"], true, "{}", logs(&r));
    let reply = tap(&mut r, reference, None);
    assert!(reply.contains(r#""event":"geofenceleave""#), "{reply}");
    by(&mut r, 0.0);
    pump(&mut r);
    assert!(slot(&r, "heard").contains("leave home"), "{}", logs(&r));
    assert_eq!(fences(&r)[0]["inside"], false);

    // Location withdrawn withdraws background location with it.
    prefer(&mut r, "geolocation", "denied");
    pump(&mut r);
    assert_eq!(
        permission(&r, "geolocation-background"),
        "denied",
        "{}",
        logs(&r)
    );

    r.act("remove", Vec::new()).unwrap();
    pump(&mut r);
    assert_eq!(fences(&r), json!([]), "{}", logs(&r));
}

/// D13 on Android: location first, then background location; a denied
/// first ticket fails the call with no second.
#[test]
fn an_android_add_asks_location_then_background_location() {
    let app = App::new("android", APP);
    let mut r = boot(app.plan(), "android");
    r.act("add", Vec::new()).unwrap();
    pump(&mut r);
    let first = tickets(&r);
    assert_eq!(first[0].1, "geolocation", "{}", logs(&r));
    tap(&mut r, first[0].0, Some("allow"));
    pump(&mut r);
    let second = tickets(&r);
    assert_eq!(second[0].1, "geolocation-background", "{}", logs(&r));
    tap(&mut r, second[0].0, Some("allow"));
    pump(&mut r);
    assert_eq!(fences(&r)[0]["name"], "home", "{}", logs(&r));

    let mut r = boot(app.plan(), "android");
    r.act("add", Vec::new()).unwrap();
    pump(&mut r);
    let first = tickets(&r);
    tap(&mut r, first[0].0, Some("deny"));
    pump(&mut r);
    assert!(tickets(&r).is_empty(), "{}", logs(&r));
    assert!(logs(&r).contains("PERMISSION_DENIED"), "{}", logs(&r));
    assert_eq!(fences(&r), json!([]));
}

/// D11: twenty per app; the twenty-first fails `QuotaExceededError`, and
/// adding a name again replaces its fence rather than adding one.
#[test]
fn the_twenty_first_fence_is_over_the_quota() {
    let app = App::new("quota", APP);
    let mut r = boot(app.plan(), "ios");
    prefer(&mut r, "geolocation-background", "granted");
    for i in 1..=21 {
        r.act("named", vec![Value::str(&format!("f{i}"))]).unwrap();
        pump(&mut r);
    }
    assert_eq!(fences(&r).as_array().unwrap().len(), 20, "{}", logs(&r));
    assert!(
        logs(&r).contains("addGeofence: refused: QuotaExceededError"),
        "{}",
        logs(&r)
    );
    // Twenty listed: adding one of their names again replaces it.
    r.act("named", vec![Value::str("f3")]).unwrap();
    pump(&mut r);
    assert_eq!(fences(&r).as_array().unwrap().len(), 20, "{}", logs(&r));
    assert_eq!(
        logs(&r).matches("refused: QuotaExceededError").count(),
        1,
        "{}",
        logs(&r)
    );
}

/// D14 step 3: a wake past its budget keeps the transition on the entry,
/// which stays listed; a later delivery clears it.
#[test]
fn an_expired_wake_keeps_its_transition_and_the_fence() {
    let app = App::with_data("expire", &pulling(), PULL);
    let mut r = boot(app.plan(), "ios");
    prefer(&mut r, "geolocation-background", "granted");
    r.act("add", Vec::new()).unwrap();
    pump(&mut r);
    let reference = fences(&r)[0]["ref"].as_u64().unwrap();
    tap(&mut r, reference, None);
    by(&mut r, 0.0);
    assert_eq!(r.take_requests().len(), 1, "{}", logs(&r));
    by(&mut r, 30_001.0);
    let journal = logs(&r);
    assert!(journal.contains("wake: expired"), "{journal}");
    assert!(
        journal.contains(&format!(
            "geofences @{reference}: geofenceenter kept, undelivered"
        )),
        "{journal}"
    );
    let kept = fences(&r);
    assert_eq!(kept.as_array().unwrap().len(), 1, "{journal}");
    assert_eq!(kept[0]["pending"], "geofenceenter", "{kept}");
    assert_eq!(kept[0]["inside"], true, "{kept}");
}

/// §12's cold wake and the one kept for a launch: `--wake` with the entry
/// as `state.geofences` printed it and its `event`.
#[test]
fn a_launch_with_an_entry_delivers_its_event() {
    let app = App::new("cold", APP);
    let mut first = boot(app.plan(), "ios");
    prefer(&mut first, "geolocation-background", "granted");
    first.act("add", Vec::new()).unwrap();
    pump(&mut first);
    let mut printed = fences(&first)[0].clone();
    drop(first);

    printed["event"] = json!("geofenceleave");
    printed["timestamp"] = json!(1_700_000_000_000u64);
    let mut r = boot(app.plan(), "ios");
    let request = json!({ "op": "launchEntry", "launch": "wake", "entry": printed.to_string() });
    let reply = agent::answer(&mut r, &request.to_string()).unwrap().0;
    assert!(reply.contains("geofenceleave"), "{reply}");
    by(&mut r, 0.0);
    pump(&mut r);
    assert_eq!(state(&r)["wake"]["cold"], true, "{}", logs(&r));
    assert!(slot(&r, "heard").contains("leave home"), "{}", logs(&r));
    assert_eq!(fences(&r)[0]["inside"], false, "{}", logs(&r));

    printed["event"] = json!("geofenceenter");
    printed["deliver"] = json!("launch");
    let mut r = boot(app.plan(), "ios");
    let request = json!({ "op": "launchEntry", "launch": "wake", "entry": printed.to_string() });
    agent::answer(&mut r, &request.to_string()).unwrap();
    by(&mut r, 0.0);
    pump(&mut r);
    assert_eq!(state(&r)["wake"]["cold"], false, "{}", logs(&r));
    assert!(
        slot(&r, "heard").contains("enter home 1700000000000"),
        "{} {}",
        slot(&r, "heard"),
        logs(&r)
    );
}

/// D20: no cell on the web; the substitute answers as production does.
#[test]
fn the_web_has_no_geofencing() {
    let app = App::new("web", APP);
    let mut r = boot(app.plan(), "web");
    pump(&mut r);
    let status = slot(&r, "status");
    assert!(
        status.contains("unknown") && status.contains("false"),
        "{status}"
    );
    r.act("add", Vec::new()).unwrap();
    pump(&mut r);
    assert!(
        logs(&r).contains("addGeofence: NotSupportedError"),
        "{}",
        logs(&r)
    );
}
