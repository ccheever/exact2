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

mod compare;
mod direct;
mod encode;
mod executor;
mod files;
mod generations;
mod linger;
mod page;
mod pages;
mod serve;
mod source;
mod stream;
mod viewport;

pub use direct::boot_swap_js;
pub use executor::Executor;
pub use page::{capture, capture_js, page, scroll_document_js};
pub use pages::pages;
pub use serve::{Serve, Server, Stopper};
pub use source::Anonymous;

// The render never lays out (the projection is style rows, not boxes): its
// kernels keep no layout engine tree unless something asks for a layout.
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
use std::sync::{Arc, Condvar, Mutex, OnceLock};
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
    /// The document's root is already as the JavaScript runtime adopts it
    /// (written so without a kernel, LLP 1048.004): the page doesn't
    /// rewrite it (`page::for_runtime`), and the digest is over this form.
    pub runtime_form: bool,
}

/// Render `plan` at `location` with a source from `data`, waiting at most
/// `deadline` for its requests. A source whose reply it cannot shape, or a
/// projection the HTML parser would undo, is the error.
pub fn render<D: DataSource + 'static>(
    plan: &Plan,
    data: impl Fn() -> D,
    viewport: exact_runner::Viewport,
    location: &str,
    site: &Site,
    deadline: Duration,
) -> Result<Rendered, String> {
    render_as(plan, data, viewport, location, site, deadline, Ids::Runtime)
}

/// Whose view ids a document must carry.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Ids {
    /// The runtime's own first tree's (the wasm runtime adopts a page whose
    /// digest, ids included, is its own): the document is projected from a
    /// runner booted from the checkpoint, as the runtime boots.
    Runtime,
    /// None the reader keeps (the JavaScript runtime adopts by tag and drops
    /// them, `page::for_runtime`): the settled runner's tree is projected
    /// when it is provably the checkpoint boot's ([`projects_as_booted`]),
    /// saving the second tree (~1.4 ms of RealWorld's ~4.5 ms CPU a page).
    Any,
}

/// Whether the tree a runner settles to projects as the tree one booted from
/// its checkpoint builds, view ids apart. Regions, rows and bindings follow
/// state; what could differ is only state fixed where a slot is first made:
/// an initializer that read a resource (or a derive, or a pending or failed
/// flag) would see the placeholder in the runner that settled and the answer
/// in a boot from the checkpoint. The compiler lets an initializer read only
/// earlier slots today; this holds the render to that if it ever changes.
pub fn projects_as_booted(plan: &Plan) -> bool {
    use exact_plan::Opcode::*;
    plan.slots.iter().all(|slot| {
        exact_runner::vm::instructions(plan.code(slot.init)).all(|i| {
            i.is_ok_and(|i| {
                !matches!(
                    i.op,
                    LoadResource | LoadDerive | PendingResource | FailedResource | PendingMutation
                )
            })
        })
    })
}

/// [`render`], with the view ids the document must carry.
pub fn render_as<D: DataSource + 'static>(
    plan: &Plan,
    data: impl Fn() -> D,
    viewport: exact_runner::Viewport,
    location: &str,
    site: &Site,
    deadline: Duration,
    ids: Ids,
) -> Result<Rendered, String> {
    render_with(
        plan,
        &data,
        viewport,
        location,
        site,
        deadline,
        ids,
        Projection::Auto,
    )
}

/// How a render writes its document (LLP 1048.004).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Projection {
    /// From the runner's instance tree, with no kernel, where the page's
    /// ids are nobody's ([`Ids::Any`]); else, or when the tree holds what
    /// the fold doesn't cover, from a kernel's nodes.
    /// `EXACT_RENDER_DIRECT=off` keeps every render on a kernel.
    Auto,
    /// From a kernel's nodes.
    Kernel,
    /// From the runner's instance tree, or the error saying why not: what
    /// the differential check compares with [`Projection::Kernel`].
    Direct,
}

/// Whether direct renders are on (`EXACT_RENDER_DIRECT=off` turns them off).
fn direct_renders() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var("EXACT_RENDER_DIRECT").as_deref() != Ok("off"))
}

