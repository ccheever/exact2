//! Work that outlives its answer (LLP 1146 D2) through the Apple host's
//! actual Bridge, executor and pump, with a scripted transport and no lock:
//! a resource renews a token while a timer asks it again three times. The
//! renewal's POST is held until every re-ask has let its answer go; the
//! runner must keep it (`holds`), its reply must reach the let-go answer's
//! continuation, whose store write stands, and the live answer must take
//! the kept token without a second POST.

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
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Condvar, Mutex};
use std::time::Duration;

const HBC: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/outlive.hbc"));
const APP: &str = "test.outlive";
const GRANTS: &str =
    "net.fetch https://outlive.test\nnet.fetch local-network development\nsecret.keep outlive.token\n";

static DRIVE: Mutex<()> = Mutex::new(());
/// Every request the transport saw: method, path and body.
static ASKED: Mutex<Vec<String>> = Mutex::new(Vec::new());
/// Whether the renewal's POST may answer.
static GATE: (Mutex<bool>, Condvar) = (Mutex::new(false), Condvar::new());
/// Whether the renewal's POST has reached the gate; reaching it wakes the
/// test as the executor does.
static ENTERED: AtomicBool = AtomicBool::new(false);
/// What only stops a hung test: no wait here is a duration the test relies on.
const HANG: Duration = Duration::from_secs(30);
static WOKE: Mutex<Option<Sender<()>>> = Mutex::new(None);

