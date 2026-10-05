//! Optional Rust data executors: embedded first frame, replacement after first pixel.
//! @ref LLP 1029.000 — independently built data modules, one encoded seam on every host.

use exact_logic_abi as abi;
#[doc(hidden)]
pub use exact_plan;
use exact_plan::{Plan, Value};
#[doc(hidden)]
pub use exact_runner;
use exact_runner::{Answer, DataError, DataSource, Outcome, Store, Target};
use sha2::{Digest, Sha256};

/// Version of the data-only Rust module protocol this host accepts.
pub const ABI: u32 = abi::ABI;

#[cfg(target_arch = "wasm32")]
mod browser;
mod configured;
#[cfg(any(
    target_os = "macos",
    target_os = "linux",
    target_os = "windows",
    target_os = "android"
))]
mod native;
#[cfg(any(
    target_os = "macos",
    target_os = "linux",
    target_os = "windows",
    target_os = "android"
))]
mod tiered;
#[cfg(not(target_arch = "wasm32"))]
mod wasm;

/// The running binary's exact Rust target triple.
pub const TARGET: &str = env!("EXACT_LOGIC_TARGET");
/// Maximum admitted compiled artifact, before any engine is created.
pub const MAX_MODULE: usize = 32 << 20;

trait Executor {
    fn call(&mut self, bytes: &[u8]) -> Result<Vec<u8>, String>;
    #[cfg(all(
        not(target_arch = "wasm32"),
        not(any(target_os = "ios", target_os = "tvos"))
    ))]
    fn stateless(&self) -> bool {
        false
    }
}
type Loader = fn(&[u8]) -> Result<Box<dyn Executor>, String>;
type Preloader = fn(&[u8]) -> Result<bool, String>;

/// A host links one concrete constructor; unused executors can be dead-stripped.
/// Replacement constructs a fresh session, retaining the admitted identity/grants.
/// Receipts pair bytes; they do not authenticate their author. The host must obtain
/// replacements through its admitted development origin or signed update channel.
pub struct Swappable<D> {
    embedded: Option<D>,
    app_id: String,
    grants: String,
    mode: &'static str,
    loader: Option<Loader>,
    preloader: Option<Preloader>,
    bytes: Option<Vec<u8>>,
    revision: Option<String>,
    plan: Option<Vec<u8>>,
    executor: Option<Box<dyn Executor>>,
    /// The embedded crate's Canvas 2D roster (LLP 1056 D1), kept across a
    /// replacement: the roster is part of the binary.
    canvas_surfaces: Vec<(String, usize)>,
}

