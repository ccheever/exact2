//! Data-only local poses, rig hierarchy and conservative bounds. No playback executor.
#![allow(missing_docs)]
use super::{Clip, Interpolation, Model, TrackPath};
use crate::{Component, Mat4, Quat, Transform, Vec3};
/// Compact saved arrays, ten floats per imported node: translation, xyzw rotation, scale.
/// Imported nodes are never entities. The composed palette is not saved.
#[derive(Default, Clone, Debug, Component)]
pub struct Pose {
    pub previous: Vec<f32>,
    pub local: Vec<f32>,
    pub phase: f32,
    pub root_motion: Vec3,
    pub crossed: Vec<String>,
    pub bounds: [f32; 6],
    pub(crate) stepped: Option<u64>,
}

impl Clip {
    pub fn duration(&self) -> f32 {
        self.tracks
            .iter()
            .filter_map(|t| t.times.last())
            .copied()
            .fold(0., f32::max)
    }
}

pub(crate) fn at(p: &[f32]) -> Transform {
    Transform {
        position: Vec3::from_slice(p),
        rotation: Quat::from_xyzw(p[3], p[4], p[5], p[6]),
        scale: Vec3::from_slice(&p[7..]),
    }
}
pub(crate) fn put(p: &mut [f32], t: Transform) {
    p[..3].copy_from_slice(&t.position.to_array());
    p[3..7].copy_from_slice(&t.rotation.to_array());
    p[7..10].copy_from_slice(&t.scale.to_array());
}
pub(crate) fn matrix(p: &[f32]) -> Mat4 {
    let t = at(p);
    Mat4::from_scale_rotation_translation(t.scale, t.rotation, t.position)
}
pub fn bind_pose(model: &Model) -> Vec<f32> {
    let mut out = vec![0.; model.nodes.len() * 10];
    for (n, p) in model.nodes.iter().zip(out.chunks_exact_mut(10)) {
        let (scale, rotation, position) =
            Mat4::from_cols_array(&n.transform).to_scale_rotation_translation();
        put(
            p,
            Transform {
                position,
                rotation: rotation.normalize(),
                scale,
            },
        );
    }
    out
}
/// Parent-first traversal used by the GPU loader; joint indices keep glTF's order.
pub fn node_order(model: &Model) -> Vec<u32> {
    let mut order = Vec::with_capacity(model.nodes.len());
    while order.len() < model.nodes.len() {
        let before = order.len();
        for (i, n) in model.nodes.iter().enumerate() {
            if !order.contains(&(i as u32)) && n.parent.is_none_or(|p| order.contains(&p)) {
                order.push(i as u32);
            }
        }
        assert!(order.len() > before, "validated parent graph");
    }
    order
}

pub fn joint_matrix(model: &Model, local: &[f32], node: u32) -> Mat4 {
    let mut out = matrix(&local[node as usize * 10..]);
    let mut parent = model.nodes[node as usize].parent;
    while let Some(p) = parent {
        out = matrix(&local[p as usize * 10..]) * out;
        parent = model.nodes[p as usize].parent;
    }
    out
}

// Conservative reach: maximum sum of local translation lengths along a chain,
// scaled by ancestor scale, plus each influenced vertex's inverse-bind radius.
pub fn animated_bounds(model: &Model) -> [f32; 6] {
    if model.skins.is_empty() {
        return model.bounds;
    }
    let rest = bind_pose(model);
    let mut lengths = vec![0.; model.nodes.len()];
    let mut scales = vec![1.; model.nodes.len()];
    for (i, p) in rest.chunks_exact(10).enumerate() {
        lengths[i] = Vec3::from_slice(p).length();
        scales[i] = Vec3::from_slice(&p[7..]).abs().max_element();
    }
    for clip in &model.clips {
        for t in &clip.tracks {
            if matches!(t.path, TrackPath::Translation | TrackPath::Scale) {
                // Cubic tangents can overshoot; include one duration times both tangents.
                let bound = t
                    .values
                    .chunks_exact(3)
                    .map(|p| Vec3::from_slice(p).length())
                    .fold(0., f32::max)
                    * if matches!(t.interpolation, Interpolation::CubicSpline) {
                        1. + 2. * clip.duration()
                    } else {
                        1.
                    };
                let slot = if matches!(t.path, TrackPath::Translation) {
                    &mut lengths[t.node as usize]
                } else {
                    &mut scales[t.node as usize]
                };
                *slot = slot.max(bound);
            }
        }
    }
    let mut reach = vec![0.; model.nodes.len()];
    let mut global_scale = vec![1.; model.nodes.len()];
    for i in node_order(model) {
        let i = i as usize;
        let (r, s) = model.nodes[i]
            .parent
            .map_or((0., 1.), |p| (reach[p as usize], global_scale[p as usize]));
        reach[i] = r + lengths[i] * s;
        global_scale[i] = s * scales[i];
    }
    let mut radius = reach.iter().copied().fold(0., f32::max);
    for node in &model.nodes {
        if let (Some(mesh), Some(skin)) = (node.mesh, node.skin) {
            let mesh = &model.meshes[mesh as usize];
            let skin = &model.skins[skin as usize];
            for (v, position) in mesh.positions.chunks_exact(3).enumerate() {
                for influence in 0..4 {
                    let j = mesh.joints[v * 4 + influence] as usize;
                    let node = skin.joints[j] as usize;
                    let inverse = Mat4::from_cols_slice(&skin.inverse_binds[j * 16..j * 16 + 16]);
                    radius = radius.max(
                        reach[node]
                            + global_scale[node]
                                * inverse
                                    .transform_point3(Vec3::from_slice(position))
                                    .length(),
                    );
                }
            }
        }
    }
    let b = model.bounds;
    [
        b[0] - radius,
        b[1] - radius,
        b[2] - radius,
        b[3] + radius,
        b[4] + radius,
        b[5] + radius,
    ]
}
