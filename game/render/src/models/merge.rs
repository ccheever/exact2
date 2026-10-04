//! Static rigid parts of one model that share a material draw as one mesh.
use super::ModelNode;
use crate::{MeshId, Vertex};
use exact_game::asset::{AlphaMode, Model};
use glam::{Mat3, Vec3};
use std::collections::BTreeMap;

impl<const ASSETS: bool> crate::renderer::RendererWithAssets<ASSETS> {
    /// The model's draw list with every group of two or more unanimated, unskinned,
    /// non-blended nodes sharing a material replaced by one mesh pre-transformed
    /// into model space (mirrored nodes rewound), at its first member's place; the
    /// merged meshes. Empty when nothing merges. `meshes[i]` is node `i`'s mesh.
    pub(super) fn merge_static(
        &mut self,
        model: &Model,
        nodes: &[ModelNode],
        meshes: &[u32],
    ) -> (Vec<ModelNode>, Vec<MeshId>) {
        let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for (i, node) in nodes.iter().enumerate() {
            let mesh = &model.meshes[meshes[i] as usize];
            let blended = model.materials[mesh.material as usize].alpha_mode == AlphaMode::Blend;
            if node.3.is_none() && mesh.joints.is_empty() && !blended {
                groups.entry(node.1 .0).or_default().push(i);
            }
        }
        groups.retain(|_, members| members.len() > 1);
        if groups.is_empty() {
            return (Vec::new(), Vec::new());
        }
        let mut first = BTreeMap::new();
        let mut merged = Vec::new();
        for members in groups.values() {
            let (mut vertices, mut indices) = (Vec::new(), Vec::new());
            for &i in members {
                let local = nodes[i].2;
                let normal = Mat3::from_mat4(local).inverse().transpose();
                let mesh = &model.meshes[meshes[i] as usize];
                let base = vertices.len() as u32;
                vertices.extend(mesh.positions.chunks_exact(3).enumerate().map(|(v, p)| {
                    let n = Vec3::from_slice(&mesh.normals[v * 3..v * 3 + 3]);
                    Vertex {
                        position: local.transform_point3(Vec3::from_slice(p)).to_array(),
                        normal: (normal * n).normalize_or_zero().to_array(),
                        uv: mesh.uvs[v * 2..v * 2 + 2].try_into().unwrap(),
                        color: if mesh.colors.is_empty() {
                            [1.; 4]
                        } else {
                            mesh.colors[v * 4..v * 4 + 4].try_into().unwrap()
                        },
                    }
                }));
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
            first.insert(members[0], id);
        }
        let grouped: std::collections::BTreeSet<_> = groups.values().flatten().copied().collect();
        let draws = nodes
            .iter()
            .enumerate()
            .filter_map(|(i, &node)| match first.get(&i) {
                Some(&id) => Some((id, node.1, glam::Mat4::IDENTITY, None)),
                None => (!grouped.contains(&i)).then_some(node),
            })
            .collect();
        (draws, merged)
    }
}