/// Whether a render of `plan` under `ids` writes its document without a
/// kernel, by `projection`; `Err` for [`Projection::Direct`] when it can't.
pub(crate) fn direct_for(plan: &Plan, ids: Ids, projection: Projection) -> Result<bool, String> {
    let allowed = || {
        if ids != Ids::Any {
            return Err("the page's view ids are the wasm runtime's".to_string());
        }
        if !projects_as_booted(plan) {
            return Err("a slot's initializer reads a resource".into());
        }
        Ok(())
    };
    match projection {
        Projection::Kernel => Ok(false),
        Projection::Auto => Ok(direct_renders() && allowed().is_ok()),
        Projection::Direct => allowed().map(|()| true),
    }
}

/// [`render_as`], written as `projection` says.
#[allow(clippy::too_many_arguments)]
pub fn render_with<D: DataSource + 'static, F: Fn() -> D>(
    plan: &Plan,
    data: &F,
    viewport: exact_runner::Viewport,
    location: &str,
    site: &Site,
    deadline: Duration,
    ids: Ids,
    projection: Projection,
) -> Result<Rendered, String> {
    render_with_at(
        plan,
        data,
        viewport,
        location,
        site,
        deadline,
        ids,
        projection,
        render_time(),
    )
}

/// [`render_with`], with a fixed render timestamp in milliseconds since the
/// Unix epoch, including every initializer, resource argument and checkpoint.
#[allow(clippy::too_many_arguments)]
pub fn render_with_at<D: DataSource + 'static, F: Fn() -> D>(
    plan: &Plan,
    data: &F,
    viewport: exact_runner::Viewport,
    location: &str,
    site: &Site,
    deadline: Duration,
    ids: Ids,
    projection: Projection,
    now_ms: f64,
) -> Result<Rendered, String> {
    let direct = direct_for(plan, ids, projection)?;
    let page = settle_at(
        plan, data, viewport, location, deadline, direct, now_ms, None,
    )?;
    let settled_tree = if direct {
        match page.runner.document_tree() {
            Ok(tree) => {
                let (document, _) =
                    exact_web::document::project_tree(&tree, &page.runner, Default::default())
                        .map_err(|e| e.to_string())?;
                Some((document, tree.handlers(plan)))
            }
            // What the kernel-free fold doesn't cover, or a tree a kernel
            // would have refused: the page renders again, with a kernel,
            // which writes it or reports the refusal as ever.
            Err(why) if projection == Projection::Auto => {
                println!("render {location}: with a kernel ({why})");
                retire(page, data);
                return render_with_at(
                    plan,
                    data,
                    viewport,
                    location,
                    site,
                    deadline,
                    ids,
                    Projection::Kernel,
                    now_ms,
                );
            }
            Err(why) => {
                retire(page, data);
                return Err(why.to_string());
            }
        }
    } else if ids == Ids::Any && projects_as_booted(plan) {
        Some((
            project(&page.runner).map_err(|e| e.to_string())?,
            page.runner.handlers(),
        ))
    } else {
        None
    };
    let (checkpoint, settled) = (page.checkpoint.clone(), page.settled);
    retire(page, data);
    // @ref LLP 1048.000 D6 — the document is the checkpoint's projection,
    // from a runner booted from the page's checkpoint with a fresh source,
    // as the runtime boots: its first tree is built in one pass, so its view
    // ids are the runtime's, a pending answer's placeholder included. The
    // runner that settled built its tree as answers arrived. What the boot
    // asks is never run. A document whose ids nobody keeps is the settled
    // tree's projection where that is the same ([`Ids::Any`]).
    let state = read_checkpoint(&checkpoint).map_err(|e| format!("the checkpoint: {e}"))?;
    let (document, handlers) = match settled_tree {
        Some(settled) => settled,
        None => {
            let booted = Runner::boot_checkpoint(
                plan.clone(),
                Anonymous::new(data()),
                Kernel::with_monospace_on_demand(),
                &state,
                Vec::new(),
                Default::default(),
                viewport,
                location,
            )
            .map_err(|e| format!("boot from the checkpoint: {e:?}"))?;
            (
                project(&booted).map_err(|e| e.to_string())?,
                booted.handlers(),
            )
        }
    };
    finish(
        plan, site, location, document, handlers, checkpoint, state, settled,
    )
}

/// A runner settled at a location (LLP 1048.000 D9), its document not yet
/// written.
pub(crate) struct Settling<D: DataSource> {
    pub(crate) runner: Runner<Anonymous<D>>,
    pub(crate) settled: Settled,
    pub(crate) checkpoint: String,
}

