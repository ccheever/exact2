//! Ownership-only web ABI. No Module/Gpu/texture storage is reachable here.
#![allow(missing_docs)]
use crate::{json, Lifecycle, Registry, Restore, Surface, SurfaceError};
use std::{cell::RefCell, collections::BTreeMap};
use wasm_bindgen::JsValue;
struct Entry {
    surface: Box<dyn Surface>,
    messages: Vec<String>,
    published: Option<String>,
}
struct Owned {
    registry: &'static Registry,
    entries: BTreeMap<u32, Entry>,
    next: u32,
    seekable: bool,
}
thread_local! {
    static OWNED: RefCell<Option<Owned>> = const { RefCell::new(None) };
    static ERROR: RefCell<String> = const { RefCell::new(String::new()) };
}
fn refuse(e: impl ToString) {
    ERROR.with(|s| *s.borrow_mut() = e.to_string());
}
fn with<T>(id: u32, f: impl FnOnce(&mut Entry) -> Result<T, String>) -> Option<T> {
    let result = OWNED.with(|m| {
        let mut m = m.borrow_mut();
        let entry = m
            .as_mut()
            .and_then(|m| m.entries.get_mut(&id))
            .ok_or("no such surface")?;
        let result = f(entry)?;
        entry.messages.extend(entry.surface.messages());
        if let Some(value) = entry.surface.published() {
            entry.published = Some(value);
        }
        if let Some(SurfaceError(e)) = entry.surface.take_error() {
            return Err(e);
        }
        Ok(result)
    });
    match result {
        Ok(v) => Some(v),
        Err(e) => {
            refuse(e);
            None
        }
    }
}
fn admit(text: &str) -> Result<(), String> {
    if text.len() > 16_384 {
        return Err("surface request exceeds 16384 bytes".into());
    }
    let (mut depth, mut string, mut escape) = (0u32, false, false);
    for b in text.bytes() {
        if string {
            if escape {
                escape = false;
            } else if b == b'\\' {
                escape = true;
            } else if b == b'"' {
                string = false;
            }
        } else {
            match b {
                b'"' => string = true,
                b'[' | b'{' => {
                    depth += 1;
                    if depth > 64 {
                        return Err("surface request exceeds depth 64".into());
                    }
                }
                b']' | b'}' => depth = depth.saturating_sub(1),
                _ => {}
            }
        }
    }
    Ok(())
}
pub fn load_headless(registry: &'static Registry) {
    OWNED.with(|m| {
        m.borrow_mut().get_or_insert_with(|| Owned {
            registry,
            entries: BTreeMap::new(),
            next: 0,
            seekable: false,
        });
    });
}
pub fn create_headless(name: &str) -> u32 {
    OWNED.with(|m| {
        let mut m = m.borrow_mut();
        let Some(m) = m.as_mut() else {
            refuse("module not owned");
            return 0;
        };
        if m.entries.len() == 256 || m.next == u32::MAX {
            refuse("surface ownership limit (256)");
            return 0;
        }
        let Some((_, _, factory)) = m.registry.surfaces.iter().find(|(n, _, _)| *n == name) else {
            refuse("unknown surface name");
            return 0;
        };
        let mut surface = factory();
        surface.clock(m.seekable);
        m.next += 1;
        m.entries.insert(
            m.next,
            Entry {
                surface,
                messages: Vec::new(),
                published: None,
            },
        );
        m.next
    })
}
pub fn bind(id: u32, text: &str) -> bool {
    bind_at(id, text, None)
}
pub fn bind_at(id: u32, text: &str, at: Option<f64>) -> bool {
    with(id, |e| {
        admit(text)?;
        let values = crate::binding::values(e.surface.as_ref(), text)?;
        e.surface.bind(&values, at).map_err(|e| e.0)
    })
    .is_some()
}
pub fn input(id: u32, text: &str) -> bool {
    with(id, |e| {
        admit(text)?;
        e.surface.input(&json::parse_input(text)?);
        Ok(())
    })
    .is_some()
}
pub fn agent(id: u32, text: &str) -> String {
    with(id, |e| {
        admit(text)?;
        let reply = e.surface.agent(text).unwrap_or_default();
        // Keep the structured agent reply when this call discovers a tick failure.
        if let Some(SurfaceError(error)) = e.surface.take_error() {
            refuse(error);
        }
        Ok(reply)
    })
    .unwrap_or_default()
}
pub fn carry(id: u32) -> Result<Option<Vec<u8>>, JsValue> {
    with(id, |e| e.surface.carry().map_err(|e| e.0)).ok_or_else(|| JsValue::from_str(&error()))
}
pub fn restore(id: u32, bytes: &[u8], mode: u32) -> bool {
    with(id, |e| {
        e.surface.restore(
            bytes,
            match mode {
                0 => Restore::Open,
                1 => Restore::Carry,
                _ => return Err("invalid restore mode".into()),
            },
        )
    })
    .is_some()
}
pub fn lifecycle(id: u32, code: u32) {
    with(id, |e| {
        match code {
            0 => e.surface.lifecycle(Lifecycle::Hidden),
            1 => e.surface.lifecycle(Lifecycle::Visible),
            2 => e.surface.lifecycle(Lifecycle::Interrupted),
            3 => e.surface.lifecycle(Lifecycle::Resumed),
            _ => {}
        }
        Ok(())
    });
}
pub fn seekable(on: bool) {
    OWNED.with(|m| {
        if let Some(m) = m.borrow_mut().as_mut() {
            m.seekable = on;
            for e in m.entries.values_mut() {
                e.surface.clock(on);
            }
        }
    });
}
pub fn wants_input(id: u32) -> bool {
    with(id, |e| Ok(e.surface.wants_input())).unwrap_or(false)
}
pub fn published(id: u32) -> Option<String> {
    with(id, |e| Ok(e.published.take())).flatten()
}
pub fn messages(id: u32) -> Option<String> {
    with(id, |e| {
        Ok((!e.messages.is_empty()).then(|| json::strings(&std::mem::take(&mut e.messages))))
    })
    .flatten()
}
pub fn destroy(id: u32) {
    OWNED.with(|m| {
        if let Some(m) = m.borrow_mut().as_mut() {
            m.entries.remove(&id);
        }
    });
}
pub fn unload() {
    OWNED.with(|m| *m.borrow_mut() = None);
}
pub fn error() -> String {
    ERROR.with(|s| std::mem::take(&mut *s.borrow_mut()))
}
// This registration is specifically for surfaces with no presentation or assets.
pub fn period(_: f64) {}
pub fn dirty(_: u32) -> bool {
    false
}
pub fn children_mode(_: u32) -> u32 {
    0
}
pub fn child(_: u32, _: u32, _: &str, _: [f32; 4]) -> bool {
    true
}
pub fn children_count(_: u32, _: u32) -> bool {
    true
}
pub fn placement(_: u32, _: u32, _: &mut [f32]) -> u32 {
    0
}
pub fn assets(_: u32) -> String {
    "{\"requests\":[],\"retired\":[]}".into()
}
pub fn asset(_: u32, _: &str, _: Option<&[u8]>) -> bool {
    refuse("device-free module has no assets");
    false
}
pub fn asset_failed(_: u32, _: &str, _: &str) -> bool {
    refuse("device-free module has no assets");
    false
}
