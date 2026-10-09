//! More answers than the ordered lane's sixteen, each awaiting one shared
//! load that fetches through the host before its own fetch: the Bluesky
//! clone's Home at launch (its sources `await loadModeration()`), through the
//! Apple host's actual Bridge, executor and pump, with the app's TypeScript
//! module and a scripted transport (LLP 1041 §8.4, amended 2026-10-09).
//!
//! Before the amendment (2eca0ad80): every answer begun before another is
//! asked again at once, and each re-ask was an opaque ordered job, counted
//! against the sixteen. The sixteenth re-ask was refused, the refusal failed
//! its answer for good ("keeps its last value"), and with twenty rows, rows
//! 15 and 16 stayed on their placeholders on every run. Now a re-ask is
//! `Dispatch::Again`: settled in its ordered place with no work, never
//! counted against the sixteen, and kept pending when the 128-ticket window
//! is full, so every row loads, each from its own fetch, each answer begun
//! once.
//!
//! The run is event-driven: the shared load's first fetch waits at a gate
//! the test opens after boot has dispatched every answer, and the pump runs
//! on the executor's wakes, not on sleeps.

#![cfg(all(exact_js_engine, target_os = "macos"))]

use exact_apple::abi::{Bridge, Hooks};
use exact_apple::executor::{Executor, Io, WakeFn};
use exact_apple::link::{IoLinks, Links};
use exact_apple::store::Endowed;
use exact_js::Module;
use exact_runner::DataSource;
use ibex2::boundary::HostError;
use ibex2::stdlib::abort::AbortSignal;
use ibex2::stdlib::fetch::{Headers, Response, StreamingResponse, Transport};
use std::ffi::c_void;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

const HBC: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/shared-load.hbc"));
const APP: &str = "test.shared-load";
const GRANTS: &str = "net.fetch https://shared-load.test\n";

/// One drive at a time: the transport, the gate and the wake are the
/// process's (a `fn` pointer starts the executor), so each test has them to
/// itself.
static DRIVE: Mutex<()> = Mutex::new(());
/// Every URL the transport was asked for in this drive, in order.
static ASKED: Mutex<Vec<String>> = Mutex::new(Vec::new());
/// Whether the shared load's first fetch may answer.
static GATE: (Mutex<bool>, Condvar) = (Mutex::new(false), Condvar::new());
/// The executor's wake, for the pump loop.
static WOKE: Mutex<Option<Sender<()>>> = Mutex::new(None);