/// Boot a fresh runner at `location` with a fresh source (LLP 1048.000 D10)
/// and run its requests until the document settles or `deadline` passes.
/// `detached`: its kernel keeps nothing, and its document is written from
/// its instance tree (LLP 1048.004).
fn render_time() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as f64
}

/// What sees a render's runner as it boots, before its first answer (a
/// route's boot document, LLP 1048.005).
pub(crate) type OnBoot<'a, D> = &'a mut dyn FnMut(&Runner<Anonymous<D>>);

#[allow(clippy::too_many_arguments)]
pub(crate) fn settle_at<D: DataSource + 'static, F: Fn() -> D>(
    plan: &Plan,
    data: &F,
    viewport: exact_runner::Viewport,
    location: &str,
    deadline: Duration,
    detached: bool,
    now_ms: f64,
    on_boot: Option<OnBoot<'_, D>>,
) -> Result<Settling<D>, String> {
    // A renderer runs any plan it is handed: every capability is linked
    // (LLP 1047 D7), so a projection never meets one it can't write.
    exact_web::link(exact_web_capabilities::ALL);
    // `EXACT_RENDER_REALMS=warm`: a worker keeps the data source (its module
    // realm, activated) of a render that settled and renders the next page
    // with it, instead of building a realm per render (LLP 1048.000 D10's
    // fresh realm). For anonymous pages only: a realm's module state (its
    // caches, counters) carries from one render to the next.
    let warm = warm_realms();
    // The last render's runner, if its worker did not drop it after sending.
    retire_renders();
    let pooled = warm
        .then(|| REALMS.with(|p| p.borrow_mut().pop()))
        .flatten()
        .and_then(|b| b.downcast::<Anonymous<D>>().ok())
        .map(|b| *b);
    // Else the realm this worker made while it was idle ([`make_realm`]):
    // nothing has run in it but the module's own initialization.
    let made = || {
        MADE.with(|m| m.borrow_mut().take())
            .and_then(|b| b.downcast::<Anonymous<D>>().ok())
            .map(|b| *b)
    };
    let settling = pooled
        .or_else(made)
        .unwrap_or_else(|| Anonymous::new(data()));
    // The deadline waits for sources, not for the transport to start. A
    // worker keeps its executor between renders that settled, so the
    // transport's connections to an origin are reused (a fresh TLS
    // connection per render cost RealWorld most of its latency, measured
    // 2026-09-28); one that hit its deadline, with work in flight, is dropped.
    let grants = exact_runner::DataSource::grants(&settling).to_string();
    let executor = EXECUTORS
        .with(|pool| {
            let mut pool = pool.borrow_mut();
            let at = pool.iter().position(|(g, _)| *g == grants)?;
            Some(pool.swap_remove(at).1)
        })
        .unwrap_or_else(|| Executor::start(&grants));
    let until = Instant::now() + deadline;
    let watchdog = Watchdog::arm(settling.interrupt(), until);
    let kernel = if detached {
        Kernel::detached()
    } else {
        Kernel::with_monospace_on_demand()
    };
    let mut runner = Runner::boot_at(plan.clone(), settling, kernel, viewport, location, now_ms)
        .map_err(|e| format!("boot: {e:?}"))?;
    // The boot document, before any answer: a route's `paint=boot`
    // (LLP 1048.005).
    if let Some(on_boot) = on_boot {
        on_boot(&runner);
    }
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
    // Whatever is still in flight is abandoned with the render; an executor
    // with nothing in flight goes back to this worker's pool.
    if matches!(settled, Settled::Deadline) {
        drop(executor);
    } else {
        executor.forget(|_| false);
        let _ = executor.drain();
        EXECUTORS.with(|pool| pool.borrow_mut().push((grants, executor)));
    }
    let checkpoint = checkpoint(&runner, location);
    Ok(Settling {
        runner,
        settled,
        checkpoint,
    })
}

