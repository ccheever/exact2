//! LLP 1133.002 D13: a native module's names, imported from its package's
//! `exact.json`, called with keyword arguments and a platform's dotted
//! words, compile to an ordinary source of one record. The package is the
//! real `@exact/geolocation`, linked into the app's `node_modules` as a
//! `file:` dependency installs it.

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
        std::os::unix::fs::symlink(
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
            "geolocationStatus geolocation.geolocationStatus send=false live=true request=never sync=geolocationStatusSync substitute=[\"default\", \"permission\"]",
            "getCurrentPosition geolocation.getCurrentPosition send=true live=false request=always sync=getCurrentPositionSync substitute=[\"cancel\", \"from\", \"permission\", \"ticket\", \"timeout\"]",
            "requestPreciseLocation geolocation.requestPreciseLocation send=true live=false request=always sync= substitute=[\"permission\"]",
            "watchPosition geolocation.watchPosition send=false live=true request=when-prompt sync= substitute=[\"default\", \"fill\", \"permission\", \"short\"]",
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
