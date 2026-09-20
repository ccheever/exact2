//! Portable little-endian, directly uploadable cluster-LOD records. See the experiment README.
pub mod oracle;
mod reader;
mod writer;
use bytemuck::{Pod, Zeroable};
pub use reader::{Error, Reader};
pub use writer::{PageData, encode};

pub const MAGIC: [u8; 8] = *b"CLOD0001";
pub const VERSION: u32 = 1;
pub const ORIGINAL: u32 = u32::MAX;
pub const HAS_COLOR: u32 = 1;

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable, PartialEq)]
pub struct Config {
    pub max_triangles: u32,
    pub page_bytes: u32,
    pub partition_size: u32,
    pub vertex_encoding: u32,
    pub normal_weight: f32,
    pub color_weight: f32,
    pub simplify_ratio: f32,
    pub simplify_threshold: f32,
    pub error_merge_previous: f32,
    pub error_merge_additive: f32,
    pub reserved: [u32; 2],
}
impl Default for Config {
    fn default() -> Self {
        Self {
            max_triangles: 128,
            page_bytes: 32 * 1024 * 1024,
            partition_size: 16,
            vertex_encoding: 1,
            normal_weight: 0.1,
            color_weight: 0.1,
            simplify_ratio: 0.5,
            simplify_threshold: 0.85,
            error_merge_previous: 1.0,
            error_merge_additive: 1.0,
            reserved: [0; 2],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Header {
    pub magic: [u8; 8],
    pub version: u32,
    pub header_bytes: u32,
    pub file_bytes: u64,
    pub source_sha256: [u8; 32],
    pub source_vertices: u32,
    pub source_triangles: u32,
    pub cluster_count: u32,
    pub group_count: u32,
    pub page_count: u32,
    pub node_count: u32,
    pub root_count: u32,
    pub flags: u32,
    pub config: Config,
    pub clusters_offset: u64,
    pub groups_offset: u64,
    pub pages_offset: u64,
    pub nodes_offset: u64,
    pub geometry_offset: u64,
    pub reserved: [u32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable, PartialEq)]
pub struct Bounds {
    pub center: [f32; 3],
    pub radius: f32,
    pub error: f32,
}
impl Bounds {
    /// Vendor rotationally invariant perspective estimate, multiplied by viewport height.
    pub fn projected(&self, position: [f32; 3], cot_half_fov: f32, near: f32, height: f32) -> f32 {
        if self.error == f32::MAX {
            return f32::MAX;
        }
        let d = self
            .center
            .iter()
            .zip(position)
            .map(|(a, b)| (a - b) * (a - b))
            .sum::<f32>()
            .sqrt();
        self.error / (d - self.radius).max(near) * (cot_half_fov * 0.5 * height)
    }
}

/// 128 bytes, scalar WGSL layout (use scalar arrays for vec3 fields).
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Cluster {
    pub sphere: [f32; 4],
    pub simplified: Bounds,
    pub refined_bounds: Bounds,
    pub cone_apex: [f32; 3],
    pub cone_cutoff: f32,
    pub cone_axis: [f32; 3],
    pub group: u32,
    pub refined: u32,
    pub page: u32,
    pub vertex_offset: u32,
    pub vertex_count: u32,
    pub triangle_offset: u32,
    pub triangle_count: u32,
    pub depth: u32,
    pub reserved: [u32; 3],
}
impl Cluster {
    pub fn selected(&self, threshold: f32) -> bool {
        self.simplified.error > threshold
            && (self.refined == ORIGINAL || self.refined_bounds.error <= threshold)
    }
    pub fn selected_camera(
        &self,
        threshold: f32,
        position: [f32; 3],
        cot_half_fov: f32,
        near: f32,
        height: f32,
    ) -> bool {
        self.simplified
            .projected(position, cot_half_fov, near, height)
            > threshold
            && (self.refined == ORIGINAL
                || self
                    .refined_bounds
                    .projected(position, cot_half_fov, near, height)
                    <= threshold)
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Group {
    pub simplified: Bounds,
    pub depth: u32,
    pub first_cluster: u32,
    pub cluster_count: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Node {
    pub bounds: Bounds,
    pub group: u32,
    pub child_offset: u32,
    pub child_count: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Page {
    pub offset: u64,
    pub byte_length: u64,
    pub sha256: [u8; 32],
    pub first_cluster: u32,
    pub cluster_count: u32,
    pub vertex_count: u32,
    pub index_count: u32,
    pub vertices_offset: u32,
    pub indices_offset: u32,
    pub reserved: [u32; 2],
}

/// 20 bytes: lossless source position, octahedral SNORM16 normal, little-endian RGBA8.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: u32,
    pub color: u32,
}
pub fn pack_normal(n: [f32; 3]) -> u32 {
    let l = n.iter().map(|v| v.abs()).sum::<f32>();
    let [mut x, mut y, z] = if l > 0.0 {
        n.map(|v| v / l)
    } else {
        [0.0, 0.0, 1.0]
    };
    if z < 0.0 {
        let ox = x;
        x = (1.0 - y.abs()) * x.signum();
        y = (1.0 - ox.abs()) * y.signum();
    }
    let sx = (x.clamp(-1.0, 1.0) * 32767.0).round() as i16;
    let sy = (y.clamp(-1.0, 1.0) * 32767.0).round() as i16;
    sx as u16 as u32 | ((sy as u16 as u32) << 16)
}
pub fn unpack_normal(n: u32) -> [f32; 3] {
    let x = (n as i16) as f32 / 32767.0;
    let y = ((n >> 16) as i16) as f32 / 32767.0;
    let mut v = [x, y, 1.0 - x.abs() - y.abs()];
    if v[2] < 0.0 {
        v[0] = (1.0 - y.abs()) * x.signum();
        v[1] = (1.0 - x.abs()) * y.signum();
    }
    let l = v.iter().map(|v| v * v).sum::<f32>().sqrt();
    v.map(|v| v / l)
}
