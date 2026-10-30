//! LLP 1133.030 stage 0, through a module outside the tree (`tests/chime`,
//! copied out of the checkout as an installed package is): a permission
//! that requires another on one target, asked one ticket each, required
//! first; an app's list whose entries come due on the agent clock, which
//! every session reads; the person's choices on a delivered entry; pause
//! and resume in place; and the declared checks every route holds a call to.

use exact_runner::{agent, CallOrigin, Runner, Value};
use serde_json::{json, Value as Json};
use std::path::{Path, PathBuf};

struct App(PathBuf);
impl App {
    fn new(name: &str) -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("exact-chime-{name}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/chime");
        copy(
            &fixture,
            &path.join("node_modules/@fixture/chime"),
            &["app"],
        );
        copy(&fixture.join("app"), &path, &[]);
        Self(path.canonicalize().unwrap())
    }
    fn plan(&self) -> exact_plan::Plan {
        contract::compile_path_all(&self.0.join("app.contract"), false)
            .map(|(plan, _)| plan)
            .unwrap_or_else(|e| panic!("{e:?}"))
    }
}
impl Drop for App {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn copy(from: &Path, to: &Path, skip: &[&str]) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name();
        if skip.iter().any(|s| name == *s) {
            continue;
        }
        if entry.file_type().unwrap().is_dir() {
            copy(&entry.path(), &to.join(&name), &[]);
        } else {
            std::fs::copy(entry.path(), to.join(&name)).unwrap();
        }
    }
}

/// An app that writes the module's grant line.
struct Granted;
impl exact_runner::DataSource for Granted {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, exact_runner::DataError> {
        Err(exact_runner::DataError::UnknownSource(source.into()))
    }
    fn grants(&self) -> &str {
        "device.alarms timer"
    }
}

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

fn chimes(r: &Runner<Granted>) -> Json {
    state(r)["chimes"].clone()
}