/// The network: answers at once, except the shared load's first fetch,
/// which waits for the gate, so every answer is parked on it first.
struct Scripted;
impl Transport for Scripted {
    fn open(
        &self,
        request: &ibex2::stdlib::fetch::Request,
        signal: &AbortSignal,
    ) -> Result<StreamingResponse, HostError> {
        ASKED.lock().unwrap().push(request.url.clone());
        let path = request.url.trim_start_matches("https://shared-load.test");
        if path == "/prefs" {
            let (open, opened) = &GATE;
            let held = opened
                .wait_timeout_while(open.lock().unwrap(), Duration::from_secs(30), |o| !*o)
                .unwrap();
            assert!(*held.0, "the gate never opened");
        }
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

extern "C" fn woke(_: *mut c_void) {
    if let Some(wake) = WOKE.lock().unwrap().as_ref() {
        let _ = wake.send(());
    }
}

fn contract(source: &str, n: usize) -> String {
    let mut src = String::from("shape Row\n  text: string\n\ncomponent App\n");
    for i in 0..n {
        src += &format!("  resource r{i} = {source}({i}) as shape Row\n");
    }
    src += "  view\n    column\n";
    for i in 0..n {
        src += &format!("      text r{i}.text testId=\"r{i}\"\n");
    }
    src
}

fn agent<D: DataSource>(bridge: &mut Bridge<D>, op: &str) -> String {
    let n = bridge.input_write(op.as_bytes());
    let len = bridge.agent(n) as usize;
    String::from_utf8_lossy(bridge.output_bytes(len)).into_owned()
}

fn module() -> Module {
    let mut module = Module::loaded(HBC.to_vec(), APP, GRANTS).expect("the fixture loads");
    module.set_budget_ms(f64::INFINITY);
    module
}

/// What a drive came to: the rows without their own text, the URLs the
/// transport saw, the pumps it took, and what the runner said of refusals.
struct Drove {
    missing: Vec<usize>,
    asked: Vec<String>,
    pumps: usize,
    said: String,
    /// The runner's last lines, for a drive that did not finish.
    tail: String,
}

/// Boot `n` rows over `data`, open the gate once boot has dispatched, and
/// pump on the executor's wakes until every row shows its own fetch's text,
/// from an answer begun once, or `within` passes.
fn drive<D: DataSource>(data: D, source: &str, n: usize, within: Duration) -> Drove {
    let _one = DRIVE.lock().unwrap_or_else(|e| e.into_inner());
    ASKED.lock().unwrap().clear();
    *GATE.0.lock().unwrap() = false;
    let (wake, wakes): (Sender<()>, Receiver<()>) = channel();
    *WOKE.lock().unwrap() = Some(wake);
    let plan = contract::compile(&contract(source, n))
        .expect("the fixture's Contract compiles")
        .encode();
    let links = Links {
        io: Some(IoLinks { endow, start }),
        ..Links::ALL
    };
    let mut bridge = Bridge::with_links(links);
    let hooks = Hooks {
        wake: Some(woke),
        ..Hooks::none()
    };
    bridge.boot(&plan, data, hooks, 390., 844.);
    let begun = Instant::now();
    let mut pumps = 0;
    let missing = |bridge: &mut Bridge<D>| {
        let tree = agent(bridge, r#"{"op":"tree"}"#);
        (0..n)
            .filter(|i| !tree.contains(&format!("\"{source}-{i} after prefs+labelers #1\"")))
            .collect::<Vec<_>>()
    };
    let mut gate = false;
    let left = loop {
        bridge.pump(begun.elapsed().as_secs_f64() * 1e3);
        pumps += 1;
        if !gate {
            // Boot's requests have all been dispatched by now.
            *GATE.0.lock().unwrap() = true;
            GATE.1.notify_all();
            gate = true;
        }
        let left = missing(&mut bridge);
        if left.is_empty() || begun.elapsed() > within {
            break left;
        }
        // A wake, or a beat to look again: the pump decides, not the clock.
        let _ = wakes.recv_timeout(Duration::from_millis(100));
    };
    let logs = agent(&mut bridge, r#"{"op":"logs"}"#);
    let said = logs
        .split("\",\"")
        .filter(|line| line.contains("refused") || line.contains("no longer pending"))
        .collect::<Vec<_>>()
        .join("\n");
    let lines: Vec<&str> = logs.split("\",\"").collect();
    let tail = lines[lines.len().saturating_sub(20)..].join("\n");
    *WOKE.lock().unwrap() = None;
    let asked = ASKED.lock().unwrap().clone();
    Drove {
        missing: left,
        asked,
        pumps,
        said,
        tail,
    }
}

/// The URLs a drive of `n` rows must ask for, each once (`row` fetches its
/// own; `quiet` nothing).
fn expected(n: usize) -> Vec<String> {
    let mut urls: Vec<String> = ["prefs", "labelers"]
        .iter()
        .map(|p| format!("https://shared-load.test/{p}"))
        .chain((0..n).map(|i| format!("https://shared-load.test/row/{i}")))
        .collect();
    urls.sort();
    urls
}

fn assert_all_loaded(drove: Drove, n: usize, own: bool) {
    assert!(
        drove.missing.is_empty(),
        "rows {:?} never loaded after {} pumps; {} fetches\nthe runner said:\n{}\nits last lines:\n{}",
        drove.missing,
        drove.pumps,
        drove.asked.len(),
        drove.said,
        drove.tail
    );
    assert!(drove.said.is_empty(), "nothing is refused:\n{}", drove.said);
    let mut asked = drove.asked;
    asked.sort();
    assert_eq!(
        asked,
        expected(if own { n } else { 0 }),
        "each fetch once, each row its own"
    );
    // The baseline for LLP 1041 §8.4 Q6 (the herd): pumps to load every row.
    eprintln!("{n} rows loaded in {} pumps", drove.pumps);
}

/// Twelve answers awaiting the shared load: every row loads.
#[test]
fn twelve_answers_awaiting_a_shared_load_all_load() {
    assert_all_loaded(
        drive(module(), "row", 12, Duration::from_secs(10)),
        12,
        true,
    );
}

/// The repro: twenty answers awaiting the shared load. Failed before the
/// amendment (rows 15 and 16 refused for good); every row loads now.
#[test]
fn twenty_answers_awaiting_a_shared_load_all_load() {
    assert_all_loaded(
        drive(module(), "row", 20, Duration::from_secs(10)),
        20,
        true,
    );
}

/// 300 answers awaiting the shared load and fetching nothing of their own,
/// through the storage composer (whose dispatch consumes its continuation
/// mapping): more re-asks than the markers' window of 128 at every wave. The
/// ones past it wait pending in the executor and are placed as room frees;
/// none is refused, none is begun again, and the shared load's own second
/// fetch is admitted beside them. (Rows that each fetched would be more than
/// 128 real reads at once, which the lane still refuses past its backlog.)
#[test]
fn more_waiting_answers_than_the_window_all_load_once() {
    let mut unloaded = Module::new(HBC.to_vec(), APP, GRANTS);
    unloaded.set_budget_ms(f64::INFINITY);
    let mut data = exact_data_host::Storage::new(unloaded);
    data.activate().expect("the fixture loads");
    assert_all_loaded(
        drive(data, "quiet", 300, Duration::from_secs(60)),
        300,
        false,
    );
}
