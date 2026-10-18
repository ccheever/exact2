//! A model's rigid parts that share a material draw as one mesh: static parts
//! pre-transformed into model space, animated parts as a mesh whose vertices
//! each follow their node's palette matrix, as a skin with weight one.
//!
//! Parts of different materials share a draw too when the materials differ
//! only in factors a vertex can carry: opaque, untextured, a base colour factor
//! of one (the colour in the vertices) and the same sidedness. The draw's
//! material has factors of one, each vertex numbers its part in `uv.x` (no
//! texture reads it), and each instance's part entries carry the part's
//! metallic, roughness, rim and emission (model_vertex.wgsl `model_transform`).
use super::{MaterialId, ModelNode};
use crate::{MeshId, Vertex};
use exact_game::asset::{AlphaMode, MaterialData, MeshData, MeshOutline, Model, Skin};
use glam::{Mat3, Mat4, Vec3};
use std::collections::BTreeMap;

fn mergeable(model: &Model, mesh: u32) -> bool {
    let mesh = &model.meshes[mesh as usize];
    mesh.joints.is_empty() && model.materials[mesh.material as usize].alpha_mode != AlphaMode::Blend
}
fn outline_key(mesh: &MeshData) -> Option<[u32; 5]> {
    mesh.outline
        .as_ref()
        .map(|o| [o.width, o.min_pixels, o.color[0], o.color[1], o.color[2]].map(f32::to_bits))
}
/// Which parts may share a draw: those of one material, or of any materials
/// whose differences a part entry carries (`None`, then their sidedness).
type Key = (Option<u32>, bool, Option<[u32; 5]>);
fn shared(m: &MaterialData) -> bool {
    m.alpha_mode == AlphaMode::Opaque
        && m.base_color == [1.; 4]
        && crate::model_pipeline::untextured(m)
}
fn key(model: &Model, mesh: u32) -> Key {
    let data = &model.meshes[mesh as usize];
    let material = &model.materials[data.material as usize];
    if shared(material) {
        (None, material.double_sided, outline_key(data))
    } else {
        (Some(data.material), false, outline_key(data))
    }
}
/// A part's entry for a draw of several materials: metallic, roughness, rim,
/// one (the run's mark), then emission.
pub(crate) type PartMaterial = [f32; 8];
fn part_material(m: &MaterialData) -> PartMaterial {
    [
        m.metallic,
        m.roughness,
        m.rim,
        1.,
        m.emissive[0],
        m.emissive[1],
        m.emissive[2],
        0.,
    ]
}
/// The draw's own material: factors of one, which its parts' entries scale,
/// with rim and emission on only when some part has them.
fn shared_material(parts: &[&MaterialData]) -> MaterialData {
    MaterialData {
        metallic: 1.,
        roughness: 1.,
        rim: if parts.iter().any(|m| m.rim > 0.) {
            1.
        } else {
            0.
        },
        emissive: if parts.iter().any(|m| m.emissive != [0.; 3]) {
            [1.; 3]
        } else {
            [0.; 3]
        },
        double_sided: parts[0].double_sided,
        ..Default::default()
    }
}
fn append_outline(
    out: &mut Option<MeshOutline>,
    reach_vectors: &mut Vec<Vec3>,
    mesh: &MeshData,
    local: Mat4,
) {
    let Some(source) = &mesh.outline else {
        return;
    };
    let target = out.get_or_insert_with(|| MeshOutline {
        width: source.width,
        min_pixels: source.min_pixels,
        color: source.color,
        ..Default::default()
    });
    let normal = Mat3::from_mat4(local).inverse().transpose();
    for (n, &r) in source.normals.chunks_exact(3).zip(&source.reach) {
        let n = Vec3::from_slice(n);
        let transformed = normal * n;
        target.normals.extend(transformed.to_array());
        let reach = local.transform_vector3(n * r);
        reach_vectors.push(reach);
        target.reach.push(reach.length() / transformed.length());
    }
}
/// Animated rigid parts (a model with clips or skins) that may share a draw,
/// two or more: a skin over those nodes, in node order.
pub(super) fn animated_groups(model: &Model) -> Vec<Skin> {
    if !model.requires_palette() {
        return Vec::new();
    }
    let mut groups: BTreeMap<Key, Vec<u32>> = BTreeMap::new();
    for (i, node) in model.nodes.iter().enumerate() {
        if let (Some(mesh), None) = (node.mesh, node.skin) {
            if mergeable(model, mesh) {
                groups.entry(key(model, mesh)).or_default().push(i as u32);
            }
        }
    }
    groups
        .into_values()
        .filter(|joints| joints.len() > 1)
        .map(|joints| {
            let inverse_binds = Mat4::IDENTITY.to_cols_array().repeat(joints.len());
            Skin {
                joints,
                inverse_binds,
                ..Default::default()
            }
        })
        .collect()
}
/// A vertex of a draw of several materials names its part in `uv.x`.
fn numbered(mut v: Vertex, several: bool, part: usize) -> Vertex {
    if several {
        v.uv = [part as f32, 0.];
    }
    v
}
fn vertex(mesh: &MeshData, v: usize, local: Mat4, normal: Mat3) -> Vertex {
    let p = Vec3::from_slice(&mesh.positions[v * 3..v * 3 + 3]);
    let n = Vec3::from_slice(&mesh.normals[v * 3..v * 3 + 3]);
    Vertex {
        position: local.transform_point3(p).to_array(),
        // Normalize after interpolation, just as the unmerged surface shader does.
        normal: (normal * n).to_array(),
        uv: mesh.uvs[v * 2..v * 2 + 2].try_into().unwrap(),
        color: if mesh.colors.is_empty() {
            [1.; 4]
        } else {
            mesh.colors[v * 4..v * 4 + 4].try_into().unwrap()
        },
    }
}

