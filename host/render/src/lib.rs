//! The async render host (LLP 1048.000 D9).
//!
//! A render boots a fresh runner at a location with the app's own data
//! source, in an environment that holds nothing private — no store, no
//! cookies, no device capabilities ([`Anonymous`]) — and runs its requests
//! through the native executor core the Apple and Linux hosts share
//! ([`Executor`]) until the document settles or its deadline passes
//! ([`settle`]). Then `exact_web::document` projects it: the document, the
//! page's head and its checkpoint. The build runs it through [`main`]; the
//! server (LLP 1048.000 D10) will run the same render per request.
//!
//! No action runs, no timer fires and the clock stays where boot put it:
//! the completion rule ignores them. A source asking for what the
//! environment doesn't hold — SQLite, kept secrets, a grant the host
//! doesn't hold, surface work — is refused there and keeps its placeholder;
//! the checkpoint lists it and the client asks it after adoption.

#![deny(missing_docs)]

mod executor;
mod page;
mod source;

pub use executor::Executor;
pub use page::page;
pub use source::Anonymous;

use exact_kernel::Kernel;
use exact_plan::Plan;
use exact_runner::{DataSource, Dispatch, FailureKind, Outcome, RequestOut, Runner, RunnerError};
use exact_web::document::{build_locations, checkpoint, digest, project, Document, Site};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
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
    /// How the render ended.
    pub settled: Settled,
}

/// Render `plan` at `location` with `data`, waiting at most `deadline` for
/// its requests. A source whose reply it cannot shape, or a projection the
/// HTML parser would undo, is the error.
pub fn render<D: DataSource>(
    plan: &Plan,
    data: D,
    viewport: exact_runner::Viewport,
    location: &str,
    site: &Site,
    deadline: Duration,
) -> Result<Rendered, String> {
    let until = Instant::now() + deadline;
    let data = Anonymous::new(data);
    let executor = Executor::start(exact_runner::DataSource::grants(&data));
    let mut runner = Runner::boot_with_delivery(
        plan.clone(),
        data,
        Kernel::with_monospace(),
        None,
        Vec::new(),
        Default::default(),
        viewport,
        location,
    )
    .map_err(|e| format!("boot: {e:?}"))?;
    activate(&mut runner, until)?;
    let settled = settle(&mut runner, &executor, until).map_err(|e| format!("{e:?}"))?;
    // Whatever is still in flight is abandoned with the render.
    drop(executor);
    let document = project(&runner).map_err(|e| e.to_string())?;
    let head = document
        .page_head(plan, site, location)
        .map_err(|e| e.to_string())?;
    let checkpoint = checkpoint(&runner, location);
    let digest = digest(plan, location, &checkpoint, &document.root);
    Ok(Rendered {
        document,
        head,
        checkpoint,
        digest,
        settled,
    })
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
    let mut parked = BTreeMap::new();
    let mut refused = BTreeSet::new();
    loop {
        hand_out(runner, executor, &mut parked, &mut refused);
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
                    refused.insert(ticket);
                    continue;
                }
                runner.fulfill(ticket, outcome)?;
            }
            continue;
        }
        if runner
            .pending()
            .iter()
            .all(|(_, ticket)| refused.contains(ticket))
        {
            return Ok(Settled::Complete);
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
fn hand_out<D: DataSource>(
    runner: &mut Runner<D>,
    executor: &Executor,
    parked: &mut BTreeMap<u64, RequestOut>,
    refused: &mut BTreeSet<u64>,
) {
    // The runner holds no refusals in a render, so nothing fences the lane.
    executor.resume_ordered();
    for r in runner.take_requests() {
        // Device capabilities: the environment has none.
        if r.request.surface.is_some() || r.request.storage.is_some() {
            refused.insert(r.ticket);
            continue;
        }
        let dispatch = match r.request.continuation {
            Some(token) => runner.dispatch_work(token),
            None => Dispatch::Missing,
        };
        run(executor, parked, refused, r, dispatch);
    }
    for (token, dispatch) in runner.release_work() {
        if let Some(r) = parked.remove(&token) {
            run(executor, parked, refused, r, dispatch);
        }
    }
}

fn run(
    executor: &Executor,
    parked: &mut BTreeMap<u64, RequestOut>,
    refused: &mut BTreeSet<u64>,
    r: RequestOut,
    dispatch: Dispatch,
) {
    let ticket = r.ticket;
    let admitted = match dispatch {
        Dispatch::Run(work) => executor.run(r, Some(work)),
        Dispatch::Held => {
            if let Some(token) = r.request.continuation {
                parked.insert(token, r);
            }
            Ok(())
        }
        Dispatch::Host(_) | Dispatch::Missing => executor.run(r, None),
    };
    // Over the executor's limits: the client asks it.
    if admitted.is_err() {
        refused.insert(ticket);
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
/// `digest`, and with `--shell` the whole `page` ([`page`]), or `error`. `--build` renders every location the plan declares
/// `render=build` (`exact_web::document::build_locations`). `baked` is the
/// app's own plan; the web build passes the one it extracted from the
/// shipped wasm instead.
pub fn main<D: DataSource + Default>(baked: &[u8]) -> std::process::ExitCode {
    use std::process::ExitCode;
    let mut args = std::env::args().skip(1);
    let mut plan = baked.to_vec();
    let mut viewport = exact_runner::Viewport::default();
    let (mut name, mut origin) = (String::new(), None::<String>);
    let mut locations: Vec<(String, bool)> = Vec::new();
    let mut build = false;
    let mut deadline = DEADLINE;
    let mut shell = None::<String>;
    let usage = || {
        eprintln!("usage: render [--plan <app.plan>] [--viewport <w>x<h>] [--name <name>] [--origin <url>] [--deadline <ms>] [--shell <index.html>] (--build | <location>…)");
        ExitCode::from(2)
    };
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--plan" => match args.next().map(std::fs::read) {
                Some(Ok(bytes)) => plan = bytes,
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
                viewport = exact_runner::Viewport { width, height };
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
            _ if arg.starts_with('/') => locations.push((arg, false)),
            _ => return usage(),
        }
    }
    let decoded = match Plan::decode(&plan) {
        Ok(decoded) => decoded,
        Err(e) => {
            eprintln!("render: the plan: {e:?}");
            return ExitCode::FAILURE;
        }
    };
    if build {
        match build_locations(&decoded) {
            Ok(found) => locations.extend(found),
            Err(e) => {
                eprintln!("render: {e}");
                return ExitCode::FAILURE;
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
    for (location, notfound) in &locations {
        let mut line = format!("{{\"location\":{}", json(location));
        let rendered = render(&decoded, D::default(), viewport, location, &site, deadline)
            .and_then(|rendered| {
                let page = shell
                    .as_deref()
                    .map(|shell| page(shell, &rendered))
                    .transpose()?;
                Ok((rendered, page))
            });
        match rendered {
            Ok((rendered, page)) => {
                let settled = rendered.settled == Settled::Complete;
                let status = match rendered.document.head.status {
                    _ if !settled => 503,
                    Some(status) => status,
                    None if *notfound => 404,
                    None => 200,
                };
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

fn json(text: &str) -> String {
    serde_json::Value::String(text.into()).to_string()
}