/// A settled render's runner, done with: its realm kept warm
/// (`EXACT_RENDER_REALMS=warm`, a render that settled), and the runner
/// dropped once the page is sent ([`retire_renders`]), off the response's
/// path.
pub(crate) fn retire<D: DataSource + 'static, F: Fn() -> D>(page: Settling<D>, data: &F) {
    let Settling {
        mut runner,
        settled,
        ..
    } = page;
    if warm_realms() && !matches!(settled, Settled::Deadline) {
        let used = std::mem::replace(runner.data(), Anonymous::new(data()));
        REALMS.with(|p| p.borrow_mut().push(Box::new(used)));
    }
    RETIRED.with(|r| r.borrow_mut().push(Box::new(runner)));
}

/// The rendered page's head, digest and activation around its document.
#[allow(clippy::too_many_arguments)]
pub(crate) fn finish(
    plan: &Plan,
    site: &Site,
    location: &str,
    document: Document,
    handlers: exact_kernel::SortedMap<exact_kernel::ViewId, Vec<exact_plan::EventKind>>,
    checkpoint: String,
    state: exact_runner::Checkpoint,
    settled: Settled,
) -> Result<Rendered, String> {
    let head = document
        .page_head(plan, site, location)
        .map_err(|e| e.to_string())?;
    let digest = digest(&encoded(plan), location, &checkpoint, &document.root);
    let activation = activation(plan, location, &state, &handlers);
    Ok(Rendered {
        document,
        head,
        checkpoint,
        digest,
        state,
        settled,
        activate: activation,
        runtime_form: false,
    })
}

