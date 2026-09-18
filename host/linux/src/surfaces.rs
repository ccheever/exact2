//! Device-free canvas ownership over the same dynamic module ABI as Apple.
//! @ref LLP 1041.001 D7; LLP 1015 §7
#![allow(unsafe_code)]
use crate::{Host, Presenter};
use base64::Engine;
use exact_runner::{DataSource, Event};
use libloading::Library;
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};

const LIMIT: usize = 256 * 1024 * 1024;
type Read = unsafe extern "C" fn(u32) -> u32;
type Text = unsafe extern "C" fn(u32, *const u8, usize) -> u32;

struct Abi {
    // Symbols never outlive this library; unload TLS before dlclose.
    library: Library,
}
impl Abi {
    fn open() -> Result<Self, String> {
        let binary = std::env::current_exe().map_err(|e| e.to_string())?;
        let stem = binary.file_stem().unwrap().to_string_lossy();
        let name = stem
            .strip_suffix("-linux")
            .unwrap_or(&stem)
            .replace('-', "_");
        let path = std::env::var_os("EXACT_GPU_MODULE")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                binary.with_file_name(format!("lib{name}_gpu.{}", std::env::consts::DLL_EXTENSION))
            });
        // SAFETY: the app's own module, with the ABI checked before any call.
        let abi = Self {
            library: unsafe { Library::new(path) }.map_err(|e| e.to_string())?,
        };
        unsafe {
            for name in [
                "gpu_load_headless",
                "gpu_unload",
                "gpu_create_headless",
                "gpu_bind_at",
                "gpu_agent",
                "gpu_input",
                "gpu_wants_input",
                "gpu_published",
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
        assert!(len as usize <= LIMIT, "surface output exceeds 256 MiB");
        // SAFETY: the ABI buffer is readable until the next call, copied now.
        Some(unsafe {
            let ptr = self.symbol::<unsafe extern "C" fn() -> *const u8>(b"gpu_out_ptr")();
            std::slice::from_raw_parts(ptr, len as usize).to_vec()
        })
    }
    fn read(&self, name: &[u8], id: u32) -> Option<Vec<u8>> {
        self.bytes(unsafe { self.symbol::<Read>(name)(id) })
    }
    fn text(&self, name: &[u8], id: u32, text: &str) -> u32 {
        unsafe { self.symbol::<Text>(name)(id, text.as_ptr(), text.len()) }
    }
    fn error(&self) -> Option<String> {
        let n = unsafe { self.symbol::<unsafe extern "C" fn() -> u32>(b"gpu_error")() };
        (n > 0).then(|| String::from_utf8_lossy(&self.bytes(n).unwrap()).into_owned())
    }
    fn agent(&self, id: u32, q: &Value) -> Value {
        let n = self.text(b"gpu_agent", id, &q.to_string());
        let bytes = self.bytes(n).unwrap();
        if let Some(error) = self.error() {
            return json!({"error":error});
        }
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
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
struct Canvas {
    id: u32,
    name: String,
    owner: bool,
    since: u64,
    held: BTreeSet<String>,
    restore_error: Option<String>,
    restore_logged: bool,
}
#[derive(Default)]
pub(crate) struct Surfaces {
    abi: Option<Abi>,
    attempted: bool,
    canvases: BTreeMap<u32, Canvas>,
    restore: Option<Result<Vec<u8>, String>>,
    restore_read: bool,
    pub(crate) error: Option<String>,
}
impl Surfaces {
    fn sync<D: DataSource>(&mut self, host: &mut Host<D>) -> bool {
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
        if !updates.is_empty() && !self.attempted {
            self.attempted = true;
            match Abi::open() {
                Ok(abi) => self.abi = Some(abi),
                Err(e) => host.log(format!("surface module unavailable: {e}")),
            }
        }
        let Some(abi) = &self.abi else {
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
                        restore_error: None,
                        restore_logged: false,
                    },
                );
            }
            let c = self.canvases.get_mut(&update.view).unwrap();
            let values = value_json(&exact_runner::Value::List(update.values.into())).to_string();
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
            if self.restore.is_some() && abi.read(b"gpu_carry", c.id).is_some() {
                let result = self.restore.take().unwrap().and_then(|bytes| {
                    let ok = unsafe {
                        abi.symbol::<unsafe extern "C" fn(u32, *const u8, usize) -> bool>(
                            b"gpu_restore",
                        )(c.id, bytes.as_ptr(), bytes.len())
                    };
                    if ok {
                        let state = abi.agent(c.id, &json!({"op":"state"}));
                        c.held = state["world"]["input"]["forwarded"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter_map(|v| v.as_str().map(str::to_owned))
                            .collect();
                        Ok(())
                    } else {
                        Err(abi.error().unwrap_or("surface refused save".into()))
                    }
                });
                if let Err(e) = result {
                    let e = format!("restore refused: {e}");
                    c.restore_error = Some(e.clone());
                    self.error = Some(e);
                }
            }
        }
        for (&view, c) in &self.canvases {
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
        changed
    }
    fn wants_input(&self, view: u32) -> bool {
        self.canvases.get(&view).is_some_and(|c| unsafe {
            self.abi
                .as_ref()
                .unwrap()
                .symbol::<Read>(b"gpu_wants_input")(c.id)
                != 0
        })
    }
    fn input(&mut self, view: u32, event: Value) {
        if let Some(c) = self.canvases.get_mut(&view) {
            if event["t"] == "key" {
                let code = event["code"].as_str().unwrap_or_default().to_string();
                if event["down"] == true {
                    c.held.insert(code);
                } else if !c.held.remove(&code) {
                    return;
                }
            }
            let abi = self.abi.as_ref().unwrap();
            if abi.text(b"gpu_input", c.id, &event.to_string()) != 0 {
                self.error = abi.error();
            }
        }
    }
}
impl<D: DataSource> Presenter<D> {
    pub(crate) fn sync_surfaces(&mut self) {
        // A publication/message may change the canvas arguments. Drain to a fixed
        // point; an app feedback loop is refused rather than hanging the carrier.
        for _ in 0..16 {
            if !self.surfaces.sync(&mut self.host) {
                return;
            }
            if let Some(e) = self.after_commit() {
                self.surfaces.error = Some(e);
            }
        }
        self.surfaces.error = Some("surface publication did not settle".into());
    }
    pub(crate) fn surface_input(&mut self, id: u32, event: Value) -> bool {
        let mut cursor = Some(id);
        while let Some(view) = cursor {
            if self.surfaces.wants_input(view) {
                self.surfaces.input(view, event);
                return true;
            }
            cursor = self.host.kernel().node(view).and_then(|n| n.parent);
        }
        false
    }
    pub(crate) fn surface_pointer(&mut self, id: u32, x: f32, y: f32, at: f64) -> Option<u32> {
        let mut cursor = Some(id);
        while let Some(view) = cursor {
            if self.surfaces.wants_input(view) {
                let (ox, oy, _, _) = self.rect_of(view)?;
                for (phase, buttons) in [("down", 1), ("up", 0)] {
                    self.surfaces.input(view, json!({"t":"pointer","id":1,"phase":phase,"kind":"mouse","buttons":buttons,"x":x-ox,"y":y-oy,"at":at}));
                }
                return Some(view);
            }
            cursor = self.host.kernel().node(view).and_then(|n| n.parent);
        }
        None
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
                None => json!({"error":"surface carries no state"}),
            };
        }
        if q["op"] == "layout" && !q["entity"].is_string() {
            return json!({"unavailable":true,"device":false,"reason":"canvas picks require a device"});
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

fn value_json(v: &exact_runner::Value) -> Value {
    use exact_runner::Value as V;
    match v {
        V::Number(n) => json!(n),
        V::Bool(b) => json!(b),
        V::Str(s) => json!(s.as_ref()),
        V::List(items) | V::Record(items) => items.iter().map(value_json).collect(),
        V::Option(Some(v)) => value_json(v),
        _ => Value::Null,
    }
}
