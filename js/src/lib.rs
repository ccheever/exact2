//! The TypeScript data-source executor (LLP 1027).
//!
//! @ref LLP 1027 D1a (`fetch` over the host's ticket path) / D2 (marshaling
//! by the plan's shapes) / D3 (the executor) / D4 (after first pixel) / D10
//! (what the module can use)
//!
//! An app's `app.ts`, bundled and compiled to Hermes bytecode at bake, is a
//! [`Module`]: a [`DataSource`] like any Rust data crate, behind the same
//! seam, answering the same `resource` and `send` declarations. The lean
//! Hermes VM runs it — bytecode only; it cannot be handed source — through
//! about two hundred lines of C++ (`shim.cc`), and the plan's `sources` table
//! says what every argument and every answer looks like, so records cross as
//! objects keyed by their declared field names and nothing is guessed.
//!
//! The module's identity and grants are the bake's outputs beside the
//! bytecode, so a host reads them at boot without an engine; the engine is
//! created by [`Module::load`], which a host calls after its first pixel
//! (D4). Until then every answer is `Unavailable`, by name.
//!
//! **The seam, from the module's side (ABI 1).** `exact.abi` is `1`;
//! `exact.appId` and `exact.grants` are strings; `exact.answer(source, args,
//! store)` returns the answer's value — or a `Promise` of it, when it awaited
//! `fetch` — and throws for an error (an object with `kind` of
//! `UnknownSource`, `BadArguments`, or `Unavailable` and a `message`; any
//! other throw is `Unavailable`). `fetch(url, init)` is the web's, over the
//! host's ticket path: the module describes, the host runs under the grants,
//! the Promise resolves to a `Response` with `status`, `ok`, `headers`,
//! `text()`, `json()`, `arrayBuffer()`. Liveness is the module's, as a
//! browser's event loop has it (LLP 1027.003.000 §13, option 1; hn-reader
//! F7): an answer that awaits a promise another answer started (a fetch
//! memoized across answers, a queue behind another's storage) waits while
//! the module has work outstanding and is asked again after each delivery;
//! one that waits with nothing outstanding is refused as pending on nothing.
//! A worker-placed source runs one turn to its end, so it still refuses one.
//! `store` is `{get, set, forget}` over the runner's [`Store`]: reads
//! counted, writes grant-checked, in Rust. `console` reaches the runner's
//! logs. Time and random seeds are ordinary source arguments (LLP
//! 1027.000): ambient Date/Math.random reads and Intl.DateTimeFormat
//! formatting without an explicit timestamp refuse, including at module
//! initialization and after await. Explicit-value Date construction and UTC
//! arithmetic remain available. There are no timers.
//!
//! **Interrupts (LLP 1048.000 D10).** Another thread may stop a running call
//! through [`DataSource::interrupt`]'s handle: the bake compiles with async
//! break checks, so Hermes stops at the next loop iteration or call, and the
//! call is refused as `Unavailable`. The per-call budget is still measured
//! after a call returns; an interrupt is what ends one that doesn't.

#![deny(missing_docs)]

mod background;
mod crypto;
mod door;
mod engine;
mod native;
mod paired;
mod pure;
mod source;
mod storage;
mod turns;
mod watch;
mod wire;

pub use engine::ENGINE_LINKED;
pub use exact_data::Placed;
pub use exact_js_value::{from_json, to_json, Shape};
pub use exact_runner::Placement;
pub use native::{Changed, LaterHandler, NativeModule, NativeReply};
pub use paired::Paired;

use door::{c_string, host_door};
use engine::{Engine, HostFn};
use exact_plan::{Plan, Value};
use exact_runner::{
    Answer, DataError, DataSource, Dispatch, InFlight, Interrupt, Outcome, Request, Store, Target,
    Work,
};
use serde_json::Value as Json;
use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::Arc;
use std::time::Instant;
use watch::{Watch, Watched};
use wire::outcome_to_json;

type NativeFactory = fn(&str) -> Box<dyn NativeModule>;

/// The seam ABI this executor speaks. ABI 2 adds Canvas 2D's `draw` and
/// `surfaces` (LLP 1056 D1); a module without them still reports 1, and
/// this executor runs both.
pub const ABI: u32 = 2;

/// Whether a module's `exact.abi` is one this executor runs.
pub fn abi_supported(abi: &str) -> bool {
    matches!(abi, "1" | "2")
}
/// Authoritative Ibex2 storage declarations included by the TypeScript bake.
pub const STORAGE_TYPES: &str = ibex2::bindings::TYPESCRIPT;
/// The per-call wall-clock budget a module is held to, unless the host says otherwise.
pub const DEFAULT_BUDGET_MS: f64 = 100.0;
/// The runtime's heap ceiling, unless the host says otherwise.
pub const DEFAULT_MAX_HEAP: u32 = 64 << 20;

#[cfg(exact_js_engine)]
const PRELUDE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/prelude.hbc"));
#[cfg(not(exact_js_engine))]
const PRELUDE: &[u8] = &[];

