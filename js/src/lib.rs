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
//! `text()`, `json()`, `arrayBuffer()`. `store` is `{get, set, forget}` over
//! the runner's [`Store`]: reads counted, writes grant-checked, in Rust.
//! `console` reaches the runner's logs. Time and random seeds are ordinary
//! source arguments (LLP 1027.000): ambient Date/Math.random reads and
//! Intl.DateTimeFormat formatting without an explicit timestamp refuse,
//! including at module initialization and after await. Explicit-value Date
//! construction and UTC arithmetic remain available. There are no timers.

#![deny(missing_docs)]

mod engine;
mod native;
mod paired;
mod pure;
mod storage;

pub use engine::ENGINE_LINKED;
pub use exact_data::Placed;
pub use exact_js_value::{from_json, to_json, Shape};
pub use exact_runner::Placement;
pub use native::NativeModule;
pub use paired::Paired;

use engine::{Engine, HostFn};
use exact_plan::{Plan, Value};
use exact_runner::{Answer, DataError, DataSource, Outcome, Request, Store};
use serde_json::{json, Value as Json};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::ffi::{c_char, c_void, CStr};
use std::time::Instant;

type NativeFactory = fn(&str) -> Box<dyn NativeModule>;

/// The seam ABI this executor speaks; a module's `exact.abi` must equal it.
pub const ABI: u32 = 1;
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

/// One source's signature, from the plan.
struct Sig {
    params: Vec<Shape>,
    result: Shape,
}

/// An answer that awaited a fetch: the prelude's call id, and the fetch
/// ticket the runner's request stands for.
struct Parked {
    call: u64,
    ticket: u64,
    work_taken: bool,
}

/// What the host door reaches during one call: the store the seam handed
/// `answer` or `parse` (none at bake — an empty store that refuses writes),
/// and the requests `fetch` recorded, by the prelude's ticket. Boxed for the
/// engine's lifetime; the shim holds a pointer to it.
#[derive(Default)]
struct HostState {
    store: Option<*mut Store>,
    requests: Vec<(u64, Request)>,
    native: Option<Box<dyn NativeModule>>,
}

