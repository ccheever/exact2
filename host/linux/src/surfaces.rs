//! Device-free canvas ownership over the same dynamic module ABI as Apple.
//! @ref LLP 1041.001 D7; LLP 1015 §7
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
mod controls;

const LIMIT: usize = 256 * 1024 * 1024;
type Read = unsafe extern "C" fn(u32) -> u32;
type Text = unsafe extern "C" fn(u32, *const u8, usize) -> u32;

struct Abi {
    // Symbols never outlive this library; unload TLS before dlclose.
    library: Library,
    output_error: std::cell::RefCell<Option<String>>,
}
impl Abi {
    fn open(compat: &Value) -> Result<Self, String> {
        let binary = std::env::current_exe().map_err(|e| e.to_string())?;
        let card = &compat["embedded"]["gpu"];
        let name = card["name"]
            .as_str()
            .ok_or("GPU module has no baked identity")?;
        let path = module_path(
            &binary.with_file_name(name),
            compat,
            std::env::var_os("EXACT_GPU_MODULE").map(PathBuf::from),
        );
        Self::open_path(&path, compat)
    }
    fn open_path(path: &std::path::Path, compat: &Value) -> Result<Self, String> {
        verify_module(path, compat)?;
        // SAFETY: the app's own module, with the ABI checked before any call.
        let abi = Self {
            library: unsafe { Library::new(path) }.map_err(|e| e.to_string())?,
            output_error: Default::default(),
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
                    .map_err(|e| e.to_string())?;
            }
            abi.symbol::<unsafe extern "C" fn()>(b"gpu_load_headless")();
        }
        Ok(abi)
    }
    // SAFETY: all callers supply the signature declared by gpu/src/native.rs.
    unsafe fn symbol<T: Copy>(&self, name: &[u8]) -> T {
        *unsafe { self.library.get::<T>(name) }.expect("validated module ABI")
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
fn verify_module(path: &std::path::Path, compat: &Value) -> Result<(), String> {
    use sha2::{Digest, Sha256};
    let card = &compat["embedded"]["gpu"];
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
    abi: Option<Abi>,
    attempted: bool,
    canvases: BTreeMap<u32, Canvas>,
    restore: Option<Result<Vec<u8>, String>>,
    restore_read: bool,
    pub(crate) error: Option<String>,
    work: Vec<RequestOut>,
    outcomes: Vec<(u64, Outcome)>,
}
impl Surfaces {
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
            self.abi.as_ref().unwrap().destroy(c.id);
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
        if !updates.is_empty() && !self.attempted {
            self.attempted = true;
            match Abi::open(&serde_json::from_str(compat).unwrap_or(Value::Null)) {
                Ok(abi) => {
                    let length =
                        unsafe { abi.symbol::<unsafe extern "C" fn() -> u32>(b"gpu_recover")() };
                    let report = abi
                        .bytes(length)
                        .and_then(|b| serde_json::from_slice::<Value>(&b).ok());
                    if report.as_ref().is_none_or(|r| r["status"] != "no device") {
                        self.error = Some("headless recovery did not report no device".into());
                    }
                    self.abi = Some(abi);
                }
                Err(e) => host.log(format!("surface module unavailable: {e}")),
            }
        }
        let Some(abi) = &self.abi else {
            if self.attempted {
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
            if self
                .canvases
                .get(&update.view)
                .is_some_and(|c| c.name != update.name)
            {
                let old = self.canvases.remove(&update.view).unwrap();
                abi.destroy(old.id);
                if old.owner {
                    self.error = self.error.take().or(host.surface_record(&old.name, None));
                    changed = true;
                }
            }
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
                .map(|(view, canvas)| (*view, canvas.id))
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
            let (view, id) = matches[0];
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
        if let Some(error) = abi.error() {
            self.error = Some(error);
        }
        changed
    }
    pub(crate) fn placements<D: DataSource>(
        &self,
        host: &Host<D>,
    ) -> BTreeMap<u32, crate::placement::Placement> {
        use crate::placement::Placement;
        let mut result = BTreeMap::new();
        let Some(abi) = &self.abi else {
            return result;
        };
        for (&view, canvas) in &self.canvases {
            let Some(node) = host.kernel().node(view) else {
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

    pub(crate) fn wants_input(&self, view: u32) -> bool {
        self.canvases.get(&view).is_some_and(|c| unsafe {
            self.abi
                .as_ref()
                .unwrap()
                .symbol::<Read>(b"gpu_wants_input")(c.id)
                != 0
        })
    }
    fn input(&mut self, view: u32, event: Value) -> bool {
        if let Some(c) = self.canvases.get_mut(&view) {
            let code = event["code"].as_str().unwrap_or_default();
            if event["t"] == "key" && event["down"] == false && !c.held.contains(code) {
                return true;
            }
            let abi = self.abi.as_ref().unwrap();
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
    pub(crate) fn sync_surfaces(&mut self) {
        // A publication/message may change the canvas arguments. Drain to a fixed
        // point; an app feedback loop is refused rather than hanging the carrier.
        for _ in 0..16 {
            let changed = self
                .surfaces
                .sync(&mut self.host, &self.compat, &self.assets);
            self.cancel_removed_controls();
            let outcomes = self.surfaces.take_outcomes();
            if !outcomes.is_empty() {
                let now = self.host.now();
                let error = self.host.fulfill_all(outcomes, now);
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
    pub(crate) fn surface_pointer(&mut self, id: u32, x: f32, y: f32, at: f64) -> Option<u32> {
        let view = self.input_surface(id)?;
        let (ox, oy, _, _) = self.rect_of(view)?;
        for (phase, buttons) in [("down", 1), ("up", 0)] {
            self.surfaces.input(view, json!({"t":"pointer","id":1,"phase":phase,"kind":"mouse","buttons":buttons,"x":x-ox,"y":y-oy,"at":at}));
        }
        Some(view)
    }
    pub(crate) fn surface_request(&mut self, view: u32, mut q: Value) -> Value {
        let rect = self.rect_of(view);
        let Some(c) = self.surfaces.canvases.get_mut(&view) else {
            return json!({"unavailable":true,"device":false});
        };
        let abi = self.surfaces.abi.as_ref().unwrap();
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
            Some("screenshot") if !self.surfaces.canvases.is_empty() => {
                r["note"] = "Contract painted; canvas rectangles are flat (no device)".into()
            }
            _ => {}
        }
        r.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use exact_runner::Request;
    use sha2::{Digest, Sha256};

    #[test]
    fn surface_work_scope_is_refused_before_presenter_effects() {
        let request = |ticket, mut request: exact_runner::Request| {
            request.grants = Some("surface.read other".into());
            RequestOut {
                ticket,
                target: "save".into(),
                request,
                forced: false,
            }
        };
        let mut surfaces = Surfaces::default();
        surfaces.enqueue(
            request(7, Request::capture_surface("world")),
            "surface.read world\nsurface.read other",
        );
        assert!(surfaces.work.is_empty());
        assert!(matches!(
            surfaces.take_outcomes().as_slice(),
            [(
                7,
                Outcome::Failed {
                    kind: FailureKind::Refused,
                    ..
                }
            )]
        ));

        let mut allowed = Request::capture_surface("world");
        allowed.grants = Some("surface.read world".into());
        surfaces.enqueue(
            RequestOut {
                ticket: 8,
                target: "save".into(),
                request: allowed,
                forced: false,
            },
            "surface.read world",
        );
        assert_eq!(surfaces.work.len(), 1);
    }

    #[test]
    fn deferred_restore_keeps_bytes_until_commit_and_late_refusal_is_once() {
        for refused in [true, false] {
            let mut c = Canvas {
                id: 1,
                name: "world".into(),
                owner: true,
                since: 0,
                held: Default::default(),
                restored_controls: None,
                restore_error: None,
                restore_input: true,
                restore_bytes: Some(vec![1]),
                restore_logged: false,
            };
            c.finish_restore(None, Some(&json!({"world":{"restored":false}})));
            assert_eq!(c.restore_bytes, Some(vec![1]));
            if refused {
                assert!(c
                    .finish_restore(Some("restore refused: invalid save".into()), None)
                    .unwrap()
                    .contains("invalid save"));
                assert!(c
                    .finish_restore(Some("restore refused: invalid save".into()), None)
                    .is_none());
                assert_eq!(
                    c.restore_error.as_deref(),
                    Some("restore refused: invalid save")
                );
                assert_eq!(c.restore_bytes, Some(vec![1]));
            } else {
                c.finish_restore(
                    None,
                    Some(&json!({"world":{"restored":true,"input":{"forwarded":["KeyW"]}}})),
                );
                assert!(c.restore_bytes.is_none());
                assert!(c.held.contains("KeyW"));
            }
            assert!(!c.restore_input);
        }
    }
    pub(super) fn fixture() -> (PathBuf, Value) {
        let dir = std::env::temp_dir().join(format!(
            "exact-gpu-loader-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("libprobe.{}", std::env::consts::DLL_EXTENSION));
        let source = dir.join("probe.c");
        std::fs::write(
            &source,
            r#"
#include <stdint.h>
#include <stdbool.h>
#include <stdio.h>
#include <string.h>
void gpu_load_headless(void) {}
uint32_t gpu_recover(void) { return 0; }
uint32_t gpu_child_view(void) { return 0; }
uint32_t gpu_children_count(void) { return 0; }
uint32_t gpu_children_mode(void) { return 3; }
uint32_t gpu_placement(uint32_t id, uint32_t index, float *h, size_t len) {
  if (len<16) return 0;
  for(int i=0;i<16;i++) h[i]=0;
  h[0]=h[4]=h[8]=h[12]=h[15]=1; h[9]=(float)index;
  return 1;
}
void gpu_unload(void) {}
uint32_t gpu_create_headless(void) { return 1; }
static char bound[4096];
uint32_t gpu_bind_at(uint32_t id, const unsigned char *text, size_t len, double at) {
  if (len >= sizeof(bound)) return 1; memcpy(bound, text, len); bound[len] = 0; return 0;
}
const char *test_bound(void) { return bound; }
static const char reply[] = "{\"hit\":{\"name\":\"cpu\"}}";
uint32_t gpu_agent(uint32_t id, const unsigned char *text, size_t len) {
  for (size_t i=0; i+6<=len; ++i) if (!memcmp(text+i, "layout", 6)) return sizeof(reply)-1;
  return 268435457;
}
static uint32_t cancels = 0;
uint32_t test_cancels(void) { return cancels; }
static char event[2048];
static uint32_t event_id, event_count;
const char *test_input(void) { return event; }
uint32_t test_input_id(void) { return event_id; }
uint32_t test_input_count(void) { return event_count; }
uint32_t gpu_input(uint32_t id, const unsigned char *text, size_t len) {
  if(len>=sizeof(event)) return 1; memcpy(event,text,len);event[len]=0;
  event_id=id; event_count++;
  if (strstr(event,"RejectKey")) return 1;
  if (strstr(event,"cancel") || strstr(event,"blur")) cancels++;
  return strstr(event,"control") && !strstr(event,"jump") ? 1 : 0;
}
uint32_t gpu_wants_input(void) { return 1; }
uint32_t gpu_assets(void) { return 0; }
bool gpu_asset(void) { return true; }
uint32_t gpu_published(void) { return 268435457; }
uint32_t gpu_messages(void) { return 268435457; }
uint32_t gpu_carry(void) { return 268435457; }
uint32_t gpu_restore(void) { return 1; }
void gpu_destroy(void) {}
uint32_t gpu_error(void) { return 0; }
const unsigned char* gpu_out_ptr(void) { return (const unsigned char*)reply; }
"#,
        )
        .unwrap();
        assert!(std::process::Command::new("cc")
            .args(["-shared", "-fPIC"])
            .arg(&source)
            .arg("-o")
            .arg(&path)
            .status()
            .unwrap()
            .success());
        let digest = format!("{:x}", Sha256::digest(std::fs::read(&path).unwrap()));
        let compat = json!({"id":"cohort", "inputs":{"app":"test.app"}, "embedded":{"gpu":{
            "name":path.file_name().unwrap().to_str().unwrap(),"sha256":digest,
            "app":"test.app","cohort":"cohort","trust":"production"}}});
        (path, compat)
    }
    #[test]
    fn module_identity_refuses_stale_foreign_and_missing_bakes_before_loading() {
        let (path, compat) = fixture();
        for (key, value, reason) in [
            ("sha256", "old-product", "digest"),
            ("app", "other.app", "app"),
            ("cohort", "old-cohort", "cohort"),
        ] {
            let mut wrong = compat.clone();
            wrong["embedded"]["gpu"][key] = value.into();
            let error = Abi::open_path(&path, &wrong)
                .err()
                .expect("mismatch must refuse");
            assert!(
                error.contains(reason) && error.contains("libprobe"),
                "{error}"
            );
        }
        assert!(Abi::open_path(&path, &Value::Null).is_err());
        drop(Abi::open_path(&path, &compat).unwrap());
        // A digest-authenticated old child ABI must refuse before any call.
        let source = path.with_file_name("probe.c");
        let old = std::fs::read_to_string(&source)
            .unwrap()
            .replace("gpu_child_view", "gpu_child");
        std::fs::write(&source, old).unwrap();
        assert!(std::process::Command::new("cc")
            .args(["-shared", "-fPIC"])
            .arg(&source)
            .arg("-o")
            .arg(&path)
            .status()
            .unwrap()
            .success());
        let mut old_compat = compat.clone();
        old_compat["embedded"]["gpu"]["sha256"] =
            format!("{:x}", Sha256::digest(std::fs::read(&path).unwrap())).into();
        let error = Abi::open_path(&path, &old_compat)
            .err()
            .expect("old ABI refused");
        assert!(error.contains("gpu_child_view"), "{error}");
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
    #[test]
    fn cpu_canvas_pick_is_forwarded_without_a_device() {
        struct NoData;
        impl DataSource for NoData {
            fn query(
                &mut self,
                name: &str,
                _: &[exact_runner::Value],
            ) -> Result<exact_runner::Value, exact_runner::DataError> {
                Err(exact_runner::DataError::UnknownSource(name.into()))
            }
        }
        let (path, compat) = fixture();
        let plan = contract::compile("component App\n  view\n    text \"test\"\n").unwrap();
        let (mut p, _) = Presenter::boot(
            &plan.encode(),
            NoData,
            (100., 100.),
            1.,
            path.parent().unwrap().into(),
        )
        .unwrap();
        p.surfaces.abi = Some(Abi::open_path(&path, &compat).unwrap());
        p.surfaces.canvases.insert(
            1,
            Canvas {
                id: 1,
                name: "world".into(),
                owner: true,
                since: 0,
                held: BTreeSet::new(),
                restored_controls: None,
                restore_error: None,
                restore_input: false,
                restore_bytes: None,
                restore_logged: false,
            },
        );
        let reply = p.surface_request(1, json!({"op":"layout","x":50,"y":50}));
        assert_eq!(reply["hit"]["name"], "cpu");
        drop(p);
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
    #[test]
    fn controls_own_release_and_focus_and_placed_siblings_cover() {
        #[derive(Default)]
        struct NoData;
        impl DataSource for NoData {
            fn query(
                &mut self,
                n: &str,
                _: &[exact_runner::Value],
            ) -> Result<exact_runner::Value, exact_runner::DataError> {
                Err(exact_runner::DataError::UnknownSource(n.into()))
            }
        }
        let (path, compat) = fixture();
        let source = r#"component Controls
  state renamed = false
  action rename writes renamed
    renamed = true
  view
    column
      canvas testId="world" width=100 height=100
        button testId="jump" action=(renamed ? "light" : "jump") width=100 height=100
      button testId="rename" press=rename width=100 height=40
"#;
        let plan = contract::compile(source).unwrap();
        let (mut p, _) = Presenter::boot(
            &plan.encode(),
            NoData,
            (100., 200.),
            1.,
            path.parent().unwrap().into(),
        )
        .unwrap();
        let find = |p: &Presenter<NoData>, name: &str| {
            p.host
                .preorder()
                .into_iter()
                .find(|id| {
                    p.host
                        .kernel()
                        .node(*id)
                        .unwrap()
                        .props
                        .str(exact_kernel::PropId::TestId)
                        == Some(name)
                })
                .unwrap()
        };
        let canvas = find(&p, "world");
        let button = find(&p, "jump");
        let rename = find(&p, "rename");
        p.surfaces.abi = Some(Abi::open_path(&path, &compat).unwrap());
        p.surfaces.canvases.insert(
            canvas,
            Canvas {
                id: 1,
                name: "world".into(),
                owner: true,
                since: 0,
                held: Default::default(),
                restored_controls: None,
                restore_error: None,
                restore_input: false,
                restore_bytes: None,
                restore_logged: false,
            },
        );
        p.boxes();
        assert!(p.control_input(button, "down", 20., 30., 1, 0.));
        assert_eq!(p.focus(), Some(button));
        p.hardware_key("Space", "Space", true, false);
        assert_eq!(p.control_bindings[&(canvas, u32::MAX - 1)].name, "jump");
        p.hardware_key("Space", "Space", false, false);
        assert!(!p.control_bindings.contains_key(&(canvas, u32::MAX - 1)));
        // A held pointer owns activation keys even when focus is elsewhere.
        // Release it before testing the raw canvas fallback.
        assert!(p.control_input(button, "up", 20., 30., 1, 0.));
        p.focus = None;
        p.hardware_key("Space", "Space", true, false);
        assert!(p.surfaces.canvases[&canvas].held.contains("Space"));
        p.hardware_key("Space", "Space", false, false);
        assert!(p.surfaces.canvases[&canvas].held.is_empty());
        assert!(p.control_input(button, "down", 20., 30., 1, 0.));
        p.focus = Some(button);
        p.hardware_key("Space", "Space", true, false);
        p.host.dispatch_at(rename, Event::Press, 0.);
        p.after_commit();
        p.hardware_key("Space", "Space", true, false); // hardware autorepeat keeps the original press
        assert_eq!(p.control_bindings[&(canvas, u32::MAX - 1)].name, "jump");
        p.hardware_key("Space", "Space", false, false);
        assert!(
            p.control_input(button, "up", 200., 300., 1, 0.),
            "release must still be jump"
        );
        // Restoration uses the same immutable action even after the binary's HUD changed.
        p.surfaces
            .canvases
            .get_mut(&canvas)
            .unwrap()
            .restored_controls = Some(vec![
            json!({"id":4294967294u32,"action":"jump","position":[0,0]}),
        ]);
        assert!(p.type_key(button, "Space", "Space", false, false).is_ok());
        assert!(!p.control_input(button, "down", 20., 30., 3, 0.));
        assert!(
            !p.control_bindings.contains_key(&(canvas, 3)),
            "a refused press owns nothing"
        );
        drop(p);
        let source = r#"component Placed
  state count = 0
  action press writes count
    count = count + 1
  view
    canvas testId="world" width=100 height=100
      button testId="under" width=100 height=100 press=press
      button testId="cover" width=100 height=100 press=press
"#;
        let plan = contract::compile(source).unwrap();
        let (mut p, _) = Presenter::boot(
            &plan.encode(),
            NoData,
            (100., 100.),
            1.,
            path.parent().unwrap().into(),
        )
        .unwrap();
        let canvas = find(&p, "world");
        let under = find(&p, "under");
        p.surfaces.abi = Some(Abi::open_path(&path, &compat).unwrap());
        p.surfaces.canvases.insert(
            canvas,
            Canvas {
                id: 1,
                name: "world".into(),
                owner: true,
                since: 0,
                held: Default::default(),
                restored_controls: None,
                restore_error: None,
                restore_input: false,
                restore_bytes: None,
                restore_logged: false,
            },
        );
        assert!(p.tap(under).unwrap_err().contains("covered or not hit"));
        assert_eq!(
            p.host.runner().slot("count"),
            Some(&exact_runner::Value::Number(0.))
        );
        drop(p);
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
    #[test]
    fn e11_development_module_requires_its_own_completed_receipt() {
        let dir = std::env::temp_dir().join(format!("e11-gpu-receipt-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let product = dir.join("gpu.dylib");
        let receipt = dir.join("gpu.dylib.proof.json");
        std::fs::write(&product, b"module").unwrap();
        // A standalone app without delivery has no trust component in its cohort.
        let mut compat = json!({"id":"cohort","inputs":{"app":"game","trust":null},"embedded":{"gpu":{"app":"game","cohort":"cohort","trust":"development","receipt":true}}});
        assert!(verify_module(&product, &compat).is_err());
        use sha2::{Digest, Sha256};
        std::fs::write(
            &receipt,
            json!({"inputs":"a".repeat(64),"artifact":format!("{:x}",Sha256::digest(b"module"))})
                .to_string(),
        )
        .unwrap();
        verify_module(&product, &compat).unwrap();
        std::fs::write(&product, b"changed").unwrap();
        assert!(verify_module(&product, &compat).is_err());
        std::fs::write(&product, b"module").unwrap();
        compat["inputs"]["trust"] = json!("production");
        assert!(verify_module(&product, &compat).is_err());
        compat["inputs"]["trust"] = Value::Null;
        compat["embedded"]["gpu"]["trust"] = json!("production");
        assert!(verify_module(&product, &compat).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn production_ignores_module_path_override() {
        let default = std::path::Path::new("baked");
        let mut compat = json!({"embedded":{"gpu":{"trust":"production"}}});
        assert_eq!(
            module_path(default, &compat, Some("override".into())),
            default
        );
        compat["embedded"]["gpu"]["trust"] = "development".into();
        assert_eq!(
            module_path(default, &compat, Some("override".into())),
            PathBuf::from("override")
        );
    }
    #[test]
    fn oversized_module_output_is_a_structured_refusal() {
        let (path, compat) = fixture();
        let abi = Abi::open_path(&path, &compat).unwrap();
        let reply = abi.agent(1, &json!({"op":"state"}));
        assert!(reply["error"].as_str().unwrap().contains("256 MiB"));
        for symbol in [b"gpu_carry".as_slice(), b"gpu_published", b"gpu_messages"] {
            assert!(abi.read(symbol, 1).is_none());
            assert!(abi.error().unwrap().contains("256 MiB"));
        }
        assert!(abi.bytes(u32::MAX).is_none());
        assert_eq!(abi.bytes(0), Some(vec![]));
        drop(abi);
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
}

#[cfg(test)]
#[path = "surface_controls_tests.rs"]
mod control_tests;