/// Bytecode version compiled into this executor's prelude, available without
/// creating an engine. Zero means this build has no native executor.
pub const BYTECODE_VERSION: u32 = if PRELUDE.len() >= 12 {
    u32::from_le_bytes([PRELUDE[8], PRELUDE[9], PRELUDE[10], PRELUDE[11]])
} else {
    0
};

/// Receipt-bound archive identity of this executor's pinned lean engine.
/// The producer compares its selected Ibex bundle before compiling app bytecode.
/// The explicit refusing stub has no identity.
pub const ENGINE_INPUTS: Option<&str> = hermes_lean_sys::LEAN_ENGINE_DIGEST;

/// One source's signature, from the plan.
struct Sig {
    params: Vec<Shape>,
    result: Shape,
}

/// A parked answer's key: the runner's target when it named one, then the
/// source and its arguments. The runner keeps one request in flight per
/// target, so two targets asking one source with equal arguments are two
/// calls; a new call on the same key replaces the old one.
type Key = (Option<Target>, String, Vec<u8>);

/// An answer that awaited a fetch: the prelude's call id, and the fetch
/// ticket the runner's request stands for.
struct Parked {
    call: u64,
    ticket: u64,
    work_taken: bool,
    /// The module's progress when it parked: an answer waiting on another
    /// answer's work is asked again once something has landed since.
    progress: u64,
    /// Asked again with nothing outstanding in the module: its last settle.
    last: bool,
}

/// The ticket of an answer that awaits another answer's work (a fetch it
/// shares, a queue behind another's storage): it waits while the module has
/// work outstanding, and is asked again after each delivery (LLP
/// 1027.003.000 §13, the module-wide rule; hn-reader F7).
const WAITING: u64 = u64::MAX - 1;

/// What the host door reaches during one call: the store the seam handed
/// `answer` or `parse` (none at bake — an empty store that refuses writes),
/// and the requests `fetch` recorded, by the prelude's ticket. Boxed for the
/// engine's lifetime; the shim holds a pointer to it.
#[derive(Default)]
struct HostState {
    store: Option<*mut Store>,
    requests: Vec<(u64, Request)>,
    native: Option<Box<dyn NativeModule>>,
    /// The native module takes long calls off this thread (`native.later`).
    later: bool,
    /// The host's app module answers this source's long calls (LLP 1067.000
    /// Q6): `native` is available, and `later` goes to it.
    hosted: bool,
    /// The host's app module's `native.call`, when it answers one (D9).
    hosted_call: Option<exact_runner::NativeCall>,
    /// The canvas a draw in progress draws: its text engine and images
    /// (LLP 1056 D8, D9), for the recorder's `measureText` and `drawImage`.
    canvas: Option<exact_runner::exact_canvas::Env>,
    /// Under the agent, the repeatable stream `crypto` draws from instead
    /// of the OS (LLP 1069.005 D2b); this instance's, from its start.
    agent: Option<exact_data::crypto::AgentStream>,
    /// `CryptoKey`s by handle, never on the heap (LLP 1069.005 D1b).
    keys: Vec<exact_data::crypto::EcKey>,
    /// `authCallback()`: this native build's, from the grants (LLP 1069.006).
    auth_callback: Option<String>,
    /// The engine has app storage: the host configured directories.
    storage: bool,
    /// The engine reaches the documents the person chose (`doc:`), with or
    /// without app storage: the grants name them (LLP 1069.010 D1).
    documents: bool,
    /// The bake's module ([`Module::inspect`]): storage refuses as `bake`.
    baking: bool,
    /// The runtime's own journal lines since the last take (LLP 1097 D8).
    journal: Vec<String>,
    /// Delivering between answers (a background round, a let-go call's
    /// steps): there is no store, and storage is not refused as at bake.
    between_answers: bool,
}

/// A TypeScript data source: bytecode, its bake-time identity, and the
/// engine that runs it once loaded.
pub struct Module {
    bytecode: Vec<u8>,
    app_id: String,
    grants: String,
    revision: std::sync::OnceLock<String>, // the bytecode's SHA-256, when first asked
    engine: Option<Watched>,
    /// What an interrupt from another thread reaches; a built worker
    /// instance shares its template's.
    watch: Arc<Watch>,
    storage: Option<storage::Session>,
    directories: Option<storage::Directories>,
    host: Box<HostState>,
    native_factory: Option<NativeFactory>,
    /// Where the host sends `native.later` requests; shared with an instance
    /// built from this template, which fills it on its owner.
    native_slot: exact_runner::Native,
    /// The bound plan, kept so an owner thread can bind its own instance.
    plan: Option<Plan>,
    sigs: HashMap<String, Sig>,
    parked: Vec<(Key, Parked)>,
    /// Stream answers (LLP 1016.000): each message is mapped by the call's
    /// `exactStream`, never resumed; forgetting the ticket ends the call.
    streams: Vec<(Key, Parked)>,
    /// Answers waiting on another answer's work whose dispatch was held.
    waiters: Vec<u64>,
    /// Deliveries and new answers so far: what a waiting answer waits for.
    progress: u64,
    budget_ms: f64,
    max_heap: u32,
    logs: Vec<String>,
    overruns: u32,
    /// The Canvas 2D roster the bake read (LLP 1056 D1), known before the
    /// engine loads.
    canvas_surfaces: Vec<(String, usize)>,
    /// The agent's launch seed, when the agent drives this process (LLP
    /// 1069.005 D2b); a built worker instance takes its template's.
    agent_seed: Option<u64>,
    /// Runs inline, on the runner's thread (LLP 1097 D6): storage an answer
    /// did not await moves to the background. An instance a worker owner
    /// builds runs one turn to its end, and keeps its answers waiting.
    main_thread: bool,
    background: background::Background,
}

