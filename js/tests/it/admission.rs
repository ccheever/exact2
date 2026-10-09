//! More answers than the ordered lane admits, each awaiting one shared load
//! that fetches through the host before its own fetch: the Bluesky clone's
//! Home at launch (its sources `await loadModeration()`), through the Apple
//! host's actual Bridge, executor and pump, with the app's TypeScript module
//! and a scripted transport (LLP 1041 §8.4, the proposed amendment of
//! 2026-10-09).
//!
//! What happens on main (2eca0ad80): the first answer starts the shared load
//! (its fetch); every later answer parks as waiting on it. Each answer that
//! began before another is asked again at once (a new answer's JavaScript
//! may have settled what it waits for), and each re-ask is an opaque ordered
//! continuation, counted against the sixteen-ticket bound, not the 128 that
//! plain reads get. The seventeenth is refused. A waiting answer's refusal
//! is not shaped by the source: it fails, keeps its last value (none), and
//! is never asked again, so its row stays on its placeholder for good. When
//! the shared load's second fetch lands, the waiters are asked again all at
//! once and one more is refused. Nothing deadlocks; the refused rows are
//! simply never asked again. (Their JavaScript still runs: every row's own
//! fetch reaches the transport, the refused rows' under other answers'
//! tickets, and those replies go nowhere.)

#![cfg(all(exact_js_engine, target_os = "macos"))]

use exact_apple::abi::{Bridge, Hooks};
use exact_apple::executor::{Executor, Io, WakeFn};
use exact_apple::link::{IoLinks, Links};
use exact_apple::store::Endowed;
use exact_js::Module;
use ibex2::boundary::HostError;
use ibex2::stdlib::abort::AbortSignal;
use ibex2::stdlib::fetch::{Headers, Response, StreamingResponse, Transport};
use std::ffi::c_void;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const HBC: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/shared-load.hbc"));
const APP: &str = "test.shared-load";
const GRANTS: &str = "net.fetch https://shared-load.test\n";

/// Every URL the transport was asked for, in order.
static ASKED: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// The network: the shared load's two fetches take a little while, as a
/// real one's do, so the answers that await it are all parked first.
struct Scripted;
impl Transport for Scripted {
    fn open(
        &self,
        request: &ibex2::stdlib::fetch::Request,
        signal: &AbortSignal,
    ) -> Result<StreamingResponse, HostError> {
        ASKED.lock().unwrap().push(request.url.clone());
        let path = request.url.trim_start_matches("https://shared-load.test");
        let wait = if path.starts_with("/row/") { 2 } else { 30 };
        std::thread::sleep(Duration::from_millis(wait));
        signal.check()?;
        let body = path.trim_start_matches('/').replace('/', "-");
        Ok(Response {
            status: 200,
            status_text: "OK".into(),
            headers: Headers::default(),
            body: body.into_bytes(),
            url: request.url.clone(),
            redirected: false,
        }
        .into_stream(request.body_limit(), signal.clone()))
    }
}

/// No platform stores: nothing kept, no keychain.
fn endow(_: &str, _: &str, _: bool) -> Endowed {
    Endowed::none()
}

/// The platform executor, its ordered owner over the scripted transport.
fn start(
    _: Option<ibex2::host::Bindings>,
    grants: &str,
    wake: Option<(WakeFn, *mut c_void)>,
) -> Box<dyn Io> {
    let bindings = ibex2::host::Host::with_transport(Box::new(Scripted))
        .endow(ibex2::grant::GrantSet::parse(&exact_runner::io_grants(grants)).unwrap());
    Box::new(Executor::start(Some(bindings), grants, wake))
}

fn contract(n: usize) -> String {
    let mut src = String::from("shape Row\n  text: string\n\ncomponent App\n");
    for i in 0..n {
        src += &format!("  resource r{i} = row({i}) as shape Row\n");
    }
    src += "  view\n    column\n";
    for i in 0..n {
        src += &format!("      text r{i}.text testId=\"r{i}\"\n");
    }
    src
}

fn agent(bridge: &mut Bridge<Module>, op: &str) -> String {
    let n = bridge.input_write(op.as_bytes());
    let len = bridge.agent(n) as usize;
    String::from_utf8_lossy(bridge.output_bytes(len)).into_owned()
}

/// Boot `n` rows and pump as a presenter does until every row shows its
/// own fetch's text or `within` passes; the rows still without it.
fn missing_after(n: usize, within: Duration) -> (Vec<usize>, String) {
    ASKED.lock().unwrap().clear();
    let plan = contract::compile(&contract(n))
        .expect("the fixture's Contract compiles")
        .encode();
    let mut module = Module::loaded(HBC.to_vec(), APP, GRANTS).expect("the fixture loads");
    module.set_budget_ms(f64::INFINITY);
    let links = Links {
        io: Some(IoLinks { endow, start }),
        ..Links::ALL
    };
    let mut bridge = Bridge::with_links(links);
    bridge.boot(&plan, module, Hooks::none(), 390., 844.);
    let begun = Instant::now();
    let missing = |bridge: &mut Bridge<Module>| {
        let tree = agent(bridge, r#"{"op":"tree"}"#);
        (0..n)
            .filter(|i| !tree.contains(&format!("row-{i} after prefs+labelers")))
            .collect::<Vec<_>>()
    };
    loop {
        bridge.pump(begun.elapsed().as_secs_f64() * 1e3);
        let left = missing(&mut bridge);
        if left.is_empty() || begun.elapsed() > within {
            // The runner's lines that say what became of the rows left.
            let logs = agent(&mut bridge, r#"{"op":"logs"}"#);
            let said: Vec<&str> = logs
                .split("\",\"")
                .filter(|line| line.contains("refused") || line.contains("no longer pending"))
                .collect();
            let asked = ASKED.lock().unwrap().len();
            return (
                left,
                format!(
                    "{asked} fetches reached the transport; the runner said:\n{}",
                    said.join("\n")
                ),
            );
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

/// The control: twelve answers awaiting the shared load fit the lane, and
/// every row loads.
#[test]
fn twelve_answers_awaiting_a_shared_load_all_load() {
    let (missing, report) = missing_after(12, Duration::from_secs(10));
    assert!(
        missing.is_empty(),
        "rows {missing:?} never loaded\n{report}"
    );
}

/// The repro: twenty answers awaiting the shared load. Every row should
/// load, as it does in a browser and with twelve; on main some never do.
#[test]
#[ignore = "repro, fails on main (2eca0ad80): answers awaiting a shared load past the 16-ticket ordered bound are refused and stay on their placeholders; LLP 1041 §8.4 proposed amendment (2026-10-09). Run: cargo test -p exact-js --test it admission -- --ignored --nocapture"]
fn twenty_answers_awaiting_a_shared_load_all_load() {
    let (missing, report) = missing_after(20, Duration::from_secs(10));
    assert!(
        missing.is_empty(),
        "rows {missing:?} never loaded\n{report}"
    );
}