/// A TypeScript data source: bytecode, its bake-time identity, and the
/// engine that runs it once loaded.
pub struct Module {
    bytecode: Vec<u8>,
    app_id: String,
    grants: String,
    revision: String,
    engine: Option<Engine>,
    storage: Option<storage::Session>,
    directories: Option<storage::Directories>,
    host: Box<HostState>,
    native_factory: Option<NativeFactory>,
    /// The bound plan, kept so an owner thread can bind its own instance.
    plan: Option<Plan>,
    sigs: HashMap<String, Sig>,
    parked: Vec<((String, Vec<u8>), Parked)>,
    budget_ms: f64,
    max_heap: u32,
    logs: Vec<String>,
    overruns: u32,
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

/// The one door from the module into Rust (`__exact_host` in the prelude).
///
/// # Safety
/// Called by the shim on the engine's thread with `ctx` the `HostState` the
/// engine was created with, and `a`/`b` NUL-terminated for the call.
unsafe extern "C" fn host_door(
    ctx: *mut c_void,
    op: u32,
    a: *const c_char,
    b: *const c_char,
    out: *mut *mut c_char,
) -> i32 {
    let state = &mut *(ctx as *mut HostState);
    let a = CStr::from_ptr(a).to_string_lossy();
    let b = CStr::from_ptr(b).to_string_lossy();
    let reply: Result<Option<String>, String> = match op {
        1 => match a.parse::<u64>() {
            Ok(ticket) => match request_from_json(&b) {
                Ok(request) => {
                    state.requests.push((ticket, request));
                    Ok(None)
                }
                Err(e) => Err(format!("fetch: {e}")),
            },
            Err(_) => Err("fetch: a ticket that is not a number".into()),
        },
        2 => Ok(state.store.and_then(|s| (*s).get(&a).map(str::to_string))),
        3 => match state.store {
            Some(s) => (*s)
                .set(&a, &b)
                .map(|_| None)
                .map_err(|e| format!("store.set: {e:?}")),
            None => Err("store.set: no store at bake".into()),
        },
        4 => match state.store {
            Some(s) => (*s)
                .forget(&a)
                .map(|_| None)
                .map_err(|e| format!("store.forget: {e:?}")),
            None => Err("store.forget: no store at bake".into()),
        },
        5 => {
            if let Some(store) = state.store {
                (*store).observe_external_read();
                Ok(None)
            } else {
                Err("storage is unavailable during bake".into())
            }
        }
        6 => {
            if a == "available" {
                Ok(Some("native".into()))
            } else {
                if let Some(store) = state.store {
                    (*store).observe_external_read();
                }
                match (&mut state.native, state.store) {
                    (Some(module), Some(_)) => serde_json::from_str(&b)
                        .map_err(|error| error.to_string())
                        .and_then(|request| module.call(&request))
                        .map(|reply| Some(reply.to_string())),
                    _ => Err(
                        "native storage is unavailable during bake or in an unconfigured host"
                            .into(),
                    ),
                }
            }
        }
        7 => pure::call(&a, &b).map(Some),
        other => Err(format!("__exact_host: no op {other}")),
    };
    *out = std::ptr::null_mut();
    match reply {
        Ok(None) => 0,
        Ok(Some(text)) => {
            *out = c_string(&text);
            0
        }
        Err(text) => {
            *out = c_string(&text);
            1
        }
    }
}

/// A malloc'd copy the shim frees.
fn c_string(text: &str) -> *mut c_char {
    let bytes = text.as_bytes();
    // SAFETY: malloc'd with room for the NUL; the shim `free`s it.
    unsafe {
        let p = libc_malloc(bytes.len() + 1) as *mut u8;
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), p, bytes.len());
        *p.add(bytes.len()) = 0;
        p as *mut c_char
    }
}

extern "C" {
    #[link_name = "malloc"]
    fn libc_malloc(size: usize) -> *mut c_void;
}

fn request_from_json(text: &str) -> Result<Request, String> {
    let j: Json = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let field = |k: &str| j.get(k).and_then(Json::as_str).map(str::to_string);
    let mut headers = Vec::new();
    if let Some(list) = j.get("headers").and_then(Json::as_array) {
        for pair in list {
            match (
                pair.get(0).and_then(Json::as_str),
                pair.get(1).and_then(Json::as_str),
            ) {
                (Some(k), Some(v)) => headers.push((k.to_string(), v.to_string())),
                _ => return Err("a header that is not a name and a value".into()),
            }
        }
    }
    Ok(Request {
        http: match j.get("max_response_bytes") {
            None => exact_runner::HttpScheduling::Ordered,
            Some(value) => exact_runner::HttpScheduling::Independent {
                max_response_bytes: value
                    .as_u64()
                    .filter(|n| (1..=64 << 20).contains(n))
                    .ok_or("invalid independent HTTP response ceiling")?
                    as u32,
            },
        },
        continuation: None,
        storage: None,
        grants: None,
        method: field("method").ok_or("no method")?,
        url: field("url").ok_or("no url")?,
        headers,
        body: field("body").unwrap_or_default().into_bytes(),
    })
}

fn outcome_to_json(outcome: &Outcome) -> Json {
    match outcome {
        Outcome::Storage(_) => {
            json!({"failed":{"kind":"Unsupported","message":"storage result supplied to a fetch continuation"}})
        }
        Outcome::Response(r) => json!({
            "response": {
                "status": r.status,
                "headers": r.headers.iter().map(|(k, v)| json!([k, v])).collect::<Vec<_>>(),
                "body": String::from_utf8_lossy(&r.body),
                "bodyBase64": base64(&r.body),
            }
        }),
        Outcome::Failed { kind, message } => json!({
            "failed": { "kind": format!("{kind:?}"), "message": message }
        }),
    }
}

fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk.len();
        let v = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        out.push(T[(v >> 18) as usize & 63] as char);
        out.push(T[(v >> 12) as usize & 63] as char);
        out.push(if n > 1 {
            T[(v >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if n > 2 {
            T[v as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

/// One step of a call as the prelude reports it.
enum Step {
    Done(Result<Value, DataError>),
    Pending { call: u64, ticket: u64 },
}

impl Module {
    /// A module, unloaded: `app_id` and `grants` are what the bake wrote
    /// beside the bytecode, cross-checked against the module's own exports
    /// at [`Module::load`].
    pub fn new(bytecode: Vec<u8>, app_id: impl Into<String>, grants: impl Into<String>) -> Module {
        Module {
            revision: format!("{:x}", Sha256::digest(&bytecode)),
            bytecode,
            app_id: app_id.into(),
            grants: grants.into(),
            engine: None,
            storage: None,
            directories: None,
            host: Box::default(),
            native_factory: None,
            plan: None,
            sigs: HashMap::new(),
            parked: Vec::new(),
            budget_ms: DEFAULT_BUDGET_MS,
            max_heap: DEFAULT_MAX_HEAP,
            logs: Vec::new(),
            overruns: 0,
        }
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
        Box::new(move || {
            let mut module = Module::new(bytecode, app_id, grants);
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
        let engine = self.load_engine()?;
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
        let mut module = Self::new(bytecode, "", "");
        let engine = module.load_engine()?;
        module.app_id = engine.string("appId")?;
        module.grants = engine.string("grants")?;
        if module.app_id.is_empty() {
            return Err("exact-js: the module exports no appId".into());
        }
        module.engine = Some(engine);
        Ok(module)
    }

    fn load_engine(&mut self) -> Result<Engine, String> {
        let ctx = &mut *self.host as *mut HostState as *mut c_void;
        let host: HostFn = host_door;
        let mut engine =
            Engine::new(self.max_heap, host, ctx).map_err(|e| format!("exact-js: {e}"))?;
        engine
            .load(PRELUDE)
            .map_err(|e| format!("exact-js: the prelude did not load: {e}"))?;
        if let Some(paths) = &self.directories {
            if let Some(factory) = self.native_factory {
                let mut native = factory(&self.grants);
                native.configure_storage(
                    paths.data.clone(),
                    paths.cache.clone(),
                    paths.temporary.clone(),
                )?;
                self.host.native = Some(native);
            }
            self.storage = Some(storage::Session::open(paths, &self.grants)?);
            // Retain the borrowed queue even if adapter initialization fails;
            // the local engine must be destroyed before its storage context.
            engine.install_storage(&self.storage.as_ref().unwrap().context)?;
        }
        engine
            .load(&self.bytecode)
            .map_err(|e| format!("exact-js: the module did not load: {e}"))?;
        let abi = engine.string("abi")?;
        if abi != ABI.to_string() {
            return Err(format!(
                "exact-js: the module speaks ABI {abi:?}; this executor speaks {ABI}"
            ));
        }
        Ok(engine)
    }

    /// Drop the runtime; answers are `Unavailable` until the next
    /// [`Module::load`], and every answer in flight is forgotten.
    pub fn unload(&mut self) {
        if let Some(mut engine) = self.engine.take() {
            self.logs.extend(engine.take_log());
        }
        self.storage = None;
        self.host.native = None;
        self.parked.clear();
        self.host.requests.clear();
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
        self.parked.len()
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
            serde_json::from_str(text).map(|fields| exact_js_value::Reply {
                fields,
                value: Ok(Value::Unit),
            })
        } else {
            exact_js_value::reply_from_json_text(text, &sig.result)
        };
        decoded.map_err(|e| {
            engine.clear_reply();
            DataError::Unavailable(format!(
                "`{source}` answered something other than JSON: {e}"
            ))
        })
    }

    /// Dispatch a decoded reply, restoring captured strings before shape checking.
    fn step(sig: &Sig, engine: &mut Engine, source: &str, decoded: exact_js_value::Reply) -> Step {
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
                    _ => DataError::Unavailable(message),
                }))
            }
            _ => Step::Done(Err(DataError::Unavailable(format!(
                "`{source}` answered with no tag (ABI {ABI} expects 0, 1, 2, or 3)"
            )))),
        }
    }

    /// The request the prelude recorded for `ticket`, if `fetch` was called.
    fn take_request(&mut self, ticket: u64) -> Option<Request> {
        let pos = self.host.requests.iter().position(|(t, _)| *t == ticket)?;
        Some(self.host.requests.remove(pos).1)
    }

    fn key(source: &str, args: &[Value]) -> (String, Vec<u8>) {
        let mut bytes = Vec::new();
        for a in args {
            bytes.extend(a.to_bytes());
        }
        (source.to_string(), bytes)
    }

    /// Begin an answer: marshal, call, drain, settle.
    fn begin(
        &mut self,
        store: Option<&mut Store>,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        if self.engine.is_none() {
            return Err(DataError::Unavailable(
                "exact-js: the engine is not loaded".into(),
            ));
        }
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
            Ok(Module::step(sig, engine, source, reply))
        })();
        self.host.store = None;
        let took_ms = started.elapsed().as_secs_f64() * 1e3;
        if result.is_err() || took_ms > self.budget_ms {
            self.engine.as_mut().expect("checked above").clear_reply();
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
            Step::Pending { call, ticket } => {
                let request = if ticket == 0 {
                    Request::continuation(call)
                } else {
                    self.take_request(ticket).ok_or_else(|| {
                        DataError::Unavailable(format!("`{source}` awaits a fetch it never made"))
                    })?
                };
                let key = Module::key(source, args);
                self.parked.retain(|(k, _)| *k != key);
                self.parked.push((
                    key,
                    Parked {
                        call,
                        ticket,
                        work_taken: false,
                    },
                ));
                Ok(Answer::Later(request))
            }
        }
    }

    /// Continue an answer: fulfil its fetch, drain, settle.
    fn resume(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        if self.engine.is_none() {
            return Err(DataError::Unavailable(
                "exact-js: the engine is not loaded".into(),
            ));
        }
        let key = Module::key(source, args);
        let Some(pos) = self.parked.iter().position(|(k, _)| *k == key) else {
            return Err(DataError::Unavailable(format!(
                "`{source}`: a reply for an answer not in flight"
            )));
        };
        let Parked { call, ticket, .. } = self.parked.remove(pos).1;
        let outcome_text = outcome_to_json(&outcome).to_string();
        self.host.store = Some(store as *mut Store);
        let started = Instant::now();
        let result: Result<Step, DataError> = (|| {
            let engine = self.engine.as_mut().expect("checked above");
            if ticket == 0 {
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
            let text = engine
                .call("__exact_settle", [&call.to_string(), "", ""])
                .map_err(|e| DataError::Unavailable(format!("`{source}` threw: {e}")))?;
            let Some(sig) = self.sigs.get(source) else {
                engine.clear_reply();
                return Err(DataError::UnknownSource(source.to_string()));
            };
            let reply = Module::decode_reply(sig, engine, source, &text)?;
            Ok(Module::step(sig, engine, source, reply))
        })();
        self.host.store = None;
        let took_ms = started.elapsed().as_secs_f64() * 1e3;
        if result.is_err() || took_ms > self.budget_ms {
            self.engine.as_mut().expect("checked above").clear_reply();
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
            Step::Pending { call, ticket } => {
                let request = if ticket == 0 {
                    Request::continuation(call)
                } else {
                    self.take_request(ticket).ok_or_else(|| {
                        DataError::Unavailable(format!("`{source}` awaits a fetch it never made"))
                    })?
                };
                self.parked.push((
                    key,
                    Parked {
                        call,
                        ticket,
                        work_taken: false,
                    },
                ));
                Ok(Answer::Later(request))
            }
        }
    }
}

impl DataSource for Module {
    fn configure_storage(
        &mut self,
        data: std::path::PathBuf,
        cache: std::path::PathBuf,
        temporary: std::path::PathBuf,
    ) -> Result<(), DataError> {
        if self.is_loaded() {
            return Err(DataError::Unavailable(
                "configure storage before loading the module".into(),
            ));
        }
        self.directories = Some(storage::Directories {
            data,
            cache,
            temporary,
        });
        Ok(())
    }

    fn continuation(&mut self, token: u64) -> Option<Box<dyn FnOnce() -> Outcome + Send>> {
        let (_, parked) = self
            .parked
            .iter_mut()
            .find(|(_, p)| p.call == token && p.ticket == 0 && !p.work_taken)?;
        let work = self.storage.as_ref()?.continuation();
        parked.work_taken = true;
        Some(work)
    }

    fn activate(&mut self) -> Result<(), DataError> {
        self.load().map_err(DataError::Unavailable)
    }

    fn activate_for_validation(&mut self) -> Result<(), DataError> {
        // A replacement may inherit directory paths from a direct consumer.
        // Validation gets neither those capabilities nor an existing adapter;
        // its disposable engine can only record ordinary host requests.
        self.unload();
        self.directories = None;
        self.activate()
    }

    fn replacement(&self, plan: &[u8], receipt: &str, module: Vec<u8>) -> Result<Self, DataError> {
        Paired::decode(receipt, plan, module, self.app_id(), self.grants())
            .map(|pair| {
                let mut module = pair.module;
                module.directories = self.directories.clone();
                module.native_factory = self.native_factory;
                module
            })
            .map_err(DataError::Unavailable)
    }

    fn app_id(&self) -> &str {
        &self.app_id
    }

    fn grants(&self) -> &str {
        &self.grants
    }

    fn revision(&self) -> Option<&str> {
        Some(&self.revision)
    }

    /// Not before the host loads it (LLP 1027 D4): the runner boots
    /// store-reading resources from their kept answers meanwhile.
    fn ready(&self) -> bool {
        self.is_loaded()
    }

    /// The seam's signatures, from the plan's `sources` table (LLP 1027 D2).
    fn bind(&mut self, plan: &Plan) {
        self.plan = Some(plan.clone());
        self.sigs.clear();
        for row in &plan.sources {
            let start = row.params.start as usize;
            let end = start + row.params.len as usize;
            let params = plan
                .source_params
                .get(start..end)
                .map(|rows| {
                    rows.iter()
                        .map(|p| Shape::from_plan(plan, p.ty))
                        .collect::<Result<Vec<_>, _>>()
                })
                .unwrap_or_else(|| Err("a source's parameters run past the table".into()));
            let result = Shape::from_plan(plan, row.ty);
            if let (Ok(params), Ok(result)) = (params, result) {
                self.sigs
                    .insert(plan.str(row.name).to_string(), Sig { params, result });
            }
        }
    }

    /// The bake's path and the in-process path: no store, and an answer that
    /// awaits a fetch cannot be given now.
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        match self.begin(None, source, args)? {
            Answer::Now(v) => Ok(v),
            Answer::Later(_) => {
                self.parked.retain(|(k, _)| *k != Module::key(source, args));
                Err(DataError::Unavailable(format!(
                    "`{source}` fetches, and there is no host to run it here"
                )))
            }
        }
    }

    fn answer(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        self.begin(Some(store), source, args)
    }

    fn parse(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        self.resume(store, source, args, outcome)
    }
}

impl Drop for Module {
    fn drop(&mut self) {
        self.unload();
    }
}
