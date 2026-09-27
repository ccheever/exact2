//! The async render host (LLP 1048.000 D9).
//!
//! A render boots a fresh runner at a location with the app's own data
//! source, in an environment that holds nothing private — no store, no
//! cookies, no device capabilities ([`Anonymous`]) — and runs its requests
//! through the native executor core the Apple and Linux hosts share
//! ([`Executor`]) until the document settles or its deadline passes
//! ([`settle`]). Its checkpoint is the page's state; the document is that
//! checkpoint's projection (`exact_web::document`), from a runner booted
//! from it as the runtime boots. The build runs it through [`main`]; the
//! server (LLP 1048.000 D10) runs the same render per request.
//!
//! No action runs, no timer fires and the clock stays where boot put it:
//! the completion rule ignores them. A source asking for what the
//! environment doesn't hold — SQLite, kept secrets, a grant the host
//! doesn't hold, surface work — is refused there and keeps its placeholder;
//! the checkpoint lists it and the client asks it after adoption.

#![deny(missing_docs)]

mod encode;
mod executor;
mod page;
mod pages;
mod serve;
mod source;

pub use executor::Executor;
pub use page::{capture, page};
pub use pages::pages;
pub use serve::{Serve, Server, Stopper};
pub use source::Anonymous;

use exact_kernel::Kernel;
use exact_plan::{Plan, RenderPolicy};
use exact_runner::{
    DataSource, Dispatch, FailureKind, Interrupt, Outcome, RequestOut, Runner, RunnerError,
};
use exact_web::document::{
    build_locations, checkpoint, digest, project, read_checkpoint, route_at, Document, Site,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// A render's deadline unless the caller names one (LLP 1048.000 D9).
pub const DEADLINE: Duration = Duration::from_secs(2);

/// How a render ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Settled {
    /// Every request the document depends on answered, failed, or was
    /// refused by the environment (its resource keeps its placeholder).
    Complete,
    /// The deadline passed with requests in flight: their resources show
    /// their placeholders with `pending(x)` true, and the checkpoint lists
    /// them for the runtime to ask again.
    Deadline,
    /// The executor turned requests away at its limits: the page is
    /// answered as one at its deadline is, and no cache keeps it.
    Busy,
}

impl Rendered {
    /// The HTTP status (LLP 1048.000 D11), in this order:
    /// 1. a location the router sends to its not-found route is 404 — or
    ///    410 if its head says so. Neither its data failing nor the
    ///    deadline makes an unknown URL one to try again;
    /// 2. a render at its deadline, or turned away (`Busy`), is 503;
    /// 3. the head's own status: 404, 410, or 503 for failed data;
    /// 4. otherwise 200.
    pub fn status(&self, notfound: bool) -> u16 {
        match (notfound, self.settled, self.document.head.status) {
            (true, _, Some(410)) => 410,
            (true, _, _) => 404,
            (false, Settled::Deadline | Settled::Busy, _) => 503,
            (false, Settled::Complete, Some(status)) => status,
            (false, Settled::Complete, None) => 200,
        }
    }
}

/// One location, rendered.
pub struct Rendered {
    /// The projected document.
    pub document: Document,
    /// What `<head>` holds after the shell's charset and base.
    pub head: String,
    /// The checkpoint, as the page carries it.
    pub checkpoint: String,
    /// The document's digest (LLP 1048.000 D6), which the page carries
    /// beside the checkpoint for the runtime to match.
    pub digest: String,
    /// What the document read: its answers and what was still pending.
    pub state: exact_runner::Checkpoint,
    /// How the render ended.
    pub settled: Settled,
    /// When the document asks for its client runtime.
    pub activate: exact_plan::ActivatePolicy,
}