impl std::fmt::Debug for Module {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Module")
            .field("app_id", &self.app_id)
            .field("bytecode", &self.bytecode.len())
            .field("loaded", &self.engine.is_some())
            .field("sources", &self.sigs.len())
            .field("parked", &self.parked.len())
            .finish()
    }
}

/// One step of a call as the prelude reports it.
enum Step {
    Done(Result<Value, DataError>),
    Pending { call: u64, ticket: u64 },
}

impl Module {
    /// A module, unloaded: `app_id` and `grants` are what the bake wrote
    /// beside the bytecode, cross-checked against the module's own exports
    /// at [`Module::load`]. Under the agent (`EXACT_AGENT=1`) its `crypto`
    /// draws the agent's repeatable stream (LLP 1069.005 D2b).
    pub fn new(bytecode: Vec<u8>, app_id: impl Into<String>, grants: impl Into<String>) -> Module {
        let grants = grants.into();
        let auth_callback = exact_runner::auth::carrier_callback(&grants, None);
        let module = Module {
            revision: std::sync::OnceLock::new(),
            bytecode,
            app_id: app_id.into(),
            grants,
            engine: None,
            watch: Arc::default(),
            storage: None,
            directories: None,
            host: Box::new(HostState {
                auth_callback,
                ..HostState::default()
            }),
            native_factory: None,
            native_slot: Default::default(),
            plan: None,
            sigs: HashMap::new(),
            parked: Vec::new(),
            streams: Vec::new(),
            waiters: Vec::new(),
            progress: 0,
            budget_ms: DEFAULT_BUDGET_MS,
            max_heap: DEFAULT_MAX_HEAP,
            logs: Vec::new(),
            overruns: 0,
            canvas_surfaces: Vec::new(),
            agent_seed: None,
            main_thread: true,
            background: Default::default(),
        };
        module.with_agent_seed(exact_data::crypto::AgentStream::agent_seed())
    }

    /// This module with `seed` as the agent's launch seed, or `None` for
    /// OS entropy whatever the environment says (LLP 1069.005 D2b): what
    /// [`Module::new`] reads from `EXACT_AGENT` and `EXACT_AGENT_SEED`,
    /// stated. The stream starts over.
    pub fn with_agent_seed(mut self, seed: Option<u64>) -> Module {
        self.agent_seed = seed;
        self.host.agent = seed.map(|s| exact_data::crypto::AgentStream::new(s, "typescript"));
        self
    }

    /// This module's Canvas 2D roster, as the bake recorded it beside the
    /// bytecode (`module.rs`'s `CANVAS_SURFACES`).
    pub fn with_canvas_surfaces(mut self, surfaces: &[(&str, usize)]) -> Self {
        self.canvas_surfaces = surfaces.iter().map(|(n, a)| (n.to_string(), *a)).collect();
        self
    }

    /// The Canvas 2D roster: name and arity.
    pub fn canvas_roster(&self) -> &[(String, usize)] {
        &self.canvas_surfaces
    }

    /// Attach this app's separately linked native implementation. The factory
    /// runs only on activation with host-selected storage directories. A
    /// replacement receives a fresh instance; validation receives none.
    pub fn with_native(mut self, factory: fn(&str) -> Box<dyn NativeModule>) -> Self {
        self.native_factory = Some(factory);
        self
    }

    /// This module, placed (LLP 1027.002 D1): `Main` is this module as it
    /// is; `Worker` builds an instance of it on a host-owned thread at
    /// activation, from what this one knows, and this one stays as the
    /// template. The engine never crosses a thread.
    pub fn placed(self, placement: Placement) -> Placed<Module> {
        Placed::built(self, placement, Module::build)
    }