/// A model's draw list with its merges.
#[derive(Default)]
pub(super) struct Merged {
    pub draws: Vec<ModelNode>,
    /// Each draw's part names, and where each part's vertices start.
    pub members: Vec<Vec<String>>,
    pub starts: Vec<Vec<u32>>,
    /// Each draw's part entries when its parts have several materials, else empty.
    pub parts: Vec<Vec<PartMaterial>>,
    pub meshes: Vec<MeshId>,
    /// The materials of draws of several materials.
    pub materials: Vec<MaterialId>,
}

impl<const ASSETS: bool> crate::renderer::RendererWithAssets<ASSETS> {
    /// The model's draw list with each merged group at its first member's place;
    /// empty when nothing merges. `meshes[i]` is drawn node `i`'s mesh;
    /// `animated` and `skins` are the animated groups and their skin templates.
    pub(super) fn merge_static(
        &mut self,
        model: &Model,
        nodes: &[ModelNode],
        meshes: &[u32],
        animated: &[Skin],
        skins: &[u32],
    ) -> Merged {
        // Static: unskinned, unanimated nodes that may share a draw.
        let mut groups: BTreeMap<Key, Vec<usize>> = BTreeMap::new();
        for (i, node) in nodes.iter().enumerate() {
            if node.3.is_none() && mergeable(model, meshes[i]) {
                groups.entry(key(model, meshes[i])).or_default().push(i);
            }
        }
        groups.retain(|_, members| members.len() > 1);
        if groups.is_empty() && animated.is_empty() {
            return Merged::default();
        }
        let drawn: Vec<usize> = (0..model.nodes.len())
            .filter(|&n| model.nodes[n].mesh.is_some())
            .collect();
        let mut first = BTreeMap::new();
        let mut merged = Vec::new();
        let mut shared_materials = Vec::new();
        // Of several materials: the draw's material and each part's entry; the
        // parts' vertices number them in `uv.x`.
        let mut share = |this: &mut Self, members: &[usize]| {
            let parts: Vec<&MaterialData> = members
                .iter()
                .map(|&i| &model.materials[model.meshes[meshes[i] as usize].material as usize])
                .collect();
            let first = model.meshes[meshes[members[0]] as usize].material;
            if members
                .iter()
                .all(|&i| model.meshes[meshes[i] as usize].material == first)
            {
                return None;
            }
            let material = this.add_shared_material(&shared_material(&parts));
            shared_materials.push(material);
            Some((
                material,
                parts.into_iter().map(part_material).collect::<Vec<_>>(),
            ))
        };
        for members in groups.values() {
            let several = share(self, members);
            let mut outline = None;
            let mut reach_vectors = Vec::new();
            let (mut vertices, mut indices, mut starts) = (Vec::new(), Vec::new(), Vec::new());
            for &i in members {
                let local = nodes[i].2;
                append_outline(
                    &mut outline,
                    &mut reach_vectors,
                    &model.meshes[meshes[i] as usize],
                    local,
                );
                let normal = Mat3::from_mat4(local).inverse().transpose();
                let mesh = &model.meshes[meshes[i] as usize];
                let base = vertices.len() as u32;
                starts.push(base);
                let count = mesh.positions.len() / 3;
                let part = starts.len() - 1;
                vertices
                    .extend((0..count).map(|v| {
                        numbered(vertex(mesh, v, local, normal), several.is_some(), part)
                    }));
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
            // Static parts carry no per-vertex part: looks find it by vertex range.
            let id = self.add_mesh(&vertices, &indices);
            self.meshes[id.0].asset = true;
            if let Some(outline) = &outline {
                self.add_outline(id, outline, Some(&reach_vectors));
            }
            merged.push(id);
            let material = several.as_ref().map_or(nodes[members[0]].1, |s| s.0);
            let draw = (id, material, Mat4::IDENTITY, None);
            let parts = several.map_or_else(Vec::new, |s| s.1);
            first.insert(members[0], (draw, members.clone(), starts, parts));
        }
        let rest = model.offsets().expect("validated model");
        for (skin, &template) in animated.iter().zip(skins) {
            let mut outline = None;
            let mut reach_vectors = Vec::new();
            let members: Vec<usize> = skin
                .joints
                .iter()
                .map(|&n| drawn.binary_search(&(n as usize)).unwrap())
                .collect();
            let several = share(self, &members);
            let (mut vertices, mut indices, mut parts) = (Vec::new(), Vec::new(), Vec::new());
            let mut starts = Vec::new();
            for (joint, &i) in members.iter().enumerate() {
                append_outline(
                    &mut outline,
                    &mut reach_vectors,
                    &model.meshes[meshes[i] as usize],
                    Mat4::IDENTITY,
                );
                let mesh = &model.meshes[meshes[i] as usize];
                let base = vertices.len() as u32;
                starts.push(base);
                let count = mesh.positions.len() / 3;
                vertices.extend((0..count).map(|v| {
                    let v = vertex(mesh, v, Mat4::IDENTITY, Mat3::IDENTITY);
                    numbered(v, several.is_some(), joint)
                }));
                parts.extend(std::iter::repeat_n(joint as u32, count));
                let mirrored = rest[skin.joints[joint] as usize].determinant() < 0.;
                for t in mesh.indices.chunks_exact(3) {
                    let t = if mirrored {
                        [t[0], t[2], t[1]]
                    } else {
                        [t[0], t[1], t[2]]
                    };
                    indices.extend(t.map(|i| base + i));
                }
            }
            let id = self.merged_mesh(&vertices, &indices, &parts);
            if let Some(outline) = &outline {
                self.add_outline(id, outline, Some(&reach_vectors));
            }
            merged.push(id);
            let material = several.as_ref().map_or(nodes[members[0]].1, |s| s.0);
            let draw = (id, material, Mat4::IDENTITY, Some(template));
            let parts = several.map_or_else(Vec::new, |s| s.1);
            first.insert(members[0], (draw, members, starts, parts));
        }
        let grouped: std::collections::BTreeSet<usize> = first
            .values()
            .flat_map(|(_, m, _, _)| m.iter().copied())
            .collect();
        let name = |i: usize| model.nodes[drawn[i]].name.clone();
        let mut out = Merged {
            meshes: merged,
            materials: shared_materials,
            ..Default::default()
        };
        for (i, &node) in nodes.iter().enumerate() {
            if let Some((draw, members, starts, parts)) = first.get(&i) {
                out.draws.push(*draw);
                out.members.push(members.iter().map(|&m| name(m)).collect());
                out.starts.push(starts.clone());
                out.parts.push(parts.clone());
            } else if !grouped.contains(&i) {
                out.draws.push(node);
                out.members.push(vec![name(i)]);
                out.starts.push(vec![0]);
                out.parts.push(Vec::new());
            }
        }
        out
    }
    /// A renderer material for a draw of several materials.
    fn add_shared_material(&mut self, data: &MaterialData) -> MaterialId {
        let slot = self
            .models
            .materials
            .iter()
            .position(|m| m.bind.is_none())
            .unwrap_or(self.models.materials.len());
        let names = Default::default();
        let material = super::material_bind(
            &self.device,
            &self.queue,
            &self.pipelines.models.as_ref().unwrap().material,
            (data, &names),
            &self.models.textures,
            self.untextured_twin(data, &names),
        );
        if slot == self.models.materials.len() {
            self.models.materials.push(material);
        } else {
            self.models.materials[slot] = material;
        }
        MaterialId(slot)
    }
    pub(super) fn mirrored_rigid_mesh(&mut self, mesh: &MeshData) -> MeshId {
        let vertices: Vec<_> = (0..mesh.positions.len() / 3)
            .map(|v| vertex(mesh, v, Mat4::IDENTITY, Mat3::IDENTITY))
            .collect();
        let indices: Vec<_> = mesh
            .indices
            .chunks_exact(3)
            .flat_map(|t| [t[0], t[2], t[1]])
            .collect();
        let id = self.add_mesh(&vertices, &indices);
        self.meshes[id.0].asset = true;
        if let Some(outline) = &mesh.outline {
            self.add_outline(id, outline, None);
        }
        id
    }
    /// An animated merged mesh: each vertex follows its part's node as a
    /// weight-one joint.
    fn merged_mesh(&mut self, vertices: &[Vertex], indices: &[u32], parts: &[u32]) -> MeshId {
        let id = self.add_mesh(vertices, indices);
        self.meshes[id.0].asset = true;
        let words: Vec<u32> = parts
            .iter()
            .flat_map(|&p| [p, 0, 0, 0, 1f32.to_bits(), 0, 0, 0])
            .collect();
        let start = self.meshes[id.0].base_vertex as u64 * 32;
        let buffer = &mut self.models.skinning.as_mut().unwrap().weights;
        self.models.reallocations +=
            u64::from(buffer.grow(&self.device, &self.queue, start + (words.len() * 4) as u64));
        buffer.write(&self.queue, start, crate::buffers::bytes(&words));
        id
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use exact_game::asset::{MaterialData, MeshData, Model, Node};
    #[test]
    fn static_merges_add_nothing_to_the_skin_weight_buffer() {
        let Some(gpu) = crate::test_device::device_or_skip(exact_gpu::fixture::device()) else {
            return;
        };
        // A large static prop: 64 parts of 4,096 vertices sharing one material.
        let part = MeshData {
            positions: (0..4096).flat_map(|i| [i as f32 * 1e-3, 0., 0.]).collect(),
            normals: [0., 0., 1.].repeat(4096),
            uvs: [0.; 2].repeat(4096),
            indices: (0..4095).flat_map(|i| [0, i, i + 1]).collect(),
            bounds: [0., 0., 0., 4.1, 0., 0.],
            ..Default::default()
        };
        let model = Model {
            meshes: vec![part; 64],
            materials: vec![MaterialData::default()],
            nodes: (0..64)
                .map(|i| Node {
                    mesh: Some(i),
                    ..Default::default()
                })
                .collect(),
            bounds: [0., 0., 0., 4.1, 0., 0.],
            ..Default::default()
        };
        let mut renderer = crate::Renderer::new(
            &gpu.device,
            &gpu.queue,
            exact_gpu::wgpu::TextureFormat::Rgba8Unorm,
        );
        renderer.prepare_model("prop.model", &model).unwrap();
        let loaded = &renderer.models.loaded["prop.model"];
        assert_eq!(loaded.merged.len(), 1, "one merged draw");
        assert_eq!(loaded.starts[0].len(), 64);
        let weights = &renderer.models.skinning.as_ref().unwrap().weights;
        assert!(
            weights.raw.size() <= 64,
            "{} weight bytes",
            weights.raw.size()
        );
    }
    #[test]
    fn a_custom_material_draws_its_parts_unmerged() {
        let Some(gpu) = crate::test_device::device_or_skip(exact_gpu::fixture::device()) else {
            return;
        };
        let part = MeshData {
            positions: vec![0., 0., 0., 1., 0., 0., 0., 1., 0.],
            normals: [0., 0., 1.].repeat(3),
            uvs: [0.; 2].repeat(3),
            indices: vec![0, 1, 2],
            bounds: [0., 0., 0., 1., 1., 0.],
            ..Default::default()
        };
        let model = Model {
            meshes: vec![part; 2],
            materials: vec![MaterialData::default()],
            nodes: (0..2)
                .map(|i| Node {
                    mesh: Some(i),
                    transform: glam::Mat4::from_translation(glam::Vec3::X * i as f32)
                        .to_cols_array(),
                    ..Default::default()
                })
                .collect(),
            bounds: [0., 0., 0., 2., 1., 0.],
            ..Default::default()
        };
        let mut renderer = crate::Renderer::new(
            &gpu.device,
            &gpu.queue,
            exact_gpu::wgpu::TextureFormat::Rgba8Unorm,
        );
        renderer.prepare_model("reeds.model", &model).unwrap();
        let mut w = exact_game::World::new(60, 0);
        w.spawn((
            exact_game::Transform::default(),
            exact_game::Mesh::asset("reeds.model"),
        ));
        let mut feed = crate::Feed::default();
        feed.feed(&w, &mut renderer).unwrap();
        assert_eq!(renderer.models.records.len(), 1, "merged");
        // A game's vertex shader now shades the material: its parts keep their nodes.
        let material = renderer.models.loaded["reeds.model"].materials[0];
        renderer.models.custom.insert(material);
        renderer.models.revision += 1;
        feed.feed(&w, &mut renderer).unwrap();
        let records = &renderer.models.records;
        assert_eq!(records.len(), 2, "unmerged");
        assert_eq!(records[1].local.w_axis.x, 1., "the node offset survives");
    }
    #[test]
    fn changed_part_looks_patch_the_instance_buffer_without_rebatching() {
        let Some(gpu) = crate::test_device::device_or_skip(exact_gpu::fixture::device()) else {
            return;
        };
        let part = MeshData {
            positions: vec![0., 0., 0., 1., 0., 0., 0., 1., 0.],
            normals: [0., 0., 1.].repeat(3),
            uvs: [0.; 2].repeat(3),
            indices: vec![0, 1, 2],
            bounds: [0., 0., 0., 1., 1., 0.],
            ..Default::default()
        };
        let model = Model {
            meshes: vec![part; 2],
            materials: vec![MaterialData::default()],
            nodes: (0..2)
                .map(|i| Node {
                    name: format!("part{i}"),
                    mesh: Some(i),
                    ..Default::default()
                })
                .collect(),
            bounds: [0., 0., 0., 1., 1., 0.],
            ..Default::default()
        };
        let mut renderer = crate::Renderer::new(
            &gpu.device,
            &gpu.queue,
            exact_gpu::wgpu::TextureFormat::Rgba8Unorm,
        );
        renderer.prepare_model("pair.model", &model).unwrap();
        let mut w = exact_game::World::new(60, 0);
        let look = |red: f32| {
            exact_game::NodeMaterials(vec![exact_game::NodeMaterial {
                node: "part1".into(),
                color: [red, 0., 0., 1.],
                ..Default::default()
            }])
        };
        let e = w.spawn((
            exact_game::Transform::default(),
            exact_game::Mesh::asset("pair.model"),
            look(0.5),
        ));
        let mut feed = crate::Feed::default();
        feed.feed(&w, &mut renderer).unwrap();
        let epoch = renderer.cull.epoch;
        assert_eq!(renderer.models.part_looks.1[1][0], 0.5);
        // A pulse: the same parts, another colour.
        w.insert(e, look(0.9));
        feed.feed(&w, &mut renderer).unwrap();
        assert_eq!(renderer.cull.epoch, epoch, "no rebatch");
        assert_eq!(renderer.models.part_looks.1[1][0], 0.9);
        let records = renderer.models.records.len();
        let word = (records + 1) * super::super::INSTANCE_WORDS + 36;
        assert_eq!(renderer.models.words[word], 0.9f32.to_bits());
    }
}