impl<D: DataSource> Swappable<D> {
    fn configured(embedded: D, mode: &'static str, loader: Option<Loader>) -> Self {
        Self {
            app_id: embedded.app_id().into(),
            grants: embedded.grants().into(),
            canvas_surfaces: embedded.canvas_surfaces(),
            embedded: Some(embedded),
            mode,
            loader,
            preloader: None,
            bytes: None,
            revision: None,
            plan: None,
            executor: None,
        }
    }
    /// Runtime policy selection. Generated entries should prefer concrete constructors.
    pub fn new(embedded: D, policy: &str) -> Self {
        match policy {
            "wasm" => Self::wasm(embedded),
            "native" => Self::native(embedded),
            "tiered" => Self::tiered(embedded),
            "browser" => Self::browser(embedded),
            _ => Self::off(embedded),
        }
    }
    /// Keep embedded data and refuse Rust replacement; links no interpreter.
    pub fn off(embedded: D) -> Self {
        Self::configured(embedded, "off", None)
    }
    /// Interpret portable wasm, never generating executable memory.
    pub fn wasm(embedded: D) -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        {
            Self::configured(embedded, "wasm", Some(wasm::load))
        }
        #[cfg(target_arch = "wasm32")]
        {
            Self::browser(embedded)
        }
    }
    /// Load the native target's data-only shared library (unavailable on iOS).
    pub fn native(embedded: D) -> Self {
        #[cfg(any(
            target_os = "macos",
            target_os = "linux",
            target_os = "windows",
            target_os = "android"
        ))]
        {
            let mut source = Self::configured(embedded, "native", Some(native::load));
            source.preloader = Some(native::preload);
            source
        }
        #[cfg(not(any(
            target_os = "macos",
            target_os = "linux",
            target_os = "windows",
            target_os = "android"
        )))]
        {
            Self::configured(embedded, "native", None)
        }
    }
    /// Run portable, explicitly stateless logic while its native image loads.
    /// Promotion changes only the executor, without restarting the host session.
    pub fn tiered(embedded: D) -> Self {
        #[cfg(any(
            target_os = "macos",
            target_os = "linux",
            target_os = "windows",
            target_os = "android"
        ))]
        {
            Self::configured(embedded, "tiered", Some(tiered::load))
        }
        #[cfg(not(any(
            target_os = "macos",
            target_os = "linux",
            target_os = "windows",
            target_os = "android"
        )))]
        {
            Self::configured(embedded, "tiered", None)
        }
    }
    /// Execute wasm in the browser's existing WebAssembly executor.
    pub fn browser(embedded: D) -> Self {
        #[cfg(target_arch = "wasm32")]
        {
            Self::configured(embedded, "wasm", Some(browser::load))
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            Self::configured(embedded, "wasm", None)
        }
    }
    fn call(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
        outcome: Option<&Outcome>,
    ) -> Result<Answer, DataError> {
        let request =
            abi::call_request(store, source, args, outcome).map_err(DataError::Interface)?;
        let engine = self.executor.as_mut().ok_or_else(|| {
            DataError::Unavailable("Rust module awaits post-pixel activation".into())
        })?;
        let bytes = engine.call(&request).map_err(DataError::Interface)?;
        abi::call_reply(&bytes, store)
    }
    fn activate_module(&mut self) -> Result<(), String> {
        if self.executor.is_some() {
            return Ok(());
        }
        let bytes = self.bytes.as_ref().ok_or("Rust candidate has no bytes")?;
        if let Some(preload) = self.preloader {
            if !preload(bytes)? {
                return Err("Rust native image preparation is pending".into());
            }
        }
        let mut engine = self
            .loader
            .ok_or("Rust executor is disabled for this client")?(bytes)?;
        let (app, grants) = abi::metadata_reply(&engine.call(&abi::metadata_request())?)?;
        if app != self.app_id || grants.trim() != self.grants.trim() {
            return Err("loaded Rust module changes admitted app identity or grants".into());
        }
        if let Some(plan) = &self.plan {
            abi::unit_reply(&engine.call(&abi::bind_request(plan)?)?)?;
        }
        abi::unit_reply(&engine.call(&abi::activate_request())?)?;
        self.executor = Some(engine);
        Ok(())
    }
}

