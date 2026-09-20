//! CPU reference: mirror these equations/order in WGSL. No renderer, I/O or clocks.
use crate::scene::{Camera, Instance};
use clod_format::{Bounds, ORIGINAL, Reader};
use glam::{Mat4, Vec3};

#[derive(Default, Debug)]
pub struct Selection {
    pub pages: Vec<Vec<[u32; 2]>>,
    pub clusters: u64,
    pub triangles: u64,
    pub padded_triangles: u64,
}
/// Terminal sentinel is infinity; scale both error and sphere before projection.
pub fn projected(b: &Bounds, model: Mat4, scale: f32, camera: &Camera, height: u32) -> f32 {
    if b.error == f32::MAX {
        return f32::MAX;
    }
    if let Some(span) = camera.orthographic_span {
        return b.error * scale * height as f32 / span;
    }
    let center = model.transform_point3(Vec3::from_array(b.center));
    let distance = ((center - camera.eye).length() - b.radius * scale).max(camera.near);
    b.error * scale / distance * (camera.cot * 0.5 * height as f32)
}
pub fn select(
    reader: &Reader<'_>,
    instances: &[Instance],
    camera: &Camera,
    height: u32,
    threshold: f32,
) -> Selection {
    let mut result = Selection {
        pages: vec![Vec::new(); reader.pages.len()],
        ..Selection::default()
    };
    let planes = camera.planes();
    for (instance, transform) in instances.iter().enumerate() {
        let model = transform.transform();
        let scale = transform.scale();
        let above: Vec<bool> = reader
            .groups
            .iter()
            .map(|g| projected(&g.simplified, model, scale, camera, height) > threshold)
            .collect();
        for (id, c) in reader.clusters.iter().enumerate() {
            if !above[c.group as usize] || (c.refined != ORIGINAL && above[c.refined as usize]) {
                continue;
            }
            let center = model.transform_point3(Vec3::new(c.sphere[0], c.sphere[1], c.sphere[2]));
            let radius = c.sphere[3] * scale;
            if planes
                .iter()
                .any(|p| p.truncate().dot(center) + p.w < -radius)
            {
                continue;
            }
            result.pages[c.page as usize].push([id as u32, instance as u32]);
            result.clusters += 1;
            result.triangles += c.triangle_count as u64;
            result.padded_triangles +=
                (reader.header.config.max_triangles - c.triangle_count) as u64;
        }
    }
    result
}
