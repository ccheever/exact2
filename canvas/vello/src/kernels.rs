//! The shaders `build.rs` compiled: Metal libraries embedded in the module.

use std::collections::HashMap;
use vello::PrecompiledShader;

/// One precompiled kernel.
#[derive(Debug)]
pub struct Kernel {
    /// Its `vello_shaders` name, or the file stem of one in `shaders/`.
    pub name: &'static str,
    /// The Metal function naga named its entry point.
    pub entry: &'static str,
    /// Threads per workgroup.
    pub workgroup: [u32; 3],
    /// Its bindings in order (see `build.rs`'s `bind_kinds`).
    pub bindings: &'static [char],
    /// Its `.metallib` in [`BLOB`].
    pub range: (usize, usize),
}

/// Every `.metallib`, back to back.
static BLOB: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/kernels.bin"));

include!(concat!(env!("OUT_DIR"), "/kernels.rs"));

impl Kernel {
    /// As vello takes it.
    pub fn precompiled(&self) -> PrecompiledShader {
        PrecompiledShader {
            metallib: &BLOB[self.range.0..self.range.1],
            entry: self.entry,
            workgroup: self.workgroup,
        }
    }
}

/// A kernel by name.
pub fn kernel(name: &str) -> Option<&'static Kernel> {
    KERNELS.iter().find(|k| k.name == name)
}

/// vello's kernels, for `RendererOptions::shaders`.
pub fn vello_shaders() -> HashMap<&'static str, PrecompiledShader> {
    KERNELS.iter().map(|k| (k.name, k.precompiled())).collect()
}
