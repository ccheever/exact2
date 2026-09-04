//! The shader registry (LLP 1030 D8): the WGSL a surface compiles against,
//! by name, handed to the module at run time — never a string in the binary.
//!
//! A shader travels as an asset beside the plan: `<app>/gpu/shaders/<name>.wgsl`
//! at build, `shaders/<name>.wgsl` in a bundle or a `dist/`. Before the
//! module creates a surface, the host registers each file's text under its
//! stem; the module refuses to create a surface while any shader its
//! registry names has no text, by name, and a surface's `module()` (the
//! generated Rust, `exact-gpu-reflect`) reads the registered text when it
//! builds its pipeline.
//!
//! **A candidate is admitted only if it validates and its interface is the
//! one the binary binds.** [`set_shader`] parses and validates the text with
//! naga and computes its interface digest — entry points, bindings, layouts,
//! inputs, outputs, overrides; never the text — and refuses, naming the
//! shader and both digests, when the expected one (the `INTERFACE_DIGEST`
//! the surface's Rust was generated with) differs: a bindings edit is a
//! binary change and this is where a running binary says so. A color edit
//! keeps the digest, is admitted, and bumps the shader generation; a
//! surface that caches a pipeline keyed by [`Frame::shader_generation`]
//! rebuilds it at its next frame, which is how a `.wgsl` edit in the dev
//! loop becomes a new picture with no rebuild.
//!
//! The registry is process-global, as the module is (one module per
//! process, `native.rs`); a host with several sessions shares it.
//!
//! On the web the digest is not computed: the browser compiles WGSL itself
//! and naga would cost the GPU module 740 KiB; the wasm and its shader
//! files are one build, checked by the bake, and the dev loop rebuilds the
//! wasm on a shader edit. There, [`set_shader`] admits any text under the
//! expected digest, and a file that does not validate is the browser's
//! error at pipeline creation.
//!
//! [`Frame::shader_generation`]: crate::Frame::shader_generation

#[cfg(not(target_arch = "wasm32"))]
pub use exact_gpu_reflect::interface_digest;

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{LazyLock, RwLock};

struct Entry {
    text: String,
    digest: u64,
}

static SHADERS: LazyLock<RwLock<HashMap<String, Entry>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
static GENERATION: AtomicU32 = AtomicU32::new(0);

/// Register (or replace) the text of shader `name`: validated by naga, its
/// interface digest checked against `expected` when one is given. On
/// success the shader generation advances, so every surface rebuilds its
/// pipeline at its next frame. A refusal names the shader and why — a
/// shader that does not validate carries naga's line and column; one whose
/// interface differs carries both digests — and leaves the registry as it
/// was.
pub fn validate_shader(name: &str, text: &str, expected: Option<u64>) -> Result<u64, String> {
    #[cfg(not(target_arch = "wasm32"))]
    let digest = {
        let digest = interface_digest(text)
            .map_err(|e| format!("shader `{name}` does not validate: {e}"))?;
        if let Some(expected) = expected {
            if expected != digest {
                return Err(format!(
                    "shader `{name}` is not the interface this binary binds: its interface digest is {digest:#018x}, the binary's {expected:#018x} — a bindings, layout, input, or output change needs a rebuild of the native host"
                ));
            }
        }
        digest
    };
    // The web: the bake's check stands in (see the module docs).
    #[cfg(target_arch = "wasm32")]
    let digest = {
        let _ = (name, text);
        expected.unwrap_or(0)
    };
    Ok(digest)
}

/// Register one shader after validating its source and interface.
pub fn set_shader(name: &str, text: String, expected: Option<u64>) -> Result<(), String> {
    let digest = validate_shader(name, &text, expected)?;
    let mut map = SHADERS.write().unwrap_or_else(|e| e.into_inner());
    map.insert(name.to_string(), Entry { text, digest });
    GENERATION.fetch_add(1, Ordering::SeqCst);
    Ok(())
}

/// Replace a complete namespace: omitted shaders must not survive a bundle.
/// The host validates all replacement sources before invoking this commit.
pub fn clear_shaders() {
    SHADERS.write().unwrap_or_else(|e| e.into_inner()).clear();
    GENERATION.fetch_add(1, Ordering::SeqCst);
}

/// The registered text of shader `name`, if any.
pub fn shader_source(name: &str) -> Option<String> {
    SHADERS
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .get(name)
        .map(|e| e.text.clone())
}

/// The interface digest of the registered text of shader `name`, if any.
pub fn shader_digest(name: &str) -> Option<u64> {
    SHADERS
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .get(name)
        .map(|e| e.digest)
}

/// How many times a shader has been registered: a surface keys its cached
/// pipeline by this beside the target format (`Frame::shader_generation`).
pub fn shader_generation() -> u32 {
    GENERATION.load(Ordering::SeqCst)
}

/// The shader module descriptor for `name` over its registered text — what
/// a generated `module()` returns. Empty text when nothing is registered,
/// which the module never reaches: it refuses to create a surface first.
pub fn shader_module(name: &'static str) -> wgpu::ShaderModuleDescriptor<'static> {
    wgpu::ShaderModuleDescriptor {
        label: Some(name),
        source: wgpu::ShaderSource::Wgsl(Cow::Owned(shader_source(name).unwrap_or_default())),
    }
}

/// The first of `shaders` (name, expected digest) with no registered text,
/// or whose registered text's digest is not the expected one; `None` when
/// every shader the registry names is in place.
pub fn missing(shaders: &[(&str, u64)]) -> Option<String> {
    let map = SHADERS.read().unwrap_or_else(|e| e.into_inner());
    shaders.iter().find_map(|(name, expected)| match map.get(*name) {
        None => Some(format!(
            "shader `{name}` has no source: register it (gpu_shader) before creating a surface"
        )),
        Some(e) if e.digest != *expected => Some(format!(
            "shader `{name}` is registered at interface {:#018x}, not the binary's {expected:#018x}",
            e.digest
        )),
        Some(_) => None,
    })
}

/// Register every shader `registry` names from `dir/<name>.wgsl`, each
/// checked against its expected interface (a fixture's and a native host's
/// way in); the first refusal, by name.
#[cfg(not(target_arch = "wasm32"))]
pub fn load_dir(dir: &std::path::Path, registry: &crate::Registry) -> Result<(), String> {
    for (name, expected) in registry.shaders {
        let path = dir.join(format!("{name}.wgsl"));
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        set_shader(name, text, Some(*expected))?;
    }
    Ok(())
}