    /// What an owner thread needs to build this module there (LLP 1027.002
    /// D2): its bytecode, identity, directories, plan and limits — all of it
    /// `Send`; the runtime is created, run and destroyed on the owner.
    fn build(template: &Module) -> exact_data::placed::Obtain<Module> {
        let bytecode = template.bytecode.clone();
        let app_id = template.app_id.clone();
        let grants = template.grants.clone();
        let directories = template.directories.clone();
        let native_factory = template.native_factory;
        let plan = template.plan.as_ref().map(Plan::encode);
        let budget_ms = template.budget_ms;
        let max_heap = template.max_heap;
        let agent_seed = template.agent_seed;
        let watch = template.watch.clone();
        let native_slot = template.native_slot.clone();
        Box::new(move || {
            let mut module = Module::new(bytecode, app_id, grants).with_agent_seed(agent_seed);
            // Before `activate`, which loads: a worker's answers wait for
            // their storage (LLP 1097 D6).
            module.main_thread = false;
            // The template's interrupt reaches the instance on its owner, and
            // its native handle finds the instance's long-call handler.
            module.watch = watch;
            module.native_slot = native_slot;
            if let Some(factory) = native_factory {
                module = module.with_native(factory);
            }
            module.set_budget_ms(budget_ms);
            module.set_max_heap(max_heap);
            if let Some(paths) = directories {
                module.configure_storage(paths.data, paths.cache, paths.temporary)?;
            }
            if let Some(bytes) = plan {
                let plan = Plan::decode(&bytes).map_err(|e| {
                    DataError::Unavailable(format!("the plan did not cross to the owner: {e:?}"))
                })?;
                module.bind(&plan);
            }
            module.activate()?;
            Ok(module)
        })
    }

    /// A module loaded at once — the bake's and a test's shape; a host loads
    /// after its first pixel instead (LLP 1027 D4).
    pub fn loaded(
        bytecode: Vec<u8>,
        app_id: impl Into<String>,
        grants: impl Into<String>,
    ) -> Result<Module, String> {
        let mut m = Module::new(bytecode, app_id, grants);
        m.load()?;
        Ok(m)
    }

    /// Create the runtime, evaluate the prelude and the bytecode, and check
    /// that the module speaks this ABI and is the app the bake said it is.
    /// Idempotent.
    pub fn load(&mut self) -> Result<(), String> {
        if self.engine.is_some() {
            return Ok(());
        }
        let engine = self.load_engine().map_err(|error| {
            if self.watch.take() {
                "exact-js: the module was interrupted while it loaded".to_string()
            } else {
                error
            }
        })?;
        let app_id = engine.string("appId")?;
        if app_id != self.app_id {
            return Err(format!(
                "exact-js: the module says it is `{app_id}`; the bake said `{}`",
                self.app_id
            ));
        }
        if engine.string("grants")?.trim() != self.grants.trim() {
            return Err("exact-js: the module's grants differ from what the bake recorded".into());
        }
        self.engine = Some(engine);
        Ok(())
    }

    /// Build-time inspection: evaluate bytecode in a private engine and read
    /// its identity/grants. Hosts must use `new`/`loaded` with admitted metadata
    /// instead; discovering a grant does not authorize it on a device.
    pub fn inspect(bytecode: Vec<u8>) -> Result<Module, String> {
        // The bake's module never draws the agent's stream (LLP 1069.005 D2b).
        let mut module = Self::new(bytecode, "", "").with_agent_seed(None);
        module.host.baking = true;
        let engine = module.load_engine()?;
        module.app_id = engine.string("appId")?;
        module.grants = engine.string("grants")?;
        // The Canvas 2D roster (LLP 1056 D1): `{name: arity}`, read at build.
        let roster = engine.string("surfacesJson")?;
        if !roster.is_empty() {
            let json: Json = serde_json::from_str(&roster)
                .map_err(|e| format!("exact-js: app.ts `surfaces`: {e}"))?;
            let object = json
                .as_object()
                .ok_or("exact-js: app.ts `surfaces` must map names to arities")?;
            for (name, arity) in object {
                let arity = arity
                    .as_u64()
                    .ok_or_else(|| format!("exact-js: surface `{name}`'s arity is not a count"))?;
                module.canvas_surfaces.push((name.clone(), arity as usize));
            }
        }
        if module.app_id.is_empty() {
            return Err("exact-js: the module exports no appId".into());
        }
        // A grant a device would refuse refuses the build instead: a native
        // host that cannot parse the grants holds none of them.
        ibex2::grant::GrantSet::parse(&exact_runner::io_grants(&module.grants))
            .map_err(|e| format!("exact-js: app.ts `grants`: {e}"))?;
        module.engine = Some(engine);
        Ok(module)
    }