/// Render `plan` at `location` with a source from `data`, waiting at most
/// `deadline` for its requests. A source whose reply it cannot shape, or a
/// projection the HTML parser would undo, is the error.
pub fn render<D: DataSource>(
    plan: &Plan,
    data: impl Fn() -> D,
    viewport: exact_runner::Viewport,
    location: &str,
    site: &Site,
    deadline: Duration,
) -> Result<Rendered, String> {
    // A renderer runs any plan it is handed: every capability is linked
    // (LLP 1047 D7), so a projection never meets one it can't write.
    exact_web::link(exact_web_capabilities::ALL);
    let settling = Anonymous::new(data());
    // The deadline waits for sources, not for the transport to start.
    let executor = Executor::start(exact_runner::DataSource::grants(&settling));
    let until = Instant::now() + deadline;
    let watchdog = Watchdog::arm(settling.interrupt(), until);
    let mut runner = Runner::boot_with_delivery(
        plan.clone(),
        settling,
        Kernel::with_monospace(),
        None,
        Vec::new(),
        Default::default(),
        viewport,
        location,
    )
    .map_err(|e| format!("boot: {e:?}"))?;
    let settled = match activate(&mut runner, until)
        .and_then(|()| settle(&mut runner, &executor, until).map_err(|e| format!("{e:?}")))
    {
        Ok(settled) => settled,
        // The call running at the deadline was stopped and refused, so what
        // it answers shows its placeholder: the render ends at the deadline.
        Err(_) if watchdog.fired() => Settled::Deadline,
        Err(e) => return Err(e),
    };
    drop(watchdog);
    // Whatever is still in flight is abandoned with the render.
    drop(executor);
    let checkpoint = checkpoint(&runner, location);
    drop(runner);
    // @ref LLP 1048.000 D6 — the document is the checkpoint's projection,
    // from a runner booted from the page's checkpoint with a fresh source,
    // as the runtime boots: its first tree is built in one pass, so its view
    // ids are the runtime's, a pending answer's placeholder included. The
    // runner that settled built its tree as answers arrived. What the boot
    // asks is never run.
    let state = read_checkpoint(&checkpoint).map_err(|e| format!("the checkpoint: {e}"))?;
    let booted = Runner::boot_checkpoint(
        plan.clone(),
        Anonymous::new(data()),
        Kernel::with_monospace(),
        &state,
        Vec::new(),
        Default::default(),
        viewport,
        location,
    )
    .map_err(|e| format!("boot from the checkpoint: {e:?}"))?;
    let document = project(&booted).map_err(|e| e.to_string())?;
    let head = document
        .page_head(plan, site, location)
        .map_err(|e| e.to_string())?;
    let digest = digest(&plan.encode(), location, &checkpoint, &document.root);
    let mut activation = route_at(plan, location)
        .map_or(exact_plan::ActivatePolicy::Inferred, |route| route.activate);
    // A partial document needs to finish without waiting for an action. Gesture,
    // media and other continuous handlers likewise need the ordinary idle boot;
    // interaction activation only replays discrete form and press semantics.
    if activation == exact_plan::ActivatePolicy::Interaction
        && (!state.pending.is_empty()
            || booted.handlers().values().flatten().any(|kind| {
                !matches!(
                    kind,
                    exact_plan::EventKind::Press
                        | exact_plan::EventKind::Change
                        | exact_plan::EventKind::Focus
                        | exact_plan::EventKind::Blur
                        | exact_plan::EventKind::Key
                        | exact_plan::EventKind::Submit
                        | exact_plan::EventKind::Navigate
                )
            }))
    {
        activation = exact_plan::ActivatePolicy::Idle;
    }
    Ok(Rendered {
        document,
        head,
        checkpoint,
        digest,
        state,
        settled,
        activate: activation,
    })
}

