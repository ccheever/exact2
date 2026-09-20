use crate::{Mesh, Result};
use clod_format::{Cluster, Config, Group, Node};
use std::ffi::c_void;

#[repr(C)]
struct View {
    clusters: *const Cluster,
    cluster_count: usize,
    groups: *const Group,
    group_count: usize,
    nodes: *const Node,
    node_count: usize,
    vertices: *const u32,
    vertex_count: usize,
    indices: *const u8,
    index_count: usize,
    owner: *mut c_void,
}
unsafe extern "C" {
    fn exact_clod_build(
        positions: *const f32,
        vertex_count: usize,
        indices: *const u32,
        index_count: usize,
        attributes: *const f32,
        colors: bool,
        config: Config,
        view: *mut View,
    ) -> bool;
    fn exact_clod_free(owner: *mut c_void);
}
pub struct Built(View);
impl Built {
    pub fn new(mesh: &Mesh, config: Config) -> Result<Self> {
        let attributes: Vec<[f32; 7]> = mesh
            .normals
            .iter()
            .enumerate()
            .map(|(i, n)| {
                let c = mesh.colors.as_ref().map_or([255; 4], |cs| cs[i]);
                [
                    n[0],
                    n[1],
                    n[2],
                    c[0] as f32 / 255.0,
                    c[1] as f32 / 255.0,
                    c[2] as f32 / 255.0,
                    c[3] as f32 / 255.0,
                ]
            })
            .collect();
        let mut view = std::mem::MaybeUninit::<View>::uninit();
        // SAFETY: validated mesh buffers and attributes outlive this synchronous C++ call; shim initializes View on success.
        let ok = unsafe {
            exact_clod_build(
                mesh.positions.as_ptr().cast(),
                mesh.positions.len(),
                mesh.indices.as_ptr(),
                mesh.indices.len(),
                attributes.as_ptr().cast(),
                mesh.colors.is_some(),
                config,
                view.as_mut_ptr(),
            )
        };
        if !ok {
            return Err("clusterlod build failed (allocation or empty result)".into());
        }
        // SAFETY: successful C++ call initialized every View field and transferred owner to Rust.
        Ok(Self(unsafe { view.assume_init() }))
    }
    pub fn clusters(&self) -> &[Cluster] {
        // SAFETY: shim-owned immutable array remains allocated until self is dropped.
        unsafe { std::slice::from_raw_parts(self.0.clusters, self.0.cluster_count) }
    }
    pub fn groups(&self) -> &[Group] {
        // SAFETY: shim-owned immutable array remains allocated until self is dropped.
        unsafe { std::slice::from_raw_parts(self.0.groups, self.0.group_count) }
    }
    pub fn nodes(&self) -> &[Node] {
        // SAFETY: shim-owned immutable array remains allocated until self is dropped.
        unsafe { std::slice::from_raw_parts(self.0.nodes, self.0.node_count) }
    }
    pub fn vertices(&self) -> &[u32] {
        // SAFETY: shim-owned immutable array remains allocated until self is dropped.
        unsafe { std::slice::from_raw_parts(self.0.vertices, self.0.vertex_count) }
    }
    pub fn indices(&self) -> &[u8] {
        // SAFETY: shim-owned immutable array remains allocated until self is dropped.
        unsafe { std::slice::from_raw_parts(self.0.indices, self.0.index_count) }
    }
}
impl Drop for Built {
    fn drop(&mut self) {
        // SAFETY: this object exclusively owns the allocation returned by exact_clod_build.
        unsafe { exact_clod_free(self.0.owner) }
    }
}
