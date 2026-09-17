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

/// Register the text of shader `name` (LLP 1030 D8): validated, its
/// interface checked against the one this module's Rust binds. `true` on
/// success; the refusal is [`error`]'s.
pub fn shader(name: &str, text: &str) -> bool {
    match with(|m| m.set_shader(name, text.to_string())) {
        Some(ok) => ok,
        None => {
            ERROR.with(|s| *s.borrow_mut() = "gpu_shader: the module is not loaded".into());
            false
        }
    }
}

/// Validate a candidate registration without changing the live registry.
pub async fn shader_check(name: &str, text: &str) -> bool {
    let device = with(|m| {
        m.expected_digest(name)?;
        m.gpu().map(|gpu| gpu.device.clone())
    })
    .flatten();
    let Some(device) = device else {
        ERROR.with(|s| *s.borrow_mut() = format!("unknown shader `{name}` or GPU unavailable"));
        return false;
    };
    let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(name),
        source: wgpu::ShaderSource::Wgsl(text.into()),
    });
    let validation = scope.pop();
    let info = module.get_compilation_info().await;
    let error = validation.await.map(|error| error.to_string()).or_else(|| {
        info.messages
            .into_iter()
            .find(|message| message.message_type == wgpu::CompilationMessageType::Error)
            .map(|message| message.message)
    });
    if let Some(error) = error {
        ERROR.with(|s| *s.borrow_mut() = format!("shader `{name}` refused: {error}"));
        false
    } else {
        true
    }
}

/// Clear the namespace before installing a fully checked replacement.
pub fn shaders_clear() {
    crate::shaders::clear_shaders();
}

/// The shaders this module's surfaces bind against, as a JSON list of names
/// — what the glue fetches (`./shaders/<name>.wgsl`) and registers before a
/// surface is created. `[]` before the module is loaded.
pub fn shader_names() -> String {
    let names = with(|m| m.shader_names()).unwrap_or_default();
    let mut s = String::from("[");
    for (i, n) in names.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push('"');
        s.push_str(n);
        s.push('"');
    }
    s.push(']');
    s
}

/// Bind inputs (a JSON array). `true` on success.
pub fn bind(id: u32, values: &str) -> bool {
    bind_at(id, values, None)
}

/// Bind inputs at an optional host commit clock.
pub fn bind_at(id: u32, values: &str, at_ms: Option<f64>) -> bool {
    match json::parse_values(values) {
        Ok(v) => with(|m| m.bind(id, &v, at_ms)).unwrap_or(false),
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
        seekable: false,
        shader_generation: 0,
    };
    match with(|m| m.render(id, &frame)).flatten() {
        Some(true) => 1,
        Some(false) => 0,
        None => 2,
    }
}

/// Whether a canvas wants raw input.
pub fn wants_input(id: u32) -> bool {
    with(|m| m.wants_input(id)).unwrap_or(false)
}

/// Deliver a JSON device event. True on success.
pub fn input(id: u32, event: &str) -> bool {
    with(|m| m.input_json(id, event)).unwrap_or(false)
}

/// Drain posted messages as a JSON array.
pub fn messages(id: u32) -> String {
    json::strings(&with(|m| m.take_messages(id)).unwrap_or_default())
}

/// Ask the surface; an empty string means no answer.
pub fn agent(id: u32, request: &str) -> String {
    with(|m| m.agent(id, request)).flatten().unwrap_or_default()
}

/// Set the host's clock ownership.
pub fn seekable(on: bool) {
    with(|m| m.set_seekable(on));
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
    let own = ERROR.with(|s| std::mem::take(&mut *s.borrow_mut()));
    if !own.is_empty() {
        return own;
    }
    with(|m| m.take_error()).unwrap_or_default()
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

        /// Register a shader's text (LLP 1030 D8). `true` on success.
        #[::wasm_bindgen::prelude::wasm_bindgen]
        pub fn gpu_shader(name: &str, text: &str) -> bool {
            $crate::web::shader(name, text)
        }

        /// Validate one registration without changing the live registry.
        #[::wasm_bindgen::prelude::wasm_bindgen]
        pub async fn gpu_shader_check(name: &str, text: &str) -> bool {
            $crate::web::shader_check(name, text).await
        }

        /// Replace the namespace, including removals.
        #[::wasm_bindgen::prelude::wasm_bindgen]
        pub fn gpu_shaders_clear() {
            $crate::web::shaders_clear()
        }

        /// The shaders to register, as a JSON list of names.
        #[::wasm_bindgen::prelude::wasm_bindgen]
        pub fn gpu_shader_names() -> String {
            $crate::web::shader_names()
        }

        /// Bind inputs (a JSON array). `true` on success.
        #[::wasm_bindgen::prelude::wasm_bindgen]
        pub fn gpu_bind(id: u32, values: &str) -> bool {
            $crate::web::bind(id, values)
        }

        /// Bind inputs at an optional host commit clock.
        #[::wasm_bindgen::prelude::wasm_bindgen]
        pub fn gpu_bind_at(id: u32, values: &str, at_ms: Option<f64>) -> bool {
            $crate::web::bind_at(id, values, at_ms)
        }

        /// Render one frame: 1 = wants another, 0 = done, 2 = failed.
        #[::wasm_bindgen::prelude::wasm_bindgen]
        pub fn gpu_render(id: u32, width: f32, height: f32, scale: f32, now_ms: f64) -> u32 {
            $crate::web::render(id, width, height, scale, now_ms)
        }

        /// Whether a canvas wants raw input.
        #[::wasm_bindgen::prelude::wasm_bindgen]
        pub fn gpu_wants_input(id: u32) -> bool {
            $crate::web::wants_input(id)
        }

        /// Deliver one JSON event; true on success.
        #[::wasm_bindgen::prelude::wasm_bindgen]
        pub fn gpu_input(id: u32, event_json: &str) -> bool {
            $crate::web::input(id, event_json)
        }

        /// Drain messages as a JSON array.
        #[::wasm_bindgen::prelude::wasm_bindgen]
        pub fn gpu_messages(id: u32) -> String {
            $crate::web::messages(id)
        }

        /// Ask the surface; empty when it has no answer.
        #[::wasm_bindgen::prelude::wasm_bindgen]
        pub fn gpu_agent(id: u32, request_json: &str) -> String {
            $crate::web::agent(id, request_json)
        }

        /// Set the host clock ownership.
        #[::wasm_bindgen::prelude::wasm_bindgen]
        pub fn gpu_seekable(on: bool) {
            $crate::web::seekable(on)
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