    fn load_engine(&mut self) -> Result<Watched, String> {
        let io = exact_runner::io_grants(&self.grants);
        self.host.documents = storage::reaches_documents(&io);
        // The bindings Context must precede the engine and outlive its
        // adapter. Module declares `engine` before `storage`, and unload takes
        // the engine first, preserving that order on every path.
        self.storage = Some(storage::Session::open(&io)?);
        let ctx = &mut *self.host as *mut HostState as *mut c_void;
        let host: HostFn = host_door;
        let bytes: engine::BytesFn = crypto::bytes_door;
        let engine = Engine::new(
            self.max_heap,
            host,
            bytes,
            ctx,
            &self.storage.as_ref().expect("Context was created").context,
        )
        .map_err(|e| format!("exact-js: {e}"))?;
        // Reachable before anything runs in it: module initialization is
        // application code too.
        let mut engine = Watched::new(engine, self.watch.clone());
        engine
            .load(PRELUDE)
            .map_err(|e| format!("exact-js: the prelude did not load: {e}"))?;
        if self.main_thread {
            engine
                .call("__exact_main_thread", ["", "", ""])
                .map_err(|e| format!("exact-js: the prelude did not load: {e}"))?;
        }
        self.host.storage = self.directories.is_some();
        if let Some(paths) = &self.directories {
            if let Some(factory) = self.native_factory {
                let mut native = factory(&self.grants);
                native.configure_storage(
                    paths.data.clone(),
                    paths.cache.clone(),
                    paths.temporary.clone(),
                )?;
                let slot = self.native_slot.clone();
                native.changes(Arc::new(move |topic: &str| slot.changed(topic)));
                let later = native.later();
                self.host.later = later.is_some();
                self.native_slot
                    .set(later.map(|handler| -> exact_runner::NativeHandler {
                        std::sync::Arc::new(move |body: Vec<u8>, reply| {
                            let reply = NativeReply::new(reply);
                            match serde_json::from_slice(&body) {
                                Ok(request) => handler(request, reply),
                                Err(e) => reply.send(Err(format!("native.later: {e}"))),
                            }
                        })
                    }));
                self.host.native = Some(native);
            }
            self.storage
                .as_ref()
                .expect("Context was created")
                .configure(paths)?;
        }
        if self.directories.is_some() || (self.host.documents && !self.host.baking) {
            engine.install_storage()?;
        }
        // The host's app module, when the app links no native module of its
        // own: present in agent mode too, where it substitutes its input
        // (LLP 1067.000 Q7), so it needs no storage directories.
        if self.host.native.is_none() && self.native_slot.hosted() {
            self.host.hosted = true;
            self.host.later = true;
            self.host.hosted_call = self.native_slot.hosted_call();
        }
        engine
            .harden()
            .map_err(|e| format!("exact-js: hardening failed: {e}"))?;
        engine
            .load(&self.bytecode)
            .map_err(|e| format!("exact-js: the module did not load: {e}"))?;
        let abi = engine.string("abi")?;
        if !abi_supported(&abi) {
            return Err(format!(
                "exact-js: the module speaks ABI {abi:?}; this executor speaks {ABI}"
            ));
        }
        Ok(engine)
    }

    /// Drop the runtime; answers are `Unavailable` until the next
    /// [`Module::load`], and every answer in flight is forgotten. Already-started
    /// external effects may finish; unloading waits only for the module's
    /// own storage, a second at most.
    pub fn unload(&mut self) {
        // A dev restart or a reload replaces the module: what it started
        // and did not await is finished first, within a second (LLP 1097
        // D10). A worker's answers waited for theirs.
        self.finish_background(std::time::Duration::from_secs(1));
        if let Some(mut engine) = self.engine.take() {
            self.logs.extend(engine.take_log());
        }
        self.storage = None;
        self.host.native = None;
        self.host.later = false;
        self.host.hosted = false;
        self.host.hosted_call = None;
        self.native_slot.set(None);
        self.parked.clear();
        self.host.requests.clear();
        self.background = Default::default();
    }

    /// Whether an engine is up.
    pub fn is_loaded(&self) -> bool {
        self.engine.is_some()
    }

    /// The per-call budget in milliseconds; a call over it is `Unavailable`
    /// and the resource keeps its last value.
    pub fn set_budget_ms(&mut self, ms: f64) {
        self.budget_ms = ms;
    }

    /// The heap ceiling for the next [`Module::load`].
    pub fn set_max_heap(&mut self, bytes: u32) {
        self.max_heap = bytes;
    }

    /// Calls that ran over the budget so far.
    pub fn overruns(&self) -> u32 {
        self.overruns
    }

    /// The module's `console` lines since the last take — for the runner's
    /// `logs` (LLP 1012).
    pub fn take_logs(&mut self) -> Vec<String> {
        if let Some(engine) = self.engine.as_mut() {
            self.logs.extend(engine.take_log());
        }
        std::mem::take(&mut self.logs)
    }

    /// The source names the bound plan declares, in no particular order.
    pub fn sources(&self) -> Vec<&str> {
        self.sigs.keys().map(String::as_str).collect()
    }

    /// Answers awaiting a fetch the host has yet to fulfil.
    pub fn in_flight(&self) -> usize {
        self.parked.len() + self.streams.len()
    }

