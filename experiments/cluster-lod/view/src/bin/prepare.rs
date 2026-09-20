//! Native load-time preparation only; the browser can supply the same Baseline buffers.
use clod_format::{ORIGINAL, Reader, Vertex};
use clod_view::Baseline;
use std::collections::HashMap;
unsafe extern "C" {
    fn meshopt_optimizeVertexCache(
        destination: *mut u32,
        indices: *const u32,
        index_count: usize,
        vertex_count: usize,
    );
    fn meshopt_optimizeVertexFetch(
        destination: *mut std::ffi::c_void,
        indices: *mut u32,
        index_count: usize,
        vertices: *const std::ffi::c_void,
        vertex_count: usize,
        vertex_size: usize,
    ) -> usize;
}
pub fn baseline(reader: &Reader<'_>) -> Vec<Baseline> {
    let mut result = Vec::new();
    let mut mesh = Baseline {
        vertices: Vec::new(),
        indices: Vec::new(),
    };
    let mut lookup = HashMap::<[u32; 5], u32>::new();
    for (id, _) in reader
        .clusters
        .iter()
        .enumerate()
        .filter(|(_, c)| c.refined == ORIGINAL)
    {
        // Bound each conventional buffer to 128 MiB, leaving headroom below 256 MiB.
        if mesh.indices.len() + 384 > 32 * 1024 * 1024
            || mesh.vertices.len() + 256 > 6 * 1024 * 1024
        {
            optimize(&mut mesh);
            result.push(mesh);
            mesh = Baseline {
                vertices: Vec::new(),
                indices: Vec::new(),
            };
            lookup.clear();
        }
        let (vertices, indices) = reader.geometry(id).expect("validated geometry");
        let remap: Vec<u32> = vertices
            .iter()
            .map(|v| {
                let key = [
                    v.position[0].to_bits(),
                    v.position[1].to_bits(),
                    v.position[2].to_bits(),
                    v.normal,
                    v.color,
                ];
                *lookup.entry(key).or_insert_with(|| {
                    let id = mesh.vertices.len() as u32;
                    mesh.vertices.push(*v);
                    id
                })
            })
            .collect();
        mesh.indices
            .extend(indices.iter().map(|i| remap[*i as usize]));
    }
    if !mesh.indices.is_empty() {
        optimize(&mut mesh);
        result.push(mesh);
    }
    result
}
fn optimize(mesh: &mut Baseline) {
    let mut indices = vec![0u32; mesh.indices.len()];
    // SAFETY: distinct initialized u32 buffers with index_count elements; indices address the validated vertex array.
    unsafe {
        meshopt_optimizeVertexCache(
            indices.as_mut_ptr(),
            mesh.indices.as_ptr(),
            mesh.indices.len(),
            mesh.vertices.len(),
        );
    }
    let mut vertices = mesh.vertices.clone();
    // SAFETY: destination has vertex_count * sizeof(Vertex) bytes; indices are valid and both arrays remain live for the synchronous call.
    let count = unsafe {
        meshopt_optimizeVertexFetch(
            vertices.as_mut_ptr().cast(),
            indices.as_mut_ptr(),
            indices.len(),
            mesh.vertices.as_ptr().cast(),
            mesh.vertices.len(),
            std::mem::size_of::<Vertex>(),
        )
    };
    vertices.truncate(count);
    mesh.vertices = vertices;
    mesh.indices = indices;
}