/// The deadline, for a source call still running then (LLP 1048.000 D10):
/// a thread that triggers the source's interrupt at `until`, unless the
/// render finished first. A source without one runs its calls to the end.
struct Watchdog {
    cancel: Option<Sender<()>>,
    fired: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Watchdog {
    fn arm(interrupt: Option<Interrupt>, until: Instant) -> Watchdog {
        let fired = Arc::new(AtomicBool::new(false));
        let Some(interrupt) = interrupt else {
            return Watchdog {
                cancel: None,
                fired,
                thread: None,
            };
        };
        let (cancel, cancelled) = channel::<()>();
        let flag = fired.clone();
        let thread = std::thread::Builder::new()
            .name("exact-render-deadline".into())
            .spawn(move || {
                let wait = until.saturating_duration_since(Instant::now());
                if cancelled.recv_timeout(wait) == Err(RecvTimeoutError::Timeout) {
                    flag.store(true, Ordering::SeqCst);
                    interrupt.trigger();
                }
            })
            .ok();
        Watchdog {
            cancel: Some(cancel),
            fired,
            thread,
        }
    }

    fn fired(&self) -> bool {
        self.fired.load(Ordering::SeqCst)
    }
}

impl Drop for Watchdog {
    fn drop(&mut self) {
        drop(self.cancel.take());
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// A module that loads after first pixel on a device (LLP 1027 D4) loads
/// now: a render has no first pixel to protect. What it would have been
/// asked at `data_ready` it is asked here.
fn activate<D: DataSource>(runner: &mut Runner<D>, until: Instant) -> Result<(), String> {
    loop {
        match runner.data_ref().preload() {
            Ok(true) => break,
            Ok(false) if Instant::now() < until => {
                std::thread::sleep(Duration::from_millis(1));
            }
            Ok(false) => return Err("the data module did not prepare before the deadline".into()),
            Err(e) => return Err(format!("prepare data: {e:?}")),
        }
    }
    runner
        .data()
        .activate()
        .map_err(|e| format!("activate data: {e:?}"))?;
    runner
        .data_ready()
        .map_err(|e| format!("data ready: {e:?}"))?;
    Ok(())
}

/// Run `runner`'s requests through `executor` until none is in flight but
/// the ones the environment refused, or until `deadline`. Each reply is a
/// commit, as on a host; a failed request is the source's data to shape
/// (LLP 1016 D4). A reply the source cannot shape is the render's failure.
pub fn settle<D: DataSource>(
    runner: &mut Runner<D>,
    executor: &Executor,
    deadline: Instant,
) -> Result<Settled, RunnerError> {
    let mut held = Held::default();
    loop {
        hand_out(runner, executor, &mut held);
        let outcomes = executor.drain();
        if !outcomes.is_empty() {
            for (ticket, outcome) in outcomes {
                // Refused by the environment (a grant the host doesn't hold):
                // the resource keeps its placeholder for the client.
                if matches!(
                    outcome,
                    Outcome::Failed {
                        kind: FailureKind::Refused,
                        ..
                    }
                ) {
                    held.refused.insert(ticket);
                    continue;
                }
                runner.fulfill(ticket, outcome)?;
            }
            continue;
        }
        if runner
            .pending()
            .iter()
            .all(|(_, t)| held.refused.contains(t) || held.busy.contains(t))
        {
            return Ok(if held.busy.is_empty() {
                Settled::Complete
            } else {
                Settled::Busy
            });
        }
        if Instant::now() >= deadline {
            return Ok(Settled::Deadline);
        }
        executor.wait(deadline);
    }
}

/// What the last commit asked for goes to the executor (LLP 1016 D2); a
/// continuation is dispatched on this thread, after that commit (LLP
/// 1027.002 D3), and one its source holds waits for a later release.
/// A render's requests that aren't simply in flight.
#[derive(Default)]
struct Held {
    /// Held by their source until a later commit releases them.
    parked: BTreeMap<u64, RequestOut>,
    /// Refused by the environment: the resource keeps its placeholder.
    refused: BTreeSet<u64>,
    /// Turned away at the executor's limits: the render is busy.
    busy: BTreeSet<u64>,
}

fn hand_out<D: DataSource>(runner: &mut Runner<D>, executor: &Executor, held: &mut Held) {
    // The runner holds no refusals in a render, so nothing fences the lane.
    executor.forget(|ticket| runner.holds(ticket));
    executor.resume_ordered();
    for r in runner.take_requests() {
        // Device capabilities: the environment has none.
        if r.request.surface.is_some() || r.request.storage.is_some() {
            held.refused.insert(r.ticket);
            continue;
        }
        let dispatch = match r.request.continuation {
            Some(token) => runner.dispatch_work(token),
            None if r.request.is_native() => runner.native_work(&r.request),
            None => Dispatch::Missing,
        };
        run(executor, held, r, dispatch);
    }
    for (token, dispatch) in runner.release_work() {
        if let Some(r) = held.parked.remove(&token) {
            run(executor, held, r, dispatch);
        }
    }
}

fn run(executor: &Executor, held: &mut Held, r: RequestOut, dispatch: Dispatch) {
    let ticket = r.ticket;
    let admitted = match dispatch {
        Dispatch::Run(work) => executor.run(r, Some(work)),
        Dispatch::Held => {
            if let Some(token) = r.request.continuation {
                held.parked.insert(token, r);
            }
            Ok(())
        }
        Dispatch::Host(_) | Dispatch::Missing => executor.run(r, None),
    };
    if admitted.is_err() {
        held.busy.insert(ticket);
    }
}

/// An app's pages as documents, for the web build and the parity check:
///
/// `<app>-render [--plan <app.plan>] [--viewport <w>x<h>] [--name <name>]
/// [--origin <url>] [--deadline <ms>] [--shell <index.html>] (--build |
/// <location>…)`
///
/// renders each location with a fresh runner and executor at the page
/// viewport (the bake's 390 × 844 unless told) within the deadline (2 s
/// unless told), and prints one JSON line each: `location`, `notfound`,
/// `status` (503 when the deadline passed with requests in flight),
/// `settled`, `robots`, `root` (what `#exact-root` holds), `head` (what
/// `<head>` holds after the shell's charset and base), `checkpoint`,
/// `digest`, and with `--shell` the whole `page` ([`page`]), or `error`.
/// `--build` renders every location the plan declares `render=build`
/// (`exact_web::document::build_locations`) and every page a build route's
/// `pages=` source lists. `baked` is the app's own plan; the web build
/// passes the one it extracted from the shipped wasm instead. `data` makes
/// the app's source: a fresh one for each render (two per render, D6), for
/// each enumeration, and for the server's grants — so an entry is one call,
/// `exact_render::main(PLAN, || …)`, with no wrapper to forward the source's
/// methods.
///
/// `--serve <dist> [--port <n>] [--renders <n>] [--queue <n>] [--lifetime
/// <s>]` is the server instead ([`Server`]): the built web app on
/// loopback, its plan the dist's own `app.plan` unless `--plan` names one.
/// It prints `serving http://127.0.0.1:<port>/`, then a line per render.
pub fn main<D: DataSource + 'static>(baked: &[u8], data: fn() -> D) -> std::process::ExitCode {
    use std::process::ExitCode;
    let mut args = std::env::args().skip(1);
    let mut plan = baked.to_vec();
    let mut viewport = exact_runner::Viewport::default();
    let (mut name, mut origin) = (String::new(), None::<String>);
    let mut locations: Vec<(String, bool)> = Vec::new();
    let mut build = false;
    let mut deadline = DEADLINE;
    let mut shell = None::<String>;
    let (mut serve, mut planned) = (None::<std::path::PathBuf>, false);
    let (mut port, mut renders, mut queue, mut lifetime) = (0u16, 4usize, 32usize, 60u64);
    let usage = || {
        eprintln!("usage: render [--plan <app.plan>] [--viewport <w>x<h>] [--name <name>] [--origin <url>] [--deadline <ms>] ([--shell <index.html>] (--build | <location>…) | --serve <dist> [--port <n>] [--renders <n>] [--queue <n>] [--lifetime <s>])");
        ExitCode::from(2)
    };
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--plan" => match args.next().map(std::fs::read) {
                Some(Ok(bytes)) => {
                    plan = bytes;
                    planned = true;
                }
                Some(Err(e)) => {
                    eprintln!("render: --plan: {e}");
                    return ExitCode::FAILURE;
                }
                None => return usage(),
            },
            "--viewport" => {
                let size = args.next().and_then(|v| {
                    let (w, h) = v.split_once('x')?;
                    Some((w.parse().ok()?, h.parse().ok()?))
                });
                let Some((width, height)) = size else {
                    return usage();
                };
                viewport = exact_runner::Viewport::sized(width, height);
            }
            "--name" => match args.next() {
                Some(value) => name = value,
                None => return usage(),
            },
            "--origin" => match args.next() {
                Some(value) => origin = Some(value),
                None => return usage(),
            },
            "--deadline" => match args.next().and_then(|ms| ms.parse().ok()) {
                Some(ms) => deadline = Duration::from_millis(ms),
                None => return usage(),
            },
            "--shell" => match args.next().map(std::fs::read_to_string) {
                Some(Ok(text)) => shell = Some(text),
                Some(Err(e)) => {
                    eprintln!("render: --shell: {e}");
                    return ExitCode::FAILURE;
                }
                None => return usage(),
            },
            "--build" => build = true,
            "--serve" => match args.next() {
                Some(dist) => serve = Some(dist.into()),
                None => return usage(),
            },
            "--port" | "--renders" | "--queue" | "--lifetime" => {
                let Some(n) = args.next().and_then(|n| n.parse::<u64>().ok()) else {
                    return usage();
                };
                match arg.as_str() {
                    "--port" => port = n.try_into().unwrap_or(0),
                    "--renders" => renders = n as usize,
                    "--queue" => queue = n as usize,
                    _ => lifetime = n,
                }
            }
            _ if arg.starts_with('/') => locations.push((arg, false)),
            _ => return usage(),
        }
    }
    if let (Some(dist), false) = (&serve, planned) {
        match std::fs::read(dist.join("app.plan")) {
            Ok(bytes) => plan = bytes,
            Err(e) => {
                eprintln!("render: {}/app.plan: {e}", dist.display());
                return ExitCode::FAILURE;
            }
        }
    }
    let decoded = match Plan::decode(&plan) {
        Ok(decoded) => decoded,
        Err(e) => {
            eprintln!("render: the plan: {e:?}");
            return ExitCode::FAILURE;
        }
    };
    if let Some(dist) = serve {
        let grants = data().grants().to_string();
        let config = Serve {
            dist,
            port,
            name,
            origin,
            deadline,
            renders,
            queue,
            viewport,
            lifetime: Duration::from_secs(lifetime),
        };
        let server = match Server::bind(config, decoded, &grants) {
            Ok(server) => server,
            Err(e) => {
                eprintln!("render: serve: {e}");
                return ExitCode::FAILURE;
            }
        };
        println!("serving http://{}/", server.addr());
        let _ = std::io::Write::flush(&mut std::io::stdout());
        drain_on_signal(server.stopper());
        return match server.run(data) {
            Ok(()) => {
                println!("drained");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("render: serve: {e}");
                ExitCode::FAILURE
            }
        };
    }
    if build {
        match build_locations(&decoded) {
            Ok(found) => locations.extend(found),
            Err(e) => {
                eprintln!("render: {e}");
                return ExitCode::FAILURE;
            }
        }
        // @ref LLP 1048.000 D2 — and every page a build route's source lists.
        for row in decoded
            .routes
            .iter()
            .filter(|r| r.render == RenderPolicy::Build)
        {
            match pages(&decoded, data(), row, deadline) {
                Ok(found) => locations.extend(found.into_iter().map(|l| (l, false))),
                Err(e) => {
                    eprintln!("render: {e}");
                    return ExitCode::FAILURE;
                }
            }
        }
    } else if locations.is_empty() {
        return usage();
    }
    let site = Site {
        name: &name,
        origin: origin.as_deref(),
    };
    let mut failed = false;
    for (location, listed) in &locations {
        // The not-found document, or any location the router sends there.
        let notfound = *listed || route_at(&decoded, location).is_some_and(|r| r.notfound);
        let mut line = format!("{{\"location\":{}", json(location));
        let rendered =
            render(&decoded, data, viewport, location, &site, deadline).and_then(|rendered| {
                let page = shell
                    .as_deref()
                    .map(|shell| page(shell, &rendered))
                    .transpose()?;
                Ok((rendered, page))
            });
        match rendered {
            Ok((rendered, page)) => {
                let settled = rendered.settled == Settled::Complete;
                let status = rendered.status(notfound);
                let _ = write!(
                    line,
                    ",\"notfound\":{notfound},\"status\":{status},\"settled\":{settled},\"robots\":{}",
                    rendered
                        .document
                        .head
                        .robots
                        .as_deref()
                        .map_or_else(|| "null".into(), json)
                );
                for (field, value) in [
                    ("root", &rendered.document.root),
                    ("head", &rendered.head),
                    ("checkpoint", &rendered.checkpoint),
                    ("digest", &rendered.digest),
                ] {
                    let _ = write!(line, ",\"{field}\":{}", json(value));
                }
                if let Some(page) = &page {
                    let _ = write!(line, ",\"page\":{}", json(page));
                }
            }
            Err(error) => {
                failed = true;
                let _ = write!(line, ",\"error\":{}", json(&error));
            }
        }
        line.push('}');
        println!("{line}");
    }
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

/// SIGTERM or SIGINT drains the server (D10): a restart loses no render
/// in flight.
fn drain_on_signal(stopper: Stopper) {
    use std::sync::atomic::{AtomicBool, Ordering};
    static SIGNALLED: AtomicBool = AtomicBool::new(false);
    extern "C" fn signalled(_: libc::c_int) {
        SIGNALLED.store(true, Ordering::SeqCst);
    }
    // SAFETY: the handler only stores to an atomic, which is signal-safe.
    unsafe {
        libc::signal(libc::SIGTERM, signalled as *const () as libc::sighandler_t);
        libc::signal(libc::SIGINT, signalled as *const () as libc::sighandler_t);
    }
    let _ = std::thread::Builder::new()
        .name("exact-render-drain".into())
        .spawn(move || {
            while !SIGNALLED.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(20));
            }
            println!("draining");
            let _ = std::io::Write::flush(&mut std::io::stdout());
            stopper.stop();
        });
}

fn json(text: &str) -> String {
    serde_json::Value::String(text.into()).to_string()
}