impl<D: DataSource> DataSource for Swappable<D> {
    fn preload(&self) -> Result<bool, DataError> {
        if let Some(embedded) = &self.embedded {
            return embedded.preload();
        }
        match (self.preloader, self.bytes.as_deref()) {
            (Some(preload), Some(bytes)) => preload(bytes).map_err(DataError::Unavailable),
            _ => Ok(true),
        }
    }
    fn app_id(&self) -> &str {
        &self.app_id
    }
    fn grants(&self) -> &str {
        &self.grants
    }
    fn revision(&self) -> Option<&str> {
        self.revision
            .as_deref()
            .or_else(|| self.embedded.as_ref().and_then(DataSource::revision))
    }
    fn ready(&self) -> bool {
        self.embedded
            .as_ref()
            .map(DataSource::ready)
            .unwrap_or(self.executor.is_some())
    }
    fn configure_storage(
        &mut self,
        data: std::path::PathBuf,
        cache: std::path::PathBuf,
        temporary: std::path::PathBuf,
    ) -> Result<(), DataError> {
        if let Some(embedded) = &mut self.embedded {
            embedded.configure_storage(data, cache, temporary)?;
        }
        Ok(())
    }
    fn continuation(&mut self, token: u64) -> Option<Box<dyn FnOnce() -> Outcome + Send>> {
        self.embedded.as_mut()?.continuation(token)
    }
    fn placement(&self) -> exact_runner::Placement {
        self.embedded
            .as_ref()
            .map(DataSource::placement)
            .unwrap_or_default()
    }
    /// A replaced Rust module's calls run to completion.
    fn interrupt(&self) -> Option<exact_runner::Interrupt> {
        self.embedded.as_ref().and_then(DataSource::interrupt)
    }
    fn native(&self) -> Option<exact_runner::Native> {
        self.embedded.as_ref().and_then(DataSource::native)
    }
    /// A replaced Rust module parks nothing.
    fn forgotten(&mut self, store: &exact_runner::Store, in_flight: &[exact_runner::InFlight<'_>]) {
        if let Some(embedded) = &mut self.embedded {
            embedded.forgotten(store, in_flight);
        }
    }
    fn dispatch(&mut self, token: u64, store: &Store) -> exact_runner::Dispatch {
        match self.embedded.as_mut() {
            Some(embedded) => embedded.dispatch(token, store),
            None => exact_runner::Dispatch::Missing,
        }
    }
    fn release(&mut self, store: &Store) -> Vec<(u64, exact_runner::Dispatch)> {
        match self.embedded.as_mut() {
            Some(embedded) => embedded.release(store),
            None => Vec::new(),
        }
    }
    fn discard(&mut self, token: u64) {
        if let Some(embedded) = self.embedded.as_mut() {
            embedded.discard(token);
        }
    }
    fn background(&mut self, store: &Store) -> Option<exact_runner::Request> {
        self.embedded.as_mut()?.background(store)
    }
    fn background_landed(
        &mut self,
        store: &Store,
        outcome: Outcome,
    ) -> Result<Option<exact_runner::Request>, DataError> {
        match self.embedded.as_mut() {
            Some(embedded) => embedded.background_landed(store, outcome),
            None => Ok(None),
        }
    }
    fn background_state(&self) -> Option<exact_runner::BackgroundState> {
        self.embedded.as_ref()?.background_state()
    }
    fn take_logs(&mut self) -> Vec<String> {
        self.embedded
            .as_mut()
            .map(DataSource::take_logs)
            .unwrap_or_default()
    }
    fn bind(&mut self, plan: &Plan) {
        if let Some(embedded) = &mut self.embedded {
            embedded.bind(plan);
        } else {
            self.plan = Some(plan.encode());
        }
    }
    /// A replaced module is not told: its values cross as copies.
    fn adopt(&mut self, source: &str, args: &[Value], value: &Value) {
        if let Some(embedded) = &mut self.embedded {
            embedded.adopt(source, args, value);
        }
    }
    fn activate(&mut self) -> Result<(), DataError> {
        if let Some(embedded) = &mut self.embedded {
            embedded.activate()
        } else {
            self.activate_module().map_err(DataError::Unavailable)
        }
    }
    fn activate_for_validation(&mut self) -> Result<(), DataError> {
        if let Some(embedded) = &mut self.embedded {
            embedded.activate_for_validation()
        } else {
            self.activate_module().map_err(DataError::Unavailable)
        }
    }
    fn replacement(&self, plan: &[u8], receipt: &str, module: Vec<u8>) -> Result<Self, DataError> {
        let build = || -> Result<Self, String> {
            if receipt.len() > 1 << 20 || plan.len() > 32 << 20 || module.len() > MAX_MODULE {
                return Err("Rust candidate exceeds artifact size limit".into());
            }
            let meta: serde_json::Value =
                serde_json::from_str(receipt).map_err(|e| e.to_string())?;
            if meta["kind"].as_str() != Some("rust") {
                let embedded = self
                    .embedded
                    .as_ref()
                    .ok_or("active Rust module cannot accept another language")?;
                let embedded = embedded
                    .replacement(plan, receipt, module)
                    .map_err(|e| format!("{e:?}"))?;
                let mut source = Self::configured(embedded, self.mode, self.loader);
                source.preloader = self.preloader;
                return Ok(source);
            }
            if self.loader.is_none() {
                return Err("Rust live updates are disabled for this client; rebuild it".into());
            }
            if meta["version"].as_u64() != Some(1) || meta["abi"].as_u64() != Some(abi::ABI.into())
            {
                return Err("unsupported Rust receipt or seam ABI".into());
            }
            if self.app_id.is_empty() || meta["appId"].as_str() != Some(&self.app_id) {
                return Err("Rust candidate names another app".into());
            }
            if meta["grants"].as_str().map(str::trim) != Some(self.grants.trim()) {
                return Err("Rust candidate changes admitted grants; rebuild the client".into());
            }
            let target = if self.mode == "wasm" {
                "wasm32-unknown-unknown"
            } else {
                TARGET
            };
            if meta["executor"].as_str() != Some(self.mode)
                || meta["target"].as_str() != Some(target)
            {
                return Err("Rust candidate executor or target differs from the client".into());
            }
            let filename = if self.mode == "tiered" {
                "app.module.bin"
            } else if self.mode == "wasm" {
                "app.module.wasm"
            } else if cfg!(target_os = "macos") {
                "app.module.dylib"
            } else if cfg!(target_os = "windows") {
                "app.module.dll"
            } else {
                "app.module.so"
            };
            for (key, name, bytes) in [
                ("plan", "app.plan", plan),
                ("module", filename, module.as_slice()),
            ] {
                let card = &meta[key];
                let digest = format!("{:x}", Sha256::digest(bytes));
                if card["file"].as_str() != Some(name)
                    || card["bytes"].as_u64() != Some(bytes.len() as u64)
                    || card["sha256"].as_str() != Some(&digest)
                {
                    return Err(format!("Rust candidate {key} does not match receipt"));
                }
            }
            let decoded = Plan::decode(plan).map_err(|e| format!("candidate plan: {e:?}"))?;
            if decoded.app_id != self.app_id {
                return Err("Rust candidate plan names another app".into());
            }
            if self.mode == "wasm" && !module.starts_with(b"\0asm\x01\0\0\0") {
                return Err("candidate is not a version 1 wasm module".into());
            }
            Ok(Self {
                embedded: None,
                app_id: self.app_id.clone(),
                grants: self.grants.clone(),
                mode: self.mode,
                loader: self.loader,
                preloader: self.preloader,
                revision: Some(format!("rust:{}:{:x}", self.mode, Sha256::digest(&module))),
                bytes: Some(module),
                plan: Some(plan.to_vec()),
                executor: None,
                canvas_surfaces: self.canvas_surfaces.clone(),
            })
        };
        build().map_err(DataError::Unavailable)
    }
    fn canvas_surfaces(&self) -> Vec<(String, usize)> {
        self.canvas_surfaces.clone()
    }
    /// A replaced Rust module does not draw yet: its canvases report the
    /// error through `state` (LLP 1056 stage 1; QUEUE).
    fn draw(
        &mut self,
        request: &exact_runner::DrawRequest<'_>,
        ctx: &exact_runner::exact_canvas::Context2d,
    ) -> exact_runner::Drawn {
        match self.embedded.as_mut() {
            Some(embedded) => embedded.draw(request, ctx),
            None => exact_runner::Drawn::Now(exact_runner::DrawReply {
                error: Some("a replaced Rust module does not draw Canvas 2D yet".into()),
                ..Default::default()
            }),
        }
    }
    fn canvases_retired(&mut self, retired: &[(u64, u32)]) {
        if let Some(embedded) = self.embedded.as_mut() {
            embedded.canvases_retired(retired);
        }
    }
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        if let Some(embedded) = &mut self.embedded {
            return embedded.query(source, args);
        }
        let mut store = Store::new(&self.grants, []);
        match self.call(&mut store, source, args, None)? {
            Answer::Now(v) => Ok(v),
            Answer::Later(_) => Err(DataError::Unavailable(
                "Rust query needs a host request executor".into(),
            )),
        }
    }
    fn answer(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        if let Some(embedded) = &mut self.embedded {
            embedded.answer(store, source, args)
        } else {
            self.call(store, source, args, None)
        }
    }
    fn parse(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        if let Some(embedded) = &mut self.embedded {
            embedded.parse(store, source, args, outcome)
        } else {
            self.call(store, source, args, Some(&outcome))
        }
    }
    /// The embedded source keys by target; a replaced module's seam carries
    /// no target, and a Rust source parks nothing between its calls.
    fn answer_for(
        &mut self,
        target: Target,
        store: &mut Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        if let Some(embedded) = &mut self.embedded {
            embedded.answer_for(target, store, source, args)
        } else {
            self.call(store, source, args, None)
        }
    }
    fn parse_for(
        &mut self,
        target: Target,
        store: &mut Store,
        source: &str,
        args: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        if let Some(embedded) = &mut self.embedded {
            embedded.parse_for(target, store, source, args, outcome)
        } else {
            self.call(store, source, args, Some(&outcome))
        }
    }
}

#[cfg(test)]
mod tests;

#[cfg(all(test, any(target_os = "macos", target_os = "linux")))]
mod native_tests;
