//! The TypeScript data-source executor (LLP 1027).
//!
//! @ref LLP 1027 D2 (marshaling by the plan's shapes) / D3 (the executor) /
//! D4 (after first pixel) / D10 (what the module can use)
//!
//! An app's `app.ts`, bundled and compiled to Hermes bytecode at bake, is a
//! [`Module`]: a [`DataSource`] like any Rust data crate, behind the same
//! seam, answering the same `resource` and `send` declarations. The lean
//! Hermes VM runs it — bytecode only; it cannot be handed source — through
//! about a hundred lines of C++ (`shim.cc`), and the plan's `sources` table
//! says what every argument and every answer looks like, so records cross as
//! objects keyed by their declared field names and nothing is guessed.
//!
//! The module's identity and grants are the bake's outputs beside the
//! bytecode, so a host reads them at boot without an engine; the engine is
//! created by [`Module::load`], which a host calls after its first pixel
//! (D4). Until then every answer is `Unavailable`, by name.
//!
//! The seam, from the module's side (ABI 1): `exact.abi` is `1`;
//! `exact.appId` and `exact.grants` are strings; `exact.answer(source,
//! argsJson)` returns a JSON string `{"tag":0,"value":…}` or
//! `{"tag":2,"kind":"UnknownSource"|"BadArguments"|"Unavailable","message":…}`.
//! The one binding is `console` (D10); `fetch` and the store arrive with
//! LLP 1027 stage 3.

#![deny(missing_docs)]

mod engine;
mod marshal;

pub use engine::ENGINE_LINKED;
pub use marshal::{from_json, to_json, Shape};

use engine::Engine;
use exact_plan::{Plan, Value};
use exact_runner::{DataError, DataSource};
use serde_json::Value as Json;
use std::collections::HashMap;
use std::time::Instant;

/// The seam ABI this executor speaks; a module's `exact.abi` must equal it.
pub const ABI: u32 = 1;
/// The per-call wall-clock budget a module is held to, unless the host says otherwise.
pub const DEFAULT_BUDGET_MS: f64 = 100.0;
/// The runtime's heap ceiling, unless the host says otherwise.
pub const DEFAULT_MAX_HEAP: u32 = 64 << 20;

/// One source's signature, from the plan.
struct Sig {
    params: Vec<Shape>,
    result: Shape,
}

/// A TypeScript data source: bytecode, its bake-time identity, and the
/// engine that runs it once loaded.
pub struct Module {
    bytecode: Vec<u8>,
    app_id: &'static str,
    grants: &'static str,
    engine: Option<Engine>,
    sigs: HashMap<String, Sig>,
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
            .finish()
    }
}

impl Module {
    /// A module, unloaded: `app_id` and `grants` are what the bake wrote
    /// beside the bytecode, cross-checked against the module's own exports
    /// at [`Module::load`].
    pub fn new(bytecode: Vec<u8>, app_id: &'static str, grants: &'static str) -> Module {
        Module {
            bytecode,
            app_id,
            grants,
            engine: None,
            sigs: HashMap::new(),
            budget_ms: DEFAULT_BUDGET_MS,
            max_heap: DEFAULT_MAX_HEAP,
            logs: Vec::new(),
            overruns: 0,
        }
    }

    /// A module loaded at once — the bake's and a test's shape; a host loads
    /// after its first pixel instead (LLP 1027 D4).
    pub fn loaded(
        bytecode: Vec<u8>,
        app_id: &'static str,
        grants: &'static str,
    ) -> Result<Module, String> {
        let mut m = Module::new(bytecode, app_id, grants);
        m.load()?;
        Ok(m)
    }

    /// Create the runtime, evaluate the bytecode, and check that the module
    /// speaks this ABI and is the app the bake said it is. Idempotent.
    pub fn load(&mut self) -> Result<(), String> {
        if self.engine.is_some() {
            return Ok(());
        }
        let mut engine = Engine::new(self.max_heap).map_err(|e| format!("exact-js: {e}"))?;
        engine
            .load(&self.bytecode)
            .map_err(|e| format!("exact-js: the module did not load: {e}"))?;
        let abi = engine.string("abi")?;
        if abi != ABI.to_string() {
            return Err(format!(
                "exact-js: the module speaks ABI {abi:?}; this executor speaks {ABI}"
            ));
        }
        let app_id = engine.string("appId")?;
        if app_id != self.app_id {
            return Err(format!(
                "exact-js: the module says it is `{app_id}`; the bake said `{}`",
                self.app_id
            ));
        }
        let grants = engine.string("grants")?;
        if grants.trim() != self.grants.trim() {
            return Err(
                "exact-js: the module's grants differ from what the bake recorded".to_string(),
            );
        }
        self.engine = Some(engine);
        Ok(())
    }

    /// Drop the runtime; answers are `Unavailable` until the next [`Module::load`].
    pub fn unload(&mut self) {
        if let Some(mut engine) = self.engine.take() {
            self.logs.extend(engine.take_log());
        }
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

    fn call(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        let Some(engine) = self.engine.as_mut() else {
            return Err(DataError::Unavailable(
                "exact-js: the engine is not loaded".into(),
            ));
        };
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
        let started = Instant::now();
        let reply = engine.call("answer", [source, &args_text, ""]);
        let took_ms = started.elapsed().as_secs_f64() * 1e3;
        if took_ms > self.budget_ms {
            self.overruns += 1;
            return Err(DataError::Unavailable(format!(
                "`{source}` took {took_ms:.1} ms, over the {} ms budget",
                self.budget_ms
            )));
        }
        let text = reply.map_err(|e| DataError::Unavailable(format!("`{source}` threw: {e}")))?;
        let reply: Json = serde_json::from_str(&text).map_err(|e| {
            DataError::Unavailable(format!(
                "`{source}` answered something other than JSON: {e}"
            ))
        })?;
        match reply.get("tag").and_then(Json::as_u64) {
            Some(0) => {
                from_json(reply.get("value").unwrap_or(&Json::Null), &sig.result).map_err(|e| {
                    DataError::Unavailable(format!("`{source}` answered outside its shape: {e}"))
                })
            }
            Some(2) => {
                let message = reply
                    .get("message")
                    .and_then(Json::as_str)
                    .unwrap_or("")
                    .to_string();
                Err(match reply.get("kind").and_then(Json::as_str) {
                    Some("UnknownSource") => DataError::UnknownSource(message),
                    Some("BadArguments") => DataError::BadArguments(message),
                    _ => DataError::Unavailable(message),
                })
            }
            _ => Err(DataError::Unavailable(format!(
                "`{source}` answered with no tag (ABI {ABI} expects 0 or 2)"
            ))),
        }
    }
}

impl DataSource for Module {
    fn app_id(&self) -> &str {
        self.app_id
    }

    fn grants(&self) -> &'static str {
        self.grants
    }

    /// The seam's signatures, from the plan's `sources` table (LLP 1027 D2).
    fn bind(&mut self, plan: &Plan) {
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

    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        self.call(source, args)
    }
}