/// The page's activation (LLP 1071 D6): the route's, unless a partial
/// document or a continuous handler needs the host's ordinary boot.
pub(crate) fn activation(
    plan: &Plan,
    location: &str,
    state: &exact_runner::Checkpoint,
    handlers: &exact_kernel::SortedMap<exact_kernel::ViewId, Vec<exact_plan::EventKind>>,
) -> exact_plan::ActivatePolicy {
    let mut activation = route_at(plan, location)
        .map_or(exact_plan::ActivatePolicy::Inferred, |route| route.activate);
    // A partial document needs to finish without waiting for an action. Gesture,
    // media and other continuous handlers likewise need the host's ordinary
    // boot (undeclared: idle on the wasm host, eager on the JavaScript one);
    // interaction activation only replays discrete form and press semantics.
    if activation == exact_plan::ActivatePolicy::Interaction
        && (!state.pending.is_empty()
            || handlers.values().flatten().any(|kind| {
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
        activation = exact_plan::ActivatePolicy::Inferred;
    }
    activation
}

/// `plan`'s bytes, for the digest: encoded once per plan a process renders,
/// not per page (57 KB for RealWorld's).
fn encoded(plan: &Plan) -> std::sync::Arc<Vec<u8>> {
    static KNOWN: Mutex<Vec<(Plan, std::sync::Arc<Vec<u8>>)>> = Mutex::new(Vec::new());
    let mut known = KNOWN
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some((_, bytes)) = known.iter().find(|(p, _)| p == plan) {
        return std::sync::Arc::clone(bytes);
    }
    let bytes = std::sync::Arc::new(plan.encode());
    if known.len() == 4 {
        known.remove(0);
    }
    known.push((plan.clone(), std::sync::Arc::clone(&bytes)));
    bytes
}

thread_local! {
    /// Each render worker's settled executors, by grants.
    static EXECUTORS: std::cell::RefCell<Vec<(String, Executor)>> = const { std::cell::RefCell::new(Vec::new()) };
    /// Each render worker's warm data sources (`EXACT_RENDER_REALMS=warm`).
    static REALMS: std::cell::RefCell<Vec<Box<dyn std::any::Any>>> = const { std::cell::RefCell::new(Vec::new()) };
    /// Each render worker's realm made ahead, never used ([`make_realm`]).
    static MADE: std::cell::RefCell<Option<Box<dyn std::any::Any>>> = const { std::cell::RefCell::new(None) };
    /// Each render worker's finished runners, dropped after the page is sent.
    static RETIRED: std::cell::RefCell<Vec<Box<dyn std::any::Any>>> = const { std::cell::RefCell::new(Vec::new()) };
}

fn warm_realms() -> bool {
    static WARM: OnceLock<bool> = OnceLock::new();
    *WARM.get_or_init(|| std::env::var("EXACT_RENDER_REALMS").as_deref() == Ok("warm"))
}

/// Drop this worker's finished renders: their runners and module realms.
pub(crate) fn retire_renders() {
    let retired = RETIRED.with(|r| std::mem::take(&mut *r.borrow_mut()));
    drop(retired);
}

/// Make this worker's next render's module realm now (LLP 1048.000 D10's
/// fresh realm, made ahead): a new source from `data`, loaded, which only
/// the next render on this thread takes, once. A worker calls it when it is
/// idle and nothing waits for it, and keeps at most one: the realm is made
/// on the thread that will use it (an engine never crosses a thread), and
/// every realm made is one a render would have made, so the CPU is moved
/// off the request's path, not added. A source that cannot load now is
/// left for the render to make, and to report.
pub(crate) fn make_realm<D: DataSource + 'static>(data: fn() -> D) {
    if warm_realms() || MADE.with(|m| m.borrow().is_some()) {
        return;
    }
    let mut source = Anonymous::new(data());
    if matches!(source.preload(), Ok(true)) && source.activate().is_ok() {
        MADE.with(|m| *m.borrow_mut() = Some(Box::new(source)));
    }
}

/// The deadline, for a source call still running then (LLP 1048.000 D10):
/// the source's interrupt triggers at `until`, unless the render finished
/// first. A source without one runs its calls to the end. One thread keeps
/// every render's deadline ([`Deadlines`]); a render arms and disarms an
/// entry, never a thread of its own (a thread per render was ~3% of
/// RealWorld's CPU a page, measured 2026-09-29).
struct Watchdog {
    armed: Option<u64>,
    fired: Arc<AtomicBool>,
}

impl Watchdog {
    fn arm(interrupt: Option<Interrupt>, until: Instant) -> Watchdog {
        let fired = Arc::new(AtomicBool::new(false));
        let armed =
            interrupt.map(|interrupt| Deadlines::get().arm(until, interrupt, fired.clone()));
        Watchdog { armed, fired }
    }

    fn fired(&self) -> bool {
        self.fired.load(Ordering::SeqCst)
    }
}

impl Drop for Watchdog {
    /// Disarmed, its interrupt never triggers: an entry fires under the
    /// same lock that removes it.
    fn drop(&mut self) {
        if let Some(id) = self.armed.take() {
            Deadlines::get().disarm(id);
        }
    }
}

/// The renders' armed deadlines and the one thread that fires them.
struct Deadlines {
    armed: Mutex<(u64, Vec<Deadline>)>,
    changed: Condvar,
}

struct Deadline {
    id: u64,
    until: Instant,
    interrupt: Interrupt,
    fired: Arc<AtomicBool>,
}

impl Deadlines {
    fn get() -> &'static Deadlines {
        static DEADLINES: OnceLock<&'static Deadlines> = OnceLock::new();
        DEADLINES.get_or_init(|| {
            let deadlines: &'static Deadlines = Box::leak(Box::new(Deadlines {
                armed: Mutex::new((0, Vec::new())),
                changed: Condvar::new(),
            }));
            std::thread::Builder::new()
                .name("exact-render-deadline".into())
                .spawn(move || deadlines.run())
                .expect("the deadline thread starts");
            deadlines
        })
    }

    fn arm(&self, until: Instant, interrupt: Interrupt, fired: Arc<AtomicBool>) -> u64 {
        let mut armed = self.armed.lock().unwrap();
        armed.0 += 1;
        let id = armed.0;
        armed.1.push(Deadline {
            id,
            until,
            interrupt,
            fired,
        });
        self.changed.notify_one();
        id
    }

    fn disarm(&self, id: u64) {
        self.armed.lock().unwrap().1.retain(|d| d.id != id);
    }

    fn run(&self) {
        let mut armed = self.armed.lock().unwrap();
        loop {
            let now = Instant::now();
            armed.1.retain(|d| {
                if d.until > now {
                    return true;
                }
                d.fired.store(true, Ordering::SeqCst);
                d.interrupt.trigger();
                false
            });
            armed = match armed.1.iter().map(|d| d.until).min() {
                Some(next) => self.changed.wait_timeout(armed, next - now).unwrap().0,
                None => self.changed.wait(armed).unwrap(),
            };
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
            for (ticket, outcome, elapsed_ms) in outcomes {
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
                runner.fulfill_measured(ticket, outcome, elapsed_ms)?;
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
/// `--compare` renders each location with a kernel and without one (LLP
/// 1048.004 D6) and prints `same`, or the first difference (`differs`), or
/// why the page has no kernel-free render (`direct`) — with `--shell` the
/// streamed page too; any difference fails the run.
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
/// <s>] [--generations <dir>]` is the server instead ([`Server`]): the built
/// web app on loopback, its plan the dist's own `app.plan` unless `--plan`
/// names one; `--generations` keeps the builds of `app.wasm` it serves, to
/// send the next as a delta ([`Serve::generations`]).
/// It prints `serving http://127.0.0.1:<port>/`, then a line per render.
pub fn main<D: DataSource + 'static>(baked: &[u8], data: fn() -> D) -> std::process::ExitCode {
    use std::process::ExitCode;
    let mut args = std::env::args().skip(1);
    let mut plan = baked.to_vec();
    let mut viewport = exact_runner::Viewport::default();
    let (mut name, mut origin) = (String::new(), None::<String>);
    let mut locations: Vec<(String, bool)> = Vec::new();
    let mut build = false;
    let mut compare = false;
    let mut deadline = DEADLINE;
    let mut shell = None::<String>;
    let (mut serve, mut planned) = (None::<std::path::PathBuf>, false);
    let mut generations = None::<std::path::PathBuf>;
    let (mut port, mut renders, mut queue, mut lifetime) = (0u16, 4usize, 32usize, 60u64);
    let usage = || {
        eprintln!("usage: render [--plan <app.plan>] [--viewport <w>x<h>] [--name <name>] [--origin <url>] [--deadline <ms>] ([--shell <index.html>] [--compare] (--build | <location>…) | --serve <dist> [--port <n>] [--renders <n>] [--queue <n>] [--lifetime <s>] [--generations <dir>])");
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
            "--compare" => compare = true,
            "--serve" => match args.next() {
                Some(dist) => serve = Some(dist.into()),
                None => return usage(),
            },
            "--generations" => match args.next() {
                Some(dir) => generations = Some(dir.into()),
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
            generations,
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
        if compare {
            let fields = compare::location(
                &decoded,
                &data,
                viewport,
                location,
                &site,
                deadline,
                shell.as_deref(),
            );
            failed |= fields.starts_with(",\"same\":false");
            line.push_str(&fields);
            line.push('}');
            println!("{line}");
            continue;
        }
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

#[cfg(test)]
mod realm_tests {
    use super::*;
    use exact_runner::{Answer, DataError, Store};
    use std::sync::atomic::AtomicUsize;

    static MADE_SOURCES: AtomicUsize = AtomicUsize::new(0);

    /// A source that names itself: its number, in the order made.
    struct Counted(usize);

    fn counted() -> Counted {
        Counted(MADE_SOURCES.fetch_add(1, Ordering::SeqCst))
    }

    impl DataSource for Counted {
        fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
            Err(DataError::UnknownSource(source.into()))
        }
        fn answer(
            &mut self,
            _: &mut Store,
            source: &str,
            _: &[Value],
        ) -> Result<Answer, DataError> {
            let n = format!("source-{}", self.0);
            match source {
                "post" | "emptyPost" => Ok(Answer::Now(Value::record(vec![
                    Value::str(&n),
                    Value::str(&n),
                ]))),
                "comments" => Ok(Answer::Now(Value::list(vec![]))),
                other => Err(DataError::UnknownSource(other.into())),
            }
        }
    }

    use exact_plan::Value;

    /// D10: a realm made ahead serves the next render, and only that one.
    #[test]
    fn a_realm_made_ahead_serves_one_render() {
        let src = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../contract/corpus/placeholder.contract"),
        )
        .unwrap();
        let plan = contract::compile(&src).unwrap();
        let site = Site {
            name: "Blog",
            origin: None,
        };
        let render = || {
            render(
                &plan,
                counted,
                Default::default(),
                "/post/7",
                &site,
                DEADLINE,
            )
            .unwrap()
            .document
            .root
        };
        make_realm(counted);
        let made = MADE_SOURCES.load(Ordering::SeqCst) - 1;
        let first = render();
        assert!(first.contains(&format!(">source-{made}<")), "{first}");
        let second = render();
        assert!(!second.contains(&format!(">source-{made}<")), "{second}");
        assert!(MADE.with(|m| m.borrow().is_none()));
        retire_renders();
        assert!(RETIRED.with(|r| r.borrow().is_empty()));
    }
}
