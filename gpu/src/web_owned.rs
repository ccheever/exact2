//! Ownership-only web ABI. No Module/Gpu/texture storage is reachable here.
#![allow(missing_docs)]
use crate::{json, Lifecycle, Registry, Restore, Surface, SurfaceError};
use std::{cell::RefCell, collections::BTreeMap};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsValue;
#[cfg(not(target_arch = "wasm32"))]
type JsValue = String;
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
    let mut text = e.to_string();
    if text.len() > 4096 {
        let mut end = 4096;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
        text.push_str(" (truncated)");
    }
    ERROR.with(|s| *s.borrow_mut() = text);
}
fn output(text: &str) -> Result<(), String> {
    if text.len() > 65_536 {
        Err("surface returned text limit (65536 bytes)".into())
    } else {
        Ok(())
    }
}
fn with<T>(id: u32, f: impl FnOnce(&mut Entry) -> Result<T, String>) -> Option<T> {
    let result = OWNED.with(|m| {
        let mut m = m
            .try_borrow_mut()
            .map_err(|_| "reentrant surface call".to_owned())?;
        let entry = m
            .as_mut()
            .and_then(|m| m.entries.get_mut(&id))
            .ok_or("no such surface")?;
        let result = f(entry)?;
        if let Some(SurfaceError(e)) = entry.surface.take_error() {
            return Err(e);
        }
        let messages = entry.surface.messages();
        let bytes = entry
            .messages
            .iter()
            .chain(&messages)
            .try_fold(0usize, |n, s| n.checked_add(s.len()));
        if messages.len() + entry.messages.len() > 1024 || bytes.is_none_or(|n| n > 65_536) {
            return Err("surface messages limit (1024 / 65536 bytes)".into());
        }
        entry.messages.extend(messages);
        if let Some(value) = entry.surface.published() {
            output(&value)?;
            entry.published = Some(value);
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
pub fn load_headless(registry: &'static Registry) {
    OWNED.with(|m| {
        let Ok(mut m) = m.try_borrow_mut() else {
            refuse("reentrant surface load");
            return;
        };
        m.get_or_insert_with(|| Owned {
            registry,
            entries: BTreeMap::new(),
            next: 0,
            seekable: false,
        });
    });
}
pub fn create_headless(name: &str) -> u32 {
    OWNED.with(|m| {
        let Ok(mut m) = m.try_borrow_mut() else {
            refuse("reentrant surface create");
            return 0;
        };
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
pub fn advance(id: u32, now_ms: f64) -> bool {
    error();
    with(id, |e| Ok(e.surface.advance(now_ms))).unwrap_or(false)
}
pub fn bind(id: u32, text: &str) -> bool {
    bind_at(id, text, None)
}
pub fn bind_at(id: u32, text: &str, at: Option<f64>) -> bool {
    error();
    with(id, |e| {
        crate::binding::admit(text)?;
        let values = crate::binding::values(e.surface.as_ref(), text)?;
        e.surface.bind(&values, at).map_err(|e| e.0)
    })
    .is_some()
}
pub fn input(id: u32, text: &str) -> bool {
    with(id, |e| {
        crate::binding::admit(text)?;
        e.surface.input(&json::parse_input(text)?);
        Ok(())
    })
    .is_some()
}
pub fn agent(id: u32, text: &str) -> String {
    with(id, |e| {
        crate::binding::admit(text)?;
        let reply = e.surface.agent(text).unwrap_or_default();
        // Keep the structured agent reply when this call discovers a tick failure.
        if let Some(SurfaceError(error)) = e.surface.take_error() {
            if !crate::advance::error_reply(Some(&reply)) {
                return Err(error);
            }
            refuse(error);
        }
        output(&reply)?;
        Ok(reply)
    })
    .unwrap_or_default()
}
pub fn carry(id: u32) -> Result<Option<Vec<u8>>, JsValue> {
    with(id, |e| {
        let bytes = e.surface.carry().map_err(|e| e.0)?;
        if bytes.as_ref().is_some_and(|b| b.len() > 256 * 1024 * 1024) {
            return Err("surface carry limit (256 MiB)".into());
        }
        Ok(bytes)
    })
    .ok_or_else(|| {
        #[cfg(target_arch = "wasm32")]
        {
            JsValue::from_str(&error())
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            error()
        }
    })
}
pub fn restore(id: u32, bytes: &[u8], mode: u32) -> bool {
    with(id, |e| {
        if bytes.len() > 256 * 1024 * 1024 {
            return Err("surface restore limit (256 MiB)".into());
        }
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
        let Ok(mut m) = m.try_borrow_mut() else {
            refuse("reentrant surface seekable");
            return;
        };
        if let Some(m) = m.as_mut() {
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
        let value =
            (!e.messages.is_empty()).then(|| json::strings(&std::mem::take(&mut e.messages)));
        if let Some(text) = &value {
            output(text)?;
        }
        Ok(value)
    })
    .flatten()
}
pub fn destroy(id: u32) {
    OWNED.with(|m| {
        let Ok(mut m) = m.try_borrow_mut() else {
            refuse("reentrant surface destroy");
            return;
        };
        if let Some(m) = m.as_mut() {
            m.entries.remove(&id);
        }
    });
}
pub fn unload() {
    OWNED.with(|m| {
        let Ok(mut m) = m.try_borrow_mut() else {
            refuse("reentrant surface unload");
            return;
        };
        *m = None;
    });
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

#[cfg(test)]
mod tests {
    use super::*;
    struct Reentrant;
    impl Surface for Reentrant {
        fn render(
            &mut self,
            _: &crate::Frame,
            _: &crate::wgpu::Device,
            _: &crate::wgpu::Queue,
            _: &crate::wgpu::TextureView,
            _: crate::wgpu::TextureFormat,
        ) -> bool {
            false
        }
        fn bind(&mut self, _: &[crate::Value], _: Option<f64>) -> Result<(), SurfaceError> {
            Ok(())
        }
        fn agent(&mut self, _: &str) -> Option<String> {
            unload();
            Some("alive".into())
        }
    }
    #[test]
    fn reentrant_game_callback_refuses_without_aborting_sibling_worlds() {
        static REGISTRY: Registry = Registry {
            surfaces: &[("reentrant", 0, || Box::new(Reentrant))],
            shaders: &[],
        };
        unload();
        load_headless(&REGISTRY);
        let id = create_headless("reentrant");
        assert_eq!(agent(id, "{}"), "alive");
        assert!(error().contains("reentrant"));
        assert_ne!(create_headless("reentrant"), 0);
        unload();
    }
    struct Large;
    impl Surface for Large {
        fn render(
            &mut self,
            _: &crate::Frame,
            _: &crate::wgpu::Device,
            _: &crate::wgpu::Queue,
            _: &crate::wgpu::TextureView,
            _: crate::wgpu::TextureFormat,
        ) -> bool {
            false
        }
        fn bind(&mut self, _: &[crate::Value], _: Option<f64>) -> Result<(), SurfaceError> {
            Ok(())
        }
        fn agent(&mut self, _: &str) -> Option<String> {
            Some("x".repeat(65_537))
        }
        fn published(&mut self) -> Option<String> {
            Some("x".repeat(65_537))
        }
        fn messages(&mut self) -> Vec<String> {
            vec!["x".repeat(65_537)]
        }
    }
    static REGISTRY: Registry = Registry {
        surfaces: &[("large", 0, || Box::new(Large))],
        shaders: &[],
    };
    #[test]
    fn returned_data_is_bounded_before_crossing_the_owned_abi() {
        unload();
        load_headless(&REGISTRY);
        let id = create_headless("large");
        assert_eq!(agent(id, "{}"), "");
        assert!(error().contains("limit"));
        assert!(published(id).is_none());
        assert!(error().contains("limit"));
        assert!(messages(id).is_none());
        assert!(error().contains("limit"));
        unload();
    }
}
