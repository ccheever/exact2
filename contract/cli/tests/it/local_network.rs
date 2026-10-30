//! `@exact/local-network` under its declared substitute (LLP 1133.036 §9,
//! D19). The interface is the package's own `exact.json`:
//!
//! - a browse while `prompt` waits on a `permission` ticket; `allow`
//!   answers the preferred network's services of the type, by name; a
//!   resolve answers its entry, filled; a network without it fails the
//!   resolve `NotFoundError` and the browse re-answers;
//! - `unknown`, the host's fact, asks as `prompt` does; `denied` fails
//!   `NotAllowedError` with no ticket;
//! - a type no `net.dns-sd` line lists fails `SecurityError`;
//! - a tool's browse with none running fails `person-required`, and joins
//!   the app's own once it runs (D3);
//! - a request a tool's commit asks carries the tool's origin and the
//!   substitute's state, which the executor's hop check reads (D6).

use exact_runner::local_network::State;
use exact_runner::{CallOrigin, Dispatch, Outcome, Reply, Runner, Work};
use std::path::PathBuf;

const GRANTS: &str = "device.local-network lan\nnet.dns-sd _wled._tcp\nnet.dns-sd _http._tcp\n\
                      net.fetch local-network development";

const APP: &str = r##"use browseServices, resolveService, localNetworkStatus, LocalServices, LocalServiceInfo, LocalNetworkStatus from "@exact/local-network"

component App
  state kind = "_wled._tcp"
  resource network = localNetworkStatus() as shape LocalNetworkStatus
  resource found = browseServices(type="_wled._tcp") as shape LocalServices
  resource desk = resolveService(name="Desk", type="_wled._tcp") as shape LocalServiceInfo
  resource other = browseServices(type=kind) as shape LocalServices
  resource light = probe() as shape string
  action look(k: string)
    kind = k
  action poke
    refresh light
  view
    column
      text network.state testId="state"
"##;

const NETWORK: &str = r#"[{"name":"Desk","type":"_wled._tcp","host":"127.0.0.1","port":4791,"txt":{"MAC":"a1b2c3d4e5f6"}},{"name":"attic","type":"_wled._tcp","host":"192.168.1.40","port":80},{"name":"Printer","type":"_ipp._tcp","host":"192.168.1.9","port":631}]"#;