    /// Decode once, retaining metadata for async dispatch and the typed answer
    /// for settlement. Captured replies retain their JSON restoration path.
    fn decode_reply(
        sig: &Sig,
        engine: &mut Engine,
        source: &str,
        text: &str,
    ) -> Result<exact_js_value::Reply, DataError> {
        // Captured large strings still use path restoration into JSON. Ordinary
        // answers decode directly to Value, without a second full value tree.
        let captured = engine.has_reply_strings();
        let decoded = if captured {
            serde_json::from_str(text)
                .map(|fields| exact_js_value::Reply {
                    fields,
                    value: Ok(Value::Unit),
                })
                .map_err(|e| e.to_string())
        } else {
            exact_js_value::reply_from_json_text(text, &sig.result).map_err(|e| e.to_string())
        };
        decoded.map_err(|e| {
            engine.clear_reply();
            DataError::Unavailable(format!(
                "`{source}` answered something other than JSON: {e}"
            ))
        })
    }

    fn step(
        sig: &Sig,
        engine: &mut Engine,
        source: &str,
        decoded: exact_js_value::Reply,
        baking: bool,
    ) -> Step {
        let exact_js_value::Reply {
            fields: mut reply,
            mut value,
        } = decoded;
        if engine.has_reply_strings() {
            if let Err(error) = engine.restore_reply(&mut reply) {
                return Step::Done(Err(DataError::Unavailable(format!(
                    "`{source}` answered outside its shape: {error}"
                ))));
            }
            value = from_json(reply.get("value").unwrap_or(&Json::Null), &sig.result);
        }
        let num = |k: &str| reply.get(k).and_then(Json::as_u64);
        match num("tag") {
            Some(0) => Step::Done(value.map_err(|e| {
                DataError::Unavailable(format!("`{source}` answered outside its shape: {e}"))
            })),
            Some(1) => match (num("call"), num("ticket")) {
                (Some(call), Some(0)) if reply.get("waiting") == Some(&Json::Bool(true)) => {
                    Step::Pending {
                        call,
                        ticket: WAITING,
                    }
                }
                (Some(call), Some(ticket)) => Step::Pending { call, ticket },
                _ => Step::Done(Err(DataError::Unavailable(format!(
                    "`{source}` is pending on no ticket"
                )))),
            },
            Some(2) => {
                let message = reply
                    .get("message")
                    .and_then(Json::as_str)
                    .unwrap_or("")
                    .to_string();
                Step::Done(Err(match reply.get("kind").and_then(Json::as_str) {
                    Some("UnknownSource") => DataError::UnknownSource(message),
                    Some("BadArguments") => DataError::BadArguments(message),
                    Some("Unavailable")
                        if baking && reply.get("code").and_then(Json::as_str) == Some("bake") =>
                    {
                        DataError::DeferredAtBake(message)
                    }
                    _ => DataError::Unavailable(message),
                }))
            }
            _ => Step::Done(Err(DataError::Unavailable(format!(
                "`{source}` answered with no tag (ABI {ABI} expects 0, 1, 2, or 3)"
            )))),
        }
    }

    fn take_request(&mut self, ticket: u64) -> Option<Request> {
        let pos = self.host.requests.iter().position(|(t, _)| *t == ticket)?;
        Some(self.host.requests.remove(pos).1)
    }

    fn key(target: Option<Target>, source: &str, args: &[Value]) -> Key {
        let mut bytes = Vec::new();
        for a in args {
            bytes.extend(a.to_bytes());
        }
        (target, source.to_string(), bytes)
    }