/// A resource's value, or a mutation's.
fn slot(r: &Runner<Granted>, name: &str) -> String {
    format!("{:?}", r.resource(name).or_else(|| r.slot(name)))
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

thread_local! {
    /// Each held call's continuation and the ticket it went out under.
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

/// D12 item 2: an app's list is one list for every session; an entry
/// comes due when a session's agent clock passes it, once, and the other
/// session reads it ringing and hears the change at its next clock step;
/// the person's snooze makes it due again, their stop takes it away.
#[test]
fn an_app_s_list_comes_due_once_for_every_session() {
    let app = App::new("due");
    let shared = exact_runner::AppState::default();
    let mut a = boot(app.plan(), "ios");
    let mut b = boot(app.plan(), "ios");
    a.share_app_state(shared.clone());
    b.share_app_state(shared);
    prefer(&mut a, "chime", "granted");
    a.act("start", Vec::new()).unwrap();
    pump(&mut a);
    let listed = chimes(&b);
    assert_eq!(listed[0]["state"], "countdown", "{listed} {}", logs(&a));
    assert_eq!(listed[0]["delivered"], false);
    assert_eq!(listed[0]["at"], json!(60_000));
    // b reads the shared list at once and hears it changed at its next step.
    b.advance(0.0).unwrap();
    pump(&mut b);
    assert!(
        slot(&b, "list").contains("countdown"),
        "{}",
        slot(&b, "list")
    );
    let reference = listed[0]["ref"].as_u64().unwrap();
    let refused = tap(&mut b, reference, Some("stop"));
    assert!(refused.contains("not delivered yet"), "{refused}");
    a.advance(60_000.0).unwrap();
    assert_eq!(chimes(&b)[0]["state"], "alerting");
    assert_eq!(chimes(&b)[0]["delivered"], true);
    assert_eq!(logs(&a).matches("chimes @").count(), 1, "{}", logs(&a));
    // b's own clock step hears a's change; it does not fire it again.
    b.advance(60_000.0).unwrap();
    pump(&mut b);
    assert!(
        slot(&b, "list").contains("alerting"),
        "{}",
        slot(&b, "list")
    );
    assert!(!logs(&b).contains(": due"), "{}", logs(&b));
    let bare = tap(&mut b, reference, None);
    assert!(
        bare.contains("takes a choice: stop or snooze")
            || bare.contains("takes a choice: snooze or stop"),
        "{bare}"
    );
    let snoozed = tap(&mut b, reference, Some("snooze"));
    assert!(snoozed.contains("snooze"), "{snoozed}");
    let again = chimes(&a)[0].clone();
    assert_eq!(
        (again["state"].clone(), again["delivered"].clone()),
        (json!("countdown"), json!(false))
    );
    assert_eq!(again["at"], json!(90_000));
    b.advance(90_000.0).unwrap();
    let stopped = tap(&mut a, reference, Some("stop"));
    assert!(stopped.contains("stop"), "{stopped}");
    assert_eq!(chimes(&b), json!([]));
}

/// D2's checks, which hold on every route: one of `at` and `duration`, an
/// instant after now, at least a second, and a title.
#[test]
fn the_declared_checks_refuse_a_malformed_alarm() {
    let app = App::new("checks");
    let mut r = boot(app.plan(), "ios");
    prefer(&mut r, "chime", "granted");
    for action in ["neither", "both", "past", "short", "untitled"] {
        r.act(action, Vec::new()).unwrap();
        pump(&mut r);
    }
    assert_eq!(
        logs(&r).matches("setChime: refused: TypeError").count(),
        5,
        "{}",
        logs(&r)
    );
    assert_eq!(chimes(&r), json!([]));
    r.act("wake", Vec::new()).unwrap();
    pump(&mut r);
    assert_eq!(chimes(&r)[0]["state"], "scheduled", "{}", logs(&r));
}

/// D12 item 2's `update`: pause keeps the time left and clears the due
/// instant; resume makes it due that long from now; the wrong state, or a
/// tag that names nothing, is `InvalidStateError`.
#[test]
fn pause_and_resume_change_the_entry_in_place() {
    let app = App::new("pause");
    let mut r = boot(app.plan(), "ios");
    prefer(&mut r, "chime", "granted");
    r.act("go", Vec::new()).unwrap();
    pump(&mut r);
    assert!(
        logs(&r).contains("goChime: refused: InvalidStateError"),
        "{}",
        logs(&r)
    );
    r.act("start", Vec::new()).unwrap();
    r.advance(20_000.0).unwrap();
    r.act("hold", Vec::new()).unwrap();
    pump(&mut r);
    let held = chimes(&r)[0].clone();
    assert_eq!(
        (
            held["state"].clone(),
            held["remaining"].clone(),
            held["at"].clone()
        ),
        (json!("paused"), json!(40_000), Json::Null),
        "{held}"
    );
    r.act("hold", Vec::new()).unwrap();
    pump(&mut r);
    assert!(
        logs(&r).contains("holdChime: refused: InvalidStateError"),
        "{}",
        logs(&r)
    );
    // Paused, it never comes due.
    r.advance(200_000.0).unwrap();
    assert_eq!(chimes(&r)[0]["state"], "paused");
    r.act("go", Vec::new()).unwrap();
    pump(&mut r);
    let resumed = chimes(&r)[0].clone();
    assert_eq!(resumed["state"], "countdown", "{resumed}");
    assert_eq!(resumed["at"], json!(240_000));
    assert_eq!(resumed["remaining"], Json::Null);
    r.act("stop", Vec::new()).unwrap();
    pump(&mut r);
    assert_eq!(chimes(&r), json!([]));
}

/// D7: only the person sets an alarm; a tool's schedule is refused
/// `person-required` while granted, and a stop is anyone's.
#[test]
fn a_tool_never_sets_an_alarm() {
    let app = App::new("tool");
    let mut r = boot(app.plan(), "ios");
    prefer(&mut r, "chime", "granted");
    r.act_from(CallOrigin::Tool, "start", Vec::new()).unwrap();
    pump(&mut r);
    assert!(
        logs(&r).contains("setChime: refused, person-required (tool origin)"),
        "{}",
        logs(&r)
    );
    assert_eq!(chimes(&r), json!([]));
    r.act("start", Vec::new()).unwrap();
    pump(&mut r);
    r.act_from(CallOrigin::Tool, "stop", Vec::new()).unwrap();
    pump(&mut r);
    assert_eq!(chimes(&r), json!([]), "{}", logs(&r));
}

/// D12 item 1 on an Android target: the state is the lesser of the
/// permission's own and the one it requires, so a denial of the required
/// one refuses a schedule while its own is granted; with both `prompt`, a
/// schedule holds the required one's ticket and then its own, and the
/// status shows each.
#[test]
fn a_permission_that_requires_another_asks_both_required_first() {
    let app = App::new("requires");
    let mut r = boot(app.plan(), "android");
    prefer(&mut r, "chime", "granted");
    prefer(&mut r, "ping", "denied");
    pump(&mut r);
    assert!(
        slot(&r, "status").contains("denied"),
        "{}",
        slot(&r, "status")
    );
    r.act("start", Vec::new()).unwrap();
    pump(&mut r);
    assert!(
        logs(&r).contains("setChime: refused: NotAllowedError"),
        "{}",
        logs(&r)
    );
    assert!(tickets(&r).is_empty());

    let mut r = boot(app.plan(), "android");
    r.act("start", Vec::new()).unwrap();
    pump(&mut r);
    let first = tickets(&r);
    assert_eq!(
        first.iter().map(|t| t.1.as_str()).collect::<Vec<_>>(),
        ["ping"],
        "{}",
        logs(&r)
    );
    tap(&mut r, first[0].0, Some("allow"));
    pump(&mut r);
    let second = tickets(&r);
    assert_eq!(
        second.iter().map(|t| t.1.as_str()).collect::<Vec<_>>(),
        ["chime"],
        "{}",
        logs(&r)
    );
    assert_eq!(chimes(&r), json!([]));
    tap(&mut r, second[0].0, Some("allow"));
    pump(&mut r);
    assert_eq!(chimes(&r)[0]["state"], "countdown", "{}", logs(&r));
    let status = slot(&r, "status");
    assert!(
        status.contains("granted") && !status.contains("prompt"),
        "{status}"
    );

    // A request while both are `prompt` asks the same two, in order.
    let mut r = boot(app.plan(), "android");
    r.act("ask", Vec::new()).unwrap();
    pump(&mut r);
    let first = tickets(&r);
    assert_eq!(first[0].1, "ping", "{}", logs(&r));
    tap(&mut r, first[0].0, Some("allow"));
    pump(&mut r);
    let second = tickets(&r);
    assert_eq!(second[0].1, "chime", "{}", logs(&r));
    tap(&mut r, second[0].0, Some("deny"));
    pump(&mut r);
    assert!(
        slot(&r, "asked").contains("denied"),
        "{} {}",
        slot(&r, "asked"),
        logs(&r)
    );
}

/// D14: on a target the module marks absent, the substitute answers as
/// production does, `available` false and `NotSupportedError`.
#[test]
fn an_absent_target_answers_absence() {
    let app = App::new("absent");
    let mut r = boot(app.plan(), "web");
    prefer(&mut r, "chime", "granted");
    pump(&mut r);
    let status = slot(&r, "status");
    assert!(
        status.contains("unknown") && status.contains("false"),
        "{status}"
    );
    r.act("start", Vec::new()).unwrap();
    pump(&mut r);
    assert!(
        logs(&r).contains("setChime: refused: NotSupportedError"),
        "{}",
        logs(&r)
    );
}
