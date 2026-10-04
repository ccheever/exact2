//! Device-free canvas ownership over the same dynamic module ABI as Apple.
//! One library per GPU artifact (LLP 1009 D6): the primary, and each declared
//! module, opened the first time a canvas of one of its surfaces appears.
//! @ref LLP 1046.001 D7; LLP 1015 §7
#![allow(unsafe_code)]
use crate::{Host, Presenter};
use base64::Engine;
use exact_runner::{
    DataSource, Event, FailureKind, Outcome, RequestOut, SurfaceOutcome, SurfaceRequest,
    MAX_HOST_WORK_BYTES,
};
use libloading::Library;
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};

#[path = "surface_controls.rs"]
pub(crate) mod controls;
mod pixels;
mod shaders;

const LIMIT: usize = 256 * 1024 * 1024;
type Read = unsafe extern "C" fn(u32) -> u32;
type Text = unsafe extern "C" fn(u32, *const u8, usize) -> u32;

struct Abi {
    // Symbols never outlive this library; unload TLS before dlclose.
    library: Library,
    output_error: std::cell::RefCell<Option<String>>,
    rendered: bool,
    shaders: shaders::Pack,
}
impl Abi {
    fn open(compat: &Value, artifact: &str) -> Result<Self, String> {
        let binary = std::env::current_exe().map_err(|e| e.to_string())?;
        let name = gpu_card(compat, artifact)["name"]
            .as_str()
            .ok_or("GPU module has no baked identity")?;
        // A declared module sits beside the binary; only the primary has a
        // development override.
        let path = if artifact.is_empty() {
            module_path(
                &binary.with_file_name(name),
                compat,
                std::env::var_os("EXACT_GPU_MODULE").map(PathBuf::from),
            )
        } else {
            binary.with_file_name(name)
        };
        Self::open_path(&path, compat, artifact)
    }
    fn open_path(path: &std::path::Path, compat: &Value, artifact: &str) -> Result<Self, String> {
        verify_module(path, compat, artifact)?;
        // SAFETY: the app's own module, with the ABI checked before any call.
        let abi = Self {
            library: unsafe { Library::new(path) }.map_err(|e| e.to_string())?,
            output_error: Default::default(),
            shaders: Default::default(),
            rendered: std::env::var("EXACT_GPU_RENDER").as_deref() == Ok("1"),
        };
        unsafe {
            for name in [
                "gpu_load_headless",
                "gpu_recover",
                "gpu_child_view",
                "gpu_children_count",
                "gpu_children_mode",
                "gpu_placement",
                "gpu_unload",
                "gpu_create_headless",
                "gpu_bind_at",
                "gpu_agent",
                "gpu_input",
                "gpu_wants_input",
                "gpu_published",
                "gpu_assets",
                "gpu_asset",
                "gpu_messages",
                "gpu_carry",
                "gpu_restore",
                "gpu_destroy",
                "gpu_error",
                "gpu_out_ptr",
            ] {
                abi.library
                    .get::<*const ()>(name.as_bytes())
                    .map_err(|e| format!("GPU ABI {name}: {e}"))?;
            }
            if abi.rendered {
                for name in ["gpu_load", "gpu_readback", "gpu_seekable", "gpu_lifecycle"] {
                    abi.library
                        .get::<*const ()>(name.as_bytes())
                        .map_err(|e| format!("GPU ABI {name}: {e}"))?;
                }
                if abi.symbol::<unsafe extern "C" fn() -> u32>(b"gpu_load")() != 0 {
                    return Err(abi.error().unwrap_or("GPU initialization failed".into()));
                }
                abi.symbol::<unsafe extern "C" fn(bool)>(b"gpu_seekable")(
                    std::env::var("EXACT_AGENT").as_deref() == Ok("1"),
                );
            } else {
                abi.symbol::<unsafe extern "C" fn()>(b"gpu_load_headless")();
            }
        }
        Ok(abi)
    }
    // SAFETY: all callers supply the signature declared by gpu/src/native.rs.
    unsafe fn symbol<T: Copy>(&self, name: &[u8]) -> T {
        *unsafe { self.library.get::<T>(name) }.expect("validated module ABI")
    }
    fn lifecycle(&self, id: u32, code: u32) {
        if self.rendered {
            // SAFETY: validated rendered-module ABI; id belongs to this module.
            unsafe { self.symbol::<unsafe extern "C" fn(u32, u32)>(b"gpu_lifecycle")(id, code) };
        }
    }
    fn bytes(&self, len: u32) -> Option<Vec<u8>> {
        if len == u32::MAX {
            return None;
        }
        if len as usize > LIMIT {
            *self.output_error.borrow_mut() = Some("surface output exceeds 256 MiB limit".into());
            return None;
        }
        if len == 0 {
            return Some(Vec::new());
        }
        // SAFETY: the ABI buffer is readable until the next call, copied now.
        Some(unsafe {
            let ptr = self.symbol::<unsafe extern "C" fn() -> *const u8>(b"gpu_out_ptr")();
            if ptr.is_null() {
                *self.output_error.borrow_mut() = Some("surface output has a null pointer".into());
                return None;
            }
            std::slice::from_raw_parts(ptr, len as usize).to_vec()
        })
    }
    fn read(&self, name: &[u8], id: u32) -> Option<Vec<u8>> {
        self.read_bounded(name, id, LIMIT)
    }
    fn read_bounded(&self, name: &[u8], id: u32, limit: usize) -> Option<Vec<u8>> {
        let length = unsafe { self.symbol::<Read>(name)(id) };
        if name == b"gpu_carry" && length == u32::MAX - 1 {
            return None;
        }
        if length as usize > limit && length < u32::MAX - 1 {
            *self.output_error.borrow_mut() = Some(format!(
                "surface output exceeds {} MiB limit",
                limit / (1024 * 1024)
            ));
            return None;
        }
        self.bytes(length)
    }
    fn text(&self, name: &[u8], id: u32, text: &str) -> u32 {
        unsafe { self.symbol::<Text>(name)(id, text.as_ptr(), text.len()) }
    }
    fn error(&self) -> Option<String> {
        if let Some(error) = self.output_error.borrow_mut().take() {
            return Some(error);
        }
        let n = unsafe { self.symbol::<unsafe extern "C" fn() -> u32>(b"gpu_error")() };
        if n == 0 {
            return None;
        }
        match self.bytes(n) {
            Some(bytes) => Some(String::from_utf8_lossy(&bytes).into_owned()),
            None => Some(
                self.output_error
                    .borrow_mut()
                    .take()
                    .unwrap_or("invalid surface error output".into()),
            ),
        }
    }
    fn agent(&self, id: u32, q: &Value) -> Value {
        let n = self.text(b"gpu_agent", id, &q.to_string());
        let bytes = self.bytes(n);

        if let Some(error) = self.error() {
            return json!({"error":error});
        }
        bytes
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or(Value::Null)
    }
    fn destroy(&self, id: u32) {
        unsafe { self.symbol::<unsafe extern "C" fn(u32)>(b"gpu_destroy")(id) }
    }
}
impl Drop for Abi {
    fn drop(&mut self) {
        // A missing symbol can occur during validation of an older artifact.
        if let Ok(unload) = unsafe { self.library.get::<unsafe extern "C" fn()>(b"gpu_unload") } {
            unsafe { unload() };
        }
    }
}
fn module_path(
    default: &std::path::Path,
    compat: &Value,
    override_path: Option<PathBuf>,
) -> PathBuf {
    if compat["embedded"]["gpu"]["trust"] == "development" {
        if let Some(path) = override_path {
            return path;
        }
    }
    default.to_path_buf()
}
/// The baked identity of `artifact`: "" the primary's, else a declared
/// module's (LLP 1009 D6), one signed digest each.
fn gpu_card<'a>(compat: &'a Value, artifact: &str) -> &'a Value {
    if artifact.is_empty() {
        &compat["embedded"]["gpu"]
    } else {
        &compat["embedded"]["gpuModules"][artifact]
    }
}
/// The artifact that owns surface `name`: the declared module that lists it, else "".
fn artifact_of(compat: &Value, name: &str) -> String {
    let owns = |(_, names): &(&String, &Value)| {
        names
            .as_array()
            .is_some_and(|n| n.iter().any(|n| n == name))
    };
    compat["inputs"]["gpuModules"]
        .as_object()
        .and_then(|modules| modules.iter().find(owns))
        .map(|(module, _)| module.clone())
        .unwrap_or_default()
}
fn verify_module(path: &std::path::Path, compat: &Value, artifact: &str) -> Result<(), String> {
    use sha2::{Digest, Sha256};
    let card = gpu_card(compat, artifact);
    let refuse = |reason: &str| format!("GPU module {}: {reason}", path.display());
    if !card.is_object() {
        return Err(refuse("missing baked identity"));
    }
    for (key, expected) in [("app", &compat["inputs"]["app"]), ("cohort", &compat["id"])] {
        if !expected.is_string() || &card[key] != expected {
            return Err(refuse(&format!("{key} identity mismatch")));
        }
    }
    let receipt;
    let identity = if card["receipt"] == true
        && card["trust"] == "development"
        // No-delivery apps omit trust from the cohort; their baked GPU card
        // still records the explicit development build trust.
        && (compat["inputs"]["trust"].is_null() || compat["inputs"]["trust"] == "development")
    {
        let bytes = std::fs::read(format!("{}.proof.json", path.display()))
            .map_err(|e| refuse(&format!("completed GPU receipt: {e}")))?;
        receipt = serde_json::from_slice::<Value>(&bytes).map_err(|e| refuse(&e.to_string()))?;
        if !receipt["inputs"]
            .as_str()
            .is_some_and(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()))
        {
            return Err(refuse("invalid GPU receipt inputs"));
        }
        &receipt["artifact"]
    } else {
        &card["sha256"]
    };
    let bytes = std::fs::read(path).map_err(|e| refuse(&e.to_string()))?;
    if identity.as_str() != Some(format!("{:x}", Sha256::digest(bytes)).as_str()) {
        return Err(refuse("digest mismatch"));
    }
    Ok(())
}
#[derive(Clone)]
pub(crate) struct ControlBinding {
    pub(crate) view: Option<u32>,
    surface: u32,
    generation: u32,
    name: String,
    offset: (f32, f32),
}
struct Canvas {
    id: u32,
    name: String,
    /// The artifact that created it (LLP 1009 D6): "" the primary.
    artifact: String,
    owner: bool,
    since: u64,
    held: BTreeSet<String>,
    restored_controls: Option<Vec<Value>>,
    restore_error: Option<String>,
    restore_input: bool,
    restore_bytes: Option<Vec<u8>>,
    restore_logged: bool,
}
impl Canvas {
    fn finish_restore(&mut self, error: Option<String>, state: Option<&Value>) -> Option<String> {
        if !self.restore_input {
            return None;
        }
        if let Some(error) = error {
            let error = format!(
                "restore refused: {}",
                error.strip_prefix("restore refused: ").unwrap_or(&error)
            );
            self.restore_error = Some(error.clone());
            self.restore_input = false;
            return Some(error);
        }
        if let Some(state) = state.filter(|s| s["world"]["restored"] == true) {
            self.held = state["world"]["input"]["forwarded"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect();
            self.restored_controls = Some(
                state["world"]["input"]["controlContacts"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default(),
            );
            self.restore_input = false;
            self.restore_bytes = None;
        }
        None
    }
}
#[derive(Default)]
pub(crate) struct Surfaces {
    /// Each opened artifact by name, "" the primary; each is tried once.
    abis: BTreeMap<String, Abi>,
    attempted: BTreeSet<String>,
    canvases: BTreeMap<u32, Canvas>,
    restore: Option<Result<Vec<u8>, String>>,
    restore_read: bool,
    pub(crate) error: Option<String>,
    work: Vec<RequestOut>,
    outcomes: Vec<(u64, Outcome)>,
    /// The canvas pointer's last event (canvas, viewport point), for its motion.
    pointer: Option<(u32, f32, f32)>,
    /// The device's motion for the next canvas pointer event, when it has one.
    motion: Option<(f32, f32)>,
    /// The secondary and middle buttons held (`PointerEvent.buttons` bits).
    aux: u32,
    /// The canvas their first press went to: it hears their release wherever
    /// the pointer is, as the web's pointer capture.
    aux_canvas: Option<u32>,
    /// The agent is driving: its taps and contacts are a finger's.
    finger: bool,
    // postMessage events waiting for a live canvas of their surface name.
    posts: BTreeMap<String, Vec<Value>>,
    // Independent lifecycle causes; also applied before a newly mounted surface
    // can bind or render, including replacement while the window is suspended.
    hidden: bool,
    interrupted: bool,
}
/// Posts held per surface name until a canvas of that name is live; past it a
/// post is dropped and logged. The same bound and rule on every host.
/// Web: glue.js POST_BOUND (gpu-glue.js reads it); Apple: Canvases.postBound.
pub(crate) const POST_BOUND: usize = 64;
impl Surfaces {
    fn lifecycle(&mut self, hidden: bool, interrupted: bool) {
        for canvas in self.canvases.values() {
            let abi = &self.abis[&canvas.artifact];
            if self.hidden != hidden {
                abi.lifecycle(canvas.id, u32::from(!hidden));
            }
            if self.interrupted != interrupted {
                abi.lifecycle(canvas.id, if interrupted { 2 } else { 3 });
            }
        }
        self.hidden = hidden;
        self.interrupted = interrupted;
    }
    fn initial_lifecycle(&self, abi: &Abi, id: u32) {
        if self.hidden {
            abi.lifecycle(id, 0);
        }
        if self.interrupted {
            abi.lifecycle(id, 2);
        }
    }

    pub(crate) fn enqueue(&mut self, request: RequestOut, admitted: &str) {
        let oversized = matches!(
            request.request.surface.as_deref(),
            Some(SurfaceRequest::Restore { bytes, .. }) if bytes.len() > MAX_HOST_WORK_BYTES
        );
        if let Some(message) = oversized
            .then(|| "surface restore exceeds 16 MiB".to_string())
            .or_else(|| request.request.check_surface_grant(admitted).err())
        {
            self.outcomes.push((
                request.ticket,
                Outcome::Failed {
                    kind: FailureKind::Refused,
                    message,
                },
            ));
        } else {
            self.work.push(request);
        }
    }

    pub(crate) fn take_outcomes(&mut self) -> Vec<(u64, Outcome)> {
        std::mem::take(&mut self.outcomes)
    }

    fn sync<D: DataSource>(
        &mut self,
        host: &mut Host<D>,
        compat: &str,
        assets: &crate::image::Assets,
    ) -> bool {
        let mut changed = false;
        let dead: Vec<_> = self
            .canvases
            .keys()
            .copied()
            .filter(|id| host.kernel().node(*id).is_none())
            .collect();
        for view in dead {
            let c = self.canvases.remove(&view).unwrap();
            self.abis[&c.artifact].destroy(c.id);
            if c.owner {
                self.error = self.error.take().or(host.surface_record(&c.name, None));
                changed = true;
            }
        }
        let updates = host.take_surface_updates();
        if updates.is_empty() && self.canvases.is_empty() && !self.work.is_empty() {
            self.outcomes.extend(self.work.drain(..).map(|request| {
                (
                    request.ticket,
                    Outcome::Failed {
                        kind: FailureKind::Refused,
                        message: "surface request has no live surface".into(),
                    },
                )
            }));
            return changed;
        }
        let compat: Value = if updates.is_empty() {
            Value::Null
        } else {
            serde_json::from_str(compat).unwrap_or(Value::Null)
        };
        for artifact in updates.iter().map(|u| artifact_of(&compat, &u.name)) {
            if !self.attempted.insert(artifact.clone()) {
                continue;
            }
            match Abi::open(&compat, &artifact).and_then(|mut abi| {
                if artifact.is_empty() {
                    let pack = abi.prepare_shaders(&compat, assets)?;
                    abi.commit_shaders(pack)?;
                }
                Ok(abi)
            }) {
                Ok(abi) => {
                    let length =
                        unsafe { abi.symbol::<unsafe extern "C" fn() -> u32>(b"gpu_recover")() };
                    let report = abi
                        .bytes(length)
                        .and_then(|b| serde_json::from_slice::<Value>(&b).ok());
                    if !abi.rendered && report.as_ref().is_none_or(|r| r["status"] != "no device") {
                        self.error = Some("headless recovery did not report no device".into());
                    }
                    self.abis.insert(artifact, abi);
                }
                Err(e) => {
                    host.log(format!("surface module unavailable: {e}"));
                    self.error = Some(e);
                }
            }
        }
        if self.abis.is_empty() {
            if !self.attempted.is_empty() {
                self.outcomes.extend(self.work.drain(..).map(|request| {
                    (
                        request.ticket,
                        Outcome::Failed {
                            kind: FailureKind::Unsupported,
                            message: "surface module unavailable".into(),
                        },
                    )
                }));
            }
            return changed;
        };
        for update in updates {
            let artifact = artifact_of(&compat, &update.name);
            if self
                .canvases
                .get(&update.view)
                .is_some_and(|c| c.name != update.name)
            {
                let old = self.canvases.remove(&update.view).unwrap();
                self.abis[&old.artifact].destroy(old.id);
                if old.owner {
                    self.error = self.error.take().or(host.surface_record(&old.name, None));
                    changed = true;
                }
            }
            let Some(abi) = self.abis.get(&artifact) else {
                continue;
            };
            if !self.canvases.contains_key(&update.view) {
                let id = unsafe {
                    abi.symbol::<unsafe extern "C" fn(*const u8, usize) -> u32>(
                        b"gpu_create_headless",
                    )(update.name.as_ptr(), update.name.len())
                };
                if id == 0 {
                    self.error = abi.error();
                    continue;
                }
                self.initial_lifecycle(abi, id);
                let owner = !self.canvases.values().any(|c| c.name == update.name);
                if !owner {
                    host.log(format!(
                        "surface {}: duplicate canvas cannot publish",
                        update.name
                    ));
                }
                self.canvases.insert(
                    update.view,
                    Canvas {
                        id,
                        name: update.name.clone(),
                        artifact,
                        owner,
                        since: 0,
                        held: BTreeSet::new(),
                        restored_controls: None,
                        restore_error: None,
                        restore_input: false,
                        restore_bytes: None,
                        restore_logged: false,
                    },
                );
            }
            let c = self.canvases.get_mut(&update.view).unwrap();
            let values = update.arguments_json();
            let code = unsafe {
                abi.symbol::<unsafe extern "C" fn(u32, *const u8, usize, f64) -> u32>(
                    b"gpu_bind_at",
                )(c.id, values.as_ptr(), values.len(), host.now())
            };
            if code != 0 {
                self.error = abi.error();
                continue;
            }
            if !self.restore_read {
                self.restore_read = true;
                self.restore = std::env::var_os("EXACT_WORLD").map(|path| {
                    std::fs::metadata(&path)
                        .map_err(|e| e.to_string())
                        .and_then(|m| {
                            if m.len() > LIMIT as u64 {
                                Err("world carrier exceeds 256 MiB limit".into())
                            } else {
                                std::fs::read(path).map_err(|e| e.to_string())
                            }
                        })
                });
            }
            if self.restore.is_some()
                && abi.agent(c.id, &json!({"op":"state"}))["world"].is_object()
            {
                let result = self.restore.take().unwrap().and_then(|bytes| {
                    let ok = unsafe {
                        abi.symbol::<unsafe extern "C" fn(u32, *const u8, usize, u32) -> bool>(
                            b"gpu_restore",
                        )(c.id, bytes.as_ptr(), bytes.len(), 0)
                    };
                    c.restore_bytes = Some(bytes);
                    if ok {
                        c.restore_input = true;
                        Ok(())
                    } else {
                        Err(abi.error().unwrap_or("surface refused save".into()))
                    }
                });
                if let Err(e) = result {
                    let e = format!(
                        "restore refused: {}",
                        e.strip_prefix("restore refused: ").unwrap_or(&e)
                    );
                    c.restore_error = Some(e.clone());
                    self.error = Some(e);
                }
            }
        }
        for (&view, c) in &mut self.canvases {
            let abi = &self.abis[&c.artifact];
            let mut delivered = false;
            for _ in 0..16 {
                let names = abi
                    .read(b"gpu_assets", c.id)
                    .and_then(|b| {
                        serde_json::from_slice::<serde_json::Value>(&b)
                            .ok()
                            .and_then(|v| {
                                serde_json::from_value::<Vec<String>>(v["requests"].clone()).ok()
                            })
                    })
                    .unwrap_or_default();
                if names.is_empty() {
                    break;
                }
                for name in names {
                    delivered = true;
                    let bytes = match assets.read_asset(&format!("assets/{name}")) {
                        Ok(bytes) => bytes,
                        Err(reason) => {
                            let ok = unsafe {
                                abi.symbol::<unsafe extern "C" fn(
                                    u32,
                                    *const u8,
                                    usize,
                                    *const u8,
                                    usize,
                                ) -> bool>(b"gpu_asset_failed")(
                                    c.id,
                                    name.as_ptr(),
                                    name.len(),
                                    reason.as_ptr(),
                                    reason.len(),
                                )
                            };
                            if !ok {
                                let error = abi.error().unwrap_or("asset delivery refused".into());
                                self.error =
                                    c.finish_restore(Some(error.clone()), None).or(Some(error));
                            }
                            continue;
                        }
                    };
                    let (ptr, len) = bytes
                        .as_ref()
                        .map_or((std::ptr::null(), 0), |b| (b.as_ptr(), b.len()));
                    let ok = unsafe {
                        abi.symbol::<unsafe extern "C" fn(u32, *const u8, usize, *const u8, usize) -> bool>(b"gpu_asset")(c.id, name.as_ptr(), name.len(), ptr, len)
                    };
                    if !ok {
                        let error = abi.error().unwrap_or("asset delivery refused".into());
                        self.error = c.finish_restore(Some(error.clone()), None).or(Some(error));
                    }
                }
            }
            if delivered {
                // Headless has no first frame to establish the ready world's
                // epoch. Do it after delivery, at the unchanged host clock.
                abi.agent(c.id, &json!({"op":"clock","now":host.now()}));
            }
            if c.restore_input {
                let state = abi.agent(c.id, &json!({"op":"state"}));
                c.finish_restore(None, Some(&state));
            }
            if let Some(bytes) = abi.read(b"gpu_published", c.id) {
                if c.owner {
                    let record = String::from_utf8_lossy(&bytes);
                    self.error = self
                        .error
                        .take()
                        .or(host.surface_record(&c.name, Some(&record)));
                    changed = true;
                }
            }
            if let Some(bytes) = abi.read(b"gpu_messages", c.id) {
                if let Ok(messages) = serde_json::from_slice::<Vec<String>>(&bytes) {
                    for message in messages {
                        // Headless Linux has no audio session or lifecycle to deliver.
                        if message == "exact:audio" {
                            continue;
                        }
                        self.error = self.error.take().or(host.dispatch_at(
                            view,
                            Event::Message(message),
                            host.now(),
                        ));
                        changed = true;
                    }
                }
            }
        }
        let mut completed = Vec::new();
        for request in std::mem::take(&mut self.work) {
            let ticket = request.ticket;
            if !host
                .runner()
                .pending()
                .iter()
                .any(|(_, held)| *held == ticket)
            {
                continue;
            }
            let Some(surface) = request.request.surface else {
                continue;
            };
            let (name, restore) = match *surface {
                SurfaceRequest::Capture { name } => (name, None),
                SurfaceRequest::Restore { name, bytes } => (name, Some(bytes)),
            };
            let matches: Vec<_> = self
                .canvases
                .iter()
                .filter(|(view, canvas)| {
                    canvas.name == name && host.kernel().node(**view).is_some()
                })
                .map(|(view, canvas)| (*view, canvas.id, canvas.artifact.clone()))
                .collect();
            if matches.len() != 1 {
                completed.push((
                    ticket,
                    Outcome::Failed {
                        kind: FailureKind::Refused,
                        message: format!(
                            "surface {name}: expected one live surface, found {}",
                            matches.len()
                        ),
                    },
                ));
                continue;
            }
            let (view, id, ref artifact) = matches[0];
            let abi = &self.abis[artifact];
            if let Some(bytes) = restore {
                let ok = unsafe {
                    abi.symbol::<unsafe extern "C" fn(u32, *const u8, usize, u32) -> bool>(
                        b"gpu_restore",
                    )(id, bytes.as_ptr(), bytes.len(), 0)
                };
                if !ok {
                    completed.push((
                        ticket,
                        Outcome::Failed {
                            kind: FailureKind::Refused,
                            message: abi
                                .error()
                                .unwrap_or_else(|| format!("surface {name}: restore refused")),
                        },
                    ));
                    continue;
                }
                let c = self.canvases.get_mut(&view).unwrap();
                if let Some(bytes) = abi.read(b"gpu_published", id).filter(|_| c.owner) {
                    let record = String::from_utf8_lossy(&bytes);
                    self.error = self
                        .error
                        .take()
                        .or(host.surface_record(&name, Some(&record)));
                    changed = true;
                }
                if let Some(bytes) = abi.read(b"gpu_messages", id) {
                    if let Ok(messages) = serde_json::from_slice::<Vec<String>>(&bytes) {
                        for message in messages.into_iter().filter(|m| m != "exact:audio") {
                            self.error = self.error.take().or(host.dispatch_at(
                                view,
                                Event::Message(message),
                                host.now(),
                            ));
                            changed = true;
                        }
                    }
                }
                completed.push((ticket, Outcome::Surface(SurfaceOutcome::Restored)));
            } else if let Some(bytes) = abi.read_bounded(b"gpu_carry", id, MAX_HOST_WORK_BYTES) {
                completed.push((ticket, Outcome::Surface(SurfaceOutcome::Captured(bytes))));
            } else {
                completed.push((
                    ticket,
                    Outcome::Failed {
                        kind: FailureKind::Unsupported,
                        message: abi
                            .error()
                            .unwrap_or_else(|| format!("surface {name}: carries no state")),
                    },
                ));
            }
        }
        self.outcomes.extend(completed);
        for abi in self.abis.values() {
            if let Some(error) = abi.error() {
                self.error = Some(error);
            }
        }
        changed
    }
    pub(crate) fn placements<D: DataSource>(
        &self,
        host: &Host<D>,
    ) -> BTreeMap<u32, crate::placement::Placement> {
        use crate::placement::Placement;
        let mut result = BTreeMap::new();
        for (&view, canvas) in &self.canvases {
            let (Some(node), Some(abi)) =
                (host.kernel().node(view), self.abis.get(&canvas.artifact))
            else {
                continue;
            };
            if unsafe { abi.symbol::<Read>(b"gpu_children_mode")(canvas.id) } != 3 {
                continue;
            }
            let children = node.children();
            for (i, id) in children.iter().enumerate() {
                let Some(child) = host.kernel().node(*id) else {
                    continue;
                };
                let f = child.frame;
                let name = child.props.str(exact_kernel::PropId::TestId).unwrap_or("");
                unsafe {
                    abi.symbol::<unsafe extern "C" fn(
                        u32,
                        u32,
                        *const u8,
                        usize,
                        f32,
                        f32,
                        f32,
                        f32,
                        u32,
                        u32,
                        *const u8,
                        usize,
                    ) -> u32>(b"gpu_child_view")(
                        canvas.id,
                        i as u32,
                        name.as_ptr(),
                        name.len(),
                        f.x - node.frame.x,
                        f.y - node.frame.y,
                        f.width,
                        f.height,
                        0,
                        0,
                        std::ptr::null(),
                        0,
                    );
                }
            }
            unsafe {
                abi.symbol::<unsafe extern "C" fn(u32, u32) -> u32>(b"gpu_children_count")(
                    canvas.id,
                    children.len() as u32,
                );
            }
            // The no-device executor computes the same placements at the committed
            // viewport/clock, without trying to render a GPU frame.
            abi.agent(canvas.id,&json!({"op":"state","now":host.now(),"width":node.frame.width,"height":node.frame.height}));
            for (i, id) in children.iter().enumerate() {
                let mut h = [0.; 16];
                let code = unsafe {
                    abi.symbol::<unsafe extern "C" fn(u32, u32, *mut f32, usize) -> u32>(
                        b"gpu_placement",
                    )(canvas.id, i as u32, h.as_mut_ptr(), h.len())
                };
                match code {
                    1 => {
                        result.insert(
                            *id,
                            Placement::Visible {
                                h: h[..9].try_into().unwrap(),
                                depth: h[9],
                                clip_depth: [
                                    h[10..13].try_into().unwrap(),
                                    h[13..16].try_into().unwrap(),
                                ],
                                canvas: view,
                            },
                        );
                    }
                    2 => {
                        result.insert(*id, Placement::Hidden);
                    }
                    _ => {}
                }
            }
        }
        result
    }

    /// `postMessage(text, name)`: one message event for the live canvas of that
    /// surface name with the lowest view id, held until one is live. False when
    /// POST_BOUND posts already wait for the name (the post is dropped).
    pub(crate) fn post(&mut self, name: &str, event: Value) -> bool {
        let queue = self.posts.entry(name.into()).or_default();
        if queue.len() >= POST_BOUND {
            return false;
        }
        queue.push(event);
        self.deliver_posts();
        true
    }
    /// Deliver held posts whose surface has a live canvas now, in order.
    pub(crate) fn deliver_posts(&mut self) {
        let names: Vec<String> = self.posts.keys().cloned().collect();
        for name in names {
            let view = self
                .canvases
                .iter()
                .filter(|(_, c)| c.name == name)
                .map(|(view, _)| *view)
                .min();
            if let Some(view) = view {
                for event in self.posts.remove(&name).unwrap_or_default() {
                    // A refusal (an oversized message) is the surface's error.
                    self.input(view, event);
                }
            }
        }
    }
    pub(crate) fn wants_input(&self, view: u32) -> bool {
        self.canvases.get(&view).is_some_and(|c| unsafe {
            self.abis[&c.artifact].symbol::<Read>(b"gpu_wants_input")(c.id) != 0
        })
    }
    fn input(&mut self, view: u32, event: Value) -> bool {
        if let Some(c) = self.canvases.get_mut(&view) {
            let code = event["code"].as_str().unwrap_or_default();
            if event["t"] == "key" && event["down"] == false && !c.held.contains(code) {
                return true;
            }
            let abi = &self.abis[&c.artifact];
            if abi.text(b"gpu_input", c.id, &event.to_string()) != 0 {
                self.error = abi.error();
                return false;
            }
            if event["t"] == "key" {
                if event["down"] == true {
                    c.held.insert(code.into());
                } else {
                    c.held.remove(code);
                }
            } else if event["t"] == "blur" {
                c.held.clear();
            }
            return true;
        }
        false
    }
}
impl<D: DataSource> Presenter<D> {
    /// Presentation visibility and interruption, independent of saved simulation.
    pub fn surface_lifecycle(&mut self, hidden: bool, interrupted: bool) {
        self.surfaces.lifecycle(hidden, interrupted);
    }

    /// Settle mounted GPU surfaces after first pixel and forward their public state.
    pub fn sync_surfaces(&mut self) {
        // LLP 1056: the 2D canvases' draws for this turn's commits.
        self.dirty |= self
            .host
            .sync_canvases(self.brush.scale as f64, false, &self.assets);
        // A publication/message may change the canvas arguments. Drain to a fixed
        // point; an app feedback loop is refused rather than hanging the carrier.
        for _ in 0..16 {
            let changed = self
                .surfaces
                .sync(&mut self.host, &self.compat, &self.assets);
            self.surfaces.deliver_posts();
            self.cancel_removed_controls();
            let outcomes = self.surfaces.take_outcomes();
            if !outcomes.is_empty() {
                let now = self.host.now();
                let error = self.host.fulfill_all(
                    outcomes.into_iter().map(|(t, o)| (t, o, None)).collect(),
                    now,
                );
                let after = self.after_commit();
                if let Some(error) = error.or(after) {
                    self.surfaces.error = Some(error);
                }
                continue;
            }
            if !changed {
                return;
            }
            if let Some(e) = self.after_commit() {
                self.surfaces.error = Some(e);
            }
        }
        self.surfaces.error = Some("surface publication did not settle".into());
    }
    pub(crate) fn surface_input(&mut self, id: u32, event: Value) -> bool {
        self.input_surface(id)
            .is_some_and(|view| self.surfaces.input(view, event))
    }
    /// Wheel targets the painted canvas itself, not its focused/captured input
    /// owner or a child HUD element. A refused ABI delivery is still consumed.
    pub(crate) fn surface_wheel(&mut self, x: f32, y: f32, dx: f32, dy: f32, at: f64) -> bool {
        let Some(view) = self
            .hit(x, y)
            .filter(|view| self.surfaces.wants_input(*view))
        else {
            return false;
        };
        let Some((ox, oy, _, _)) = self.rect_of(view) else {
            return false;
        };
        if !self.surfaces.input(
            view,
            json!({"t":"wheel","dx":dx,"dy":dy,"x":x-ox,"y":y-oy,"at":at}),
        ) {
            self.surfaces
                .error
                .get_or_insert_with(|| "GPU surface refused wheel input".into());
        }
        true
    }
    pub(crate) fn surface_pointer(&mut self, id: u32, x: f32, y: f32, at: f64) -> Option<u32> {
        let view = self.input_surface(id)?;
        self.rect_of(view)?;
        for (phase, buttons) in [("down", 1), ("up", 0)] {
            self.canvas_pointer(view, phase, buttons, x, y, at);
        }
        Some(view)
    }
    /// One mouse pointer event to canvas `view`, at a viewport point, with
    /// the primary button's state. A held secondary or middle button joins
    /// it, and a primary down or up under one is a move, as the web's
    /// chorded buttons are.
    pub(crate) fn canvas_pointer(
        &mut self,
        view: u32,
        phase: &str,
        buttons: u32,
        x: f32,
        y: f32,
        at: f64,
    ) -> bool {
        let aux = self.surfaces.aux;
        let phase = if aux != 0 && matches!(phase, "down" | "up") {
            "move"
        } else {
            phase
        };
        self.send_canvas_pointer(view, phase, buttons | aux, x, y, at)
    }
    fn send_canvas_pointer(
        &mut self,
        view: u32,
        phase: &str,
        buttons: u32,
        x: f32,
        y: f32,
        at: f64,
    ) -> bool {
        let Some((ox, oy, _, _)) = self.rect_of(view) else {
            return false;
        };
        // Without the device's own, a move's motion is its position's change;
        // a down or up at a new point (the agent's, VNC's) is no motion, as the
        // web's pointerdown carries none.
        let (dx, dy) = match (self.surfaces.motion.take(), self.surfaces.pointer) {
            (Some(motion), _) => motion,
            (None, Some((last, lx, ly))) if last == view && phase == "move" => (x - lx, y - ly),
            _ => (0., 0.),
        };
        self.surfaces.pointer = Some((view, x, y));
        // The agent's hover is a mouse with nothing held, as everywhere.
        let hover = phase == "move" && buttons & 1 == 0;
        let kind = if self.surfaces.finger && !hover {
            "touch"
        } else {
            "mouse"
        };
        self.surfaces.input(view, json!({"t":"pointer","id":1,"phase":phase,"kind":kind,"buttons":buttons,"x":x-ox,"y":y-oy,"dx":dx,"dy":dy,"at":at}))
    }
    /// The device's motion for the next canvas pointer event (evdev's
    /// relative axes), so mouse look continues past the screen's edge.
    pub fn raw_motion(&mut self, dx: f32, dy: f32) {
        self.surfaces.motion = Some((dx, dy));
    }
    pub(crate) fn agent_finger(&mut self, on: bool) {
        self.surfaces.finger = on;
    }
    pub(crate) fn has_raw_motion(&self) -> bool {
        self.surfaces.motion.is_some()
    }
    pub(crate) fn clear_raw_motion(&mut self) {
        self.surfaces.motion = None;
    }
    /// The secondary or middle button at a viewport point: the canvas
    /// holding the contact, else the one under the pointer, sees it as the
    /// web does — the first button held is a down, the last released an up.
    pub fn pointer_aux(&mut self, bit: u32, down: bool, x: f32, y: f32, at: f64) {
        let before = self.surfaces.aux;
        self.surfaces.aux = if down { before | bit } else { before & !bit };
        let after = self.surfaces.aux;
        if before == after {
            return;
        }
        let held = self.contact_canvas();
        let captured = self.surfaces.aux_canvas.filter(|_| before != 0);
        let target = held.or(captured).or_else(|| self.hover_canvas(x, y));
        self.surfaces.aux_canvas = target.filter(|_| after != 0);
        let Some(view) = target else {
            return;
        };
        let primary = u32::from(held.is_some());
        let phase = match (before | primary, after | primary) {
            (0, _) => "down",
            (_, 0) => "up",
            _ => "move",
        };
        self.send_canvas_pointer(view, phase, after | primary, x, y, at);
    }
    /// A cancelled pointer (Escape, a lost device, a dropped report) holds no
    /// button: the canvas that heard the secondary or middle press hears a
    /// `cancel` unless `except` (the contact's canvas, cancelled by its caller).
    pub(crate) fn cancel_aux(&mut self, except: Option<u32>, at: f64) {
        let captured = self.surfaces.aux_canvas.take();
        self.surfaces.aux = 0;
        if let Some(view) = captured.filter(|v| Some(*v) != except) {
            let (x, y) = self.surfaces.pointer.map_or((0., 0.), |(_, x, y)| (x, y));
            self.send_canvas_pointer(view, "cancel", 0, x, y, at);
        }
    }
    /// The contact's canvas hears its `cancel` with no button held.
    pub(crate) fn cancel_canvas(&mut self, view: u32, x: f32, y: f32, at: f64) {
        self.send_canvas_pointer(view, "cancel", 0, x, y, at);
    }
    pub(crate) fn surface_request(&mut self, view: u32, mut q: Value) -> Value {
        let rect = self.rect_of(view);
        let Some(c) = self.surfaces.canvases.get_mut(&view) else {
            return json!({"unavailable":true,"device":false});
        };
        let abi = &self.surfaces.abis[&c.artifact];
        q["now"] = self.host.now().into();
        if let Some((x, y, w, h)) = rect.filter(|r| r.2 > 0. && r.3 > 0.) {
            q["width"] = w.into();
            q["height"] = h.into();
            if let Some(v) = q["x"].as_f64() {
                q["x"] = (v - f64::from(x)).into();
            }
            if let Some(v) = q["y"].as_f64() {
                q["y"] = (v - f64::from(y)).into();
            }
        }
        if q["op"] == "screenshot" {
            if q["form"] != "save" {
                return json!({"unavailable":true,"device":false,"reason":"canvas pixels require a device"});
            }
            let state = abi.agent(c.id, &json!({"op":"state","now":self.host.now()}));
            return match abi.read(b"gpu_carry", c.id) {
                Some(bytes) => {
                    json!({"bytes":bytes.len(),"data":base64::engine::general_purpose::STANDARD.encode(&bytes),"tick":state["world"]["tick"],"hash":state["world"]["hash"]})
                }
                None => {
                    json!({"error":abi.error().unwrap_or_else(|| "surface carries no state".into())})
                }
            };
        }
        if q["op"] == "logs" {
            q["since"] = c.since.into();
        }
        let mut r = abi.agent(c.id, &q);
        if q["op"] == "logs" {
            c.since = r["next"].as_u64().unwrap_or(c.since);
            if !c.restore_logged {
                if let Some(e) = c.restore_error.as_ref() {
                    if let Some(lines) = r.get_mut("lines").and_then(Value::as_array_mut) {
                        lines.push(e.clone().into());
                        c.restore_logged = true;
                    }
                }
            }
        }
        if let Some(world) = r.get_mut("world").and_then(Value::as_object_mut) {
            world.insert("canvas".into(), view.into());
            world.insert("device".into(), false.into());
            if let Some(e) = &c.restore_error {
                world.insert("restoreError".into(), e.clone().into());
            }
        }
        if let Some(visible) = r
            .pointer_mut("/entity/visible")
            .and_then(Value::as_object_mut)
        {
            visible.insert("occluded".into(), json!({"unavailable":true}));
        }
        if let Some((x, y, _, _)) = rect {
            if let Some(b) = r
                .pointer_mut("/entity/screen")
                .and_then(Value::as_object_mut)
            {
                for (key, offset) in [("x", x), ("y", y)] {
                    if let Some(v) = b.get(key).and_then(Value::as_f64) {
                        b.insert(key.into(), (v + f64::from(offset)).into());
                    }
                }
            }
        }
        r
    }
    pub(crate) fn worlds(&mut self, q: Value) -> Vec<Value> {
        let views: Vec<_> = self.surfaces.canvases.keys().copied().collect();
        views
            .into_iter()
            .filter_map(|view| {
                let mut r = self.surface_request(view, q.clone());
                if r.is_null() {
                    return None;
                }
                if r["world"].is_object() {
                    r = r["world"].take();
                }
                r["canvas"] = view.into();
                Some(r)
            })
            .collect()
    }
    pub(crate) fn merge_surfaces(&mut self, line: &str, reply: String) -> String {
        if self.surfaces.canvases.is_empty() {
            return reply;
        }
        let q: Value = serde_json::from_str(line).unwrap_or(Value::Null);
        if !matches!(
            q["op"].as_str(),
            Some("tree" | "state" | "logs" | "screenshot")
        ) {
            return reply;
        }
        if q["entity"].is_string()
            || q["world"] == true
            || (q["op"] == "state" && q["id"].is_number())
        {
            return reply;
        }
        let mut r: Value = serde_json::from_str(&reply).unwrap_or(Value::Null);
        if r.get("error").is_some() {
            return reply;
        }
        match q["op"].as_str() {
            Some("tree") => {
                if let Some(nodes) = r["nodes"].as_array_mut() {
                    for row in nodes {
                        if let Some(view) = row["id"]
                            .as_u64()
                            .filter(|v| self.surfaces.canvases.contains_key(&(*v as u32)))
                        {
                            let w = self
                                .surface_request(view as u32, json!({"op":"tree","summary":true}));
                            if w["world"].is_object() {
                                row["world"] = w["world"].clone();
                            }
                        }
                    }
                }
            }
            Some("state") => r["world"] = self.worlds(json!({"op":"state"})).into(),
            Some("logs") => r["world"] = self.worlds(json!({"op":"logs"})).into(),
            Some("screenshot")
                if self.surfaces.canvases.values().any(|canvas| {
                    self.surfaces
                        .abis
                        .get(&canvas.artifact)
                        .is_some_and(|abi| !abi.rendered)
                }) =>
            {
                r["note"] = "Contract painted; canvas rectangles are flat (no device)".into()
            }
            _ => {}
        }
        r.to_string()
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "surface_controls_tests.rs"]
mod control_tests;
