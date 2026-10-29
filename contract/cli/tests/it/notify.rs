//! `@exact/notifications` (LLP 1133.004 stage 1), the real package linked
//! into the app's `node_modules` as a `file:` dependency installs it: its
//! sends are statements, its click an event on the root, and under the agent
//! its declared substitute lists posts, keeps the app's badge and holds the
//! permission ticket; without the `device.notifications` grant a post is
//! refused before any module sees it.

use exact_runner::{agent, Runner, Value};
use std::path::PathBuf;

struct App(PathBuf);
impl App {
    fn new(name: &str) -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("exact-notify-{name}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(path.join("node_modules/@exact")).unwrap();
        let package =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../packages/notifications");
        std::os::unix::fs::symlink(
            package.canonicalize().unwrap(),
            path.join("node_modules/@exact/notifications"),
        )
        .unwrap();
        Self(path.canonicalize().unwrap())
    }
    fn compile(&self, body: &str) -> Result<exact_plan::Plan, Vec<contract::CompileError>> {
        let path = self.0.join("app.contract");
        std::fs::write(&path, format!("{HEAD}{body}")).unwrap();
        contract::compile_path_all(&path, false).map(|(plan, _)| plan)
    }
}
impl Drop for App {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const HEAD: &str = "use showNotification, closeNotification, setAppBadge, clearAppBadge, notificationStatus, requestNotificationPermission, notificationclick, NotificationStatus, NotificationEvent from \"@exact/notifications\"\n";

const APP: &str = r#"component App
  state opened = ""
  resource status = notificationStatus() as shape NotificationStatus
  mutation asked as shape NotificationStatus
  action clicked(tag: string, e: NotificationEvent)
    match e.data
      case some(d)
        opened = `${tag} ${d}`
      case none
        opened = tag
  action post
    showNotification(title="Build finished", body="t3", tag="thread-3", silent=true, data="/thread/3")
  action again
    showNotification(title="Build finished again", tag="thread-3", data="/thread/3")
  action later
    showNotification(title="Stretch", tag="stretch", showTrigger=1800000000000)
  action route
    showNotification(title="Inbox", tag="inbox", navigate="/inbox")
  action close
    closeNotification(tag="thread-3")
  action count
    setAppBadge(contents=3)
  action flag
    setAppBadge()
  action zero
    setAppBadge(contents=0)
  action badgeOf(n: number)
    setAppBadge(contents=n)
  action clear
    clearAppBadge()
  action ask
    send asked = requestNotificationPermission()
  view
    column notificationclick=clicked
      text status.state
      text opened
"#;

struct Grants(&'static str);
impl exact_runner::DataSource for Grants {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, exact_runner::DataError> {
        Err(exact_runner::DataError::UnknownSource(source.into()))
    }
    fn grants(&self) -> &str {
        self.0
    }
}

const GRANTED: &str = "device.notifications purpose.notifications\n";

fn boot(grants: &'static str) -> (App, Runner<Grants>) {
    let app = App::new("runner");
    let plan = app.compile(APP).unwrap_or_else(|e| panic!("{e:?}"));
    let r = Runner::boot(
        plan,
        Grants(grants),
        exact_kernel::Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    (app, r)
}

fn state(r: &Runner<Grants>) -> serde_json::Value {
    serde_json::from_str(&agent::handle(r, r#"{"op":"state"}"#)).unwrap()
}

fn logs(r: &Runner<Grants>) -> String {
    agent::handle(r, r#"{"op":"logs"}"#)
}

fn tap(r: &mut Runner<Grants>, ticket: u64, choice: Option<&str>) -> String {
    let choice = choice
        .map(|c| format!(r#","choice":"{c}""#))
        .unwrap_or_default();
    agent::answer(r, &format!(r#"{{"op":"tap","ticket":{ticket}{choice}}}"#))
        .unwrap()
        .0
}

fn permission(r: &Runner<Grants>) -> u64 {
    (r.device_holds().iter())
        .find(|h| h.capability == "permission")
        .unwrap_or_else(|| panic!("no permission ticket: {}", logs(r)))
        .ticket
}

#[test]
fn the_package_compiles_its_sends_as_statements_and_its_click_as_a_root_event() {
    let app = App::new("compile");
    let plan = app.compile(APP).unwrap_or_else(|e| panic!("{e:?}"));
    let events: Vec<String> = (plan.module_events.iter())
        .map(|e| {
            format!(
                "{}.{} {}",
                plan.str(e.extension),
                plan.str(e.declared),
                e.hold
            )
        })
        .collect();
    assert_eq!(events, ["notifications.notificationclick 16"]);
    let refused = |body: &str| -> Vec<String> {
        let app = App::new("refused");
        match app.compile(body) {
            Ok(_) => panic!("compiled:\n{body}"),
            Err(all) => all
                .into_iter()
                .map(|e| format!("{} {}", e.id, e.message))
                .collect(),
        }
    };
    let go = |call: &str| {
        format!("component App\n  action go\n    {call}\n  view\n    button press=go\n      text \"go\"\n")
    };
    for (call, want) in [
        (
            "closeNotification(\"stretch\")",
            "contract-module-positional",
        ),
        ("showNotification(body=\"Five minutes\")", "needs `title=…`"),
        (
            "showNotification(title=\"Stretch\", showTrigger=\"7:30\")",
            "type-",
        ),
        (
            "showNotification(title=\"Stretch\", apple.interruptionLevel=\"loud\")",
            "contract-module-argument",
        ),
    ] {
        let all = refused(&go(call));
        assert!(all.iter().any(|e| e.contains(want)), "{call}: {all:?}");
    }
}

/// §7 under the agent: the post waits on the permission ticket, then is
/// listed with what it wrote; a tag replaces; `closeNotification` takes it
/// away; the badge is a number, a flag or none; `tap @ref` raises
/// `notificationclick` with the tag and `data`, or opens `navigate`; a post
/// for later is not delivered until the clock passes it.
#[test]
fn the_substitute_lists_posts_holds_the_badge_and_taps_a_click() {
    let (_app, mut r) = boot(GRANTED);
    assert_eq!(state(&r)["notifications"], serde_json::json!([]));
    r.act("post", Vec::new()).unwrap();
    let ticket = permission(&r);
    assert!(tap(&mut r, ticket, Some("allow")).contains("allow"));
    let listed = state(&r)["notifications"].clone();
    assert_eq!(listed[0]["title"], "Build finished", "{listed}");
    assert_eq!(
        (listed[0]["silent"].clone(), listed[0]["data"].clone()),
        (serde_json::json!(true), serde_json::json!("/thread/3"))
    );
    assert_eq!(listed[0]["delivered"], true);
    r.act("again", Vec::new()).unwrap();
    let again = state(&r)["notifications"].clone();
    assert_eq!(
        again.as_array().map(Vec::len),
        Some(1),
        "a tag replaces: {again}"
    );
    assert_eq!(again[0]["title"], "Build finished again");
    r.act("close", Vec::new()).unwrap();
    assert_eq!(state(&r)["notifications"], serde_json::json!([]));
    for (action, badge) in [
        ("count", serde_json::json!(3)),
        ("flag", serde_json::json!("flag")),
        ("zero", serde_json::Value::Null),
        ("count", serde_json::json!(3)),
        ("clear", serde_json::Value::Null),
    ] {
        r.act(action, Vec::new()).unwrap();
        assert_eq!(state(&r)["badge"], badge, "{action}");
    }
    r.act("post", Vec::new()).unwrap();
    let reference = state(&r)["notifications"][0]["ref"].as_u64().unwrap();
    assert!(tap(&mut r, reference, None).contains("notificationclick"));
    r.apply_announced();
    assert!(
        format!("{:?}", r.slot("opened")).contains("thread-3 /thread/3"),
        "{}",
        logs(&r)
    );
    r.act("route", Vec::new()).unwrap();
    let inbox = state(&r)["notifications"][0]["ref"].as_u64().unwrap();
    assert!(tap(&mut r, inbox, None).contains(r#""navigate":"/inbox""#));
    r.act("later", Vec::new()).unwrap();
    let later = state(&r)["notifications"][0].clone();
    assert_eq!(later["delivered"], false, "{later}");
    assert!(tap(&mut r, later["ref"].as_u64().unwrap(), None).contains("not delivered yet"));
    r.act("ask", Vec::new()).unwrap();
    assert!(
        format!("{:?}", r.slot("asked")).contains("granted"),
        "{}",
        logs(&r)
    );
}

/// The grant rule moved into the package's admission: without
/// `device.notifications` a post is refused before any module sees it, and
/// the badge, which needs no grant, is set.
#[test]
fn without_the_grant_a_post_is_refused_and_the_badge_is_not() {
    let (_app, mut r) = boot("");
    r.act("post", Vec::new()).unwrap();
    assert!(r.device_holds().is_empty());
    assert!(
        logs(&r).contains(
            "notifications.showNotification: refused, grant-missing (device.notifications)"
        ),
        "{}",
        logs(&r)
    );
    r.act("count", Vec::new()).unwrap();
    assert_eq!(state(&r)["badge"], 3);
}

/// D1's `[EnforceRange] unsigned long long`: a negative count refuses
/// `TypeError` (the declaration's `checks`) and leaves the badge.
#[test]
fn a_negative_badge_count_is_refused() {
    let (_app, mut r) = boot(GRANTED);
    r.act("count", Vec::new()).unwrap();
    r.act("badgeOf", vec![Value::Number(-1.0)]).unwrap();
    assert_eq!(state(&r)["badge"], 3, "a refusal leaves the badge");
    assert!(
        logs(&r).contains("setAppBadge: refused: TypeError"),
        "{}",
        logs(&r)
    );
}

/// §8: on a target the package does not implement (Linux, the terminal,
/// Android, Windows, tvOS) its sends are refused, journaled, and the status
/// reads unavailable, under the agent as outside it.
#[test]
fn a_target_with_no_implementation_refuses_and_reads_unavailable() {
    let (_app, mut r) = boot(GRANTED);
    r.set_module_target("linux");
    // The status is asked again for the new target.
    let at = r.now_ms();
    r.advance(at).unwrap();
    r.apply_announced();
    r.act("post", Vec::new()).unwrap();
    r.act("count", Vec::new()).unwrap();
    assert!(r.device_holds().is_empty());
    let s = state(&r);
    assert_eq!(
        (s["notifications"].clone(), s["badge"].clone()),
        (serde_json::json!([]), serde_json::Value::Null)
    );
    assert_eq!(s["resources"]["status"]["available"], false, "{s}");
    let journal = logs(&r);
    for line in [
        "showNotification: refused: NotSupportedError",
        "setAppBadge: refused: NotSupportedError",
    ] {
        assert!(journal.contains(line), "{line} in {journal}");
    }
}
