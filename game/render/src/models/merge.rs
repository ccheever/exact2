//! A model's rigid parts that share a material draw as one mesh: static parts
//! pre-transformed into model space, animated parts as a mesh whose vertices
//! each follow their node's palette matrix, as a skin with weight one.
use super::ModelNode;
use crate::{buffers::bytes, MaterialId, MeshId, RenderError, Vertex};
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
/// Static rigid parts (a model without clips or skins) sharing a material, two
/// or more: each group's drawn-node indices, by model material.
fn static_groups(model: &Model) -> BTreeMap<u32, Vec<usize>> {
    let mut groups: BTreeMap<u32, Vec<usize>> = BTreeMap::new();
    if !model.skins.is_empty() || !model.clips.is_empty() {
        return groups;
    }
    let drawn = model.nodes.iter().filter_map(|n| n.mesh);
    for (i, mesh) in drawn.enumerate() {
        if mergeable(model, mesh) {
            let material = model.meshes[mesh as usize].material;
            groups.entry(material).or_default().push(i);
        }
    }
    groups.retain(|_, members| members.len() > 1);
    groups
}
/// Per model mesh: every node that draws it merges, so its own mesh is needed
/// only to draw the parts unmerged.
pub(super) fn merged_away(model: &Model) -> Vec<bool> {
    let drawn: Vec<usize> = (0..model.nodes.len())
        .filter(|&n| model.nodes[n].mesh.is_some())
        .collect();
    let mut grouped = vec![false; drawn.len()];
    for &i in static_groups(model).values().flatten() {
        grouped[i] = true;
    }
    for (_, skin) in animated_groups(model) {
        for n in skin.joints {
            grouped[drawn.binary_search(&(n as usize)).unwrap()] = true;
        }
    }
    let mut away: Vec<Option<bool>> = vec![None; model.meshes.len()];
    for (&node, grouped) in drawn.iter().zip(grouped) {
        let mesh = &mut away[model.nodes[node].mesh.unwrap() as usize];
        *mesh = Some(mesh.unwrap_or(true) && grouped);
    }
    away.into_iter().map(|a| a.unwrap_or(false)).collect()
}
/// Each drawn node's draw: its mesh, material, offset (identity when skinned)
/// and skin template. A part not uploaded has no mesh (`usize::MAX`).
pub(super) fn draw_nodes(
    model: &Model,
    parts: &[Option<MeshId>],
    materials: &[MaterialId],
    skins: &[u32],
) -> Result<Vec<ModelNode>, RenderError> {
    let mut rigid = skins.iter().skip(model.skins.len());
    let offsets = model.offsets().map_err(RenderError::scene)?;
    Ok((model.nodes.iter().zip(offsets))
        .filter_map(|(n, local)| {
            n.mesh.map(|m| {
                let skin = n
                    .skin
                    .map(|s| skins[s as usize])
                    .or_else(|| rigid.next().copied());
                (
                    parts[m as usize].unwrap_or(MeshId(usize::MAX)),
                    materials[model.meshes[m as usize].material as usize],
                    if skin.is_some() {
                        Mat4::IDENTITY
                    } else {
                        local
                    },
                    skin,
                )
            })
        })
        .collect())
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

/// A model's draw list, each draw's part names and part first vertices, and the
/// merged meshes.
pub(super) type Merged = (Vec<ModelNode>, Vec<Vec<String>>, Vec<Vec<u32>>, Vec<MeshId>);

impl<const ASSETS: bool> crate::renderer::RendererWithAssets<ASSETS> {
    /// The model's draw list with each merged group at its first member's place,
    /// each draw's part names, and the merged meshes; empty when nothing merges.
    /// `meshes[i]` is drawn node `i`'s mesh; `animated` and `skins` are the
    /// animated groups and their skin templates.
    pub(super) fn merge_static(
        &mut self,
        model: &Model,
        nodes: &[ModelNode],
        meshes: &[u32],
        animated: &[(u32, Skin)],
        skins: &[u32],
    ) -> Merged {
        // Static: unskinned, unanimated nodes sharing a material.
        let groups = static_groups(model);
        if groups.is_empty() && animated.is_empty() {
            return (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        }
        let drawn: Vec<usize> = (0..model.nodes.len())
            .filter(|&n| model.nodes[n].mesh.is_some())
            .collect();
        let mut first = BTreeMap::new();
        let mut merged = Vec::new();
        for members in groups.values() {
            let (mut vertices, mut indices, mut starts) = (Vec::new(), Vec::new(), Vec::new());
            for &i in members {
                let local = nodes[i].2;
                let normal = Mat3::from_mat4(local).inverse().transpose();
                let mesh = &model.meshes[meshes[i] as usize];
                let base = vertices.len() as u32;
                starts.push(base);
                let count = mesh.positions.len() / 3;
                vertices.extend((0..count).map(|v| vertex(mesh, v, local, normal)));
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
            merged.push(id);
            let draw = (id, nodes[members[0]].1, Mat4::IDENTITY, None);
            first.insert(members[0], (draw, members.clone(), starts));
        }
        for ((_, skin), &template) in animated.iter().zip(skins) {
            let members: Vec<usize> = skin
                .joints
                .iter()
                .map(|&n| drawn.binary_search(&(n as usize)).unwrap())
                .collect();
            let (mut vertices, mut indices, mut parts) = (Vec::new(), Vec::new(), Vec::new());
            let mut starts = Vec::new();
            for (joint, &i) in members.iter().enumerate() {
                let mesh = &model.meshes[meshes[i] as usize];
                let base = vertices.len() as u32;
                starts.push(base);
                let count = mesh.positions.len() / 3;
                vertices
                    .extend((0..count).map(|v| vertex(mesh, v, Mat4::IDENTITY, Mat3::IDENTITY)));
                parts.extend(std::iter::repeat_n(joint as u32, count));
                indices.extend(mesh.indices.iter().map(|i| base + i));
            }
            let id = self.merged_mesh(&vertices, &indices, &parts);
            merged.push(id);
            let draw = (id, nodes[members[0]].1, Mat4::IDENTITY, Some(template));
            first.insert(members[0], (draw, members, starts));
        }
        let grouped: std::collections::BTreeSet<usize> = first
            .values()
            .flat_map(|(_, m, _)| m.iter().copied())
            .collect();
        let name = |i: usize| model.nodes[drawn[i]].name.clone();
        let (mut draws, mut parts, mut ranges) = (Vec::new(), Vec::new(), Vec::new());
        for (i, &node) in nodes.iter().enumerate() {
            if let Some((draw, members, starts)) = first.get(&i) {
                draws.push(*draw);
                parts.push(members.iter().map(|&m| name(m)).collect());
                ranges.push(starts.clone());
            } else if !grouped.contains(&i) {
                draws.push(node);
                parts.push(vec![name(i)]);
                ranges.push(vec![0]);
            }
        }
        (draws, parts, ranges, merged)
    }
    /// Whether a `CustomMaterial` shades a merged draw of a model whose parts
    /// are not resident: preparing that model again uploads them.
    pub(crate) fn wants_parts(&self) -> bool {
        (self.models.loaded.keys()).any(|name| self.parts_wanted(name))
    }
    fn parts_wanted(&self, name: &str) -> bool {
        self.models.loaded.get(name).is_some_and(|m| {
            m.active
                && m.nodes.is_empty()
                && m.merged.iter().any(|n| self.models.custom.contains(&n.1))
        })
    }
    /// Upload a model's merged-away parts once a `CustomMaterial` needs them
    /// (its vertex shader sees node-local positions, which a merge has not).
    pub(super) fn upload_parts(&mut self, name: &str, model: &Model) -> Result<(), RenderError> {
        if !self.parts_wanted(name) {
            return Ok(());
        }
        let mut parts = self.models.loaded[name].parts.clone();
        for (part, mesh) in parts.iter_mut().zip(&model.meshes) {
            if part.is_none() {
                let id = self.add_model_mesh(mesh);
                self.meshes[id.0].asset = true;
                *part = Some(id);
            }
        }
        let m = &self.models.loaded[name];
        let nodes = draw_nodes(model, &parts, &m.materials, &m.skins)?;
        let m = self.models.loaded.get_mut(name).unwrap();
        m.meshes.extend(
            (parts.iter().zip(&m.parts))
                .filter(|(_, old)| old.is_none())
                .map(|(new, _)| new.unwrap()),
        );
        m.parts = parts;
        m.nodes = nodes;
        self.models.revision += 1;
        Ok(())
    }
    /// Upload one model mesh, and its joints and weights when skinned.
    pub(super) fn add_model_mesh(&mut self, mesh: &MeshData) -> MeshId {
        let vertices: Vec<_> = (mesh.positions.chunks_exact(3).enumerate())
            .map(|(i, p)| Vertex {
                position: [p[0], p[1], p[2]],
                normal: mesh.normals[i * 3..i * 3 + 3].try_into().unwrap(),
                uv: mesh.uvs[i * 2..i * 2 + 2].try_into().unwrap(),
                color: if mesh.colors.is_empty() {
                    [1.; 4]
                } else {
                    mesh.colors[i * 4..i * 4 + 4].try_into().unwrap()
                },
            })
            .collect();
        let id = self.add_mesh(&vertices, &mesh.indices);
        if !mesh.joints.is_empty() {
            let start = self.meshes[id.0].base_vertex as u64 * 32;
            let mut words = Vec::with_capacity(mesh.joints.len() * 2);
            for (j, w) in (mesh.joints.chunks_exact(4)).zip(mesh.weights.chunks_exact(4)) {
                words.extend(j.iter().map(|j| u32::from(*j)));
                words.extend(w.iter().map(|w| w.to_bits()));
            }
            let weights = &mut self.models.skinning.as_mut().unwrap().weights;
            self.models.reallocations += u64::from(weights.grow(
                &self.device,
                &self.queue,
                start + (words.len() * 4) as u64,
            ));
            weights.write(&self.queue, start, bytes(&words));
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
        buffer.write(&self.queue, start, bytes(&words));
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
        // Only the merged mesh is uploaded and resident: the parts are not.
        let resident: u64 = renderer.meshes.iter().map(|m| m.vertex_bytes).sum();
        eprintln!(
            "64-part prop: {} mesh uploads, {resident} resident vertex bytes",
            renderer.mesh_uploads
        );
        assert_eq!(renderer.mesh_uploads, 1);
        assert_eq!(resident, 64 * 4096 * 48);
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
        assert_eq!(renderer.mesh_uploads, 1, "the parts are not uploaded");
        assert!(!renderer.wants_parts());
        // A game's vertex shader now shades the material: its parts keep their nodes.
        let material = renderer.models.loaded["reeds.model"].materials[0];
        renderer.models.custom.insert(material);
        renderer.models.revision += 1;
        // Until the model is prepared again its parts are not resident: it
        // draws merged and asks for them.
        feed.feed(&w, &mut renderer).unwrap();
        assert_eq!(renderer.models.records.len(), 1, "still merged");
        assert!(renderer.wants_parts());
        renderer.prepare_model("reeds.model", &model).unwrap();
        assert!(!renderer.wants_parts());
        assert_eq!(renderer.mesh_uploads, 3, "both parts uploaded once");
        feed.feed(&w, &mut renderer).unwrap();
        let records = &renderer.models.records;
        assert_eq!(records.len(), 2, "unmerged");
        assert_eq!(records[1].local.w_axis.x, 1., "the node offset survives");
        renderer.prepare_model("reeds.model", &model).unwrap();
        assert_eq!(
            renderer.mesh_uploads, 3,
            "resident parts are not uploaded again"
        );
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
        // The run's first entry keeps its part count for the shader's search.
        let run = records * super::super::INSTANCE_WORDS;
        assert_eq!(renderer.models.words[run], 2, "two parts in the run");
    }
}