    /// Begin an answer: marshal, call, drain, settle.
    fn begin(
        &mut self,
        store: Option<&mut Store>,
        target: Option<Target>,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        if self.engine.is_none() {
            return Err(DataError::Unavailable(
                "exact-js: the engine is not loaded".into(),
            ));
        }
        // No answer waits for another to begin (LLP 1097 D4.5): answers
        // interleave at their awaits, as two async calls do on the web, and
        // storage keeps the order it was issued in (the prelude's queue).
        let Some(sig) = self.sigs.get(source) else {
            return Err(DataError::UnknownSource(source.to_string()));
        };
        if args.len() != sig.params.len() {
            return Err(DataError::BadArguments(format!(
                "expected {} arguments, got {}",
                sig.params.len(),
                args.len()
            )));
        }
        let mut json_args = Vec::with_capacity(args.len());
        for (i, (arg, shape)) in args.iter().zip(&sig.params).enumerate() {
            json_args.push(
                to_json(arg, shape)
                    .map_err(|_| DataError::BadArguments(format!("argument {i}")))?,
            );
        }
        let args_text = Json::Array(json_args).to_string();
        // A new answer's JavaScript may settle what another answer awaits.
        self.progress += 1;
        self.host.store = store.map(|s| s as *mut Store);
        let started = Instant::now();
        let result: Result<Step, DataError> = (|| {
            let engine = self.engine.as_mut().expect("checked above");
            let text = engine
                .call("__exact_call", [source, &args_text, ""])
                .map_err(|e| DataError::Unavailable(format!("`{source}` threw: {e}")))?;
            let mut reply = Module::decode_reply(sig, engine, source, &text)?;
            if reply.fields.get("tag").and_then(Json::as_u64) == Some(3) {
                let call = reply
                    .fields
                    .get("call")
                    .and_then(Json::as_u64)
                    .ok_or_else(|| {
                        DataError::Unavailable(format!("`{source}`: a call with no id"))
                    })?;
                engine
                    .drain()
                    .map_err(|e| DataError::Unavailable(format!("`{source}` threw: {e}")))?;
                let text = engine
                    .call("__exact_settle", [&call.to_string(), "", ""])
                    .map_err(|e| DataError::Unavailable(format!("`{source}` threw: {e}")))?;
                reply = Module::decode_reply(sig, engine, source, &text)?;
            }
            Ok(Module::step(sig, engine, source, reply, self.host.baking))
        })();
        self.host.store = None;
        let took_ms = started.elapsed().as_secs_f64() * 1e3;
        if result.is_err() || took_ms > self.budget_ms {
            self.engine.as_mut().expect("checked above").clear_reply();
        }
        if result.is_err() && self.watch.take() {
            // After the store is cleared, so a let-go step is not this
            // answer's write (LLP 1097; a934686a0).
            self.finish_let_go();
            return Err(DataError::Unavailable(format!(
                "`{source}` was interrupted"
            )));
        }
        if took_ms > self.budget_ms {
            self.overruns += 1;
            self.finish_let_go();
            return Err(DataError::Unavailable(format!(
                "`{source}` took {took_ms:.1} ms, over the {} ms budget",
                self.budget_ms
            )));
        }
        let answer = match result {
            Err(e) => Err(e),
            Ok(Step::Done(r)) => r.map(Answer::Now),
            Ok(Step::Pending { call, ticket }) => {
                let request = if ticket == 0 || ticket == WAITING {
                    Request::continuation(call)
                } else {
                    let Some(request) = self.take_request(ticket) else {
                        self.finish_let_go();
                        return Err(DataError::Unavailable(format!(
                            "`{source}` awaits a fetch it never made"
                        )));
                    };
                    request
                };
                let key = Module::key(target, source, args);
                // A new call replaces one parked on its key. Not a targeted
                // continuation's: the runner names which of two is in flight
                // by its token (`forgotten`) or drops the new one (`discard`),
                // and the call it would replace may be the one the runner
                // keeps (files F18: a re-read dropped by a refused pass).
                let replaced = if target.is_none() || request.continuation.is_none() {
                    let replaced: Vec<u64> = self
                        .parked
                        .iter()
                        .chain(&self.streams)
                        .filter(|(k, _)| *k == key)
                        .map(|(_, parked)| parked.call)
                        .collect();
                    self.parked.retain(|(k, _)| *k != key);
                    self.streams.retain(|(k, _)| *k != key);
                    replaced
                } else {
                    Vec::new()
                };
                // Park first. The let-go delivery below bumps progress after
                // that, so an answer waiting on the discarded call is asked
                // again (files F18). Delivering first parks it at the new
                // progress, and nothing asks it again.
                self.park(key, call, ticket, &request);
                self.forget_calls(replaced);
                Ok(Answer::Later(request))
            }
        };
        // A let-go call's operation may now be the one in flight. Deliver it
        // with no store, so its file writes land and its Store writes do not
        // (LLP 1097; a934686a0). Not while this answer's store is installed.
        self.finish_let_go();
        answer
    }

