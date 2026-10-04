//! A model's rigid parts that share a material draw as one mesh: static parts
//! pre-transformed into model space, animated parts as a mesh whose vertices
//! each follow their node's palette matrix, as a skin with weight one.
use super::ModelNode;
use crate::{MeshId, Vertex};
use exact_game::asset::{AlphaMode, MeshData, Model, Skin};
use glam::{Mat3, Mat4, Vec3};
use std::collections::BTreeMap;

fn mergeable(model: &Model, mesh: u32) -> bool {
    let mesh = &model.meshes[mesh as usize];
    mesh.joints.is_empty() && model.materials[mesh.material as usize].alpha_mode != AlphaMode::Blend
}
/// Animated rigid parts (a model with clips or skins) sharing a material, two or
/// more: the material and a skin over those nodes, in node order.
pub(super) fn animated_groups(model: &Model) -> Vec<(u32, Skin)> {
    if model.skins.is_empty() && model.clips.is_empty() {
        return Vec::new();
    }
    let mut groups: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for (i, node) in model.nodes.iter().enumerate() {
        if let (Some(mesh), None) = (node.mesh, node.skin) {
            if mergeable(model, mesh) {
                let material = model.meshes[mesh as usize].material;
                groups.entry(material).or_default().push(i as u32);
            }
        }
    }
    groups
        .into_iter()
        .filter(|(_, joints)| joints.len() > 1)
        .map(|(material, joints)| {
            let inverse_binds = Mat4::IDENTITY.to_cols_array().repeat(joints.len());
            (
                material,
                Skin {
                    joints,
                    inverse_binds,
                    ..Default::default()
                },
            )
        })
        .collect()
}
fn vertex(mesh: &MeshData, v: usize, local: Mat4, normal: Mat3) -> Vertex {
    let p = Vec3::from_slice(&mesh.positions[v * 3..v * 3 + 3]);
    let n = Vec3::from_slice(&mesh.normals[v * 3..v * 3 + 3]);
    Vertex {
        position: local.transform_point3(p).to_array(),
        normal: (normal * n).normalize_or_zero().to_array(),
        uv: mesh.uvs[v * 2..v * 2 + 2].try_into().unwrap(),
        color: if mesh.colors.is_empty() {
            [1.; 4]
        } else {
            mesh.colors[v * 4..v * 4 + 4].try_into().unwrap()
        },
    }
}

impl<const ASSETS: bool> crate::renderer::RendererWithAssets<ASSETS> {
    /// The model's draw list with each merged group at its first member's place,
    /// and the merged meshes; empty when nothing merges. `meshes[i]` is drawn
    /// node `i`'s mesh; `animated` and `skins` are the animated groups and their
    /// skin templates.
    pub(super) fn merge_static(
        &mut self,
        model: &Model,
        nodes: &[ModelNode],
        meshes: &[u32],
        animated: &[(u32, Skin)],
        skins: &[u32],
    ) -> (Vec<ModelNode>, Vec<MeshId>) {
        // Static: unskinned, unanimated nodes sharing a material.
        let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for (i, node) in nodes.iter().enumerate() {
            if node.3.is_none() && mergeable(model, meshes[i]) {
                groups.entry(node.1 .0).or_default().push(i);
            }
        }
        groups.retain(|_, members| members.len() > 1);
        if groups.is_empty() && animated.is_empty() {
            return (Vec::new(), Vec::new());
        }
        let drawn: Vec<usize> = (0..model.nodes.len())
            .filter(|&n| model.nodes[n].mesh.is_some())
            .collect();
        let mut first = BTreeMap::new();
        let mut grouped = std::collections::BTreeSet::new();
        let mut merged = Vec::new();
        for members in groups.values() {
            let (mut vertices, mut indices) = (Vec::new(), Vec::new());
            for &i in members {
                let local = nodes[i].2;
                let normal = Mat3::from_mat4(local).inverse().transpose();
                let mesh = &model.meshes[meshes[i] as usize];
                let base = vertices.len() as u32;
                vertices
                    .extend((0..mesh.positions.len() / 3).map(|v| vertex(mesh, v, local, normal)));
                // A mirrored node keeps its front faces by reversing its winding.
                let mirrored = local.determinant() < 0.;
                for t in mesh.indices.chunks_exact(3) {
                    let t = if mirrored {
                        [t[0], t[2], t[1]]
                    } else {
                        [t[0], t[1], t[2]]
                    };
                    indices.extend(t.map(|i| base + i));
                }
            }
            let id = self.add_mesh(&vertices, &indices);
            self.meshes[id.0].asset = true;
            merged.push(id);
            first.insert(members[0], (id, nodes[members[0]].1, Mat4::IDENTITY, None));
            grouped.extend(members);
        }
        for ((_, skin), &template) in animated.iter().zip(skins) {
            let members: Vec<usize> = skin
                .joints
                .iter()
                .map(|&n| drawn.binary_search(&(n as usize)).unwrap())
                .collect();
            let (mut vertices, mut indices, mut weights) = (Vec::new(), Vec::new(), Vec::new());
            for (joint, &i) in members.iter().enumerate() {
                let mesh = &model.meshes[meshes[i] as usize];
                let base = vertices.len() as u32;
                let count = mesh.positions.len() / 3;
                vertices
                    .extend((0..count).map(|v| vertex(mesh, v, Mat4::IDENTITY, Mat3::IDENTITY)));
                indices.extend(mesh.indices.iter().map(|i| base + i));
                for _ in 0..count {
                    weights.extend([joint as u32, 0, 0, 0, 1f32.to_bits(), 0, 0, 0]);
                }
            }
            let id = self.add_mesh(&vertices, &indices);
            self.meshes[id.0].asset = true;
            let start = self.meshes[id.0].base_vertex as u64 * 32;
            let buffer = &mut self.models.skinning.as_mut().unwrap().weights;
            self.models.reallocations += u64::from(buffer.grow(
                &self.device,
                &self.queue,
                start + (weights.len() * 4) as u64,
            ));
            buffer.write(&self.queue, start, crate::buffers::bytes(&weights));
            merged.push(id);
            first.insert(
                members[0],
                (id, nodes[members[0]].1, Mat4::IDENTITY, Some(template)),
            );
            grouped.extend(members);
        }
        let draws = nodes
            .iter()
            .enumerate()
            .filter_map(|(i, &node)| match first.get(&i) {
                Some(&draw) => Some(draw),
                None => (!grouped.contains(&i)).then_some(node),
            })
            .collect();
        (draws, merged)
    }
}
