//! CPU reference. The scalar predicates are mirrored by select.wgsl.
use crate::scene::{Camera, Instance};
use clod_format::{Bounds, Cluster, ORIGINAL, Reader};
use glam::{Mat4, Vec3, Vec4};

#[derive(Default, Debug, Clone)]
pub struct Selection {
    pub pages: Vec<Vec<[u32; 2]>>,
    pub clusters: u64,
    pub triangles: u64,
    pub padded_triangles: u64,
    pub candidates: u64,
    pub overflow: u64,
}
/// Runtime index only: preserve baked cluster/page order and hence raster tie order.
pub struct CandidateIndex {
    pub envelopes: Vec<[f32; 2]>,
    pub sphere: [f32; 4],
}
impl CandidateIndex {
    pub fn new(reader: &Reader<'_>) -> Self {
        let mut lo = Vec3::splat(f32::INFINITY);
        let mut hi = Vec3::splat(f32::NEG_INFINITY);
        for c in reader.clusters {
            for (center, radius) in [
                (Vec3::from_slice(&c.sphere), c.sphere[3]),
                (Vec3::from_array(c.simplified.center), c.simplified.radius),
                (
                    Vec3::from_array(c.refined_bounds.center),
                    c.refined_bounds.radius,
                ),
            ] {
                lo = lo.min(center - Vec3::splat(radius));
                hi = hi.max(center + Vec3::splat(radius));
            }
        }
        let center = (lo + hi) * 0.5;
        // Outward rounding covers both bounds and arithmetic differences in range pruning.
        let radius = (hi - lo).length() * 0.50001;
        let mut envelopes = vec![[0.0; 2]; reader.clusters.len()];
        let mut suffix = 0.0f32;
        for (c, e) in reader.clusters.iter().zip(&mut envelopes).rev() {
            suffix = suffix.max(c.simplified.error);
            e[0] = suffix;
        }
        let mut prefix = f32::MAX;
        for (c, e) in reader.clusters.iter().zip(&mut envelopes) {
            prefix = prefix.min(if c.refined == ORIGINAL {
                0.0
            } else {
                c.refined_bounds.error
            });
            e[1] = prefix;
        }
        Self {
            envelopes,
            sphere: center.extend(radius).to_array(),
        }
    }
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
pub fn sphere_visible(sphere: [f32; 4], model: Mat4, scale: f32, planes: &[Vec4; 6]) -> bool {
    let center = model.transform_point3(Vec3::from_slice(&sphere));
    let radius = sphere[3] * scale;
    !planes
        .iter()
        .any(|p| p.truncate().dot(center) + p.w < -radius - 1e-5)
}
pub fn cone_visible(c: &Cluster, model: Mat4, scale: f32, camera: &Camera) -> bool {
    if c.cone_cutoff >= 1.0 {
        return true;
    }
    let axis = model.transform_vector3(Vec3::from_array(c.cone_axis)) / scale;
    let view = if camera.orthographic_span.is_some() {
        camera.matrix.transpose().z_axis.truncate().normalize()
    } else {
        let delta = model.transform_point3(Vec3::from_array(c.cone_apex)) - camera.eye;
        if delta.length_squared() < 1e-20 {
            return true;
        }
        delta.normalize()
    };
    view.dot(axis) < c.cone_cutoff + 1e-5
}
pub fn select(
    reader: &Reader<'_>,
    instances: &[Instance],
    camera: &Camera,
    height: u32,
    threshold: f32,
) -> Selection {
    select_culled(reader, instances, camera, height, threshold, true)
}
pub fn select_culled(
    reader: &Reader<'_>,
    instances: &[Instance],
    camera: &Camera,
    height: u32,
    threshold: f32,
    cull: bool,
) -> Selection {
    let mut result = Selection {
        pages: vec![Vec::new(); reader.pages.len()],
        ..Selection::default()
    };
    let planes = camera.planes();
    let sphere = CandidateIndex::new(reader).sphere;
    for (instance, transform) in instances.iter().enumerate() {
        let model = transform.transform();
        let scale = transform.scale();
        if cull && !sphere_visible(sphere, model, scale, &planes) {
            continue;
        }
        // Cache shared group predicates; WGSL evaluates identical bounds per candidate.
        let above: Vec<bool> = reader
            .groups
            .iter()
            .map(|g| projected(&g.simplified, model, scale, camera, height) > threshold)
            .collect();
        for (id, c) in reader.clusters.iter().enumerate() {
            result.candidates += 1;
            if !above[c.group as usize] || (c.refined != ORIGINAL && above[c.refined as usize]) {
                continue;
            }
            if cull
                && (!sphere_visible(c.sphere, model, scale, &planes)
                    || !cone_visible(c, model, scale, camera))
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