struct Scripted;
impl Transport for Scripted {
    fn open(
        &self,
        request: &ibex2::stdlib::fetch::Request,
        signal: &AbortSignal,
    ) -> Result<StreamingResponse, HostError> {
        let path = request
            .url
            .trim_start_matches("https://outlive.test")
            .to_string();
        let sent =
            String::from_utf8_lossy(request.body.as_deref().unwrap_or_default()).into_owned();
        ASKED
            .lock()
            .unwrap()
            .push(format!("{} {path} {sent}", request.method));
        let body = if path == "/token" {
            ENTERED.store(true, Ordering::SeqCst);
            woke(std::ptr::null_mut());
            let (open, opened) = &GATE;
            let held = opened
                .wait_timeout_while(open.lock().unwrap(), HANG, |o| !*o)
                .unwrap();
            assert!(*held.0, "the gate never opened");
            format!(
                "t{}",
                sent.trim_start_matches('t').parse::<u32>().unwrap_or(0) + 1
            )
        } else {
            format!("me-{}", path.rsplit('=').next().unwrap_or(""))
        };
        signal.check()?;
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

fn endow(_: &str, _: &str, _: bool) -> Endowed {
    Endowed::none()
}

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

/// Boot `contract` on the Bridge with the fixture, waking `wakes` as a
/// presenter is woken.
fn boot(contract: &str, wakes: Sender<()>) -> Bridge<Module> {
    *WOKE.lock().unwrap() = Some(wakes);
    let plan = contract::compile(contract)
        .expect("the fixture's Contract compiles")
        .encode();
    let links = Links {
        io: Some(IoLinks {
            endow,
            start,
            ..IoLinks::LINKED
        }),
        ..Links::ALL
    };
    let mut module = Module::loaded(HBC.to_vec(), APP, GRANTS).expect("the fixture loads");
    module.set_budget_ms(f64::INFINITY);
    let mut bridge = Bridge::with_links(links);
    bridge.boot(
        &plan,
        module,
        Hooks {
            wake: Some(woke),
            ..Hooks::none()
        },
        390.,
        844.,
    );
    bridge
}

/// The renewals the transport saw.
fn renewals() -> Vec<String> {
    let asked = ASKED.lock().unwrap();
    asked
        .iter()
        .filter(|a| a.starts_with("POST /token"))
        .cloned()
        .collect()
}

fn agent<D: DataSource>(bridge: &mut Bridge<D>, op: &str) -> String {
    let n = bridge.input_write(op.as_bytes());
    let len = bridge.agent(n) as usize;
    String::from_utf8_lossy(bridge.output_bytes(len)).into_owned()
}

/// Pump on the executor's wakes, as a presenter does, until `done`; a wake
/// that never comes fails the test, naming what it waited for.
fn pump_until<D: DataSource>(
    bridge: &mut Bridge<D>,
    wakes: &Receiver<()>,
    what: &str,
    mut done: impl FnMut(&mut Bridge<D>) -> bool,
) {
    loop {
        bridge.pump(0.0);
        if done(bridge) {
            return;
        }
        if wakes.recv_timeout(HANG).is_err() {
            let logs = agent(bridge, r#"{"op":"logs"}"#);
            // Copied first: a panic holding the lock would poison it for the next test.
            let asked = ASKED.lock().unwrap().clone();
            panic!("no wake while waiting for {what}: {logs}\nasked: {asked:?}");
        }
    }
}

fn text<D: DataSource>(bridge: &mut Bridge<D>, id: &str) -> String {
    agent(
        bridge,
        &format!(r#"{{"op":"tree","target":"{id}","shallow":true}}"#),
    )
}

const CONTRACT: &str = "shape Row
  text: string

component App
  state n = 0
  resource token = token() as shape Row
  resource read = read(n) as shape Row else empty(text=\"\")
  task bump mount
    every(100, step)
  action step
    if n < 3
      n = n + 1
  view
    column
      text token.text testId=\"token\"
      text read.text testId=\"read\"
";

#[test]
fn a_renewal_outlives_the_answers_that_asked_for_it() {
    let _one = DRIVE.lock().unwrap_or_else(|e| e.into_inner());
    ASKED.lock().unwrap().clear();
    *GATE.0.lock().unwrap() = false;
    ENTERED.store(false, Ordering::SeqCst);
    let (wake, wakes): (Sender<()>, Receiver<()>) = channel();
    let mut bridge = boot(CONTRACT, wake);
    // The renewal reaches the server and is held there.
    pump_until(
        &mut bridge,
        &wakes,
        "the renewal to reach the server",
        |_| ENTERED.load(Ordering::SeqCst),
    );
    // The timer asks `read` again three times while it is held: each lets
    // its answer go, and the first answer's POST is kept.
    for k in 1..=3 {
        bridge.advance(f64::from(k) * 100.0, 0);
        bridge.pump(0.0);
    }
    let held = agent(&mut bridge, r#"{"op":"logs"}"#);
    assert!(
        held.matches("forget request").count() >= 3,
        "three re-asks let go while the renewal was held: {held}"
    );
    assert_eq!(
        held.matches("so it runs to its end").count(),
        1,
        "only the POST is kept: {held}"
    );
    *GATE.0.lock().unwrap() = true;
    GATE.1.notify_all();
    pump_until(
        &mut bridge,
        &wakes,
        "the live answer to take the renewed token",
        |b| text(b, "read").contains("read me-t1 kept t1"),
    );
    let logs = agent(&mut bridge, r#"{"op":"logs"}"#);
    *WOKE.lock().unwrap() = None;
    assert!(
        text(&mut bridge, "token").contains("\"t1\""),
        "the let-go answer's continuation kept the token: {}",
        text(&mut bridge, "token")
    );
    assert_eq!(
        renewals(),
        ["POST /token 0"],
        "one renewal, never a spent token again\n{logs}"
    );
}

const LATE: &str = "shape Row
  text: string

component App
  state n = 0
  resource late = late() as shape Row else empty(text=\"\")
  resource renewed = renewed(n) as shape Row else empty(text=\"\")
  task bump mount
    every(100, step)
  action step
    if n < 1
      n = n + 1
  view
    column
      text late.text testId=\"late\"
      text renewed.text testId=\"renewed\"
";

/// A resource that reads the store only after a reply is a store reader:
/// a renewal that starts after it answered asks it again.
#[test]
fn a_store_read_after_a_reply_is_asked_again_when_the_store_changes() {
    let _one = DRIVE.lock().unwrap_or_else(|e| e.into_inner());
    ASKED.lock().unwrap().clear();
    *GATE.0.lock().unwrap() = true;
    let (wake, wakes): (Sender<()>, Receiver<()>) = channel();
    let mut bridge = boot(LATE, wake);
    pump_until(&mut bridge, &wakes, "`late` to read no token", |b| {
        text(b, "late").contains("late none") && text(b, "renewed").contains("idle")
    });
    // The timer starts the renewal, which keeps the token.
    bridge.advance(100.0, 0);
    pump_until(
        &mut bridge,
        &wakes,
        "`late` to read the renewed token",
        |b| text(b, "renewed").contains("renewed t1") && text(b, "late").contains("late t1"),
    );
    *WOKE.lock().unwrap() = None;
    assert_eq!(renewals(), ["POST /token 0"]);
}

const STANDS: &str = "shape Row
  text: string

component App
  resource token = token() as shape Row
  resource stands = stands() as shape Row else empty(text=\"\")
  view
    column
      text token.text testId=\"token\"
      text stands.text testId=\"stands\"
";

/// The commit that lands an answer is refused after the answer's POST: what
/// the reply's turn wrote to the store stands, and its readers read it.
#[test]
fn a_refused_commit_keeps_what_a_reply_with_effects_wrote() {
    let _one = DRIVE.lock().unwrap_or_else(|e| e.into_inner());
    ASKED.lock().unwrap().clear();
    *GATE.0.lock().unwrap() = true;
    let (wake, wakes): (Sender<()>, Receiver<()>) = channel();
    let mut bridge = boot(STANDS, wake);
    pump_until(
        &mut bridge,
        &wakes,
        "`token` to read the renewed token",
        |b| text(b, "token").contains("\"t1\""),
    );
    let logs = agent(&mut bridge, r#"{"op":"logs"}"#);
    *WOKE.lock().unwrap() = None;
    assert!(
        logs.contains("stands") && logs.contains("refused"),
        "the answer's commit was refused: {logs}"
    );
    assert_eq!(renewals(), ["POST /token 0"]);
}

/// A server on 127.0.0.1 for the abortable write, which runs beside the
/// ordered lane (`exactIndependentHttp`) so that lane stays free for the
/// answers that abort it: its one request, `POST /token`, is recorded,
/// reached and held at the gate as the scripted one is.
struct WriteServer {
    port: u16,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl WriteServer {
    fn start() -> WriteServer {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let thread = std::thread::spawn(move || {
            let Ok((mut stream, _)) = listener.accept() else {
                return;
            };
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut first = String::new();
            let _ = reader.read_line(&mut first);
            let mut length = 0;
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                    break;
                }
                if let Some(n) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    length = n.trim().parse().unwrap_or(0);
                }
            }
            let mut body = vec![0; length];
            let _ = reader.read_exact(&mut body);
            let mut words = first.split(' ');
            let (method, path) = (words.next().unwrap_or(""), words.next().unwrap_or(""));
            ASKED
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push(format!(
                    "{method} {path} {}",
                    String::from_utf8_lossy(&body)
                ));
            ENTERED.store(true, Ordering::SeqCst);
            woke(std::ptr::null_mut());
            let (open, opened) = &GATE;
            drop(
                opened
                    .wait_timeout_while(open.lock().unwrap_or_else(|e| e.into_inner()), HANG, |o| {
                        !*o
                    })
                    .unwrap_or_else(|e| e.into_inner()),
            );
            let _ = stream
                .write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\nconnection: close\r\n\r\nt1");
        });
        WriteServer {
            port,
            thread: Some(thread),
        }
    }
}

/// Ended however its test ends: the gate opens and its thread is joined, so
/// the next test's gate is its own.
impl Drop for WriteServer {
    fn drop(&mut self) {
        *GATE.0.lock().unwrap_or_else(|e| e.into_inner()) = true;
        GATE.1.notify_all();
        if let Some(thread) = self.thread.take() {
            // A write that never came leaves it accepting: a connection ends that.
            if !ENTERED.load(Ordering::SeqCst) {
                let _ = std::net::TcpStream::connect(("127.0.0.1", self.port));
            }
            let _ = thread.join();
        }
    }
}

/// A contract whose `write` resource asks the write server on `port` and
/// whose `cancel` resource asks `cancel`: the timer takes `n` to 2.
fn aborts(port: u16, cancel: &str) -> String {
    format!(
        "shape Row
  text: string

component App
  state n = 0
  resource token = token() as shape Row
  resource write = write(n, \"http://127.0.0.1:{port}/token\") as shape Row else empty(text=\"\")
  resource cancel = {cancel}(n) as shape Row
  task bump mount
    every(100, step)
  action step
    if n < 2
      n = n + 1
  view
    column
      text token.text testId=\"token\"
      text write.text testId=\"write\"
      text cancel.text testId=\"cancel\"
"
    )
}

/// Start the held write, let its answer go so the runner keeps it, then take
/// the timer to the step whose `cancel` answer aborts it.
fn kept_then_aborted(cancel: &str, server: &WriteServer) -> (Bridge<Module>, Receiver<()>) {
    let (wake, wakes): (Sender<()>, Receiver<()>) = channel();
    let mut bridge = boot(&aborts(server.port, cancel), wake);
    pump_until(&mut bridge, &wakes, "the write to reach the server", |_| {
        ENTERED.load(Ordering::SeqCst)
    });
    bridge.advance(100.0, 0);
    bridge.pump(0.0);
    let held = agent(&mut bridge, r#"{"op":"logs"}"#);
    assert_eq!(
        held.matches("so it runs to its end").count(),
        1,
        "the let-go write is kept: {held}"
    );
    bridge.advance(200.0, 0);
    (bridge, wakes)
}

/// A fresh write server, with the gate closed and nothing asked yet.
fn held_write() -> WriteServer {
    ASKED.lock().unwrap_or_else(|e| e.into_inner()).clear();
    *GATE.0.lock().unwrap_or_else(|e| e.into_inner()) = false;
    ENTERED.store(false, Ordering::SeqCst);
    WriteServer::start()
}

/// An app aborts a kept write in a reply's turn whose commit is then refused:
/// the write's rejection reaches its let-go answer's continuation in a module
/// turn, outside that commit's checkpoint, so what the continuation writes
/// stands (LLP 1146 D2).
#[test]
fn an_aborted_kept_write_s_continuation_writes_in_a_module_turn() {
    let _one = DRIVE.lock().unwrap_or_else(|e| e.into_inner());
    let server = held_write();
    let (mut bridge, wakes) = kept_then_aborted("cancel", &server);
    pump_until(
        &mut bridge,
        &wakes,
        "the continuation's write to stand",
        |b| text(b, "token").contains("aborted AbortError"),
    );
    drop(server);
    let logs = agent(&mut bridge, r#"{"op":"logs"}"#);
    let state = agent(&mut bridge, r#"{"op":"state"}"#);
    *WOKE.lock().unwrap() = None;
    assert!(
        logs.contains("(cancel)") && logs.contains("refused"),
        "the aborting answer's commit was refused: {logs}"
    );
    assert!(
        logs.contains("module work 1: aborted: the app aborted the fetch"),
        "the host cancelled the kept request and answered it aborted: {logs}"
    );
    assert!(
        !state.contains("module work"),
        "no module work is left pending: {state}"
    );
    assert_eq!(renewals(), ["POST /token w"]);
}

/// An answer that aborts a kept write and then awaits it is not left
/// waiting on itself: its turn ends, the module turn rejects the write, and
/// the answer resumes in a later turn.
#[test]
fn an_answer_that_aborts_a_kept_write_and_awaits_it_completes() {
    let _one = DRIVE.lock().unwrap_or_else(|e| e.into_inner());
    let server = held_write();
    let (mut bridge, wakes) = kept_then_aborted("cancelAwait", &server);
    pump_until(
        &mut bridge,
        &wakes,
        "the aborting answer to take the write's end",
        |b| text(b, "cancel").contains("awaited aborted AbortError"),
    );
    drop(server);
    *WOKE.lock().unwrap() = None;
    assert!(
        text(&mut bridge, "token").contains("aborted AbortError"),
        "the continuation's write stands: {}",
        text(&mut bridge, "token")
    );
    assert_eq!(renewals(), ["POST /token w"]);
}