struct App(PathBuf);
impl App {
    fn new(name: &str) -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("exact-lan-{name}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        let package = path.join("node_modules/@exact/local-network");
        let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../packages/local-network");
        std::fs::create_dir_all(&package).unwrap();
        for file in ["package.json", "exact.json"] {
            std::fs::copy(source.join(file), package.join(file)).unwrap();
        }
        for platform in ["apple", "android", "web", "linux", "windows"] {
            std::fs::create_dir_all(package.join(platform)).unwrap();
            let json = source.join(platform).join("exact.json");
            std::fs::copy(json, package.join(platform).join("exact.json")).unwrap();
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

/// The app's data code: `probe()` fetches a light at a LAN address.
struct Data;
impl exact_runner::DataSource for Data {
    fn query(
        &mut self,
        source: &str,
        _: &[exact_runner::Value],
    ) -> Result<exact_runner::Value, exact_runner::DataError> {
        Err(exact_runner::DataError::UnknownSource(source.into()))
    }
    fn answer(
        &mut self,
        _: &mut exact_runner::Store,
        source: &str,
        _: &[exact_runner::Value],
    ) -> Result<exact_runner::Answer, exact_runner::DataError> {
        match source {
            "probe" => Ok(exact_runner::Answer::Later(exact_runner::Request::get(
                "http://192.168.1.40/json/si",
            ))),
            _ => Err(exact_runner::DataError::UnknownSource(source.into())),
        }
    }
    fn parse(
        &mut self,
        _: &mut exact_runner::Store,
        _: &str,
        _: &[exact_runner::Value],
        _: Outcome,
    ) -> Result<exact_runner::Answer, exact_runner::DataError> {
        Ok(exact_runner::Answer::Now(exact_runner::Value::str("on")))
    }
    fn grants(&self) -> &str {
        GRANTS
    }
}

fn boot(name: &str) -> (App, Runner<Data>) {
    let app = App::new(name);
    let plan = contract::compile_path_all(&app.0.join("app.contract"), false)
        .map(|(plan, _)| plan)
        .unwrap_or_else(|e| panic!("{e:?}"));
    let r = Runner::boot(
        plan,
        Data,
        exact_kernel::Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    (app, r)
}

thread_local! {
    /// Each held module request's continuation token and ticket, kept
    /// across pumps: a ticket the agent answers later releases it.
    static PARKED: std::cell::RefCell<Vec<(u64, u64)>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// The host's part: module work run, released work run, outcomes
/// fulfilled; the HTTP requests the data code asks are kept, unsent.
fn pump(r: &mut Runner<Data>, sent: &mut Vec<exact_runner::RequestOut>) {
    let mut parked = PARKED.take();
    pump_parked(r, sent, &mut parked);
    PARKED.set(parked);
}

fn pump_parked(
    r: &mut Runner<Data>,
    sent: &mut Vec<exact_runner::RequestOut>,
    parked: &mut Vec<(u64, u64)>,
) {
    for _ in 0..8 {
        let outs = r.take_requests();
        let released = r.release_work();
        if outs.is_empty() && released.is_empty() && r.apply_announced().0.is_empty() {
            return;
        }
        for out in outs {
            let Some(token) = out.request.continuation else {
                sent.push(out);
                continue;
            };
            parked.push((token, out.ticket));
            let dispatch = r.dispatch_work(token);
            run(r, out.ticket, dispatch);
        }
        for (token, dispatch) in released {
            if let Some(&(_, ticket)) = parked.iter().find(|(t, _)| *t == token) {
                run(r, ticket, dispatch);
            }
        }
    }
}

fn run(r: &mut Runner<Data>, ticket: u64, dispatch: Dispatch) {
    let outcome: Outcome = match dispatch {
        Dispatch::Run(Work::Now(work)) => work(),
        Dispatch::Run(Work::Later(work)) => {
            let (tx, rx) = std::sync::mpsc::channel();
            work(Reply::new(move |o| tx.send(o).unwrap()));
            match rx.try_recv() {
                Ok(o) => o,
                Err(_) => return,
            }
        }
        _ => return,
    };
    let _ = r.fulfill(ticket, outcome);
}

fn agent(r: &mut Runner<Data>, request: &str) {
    let (_, reply) = r.extension_agent(request).expect("a prefer");
    reply.unwrap();
}

fn show(r: &Runner<Data>, name: &str) -> String {
    format!("{:?}", r.resource(name))
}

fn logs(r: &Runner<Data>) -> String {
    exact_runner::agent::handle(r, r#"{"op":"logs"}"#)
}

fn tap(r: &mut Runner<Data>, choice: &str) {
    let hold = (r.device_holds().iter())
        .find(|h| h.capability == "permission")
        .unwrap_or_else(|| panic!("no permission ticket: {:?}\n{}", r.device_holds(), logs(r)))
        .ticket;
    let request = format!(r#"{{"op":"tap","ticket":{hold},"choice":"{choice}"}}"#);
    exact_runner::agent::answer(r, &request).unwrap();
}

#[test]
fn a_browse_asks_then_answers_the_preferred_network_and_a_resolve_its_entry() {
    let (_app, mut r) = boot("browse");
    let mut sent = Vec::new();
    agent(
        &mut r,
        &format!(r#"{{"op":"prefer","extension":{{"local-network.browseServices":{NETWORK}}}}}"#),
    );
    pump(&mut r, &mut sent);
    assert!(
        show(&r, "network").contains("\"prompt\""),
        "{}",
        show(&r, "network")
    );
    tap(&mut r, "allow");
    pump(&mut r, &mut sent);
    assert!(
        show(&r, "network").contains("\"granted\""),
        "{}",
        show(&r, "network")
    );
    let found = show(&r, "found");
    let at = |name: &str| {
        found
            .find(name)
            .unwrap_or_else(|| panic!("{name} in {found}\n{}", logs(&r)))
    };
    let (attic, desk) = (at("attic"), at("Desk"));
    assert!(attic < desk, "by name, case-insensitively: {found}");
    assert!(!found.contains("Printer"), "{found}");
    let desk = show(&r, "desk");
    for part in ["127.0.0.1", "4791", "\"mac\"", "a1b2c3d4e5f6"] {
        assert!(desk.contains(part), "{part} in {desk}");
    }
    // A network without Desk: the resolve fails and the browse re-answers.
    agent(
        &mut r,
        r#"{"op":"prefer","extension":{"local-network.browseServices":[{"name":"attic","type":"_wled._tcp","host":"192.168.1.40","port":80}]}}"#,
    );
    pump(&mut r, &mut sent);
    assert!(!show(&r, "found").contains("Desk"), "{}", show(&r, "found"));
    assert!(logs(&r).contains("NotFoundError"), "{}", logs(&r));
    // A type no `net.dns-sd` line lists.
    r.act("look", vec![exact_runner::Value::str("_ipp._tcp")])
        .unwrap();
    pump(&mut r, &mut sent);
    assert!(logs(&r).contains("SecurityError"), "{}", logs(&r));
}

#[test]
fn unknown_asks_as_prompt_does_and_denied_fails_without_a_ticket() {
    let (_app, mut r) = boot("states");
    let mut sent = Vec::new();
    agent(
        &mut r,
        r#"{"op":"prefer","permission":{"local-network":"unknown"}}"#,
    );
    pump(&mut r, &mut sent);
    assert!(
        r.device_holds()
            .iter()
            .any(|h| h.capability == "permission"),
        "{:?}\n{}",
        r.device_holds(),
        logs(&r)
    );
    agent(
        &mut r,
        r#"{"op":"prefer","permission":{"local-network":"denied"}}"#,
    );
    pump(&mut r, &mut sent);
    assert!(
        show(&r, "network").contains("\"denied\""),
        "{}",
        show(&r, "network")
    );
    assert!(logs(&r).contains("NotAllowedError"), "{}", logs(&r));
}

#[test]
fn a_tool_s_reach_carries_its_origin_and_the_substitute_s_state() {
    let (_app, mut r) = boot("reach");
    let mut sent = Vec::new();
    pump(&mut r, &mut sent);
    let first = sent
        .iter()
        .find(|o| o.request.url.contains("192.168.1.40"))
        .expect("the probe");
    assert_eq!(first.request.reach.origin, CallOrigin::App);
    assert_eq!(first.request.reach.substitute, Some(State::Prompt));
    sent.clear();
    r.act_from(CallOrigin::Tool, "poke", vec![]).unwrap();
    pump(&mut r, &mut sent);
    let tool = sent
        .iter()
        .find(|o| o.request.url.contains("192.168.1.40"))
        .expect("the tool's");
    assert_eq!(tool.request.reach.origin, CallOrigin::Tool, "{}", logs(&r));
    assert_eq!(tool.request.reach.substitute, Some(State::Prompt));
}

#[test]
fn a_tool_s_browse_with_none_running_waits_for_the_person_and_joins_a_running_one() {
    let (_app, mut r) = boot("joins");
    let mut sent = Vec::new();
    agent(
        &mut r,
        r#"{"op":"prefer","permission":{"local-network":"granted"}}"#,
    );
    agent(
        &mut r,
        &format!(r#"{{"op":"prefer","extension":{{"local-network.browseServices":{NETWORK}}}}}"#),
    );
    pump(&mut r, &mut sent);
    assert!(show(&r, "found").contains("Desk"), "{}", show(&r, "found"));
    // A type the app's own readers do not browse: nothing to join.
    r.act_from(
        CallOrigin::Tool,
        "look",
        vec![exact_runner::Value::str("_http._tcp")],
    )
    .unwrap();
    pump(&mut r, &mut sent);
    assert!(logs(&r).contains("person-required"), "{}", logs(&r));
    // The type Lights' own browse runs: the tool's reader joins it.
    r.act_from(
        CallOrigin::Tool,
        "look",
        vec![exact_runner::Value::str("_wled._tcp")],
    )
    .unwrap();
    pump(&mut r, &mut sent);
    assert!(
        show(&r, "other").contains("Desk"),
        "{}\n{}",
        show(&r, "other"),
        logs(&r)
    );
}
