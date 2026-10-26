//! LLP 1133.002 D13: a native module's names, imported from its package's
//! `exact.json`, called with keyword arguments and a platform's dotted
//! words, compile to an ordinary source of one record. The package is the
//! real `@exact/geolocation`, linked into the app's `node_modules` as a
//! `file:` dependency installs it.

#[cfg(unix)]
use std::os::unix::fs::symlink as symlink_dir;
#[cfg(windows)]
use std::os::windows::fs::symlink_dir;
use std::path::PathBuf;

struct App(PathBuf);
impl App {
    fn new(name: &str) -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("exact-modules-{name}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(path.join("node_modules/@exact")).unwrap();
        let package = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../packages/geolocation");
        symlink_dir(
            package.canonicalize().unwrap(),
            path.join("node_modules/@exact/geolocation"),
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

const HEAD: &str = "use getCurrentPosition, geolocationStatus, GeolocationPosition, GeolocationStatus from \"@exact/geolocation\"\n";

const APP: &str = r#"component App
  resource status = geolocationStatus() as shape GeolocationStatus
  mutation located as shape GeolocationPosition
  action locate
    send located = getCurrentPosition(enableHighAccuracy=false, maximumAge=600000, apple.desiredAccuracy="threeKilometers", android.quality="lowPower")
  view
    column
      text status.state
      match located
        case some(p)
          text `${p.coords.latitude}`
        case none
          text "none"
      button press=locate testId="locate"
        text "Use my location"
"#;

#[test]
fn a_module_send_and_source_compile_to_a_source_of_one_record() {
    let app = App::new("ok");
    let plan = app.compile(APP).unwrap_or_else(|e| panic!("{e:?}"));
    let source = |name: &str| {
        plan.sources
            .iter()
            .find(|s| plan.str(s.name) == name)
            .unwrap_or_else(|| panic!("no source {name}"))
    };
    assert_eq!(source("getCurrentPosition").params.len, 1);
    assert_eq!(source("geolocationStatus").params.len, 0);
    let rows: Vec<String> = plan
        .modules
        .iter()
        .map(|m| {
            format!(
                "{} {}.{} send={} live={} request={} sync={} substitute={:?}",
                plan.str(plan.sources[m.source.0 as usize].name),
                plan.str(m.extension),
                plan.str(m.declared),
                m.send,
                m.live,
                plan.str(m.request),
                plan.str(m.sync),
                serde_json::from_str::<serde_json::Value>(plan.str(m.substitute))
                    .unwrap()
                    .as_object()
                    .unwrap()
                    .keys()
                    .collect::<Vec<_>>(),
            )
        })
        .collect();
    assert_eq!(
        rows,
        [
            "geolocationStatus geolocation.geolocationStatus send=false live=true request=never sync=geolocationStatusSync substitute=[\"default\", \"grants\", \"permission\"]",
            "getCurrentPosition geolocation.getCurrentPosition send=true live=false request=always sync=getCurrentPositionSync substitute=[\"cancel\", \"from\", \"grants\", \"permission\", \"ticket\", \"timeout\"]",
            "requestPreciseLocation geolocation.requestPreciseLocation send=true live=false request=always sync= substitute=[\"answers\", \"grants\", \"permission\", \"raise\", \"unsupported\"]",
            "watchPosition geolocation.watchPosition send=false live=true request=when-prompt sync= substitute=[\"default\", \"fill\", \"grants\", \"permission\", \"short\"]",
        ]
    );
}

fn refused(body: &str) -> Vec<(String, String)> {
    let app = App::new("refused");
    match app.compile(body) {
        Ok(_) => panic!("compiled:\n{body}"),
        Err(all) => all.into_iter().map(|e| (e.id, e.message)).collect(),
    }
}

fn locate(call: &str) -> String {
    format!(
        "component App\n  mutation located as shape GeolocationPosition\n  action locate\n    send located = {call}\n  view\n    button press=locate\n      text \"go\"\n"
    )
}

#[test]
fn a_module_call_refuses_what_its_declarations_do_not_say() {
    let one = |call: &str| {
        let all = refused(&locate(call));
        assert_eq!(all.len(), 1, "{call}: {all:?}");
        all.into_iter().next().unwrap()
    };
    let (id, message) = one("getCurrentPosition(true)");
    assert_eq!(id, "contract-module-positional");
    assert!(message.contains("`enableHighAccuracy`"), "{message}");
    let (id, message) = one("getCurrentPosition(highAccuracy=true)");
    assert_eq!(id, "contract-module-argument");
    assert!(message.contains("no option `highAccuracy`"), "{message}");
    let (id, message) = one("getCurrentPosition(apple.distanceFilter=5)");
    assert_eq!(id, "contract-module-argument");
    assert!(
        message.contains("`@exact/geolocation/apple` declares no word `distanceFilter`"),
        "{message}"
    );
    let (id, message) = one("getCurrentPosition(windows.desiredAccuracyInMeters=5)");
    assert_eq!(id, "contract-module-argument");
    assert!(message.contains("`android`, `apple`"), "{message}");
    let (id, _) = one("getCurrentPosition(timeout=1, timeout=2)");
    assert_eq!(id, "call-argument-twice");
    let (id, message) = one("geolocationStatus()");
    assert_eq!(id, "contract-module-kind");
    assert!(message.contains("a resource reads it"), "{message}");
}

#[test]
fn a_module_name_answers_its_declared_shape() {
    let all = refused(
        "shape Spot\n  latitude: number\ncomponent App\n  resource here = geolocationStatus() as shape Spot\n  view\n    text \"x\"\n",
    );
    assert_eq!(all[0].0, "contract-module-answers", "{all:?}");
    assert!(
        all[0].1.contains("write `as shape GeolocationStatus`"),
        "{all:?}"
    );
}

#[test]
fn a_platform_word_takes_its_declared_vocabulary() {
    let all = refused(&locate(
        "getCurrentPosition(apple.desiredAccuracy=\"veryClose\")",
    ));
    assert!(all.iter().any(|(_, m)| m.contains("veryClose")), "{all:?}");
}

/// No app data: every source here is the module's.
struct NoData;
impl exact_runner::DataSource for NoData {
    fn query(
        &mut self,
        source: &str,
        _: &[exact_runner::Value],
    ) -> Result<exact_runner::Value, exact_runner::DataError> {
        Err(exact_runner::DataError::UnknownSource(source.into()))
    }
    fn grants(&self) -> &str {
        "device.geolocation-approximate locationPurpose\n"
    }
}

fn logs(r: &exact_runner::Runner<NoData>) -> String {
    exact_runner::agent::handle(r, r#"{"op":"logs"}"#)
}

/// LLP 1124 D7, D9: with no implementation bound, a module's source answers
/// its declared default, or the agent's preferred value; `clear` returns it
/// to the default; a value outside the source's shape is refused, and so is
/// the permission's state, which `prefer permission` sets (§6). Every name
/// of a used interface is in the plan, read or not.
#[test]
fn an_unbound_module_answers_its_default_or_the_preferred_value() {
    let app = App::new("runner");
    let plan = app.compile(APP).unwrap_or_else(|e| panic!("{e:?}"));
    let mut r = exact_runner::Runner::boot(
        plan,
        NoData,
        exact_kernel::Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let status = |r: &exact_runner::Runner<NoData>| format!("{:?}", r.resource("status"));
    assert!(
        status(&r).contains("\"prompt\""),
        "{} {}",
        status(&r),
        logs(&r)
    );
    let prefer = r#"{"op":"prefer","extension":{"geolocation.geolocationStatus":{"available":true,"accuracy":"precise"}}}"#;
    let (_, reply) = r.extension_agent(prefer).expect("a prefer extension");
    assert!(reply.unwrap().contains("precise"));
    assert!(status(&r).contains("\"precise\""), "{}", status(&r));
    let state = r#"{"op":"prefer","extension":{"geolocation.geolocationStatus":{"available":true,"state":"granted","accuracy":"precise"}}}"#;
    let (_, reply) = r.extension_agent(state).unwrap();
    assert!(reply
        .unwrap_err()
        .contains("set by `prefer permission.geolocation`"));
    assert!(
        logs(&r).contains("prefer extension geolocation.geolocationStatus"),
        "{}",
        logs(&r)
    );
    let wrong = r#"{"op":"prefer","extension":{"geolocation.geolocationStatus":{"available":true,"accuracy":"maybe"}}}"#;
    let (_, reply) = r.extension_agent(wrong).unwrap();
    assert!(reply.unwrap_err().contains("maybe"));
    let unknown = r#"{"op":"prefer","extension":{"geolocation.where":{}}}"#;
    let (_, reply) = r.extension_agent(unknown).unwrap();
    assert!(reply
        .unwrap_err()
        .contains("its module sources are geolocation.geolocationStatus"),);
    let clear = r#"{"op":"prefer","extension":{"geolocation.geolocationStatus":"clear"}}"#;
    r.extension_agent(clear).unwrap().1.unwrap();
    assert!(status(&r).contains("\"unknown\""), "{}", status(&r));
}

/// The host's part, as a native host plays it: each request dispatched (a
/// held one parked), released work run, and each outcome fulfilled.
fn pump(r: &mut exact_runner::Runner<NoData>, parked: &mut Vec<(u64, u64)>) {
    for out in r.take_requests() {
        let token = out
            .request
            .continuation
            .expect("a module request is held work");
        parked.push((token, out.ticket));
        let dispatch = r.dispatch_work(token);
        run(r, out.ticket, dispatch);
    }
    for (token, dispatch) in r.release_work() {
        let ticket = parked.iter().find(|(t, _)| *t == token).expect("parked").1;
        run(r, ticket, dispatch);
    }
    r.apply_announced();
}

fn run(r: &mut exact_runner::Runner<NoData>, ticket: u64, dispatch: exact_runner::Dispatch) {
    if let exact_runner::Dispatch::Run(exact_runner::Work::Now(work)) = dispatch {
        r.fulfill(ticket, work()).unwrap();
    }
}

fn answer(
    r: &mut exact_runner::Runner<NoData>,
    op: &str,
    capability: &str,
    answer: &str,
) -> String {
    let hold = (r.device_holds().iter())
        .find(|h| h.capability == capability)
        .unwrap_or_else(|| panic!("no {capability} ticket: {:?}", r.device_holds()))
        .ticket;
    let field = if op == "tap" { "choice" } else { "text" };
    let request = format!(r#"{{"op":"{op}","ticket":{hold},"{field}":{answer:?}}}"#);
    exact_runner::agent::answer(r, &request).unwrap().0
}

/// LLP 1133.002 §6 through the agent's tickets: a send while `prompt` waits
/// on a permission ticket; `allow` grants it (approximate, under the app's
/// approximate grant) and, with no position preferred, the send waits on a
/// position ticket, whose typed answer answers it and is filled; `deny`
/// and a later send fail `PERMISSION_DENIED`.
#[test]
fn a_module_send_waits_on_the_agent_s_permission_and_position_tickets() {
    let app = App::new("tickets");
    let plan = app.compile(APP).unwrap_or_else(|e| panic!("{e:?}"));
    let mut r = exact_runner::Runner::boot(
        plan,
        NoData,
        exact_kernel::Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let show = |r: &exact_runner::Runner<NoData>, name: &str| {
        format!("{:?}", r.resource(name).or(r.slot(name)))
    };
    let mut parked = Vec::new();
    r.act("locate", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    assert!(!r.has_pending(), "a held request is not I/O");
    let reply = answer(&mut r, "type", "permission", "x");
    assert!(reply.contains("answered by tap"), "{reply}");
    answer(&mut r, "tap", "permission", "allow");
    pump(&mut r, &mut parked);
    assert!(
        show(&r, "status").contains("\"granted\""),
        "{}",
        show(&r, "status")
    );
    assert!(
        show(&r, "status").contains("\"approximate\""),
        "{}",
        show(&r, "status")
    );
    let wrong = answer(&mut r, "type", "position", "north");
    assert!(
        wrong.contains("coords.latitude,coords.longitude,coords.accuracy"),
        "{wrong}"
    );
    answer(&mut r, "type", "position", "51.5074,-0.1278");
    pump(&mut r, &mut parked);
    let located = show(&r, "located");
    assert!(
        located.contains("51.5074") && located.contains("Number(100.0)"),
        "{located}\n{}",
        logs(&r)
    );
    let (_, reply) = r
        .extension_agent(r#"{"op":"prefer","permission":{"geolocation":"denied"}}"#)
        .unwrap();
    reply.unwrap();
    assert!(
        show(&r, "status").contains("\"denied\""),
        "{}",
        show(&r, "status")
    );
    r.act("locate", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    assert!(logs(&r).contains("PERMISSION_DENIED"), "{}", logs(&r));
    let state = exact_runner::agent::handle(&r, r#"{"op":"state","name":"failed"}"#);
    let _ = state;
}

/// LLP 1133.002 §6: a request's own `timeout` runs on the agent's clock. A
/// position ticket fails `TIMEOUT` once the clock passes it, and is retired;
/// `timeout=0` with no position preferred fails at once.
#[test]
fn a_position_ticket_times_out_on_the_agent_clock() {
    let app = App::new("timeout");
    let body = "component App\n  mutation slow as shape GeolocationPosition\n  mutation now as shape GeolocationPosition\n  action wait\n    send slow = getCurrentPosition(timeout=100)\n  action instant\n    send now = getCurrentPosition(timeout=0)\n  view\n    text \"x\"\n";
    let plan = app.compile(body).unwrap_or_else(|e| panic!("{e:?}"));
    let mut r = exact_runner::Runner::boot(
        plan,
        NoData,
        exact_kernel::Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let mut parked = Vec::new();
    r.extension_agent(r#"{"op":"prefer","permission":{"geolocation":"granted"}}"#)
        .unwrap()
        .1
        .unwrap();
    r.act("wait", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    assert!(r.device_holds().iter().any(|h| h.capability == "position"));
    r.advance(99.0).unwrap();
    pump(&mut r, &mut parked);
    assert!(
        r.device_holds().iter().any(|h| h.capability == "position"),
        "not yet"
    );
    r.advance(101.0).unwrap();
    pump(&mut r, &mut parked);
    assert!(r.device_holds().is_empty(), "{:?}", r.device_holds());
    assert!(
        logs(&r).contains("geolocation.getCurrentPosition: TIMEOUT"),
        "{}",
        logs(&r)
    );
    r.act("instant", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    assert!(r.device_holds().is_empty());
    assert!(
        logs(&r).contains("send now failed: geolocation.getCurrentPosition: TIMEOUT"),
        "{}",
        logs(&r)
    );
}

/// LLP 1133.002 D10 through the substitute: precise location for a task.
/// While `granted` and `approximate` the send waits on a `precision` ticket;
/// `allow` raises `accuracy` and the send answers the status; declined, it
/// answers the status unchanged, never a failure; once `precise`, at once.
#[test]
fn a_precision_ticket_raises_the_status_or_answers_it_unchanged() {
    let app = App::new("precision");
    let head = "use requestPreciseLocation from \"@exact/geolocation\"\n";
    let body = format!("{head}component App\n  resource status = geolocationStatus() as shape GeolocationStatus\n  mutation raised as shape GeolocationStatus\n  action raise\n    send raised = requestPreciseLocation(purposeKey=\"forecastPurpose\")\n  view\n    text status.accuracy\n");
    let plan = app.compile(&body).unwrap_or_else(|e| panic!("{e:?}"));
    let mut r = exact_runner::Runner::boot(
        plan,
        NoData,
        exact_kernel::Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let mut parked = Vec::new();
    let show = |r: &exact_runner::Runner<NoData>, name: &str| {
        format!("{:?}", r.resource(name).or(r.slot(name)))
    };
    r.act("raise", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    answer(&mut r, "tap", "permission", "allow");
    pump(&mut r, &mut parked);
    assert!(
        show(&r, "status").contains("\"approximate\""),
        "{}",
        show(&r, "status")
    );
    answer(&mut r, "tap", "precision", "deny");
    pump(&mut r, &mut parked);
    assert!(
        show(&r, "raised").contains("\"approximate\""),
        "declined is the status unchanged: {}",
        show(&r, "raised")
    );
    r.act("raise", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    answer(&mut r, "tap", "precision", "allow");
    pump(&mut r, &mut parked);
    assert!(
        show(&r, "raised").contains("\"precise\""),
        "{}",
        show(&r, "raised")
    );
    assert!(
        show(&r, "status").contains("\"precise\""),
        "{}",
        show(&r, "status")
    );
    r.act("raise", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    assert!(
        r.device_holds().is_empty(),
        "precise already: answered at once"
    );
}

/// LLP 1133.002 D6, D13: an app that reads or sends a module's name writes
/// one of its interface's grant lines; with none, the compile says which.
#[test]
fn a_module_without_its_grant_line_is_named() {
    let app = App::new("grant");
    std::fs::write(
        app.0.join("app.json"),
        r#"{"name":"G","app":{"id":"test.grant","name":"G"}}"#,
    )
    .unwrap();
    std::fs::write(
        app.0.join("app.ts"),
        "export const grants = 'net.fetch https://x.test';\n",
    )
    .unwrap();
    let body = "component App\n  resource status = geolocationStatus() as shape GeolocationStatus\n  view\n    text status.state\n";
    std::fs::write(app.0.join("app.contract"), format!("{HEAD}{body}")).unwrap();
    let warned = |app: &App| {
        let (result, warnings) =
            contract::compile_path_all_with_warnings(&app.0.join("app.contract"), false);
        let errors = result.err().unwrap_or_default();
        format!(
            "{:?}",
            warnings
                .iter()
                .chain(&errors)
                .map(|e| (&e.id, &e.message))
                .collect::<Vec<_>>()
        )
    };
    let all = warned(&app);
    assert!(all.contains("grant-missing"), "{all}");
    assert!(
        all.contains("`device.geolocation` or `device.geolocation-approximate`"),
        "{all}"
    );
    std::fs::write(
        app.0.join("app.ts"),
        "export const grants = 'device.geolocation-approximate purpose.geolocation';\n",
    )
    .unwrap();
    assert!(!warned(&app).contains("grant-missing"), "{}", warned(&app));
}

/// `contract fmt` keeps a platform's word against its `=`, as it keeps a
/// named argument's (LLP 1133.002 D13).
#[test]
fn fmt_keeps_a_dotted_keyword_tight() {
    let src = format!(
        "{HEAD}{}",
        locate("getCurrentPosition(maximumAge=600000, apple.desiredAccuracy=\"threeKilometers\")")
    );
    let out = contract_syntax::fmt::format(&src).unwrap();
    assert!(
        out.contains("apple.desiredAccuracy=\"threeKilometers\""),
        "{out}"
    );
}

/// LLP 1133.001 D4: a host that binds the extension sends its names to the
/// native slot, the substitute set aside: no ticket, the module answers,
/// and a call's options travel as written.
#[test]
fn a_bound_module_answers_through_the_native_slot() {
    use std::sync::{Arc, Mutex};
    let app = App::new("bound");
    let plan = app.compile(APP).unwrap_or_else(|e| panic!("{e:?}"));
    let mut r = exact_runner::Runner::boot(
        plan,
        NoData,
        exact_kernel::Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let asked: Arc<Mutex<Vec<String>>> = Arc::default();
    let seen = asked.clone();
    r.native_slot().host(Some(Arc::new(move |body: Vec<u8>, reply: exact_runner::Reply| {
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        seen.lock().unwrap().push(v.to_string());
        let answer = match (v["source"].as_str(), v["send"].as_str()) {
            (Some("geolocationStatus"), _) => serde_json::json!({ "available": true, "state": "granted", "accuracy": "precise" }),
            (_, Some("getCurrentPosition")) => serde_json::json!({ "coords": { "latitude": 35.0, "longitude": 139.0, "accuracy": 10 }, "timestamp": 1 }),
            _ => serde_json::json!({}),
        };
        reply.send(exact_runner::Outcome::Response(exact_runner::Response { status: 200, headers: Vec::new(), body: answer.to_string().into_bytes() }));
    })));
    r.bind_extensions(&["geolocation"], &[]);
    r.apply_announced();
    let pump = |r: &mut exact_runner::Runner<NoData>| {
        for out in r.take_requests() {
            let exact_runner::Dispatch::Run(exact_runner::Work::Later(work)) =
                r.native_work(&out.request)
            else {
                panic!("a bound name goes to the native slot");
            };
            let (tx, rx) = std::sync::mpsc::channel();
            work(exact_runner::Reply::new(move |o| tx.send(o).unwrap()));
            r.fulfill(out.ticket, rx.recv().unwrap()).unwrap();
        }
    };
    pump(&mut r);
    let show = |r: &exact_runner::Runner<NoData>, name: &str| {
        format!("{:?}", r.resource(name).or(r.slot(name)))
    };
    assert!(
        show(&r, "status").contains("\"granted\""),
        "{}",
        show(&r, "status")
    );
    r.act("locate", Vec::new()).unwrap();
    pump(&mut r);
    assert!(
        show(&r, "located").contains("35.0"),
        "{}",
        show(&r, "located")
    );
    assert!(r.device_holds().is_empty(), "no ticket: the module answers");
    let calls = asked.lock().unwrap().clone();
    assert!(
        calls.iter().any(|c| c.contains("\"maximumAge\":600000")
            && c.contains("\"desiredAccuracy\":\"threeKilometers\"")),
        "the call's options travel: {calls:?}"
    );
}

#[test]
fn a_synchronous_form_answers_from_the_substitute_until_its_module_is_bound() {
    use std::sync::Arc;
    let app = App::new("sync");
    let plan = app.compile(APP).unwrap_or_else(|e| panic!("{e:?}"));
    let mut r = exact_runner::Runner::boot(
        plan,
        NoData,
        exact_kernel::Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    r.native_slot().host_call(Some(Arc::new(|_: &[u8]| {
        Ok(br#"{"state":"module"}"#.to_vec())
    })));
    r.advance(0.0).unwrap();
    let call = r
        .native_slot()
        .hosted_call()
        .expect("the host's call, behind the runner's answers");
    let now = |body: &str| String::from_utf8(call(body.as_bytes()).unwrap()).unwrap();
    let status = r#"{"extension":"geolocation","sync":"geolocationStatusSync","args":{}}"#;
    let position = r#"{"extension":"geolocation","sync":"getCurrentPositionSync","args":{"maximumAge":600000}}"#;
    assert!(now(status).contains("\"prompt\""), "{}", now(status));
    // While `prompt` a cached fix is never answered: the request would ask.
    assert_eq!(now(position), r#"{"error":"POSITION_UNAVAILABLE"}"#);
    let grant = r#"{"op":"prefer","permission":{"geolocation":"granted"}}"#;
    r.extension_agent(grant).unwrap().1.unwrap();
    let paris = r#"{"op":"prefer","extension":{"geolocation.watchPosition":{"coords":{"latitude":48.8566,"longitude":2.3522}}}}"#;
    r.extension_agent(paris).unwrap().1.unwrap();
    assert!(now(status).contains("\"granted\""), "{}", now(status));
    assert!(now(position).contains("48.8566"), "{}", now(position));
    assert_eq!(
        now(r#"{"extension":"geolocation","sync":"where","args":{}}"#),
        r#"{"error":"NotSupportedError"}"#
    );
    // Anything else is the host's module's own call.
    assert_eq!(now(r#"{"op":"level"}"#), r#"{"state":"module"}"#);
    r.bind_extensions(&["geolocation"], &[]);
    assert_eq!(now(status), r#"{"state":"module"}"#);
}

#[test]
fn the_rust_binding_is_the_plans_modules_with_their_wire_names() {
    let app = App::new("rust");
    let plan = app.compile(APP).unwrap_or_else(|e| panic!("{e:?}"));
    let rust = contract::module_rust(&plan).unwrap();
    for expected in [
        "pub mod geolocation {",
        // A call's options and a platform's words, in Rust's names.
        "pub struct GetCurrentPositionOptions {",
        "pub struct GetCurrentPositionApple {",
        "pub fn get_current_position(options: &GetCurrentPositionOptions) -> Result<exact_runner::Request, m::ModuleError>",
        "pub fn get_current_position_reply(outcome: &exact_runner::Outcome) -> Result<GeolocationPosition, m::ModuleError>",
        // A name that takes nothing; every name of a used module, read or not.
        "pub fn geolocation_status() -> exact_runner::Request",
        "pub fn watch_position(",
        // The wire keeps the declared names; an enum is checked.
        "m::put(&mut o, \"enableHighAccuracy\", &self.enable_high_accuracy)?;",
        "m::member(x, &[\"bestForNavigation\"",
    ] {
        assert!(rust.contains(expected), "{expected}\n---\n{rust}");
    }
    // Rust has no synchronous form (LLP 1133.002 D14).
    assert!(!rust.contains("Sync"), "{rust}");
    // A plan that calls no module has no binding.
    let none = App::new("rust-none");
    let plan = none
        .compile("component App\n  view\n    text \"x\"\n")
        .unwrap_or_else(|e| panic!("{e:?}"));
    assert_eq!(contract::module_rust(&plan).unwrap().lines().count(), 1);
}
