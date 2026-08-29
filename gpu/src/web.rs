//! The web ABI: wasm-bindgen exports the glue calls after the module's
//! wasm is fetched on demand (LLP 1009 D2).

use crate::{json, Frame, Module, Registry};
use std::cell::RefCell;
use wasm_bindgen::prelude::*;

thread_local! {
    static MODULE: RefCell<Option<Module>> = const { RefCell::new(None) };
    static ERROR: RefCell<String> = const { RefCell::new(String::new()) };
}

fn with<T>(f: impl FnOnce(&mut Module) -> T) -> Option<T> {
    MODULE.with(|m| m.borrow_mut().as_mut().map(f))
}

/// Create the device and the module (asynchronous: WebGPU's adapter and
/// device requests are).
pub async fn load(registry: &'static Registry) -> Result<(), JsValue> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::BROWSER_WEBGPU,
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    match crate::load_gpu(instance, None).await {
        Ok(gpu) => {
            let mut module = Module::new(registry);
            module.set_gpu(gpu);
            MODULE.with(|m| *m.borrow_mut() = Some(module));
            Ok(())
        }
        Err(e) => {
            ERROR.with(|s| *s.borrow_mut() = e.clone());
            Err(JsValue::from_str(&e))
        }
    }
}

/// Create a canvas's surface on a `<canvas>` element. The id, or 0.
pub fn create(name: &str, canvas: web_sys::HtmlCanvasElement, width: u32, height: u32) -> u32 {
    with(|m| {
        let gpu = m.gpu()?;
        match gpu
            .instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas))
        {
            Ok(t) => m.create(name, t, width, height),
            Err(e) => {
                ERROR.with(|s| *s.borrow_mut() = format!("{e}"));
                None
            }
        }
    })
    .flatten()
    .unwrap_or(0)
}

/// Bind inputs (a JSON array). `true` on success.
pub fn bind(id: u32, values: &str) -> bool {
    match json::parse_values(values) {
        Ok(v) => with(|m| m.bind(id, &v)).unwrap_or(false),
        Err(e) => {
            ERROR.with(|s| *s.borrow_mut() = e);
            false
        }
    }
}

/// Render one frame: 1 = wants another, 0 = done, 2 = failed.
pub fn render(id: u32, width: f32, height: f32, scale: f32, now_ms: f64) -> u32 {
    let frame = Frame {
        width,
        height,
        scale,
        now_ms,
        children_generation: 0,
    };
    match with(|m| m.render(id, &frame)).flatten() {
        Some(true) => 1,
        Some(false) => 0,
        None => 2,
    }
}

/// Whether a canvas has unrendered inputs.
pub fn dirty(id: u32) -> bool {
    with(|m| m.dirty(id)).unwrap_or(false)
}

/// Drop a canvas's surface.
pub fn destroy(id: u32) {
    with(|m| m.destroy(id));
}

/// The last failure's text.
pub fn error() -> String {
    let own = ERROR.with(|s| s.borrow().clone());
    if !own.is_empty() {
        return own;
    }
    with(|m| m.error().to_string()).unwrap_or_default()
}

/// The exports for one app's registry, wasm-bindgen (see LLP 1009 D2).
#[macro_export]
macro_rules! module {
    ($registry:expr) => {
        /// Create the device.
        #[::wasm_bindgen::prelude::wasm_bindgen]
        pub async fn gpu_load() -> Result<(), ::wasm_bindgen::JsValue> {
            $crate::web::load(&$registry).await
        }

        /// Create a canvas's surface on a `<canvas>`. The id, or 0.
        #[::wasm_bindgen::prelude::wasm_bindgen]
        pub fn gpu_create(
            name: &str,
            canvas: ::web_sys::HtmlCanvasElement,
            width: u32,
            height: u32,
        ) -> u32 {
            $crate::web::create(name, canvas, width, height)
        }

        /// Bind inputs (a JSON array). `true` on success.
        #[::wasm_bindgen::prelude::wasm_bindgen]
        pub fn gpu_bind(id: u32, values: &str) -> bool {
            $crate::web::bind(id, values)
        }

        /// Render one frame: 1 = wants another, 0 = done, 2 = failed.
        #[::wasm_bindgen::prelude::wasm_bindgen]
        pub fn gpu_render(id: u32, width: f32, height: f32, scale: f32, now_ms: f64) -> u32 {
            $crate::web::render(id, width, height, scale, now_ms)
        }

        /// Whether a canvas has unrendered inputs.
        #[::wasm_bindgen::prelude::wasm_bindgen]
        pub fn gpu_dirty(id: u32) -> bool {
            $crate::web::dirty(id)
        }

        /// Drop a canvas's surface.
        #[::wasm_bindgen::prelude::wasm_bindgen]
        pub fn gpu_destroy(id: u32) {
            $crate::web::destroy(id)
        }

        /// The last failure's text.
        #[::wasm_bindgen::prelude::wasm_bindgen]
        pub fn gpu_error() -> String {
            $crate::web::error()
        }
    };
}
