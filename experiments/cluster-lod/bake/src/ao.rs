//! Fixed cosine samples and an immutable BVH; workers only write disjoint vertices.
use crate::{Mesh, Result};
unsafe extern "C" {
    fn exact_clod_ao(
        positions: *const f32,
        normals: *const f32,
        vertex_count: usize,
        indices: *const u32,
        index_count: usize,
        output: *mut u8,
        proxy_count: *mut u32,
        workers: u32,
    ) -> bool;
}
pub fn bake(mesh: &Mesh) -> Result<(Vec<u8>, u32)> {
    bake_with_workers(mesh, 0)
}
/// Explicit worker count for deterministic bake validation; zero uses up to 16 host workers.
pub fn bake_with_workers(mesh: &Mesh, workers: u32) -> Result<(Vec<u8>, u32)> {
    if workers > 16 {
        return Err("AO workers must be 0..16".into());
    }
    mesh.validate()?;
    if mesh.normals.len() != mesh.positions.len() {
        return Err("AO needs normalized vertex normals".into());
    }
    let mut output = vec![255; mesh.positions.len()];
    let mut proxy_count = 0;
    // SAFETY: validated immutable input arrays and disjoint output remain live through the synchronous joined workers.
    let ok = unsafe {
        exact_clod_ao(
            mesh.positions.as_ptr().cast(),
            mesh.normals.as_ptr().cast(),
            mesh.positions.len(),
            mesh.indices.as_ptr(),
            mesh.indices.len(),
            output.as_mut_ptr(),
            &mut proxy_count,
            workers,
        )
    };
    if !ok {
        return Err("AO proxy/BVH allocation failed".into());
    }
    Ok((output, proxy_count))
}