    /// Continue an answer: fulfil its fetch, drain, settle.
    fn resume(
        &mut self,
        store: &mut Store,
        target: Option<Target>,
        source: &str,
        args: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        if self.engine.is_none() {
            return Err(DataError::Unavailable(
                "exact-js: the engine is not loaded".into(),
            ));
        }
        let key = Module::key(target, source, args);
        if self.streams.iter().any(|(k, _)| *k == key) {
            return self.message(store, source, key, outcome);
        }
        let Some(pos) = self.parked.iter().position(|(k, _)| *k == key) else {
            return Err(DataError::Unavailable(format!(
                "`{source}`: a reply for an answer not in flight"
            )));
        };
        let Parked {
            call, ticket, last, ..
        } = self.parked.remove(pos).1;
        if ticket == WAITING {
            if let Outcome::Failed { message, .. } = &outcome {
                return Err(DataError::Unavailable(message.clone()));
            }
        } else {
            self.progress += 1; // a delivery: what a waiting answer waits for
        }
        let outcome_text = outcome_to_json(&outcome).to_string();
        self.host.store = Some(store as *mut Store);
        let started = Instant::now();
        let result: Result<Step, DataError> = (|| {
            let engine = self.engine.as_mut().expect("checked above");
            if ticket == WAITING {
                // Nothing to deliver: the work it waits on was another's.
            } else if ticket == 0 {
                if matches!(outcome, Outcome::Failed { .. }) {
                    engine
                        .call(
                            "__exact_storage_failed",
                            [&call.to_string(), &outcome_text, ""],
                        )
                        .map_err(DataError::Unavailable)?;
                } else {
                    engine
                        .deliver_storage_one()
                        .map_err(DataError::Unavailable)?;
                }
            } else {
                engine
                    .call("__exact_fulfill", [&ticket.to_string(), &outcome_text, ""])
                    .map_err(|e| DataError::Unavailable(format!("`{source}` threw: {e}")))?;
            }
            engine
                .drain()
                .map_err(|e| DataError::Unavailable(format!("`{source}` threw: {e}")))?;
            let settle = if last { "final" } else { "" };
            let text = engine
                .call("__exact_settle", [&call.to_string(), settle, ""])
                .map_err(|e| DataError::Unavailable(format!("`{source}` threw: {e}")))?;
            let Some(sig) = self.sigs.get(source) else {
                engine.clear_reply();
                return Err(DataError::UnknownSource(source.to_string()));
            };
            let reply = Module::decode_reply(sig, engine, source, &text)?;
            Ok(Module::step(sig, engine, source, reply, self.host.baking))
        })();
        self.host.store = None;
        let took_ms = started.elapsed().as_secs_f64() * 1e3;
        if result.is_err() || took_ms > self.budget_ms {
            self.engine.as_mut().expect("checked above").clear_reply();
        }
        if result.is_err() && self.watch.take() {
            self.finish_let_go();
            return Err(DataError::Unavailable(format!(
                "`{source}` was interrupted"
            )));
        }
        if took_ms > self.budget_ms {
            self.overruns += 1;
            self.finish_let_go();
            return Err(DataError::Unavailable(format!(
                "`{source}` took {took_ms:.1} ms, over the {} ms budget",
                self.budget_ms
            )));
        }
        let answer = match result {
            Err(e) => Err(e),
            Ok(Step::Done(r)) => r.map(Answer::Now),
            Ok(Step::Pending { call, ticket }) => {
                let request = if ticket == 0 || ticket == WAITING {
                    Request::continuation(call)
                } else {
                    let Some(request) = self.take_request(ticket) else {
                        self.finish_let_go();
                        return Err(DataError::Unavailable(format!(
                            "`{source}` awaits a fetch it never made"
                        )));
                    };
                    request
                };
                self.park(key, call, ticket, &request);
                Ok(Answer::Later(request))
            }
        };
        // After the park, so a waiter records this progress and the let-go
        // delivery is a later one it is asked again for (files F18). No
        // store: the let-go call's file writes land and its Store writes do
        // not (LLP 1097; a934686a0).
        self.finish_let_go();
        answer
    }

    /// Park a call on the request it waits for. A stream's call is not
    /// resumed by its reply: each message is mapped (`__exact_message`).
    fn park(&mut self, key: Key, call: u64, ticket: u64, request: &Request) {
        let parked = Parked {
            call,
            ticket,
            work_taken: false,
            progress: self.progress,
            last: false,
        };
        if request.stream {
            self.streams.push((key, parked));
        } else {
            self.parked.push((key, parked));
        }
    }

    /// One message of the stream answer `key` began, or its end: the
    /// source's `exactStream` maps it to the answer, now (LLP 1016.000 D1).
    fn message(
        &mut self,
        store: &mut Store,
        source: &str,
        key: Key,
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        let Some(at) = self.streams.iter().position(|(k, _)| *k == key) else {
            return Err(DataError::Unavailable(format!(
                "`{source}`: a message for a stream not open"
            )));
        };
        let call = self.streams[at].1.call;
        let ended = !matches!(outcome, Outcome::Message(_));
        self.progress += 1;
        let outcome_text = outcome_to_json(&outcome).to_string();
        self.host.store = Some(store as *mut Store);
        let started = Instant::now();
        let result: Result<Step, DataError> = (|| {
            let engine = self.engine.as_mut().expect("checked by resume");
            let text = engine
                .call("__exact_message", [&call.to_string(), &outcome_text, ""])
                .map_err(|e| DataError::Unavailable(format!("`{source}` threw: {e}")))?;
            let Some(sig) = self.sigs.get(source) else {
                engine.clear_reply();
                return Err(DataError::UnknownSource(source.to_string()));
            };
            let reply = Module::decode_reply(sig, engine, source, &text)?;
            Ok(Module::step(sig, engine, source, reply, self.host.baking))
        })();
        self.host.store = None;
        if ended {
            self.streams.remove(at);
            self.forget_calls(vec![call]);
        }
        let took_ms = started.elapsed().as_secs_f64() * 1e3;
        if result.is_err() || took_ms > self.budget_ms {
            self.engine
                .as_mut()
                .expect("checked by resume")
                .clear_reply();
        }
        if took_ms > self.budget_ms {
            self.overruns += 1;
            return Err(DataError::Unavailable(format!(
                "`{source}` took {took_ms:.1} ms, over the {} ms budget",
                self.budget_ms
            )));
        }
        match result? {
            Step::Done(r) => r.map(Answer::Now),
            Step::Pending { .. } => Err(DataError::Unavailable(format!(
                "`{source}`: exactStream answers each event now"
            ))),
        }
    }
}

impl Drop for Module {
    fn drop(&mut self) {
        self.unload();
    }
}
