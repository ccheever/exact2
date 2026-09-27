//! Caltrain's GPU module (LLP 1009): the aurora, the glass and the deck,
//! which are shaders. The line map left it for Canvas 2D (LLP 1056); it is
//! drawn by the data crate (`caltrain_data::map`).

#![deny(missing_docs)]

pub mod aurora;
pub mod glass;
pub mod stack;
pub use aurora::AuroraSurface;
pub use glass::GlassSurface;
pub use stack::StackSurface;

use exact_gpu::{Registry, Surface};

/// The shaders under `shaders/`, reflected at build (`build.rs`,
/// `exact-gpu-reflect`): each one's entry points, bindings, and layouts as
/// Rust, and its interface digest. The WGSL is the declaration authority — a
/// binding number, an offset, or an entry point's name is never restated
/// here by hand, and a shader edit that moves one is a build error, not a
/// wrong picture. The text itself is not here: it travels as an asset and
/// is registered at run time (LLP 1030 D8, `exact_gpu::shaders`).
pub mod shaders {
    include!(concat!(env!("OUT_DIR"), "/shaders.rs"));
}

/// Where this crate's shaders live in the source tree: what a fixture
/// registers before it renders (`exact_gpu::shaders::load_dir`).
#[cfg(not(target_arch = "wasm32"))]
pub fn shader_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("shaders")
}

fn aurora() -> Box<dyn Surface> {
    Box::new(AuroraSurface::new())
}

fn glass() -> Box<dyn Surface> {
    Box::new(GlassSurface::new())
}

fn stack() -> Box<dyn Surface> {
    Box::new(StackSurface::new())
}

/// The module's surfaces — name, arity, factory — and the shaders they
/// bind against, at the interfaces the build reflected.
pub static REGISTRY: Registry = Registry {
    surfaces: &[
        ("aurora", 1, aurora),
        ("glass", 2, glass),
        ("stack", 4, stack),
    ],
    shaders: shaders::SHADERS,
};

exact_gpu::module!(REGISTRY);
