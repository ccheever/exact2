//! Baked, renderer-neutral assets. Runtime reads only the engine's binary codec.
//! Geometry and RGBA8 mip chains are ready to upload; skins/clips are data, not playback.
#![allow(missing_docs)]
use crate::Data;
#[path = "../../../gpu/src/asset_name.rs"]
mod names;
pub use names::asset_name;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

#[derive(Data, Default, Clone, Debug)]
pub struct Model {
    pub meshes: Vec<MeshData>,
    pub materials: Vec<MaterialData>,
    pub textures: Vec<String>,
    pub nodes: Vec<Node>,
    pub skins: Vec<Skin>,
    pub clips: Vec<Clip>,
    pub bounds: [f32; 6],
}
#[derive(Data, Default, Clone, Debug)]
pub struct MeshData {
    pub positions: Vec<f32>,
    pub normals: Vec<f32>,
    pub uvs: Vec<f32>,
    pub tangents: Vec<f32>,
    pub joints: Vec<u16>,
    pub weights: Vec<f32>,
    pub indices: Vec<u32>,
    pub material: u32,
    pub bounds: [f32; 6],
}
#[derive(Data, Clone, Debug)]
pub struct MaterialData {
    pub base_color: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    pub emissive: [f32; 3],
    pub base_color_texture: Option<u32>,
    pub normal_texture: Option<u32>,
    pub metallic_roughness_texture: Option<u32>,
    pub emissive_texture: Option<u32>,
    pub occlusion_texture: Option<u32>,
    /// Per texture affine UV transforms (column major 2x3), in the order above.
    pub uv_transforms: [[f32; 6]; 5],
    pub normal_scale: f32,
    pub occlusion_strength: f32,
    pub alpha_mode: AlphaMode,
    pub alpha_cutoff: f32,
    pub double_sided: bool,
}
impl Default for MaterialData {
    fn default() -> Self {
        Self {
            base_color: [1.0; 4],
            metallic: 1.0,
            roughness: 1.0,
            emissive: [0.0; 3],
            base_color_texture: None,
            normal_texture: None,
            metallic_roughness_texture: None,
            emissive_texture: None,
            occlusion_texture: None,
            uv_transforms: [[1.0, 0.0, 0.0, 1.0, 0.0, 0.0]; 5],
            normal_scale: 1.0,
            occlusion_strength: 1.0,
            alpha_mode: AlphaMode::Opaque,
            alpha_cutoff: 0.5,
            double_sided: false,
        }
    }
}
#[derive(Data, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlphaMode {
    #[default]
    Opaque,
    Mask,
    Blend,
}
#[derive(Data, Default, Clone, Debug)]
pub struct TextureData {
    pub width: u32,
    pub height: u32,
    pub mips: Vec<Vec<u8>>,
    pub srgb: bool,
    pub wrap: [Wrap; 2],
    pub filter: [Filter; 3],
}
#[derive(Data, Default, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Wrap {
    #[default]
    Repeat,
    Clamp,
    Mirror,
}
#[derive(Data, Default, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Filter {
    Nearest,
    #[default]
    Linear,
}
#[derive(Data, Clone, Debug)]
pub struct Node {
    pub name: String,
    pub parent: Option<u32>,
    /// Column-major local matrix. Preserves authored shear and negative scale.
    pub transform: [f32; 16],
    pub mesh: Option<u32>,
    pub skin: Option<u32>,
}
impl Default for Node {
    fn default() -> Self {
        Self {
            name: String::new(),
            parent: None,
            transform: glam::Mat4::IDENTITY.to_cols_array(),
            mesh: None,
            skin: None,
        }
    }
}
#[derive(Data, Default, Clone, Debug)]
pub struct Skin {
    pub name: String,
    pub joints: Vec<u32>,
    pub inverse_binds: Vec<f32>,
}
#[derive(Data, Default, Clone, Debug)]
pub struct Clip {
    pub name: String,
    pub tracks: Vec<Track>,
}
#[derive(Data, Default, Clone, Debug)]
pub struct Track {
    pub node: u32,
    pub path: TrackPath,
    pub interpolation: Interpolation,
    pub times: Vec<f32>,
    /// xyz / xyzw. Cubic tracks retain glTF's in-tangent, value, out-tangent triplets.
    pub values: Vec<f32>,
}
#[derive(Data, Default, Clone, Copy, Debug)]
pub enum TrackPath {
    #[default]
    Translation,
    Rotation,
    Scale,
}
#[derive(Data, Default, Clone, Copy, Debug)]
pub enum Interpolation {
    Step,
    #[default]
    Linear,
    CubicSpline,
}
impl Model {
    /// Validate all upload ranges before any renderer allocation.
    pub fn validate(&self) -> Result<(), String> {
        let fail = |s: &str| Err(format!("model: {s}"));
        for m in &self.meshes {
            let n = m.positions.len() / 3;
            if n == 0
                || m.positions.len() != n * 3
                || m.normals.len() != n * 3
                || m.uvs.len() != n * 2
                || (!m.tangents.is_empty() && m.tangents.len() != n * 4)
                || (!m.joints.is_empty() && m.joints.len() != n * 4)
                || (!m.weights.is_empty() && m.weights.len() != n * 4)
                || m.joints.is_empty() != m.weights.is_empty()
                || !valid_bounds(&m.bounds)
                || m.indices.is_empty()
                || !m.indices.len().is_multiple_of(3)
                || m.indices.iter().any(|&i| i as usize >= n)
                || m.material as usize >= self.materials.len()
            {
                return fail("invalid mesh upload range");
            }
            if m.positions
                .iter()
                .chain(&m.normals)
                .chain(&m.uvs)
                .chain(&m.tangents)
                .chain(&m.weights)
                .any(|v| !v.is_finite())
            {
                return fail("non-finite vertex");
            }
        }
        for (index, m) in self.materials.iter().enumerate() {
            if m.base_color
                .iter()
                .chain(&m.emissive)
                .chain(m.uv_transforms.iter().flatten())
                .chain([
                    &m.metallic,
                    &m.roughness,
                    &m.normal_scale,
                    &m.occlusion_strength,
                    &m.alpha_cutoff,
                ])
                .any(|v| !v.is_finite())
            {
                return Err(format!("model material {index}: non-finite scalar"));
            }
            if [
                m.base_color_texture,
                m.normal_texture,
                m.metallic_roughness_texture,
                m.emissive_texture,
                m.occlusion_texture,
            ]
            .iter()
            .flatten()
            .any(|&i| i as usize >= self.textures.len())
            {
                return fail("invalid material texture");
            }
        }
        for name in &self.textures {
            if !asset_name(name) || !name.ends_with(".tex") {
                return Err(format!(
                    "model texture `{name}`: invalid texture asset name"
                ));
            }
        }
        self.offsets()?;
        for s in &self.skins {
            if s.inverse_binds.len() != s.joints.len() * 16
                || s.joints.iter().any(|&i| i as usize >= self.nodes.len())
                || s.inverse_binds.iter().any(|v| !v.is_finite())
            {
                return fail("invalid skin joints/inverse binds");
            }
        }
        for node in &self.nodes {
            if let (Some(mesh), Some(skin)) = (node.mesh, node.skin) {
                let mesh = &self.meshes[mesh as usize];
                let skin = &self.skins[skin as usize];
                if mesh.joints.is_empty()
                    || mesh.joints.iter().any(|&j| j as usize >= skin.joints.len())
                {
                    return Err(format!(
                        "model node `{}`: invalid mesh joints for skin `{}`",
                        node.name, skin.name
                    ));
                }
            }
        }
        for clip in &self.clips {
            let mut targets = BTreeSet::new();
            for track in &clip.tracks {
                let arity = if matches!(track.path, TrackPath::Rotation) {
                    4
                } else {
                    3
                };
                let count = if matches!(track.interpolation, Interpolation::CubicSpline) {
                    3
                } else {
                    1
                };
                if track.node as usize >= self.nodes.len()
                    || !targets.insert((track.node, track.path as u8))
                    || track.times.is_empty()
                    || track.times.iter().any(|v| !v.is_finite() || *v < 0.)
                    || track.times.windows(2).any(|v| v[0] >= v[1])
                    || track.values.len() != track.times.len() * arity * count
                    || track.values.iter().any(|v| !v.is_finite())
                {
                    return Err(format!(
                        "model clip `{}` node {}: invalid node, arity, values or times",
                        clip.name, track.node
                    ));
                }
                if matches!(track.path, TrackPath::Rotation)
                    && track.values.chunks_exact(arity * count).any(|v| {
                        v[(if count == 3 { 4 } else { 0 })..][..4]
                            .iter()
                            .map(|v| v * v)
                            .sum::<f32>()
                            < 1e-12
                    })
                {
                    return Err(format!("model clip `{}`: zero rotation", clip.name));
                }
            }
        }
        if !valid_bounds(&self.bounds) {
            return fail("invalid bounds");
        }
        Ok(())
    }
    /// Compose the hierarchy once at load, preserving full affine offsets.
    pub fn offsets(&self) -> Result<Vec<glam::Mat4>, String> {
        let mut out = vec![glam::Mat4::IDENTITY; self.nodes.len()];
        let mut done = vec![false; self.nodes.len()];
        for start in 0..self.nodes.len() {
            let mut chain = Vec::new();
            let mut at = start;
            loop {
                if done[at] {
                    break;
                }
                if chain.contains(&at) {
                    return Err(format!("model node {at}: parent cycle"));
                }
                let n = &self.nodes[at];
                if n.transform.iter().any(|v| !v.is_finite())
                    || n.mesh.is_some_and(|i| i as usize >= self.meshes.len())
                    || n.skin.is_some_and(|i| i as usize >= self.skins.len())
                {
                    return Err(format!("model node {at}: invalid transform, mesh or skin"));
                }
                chain.push(at);
                match n.parent {
                    Some(i) if (i as usize) < self.nodes.len() => at = i as usize,
                    Some(_) => return Err(format!("model node {at}: invalid parent")),
                    None => break,
                }
            }
            for i in chain.into_iter().rev() {
                let n = &self.nodes[i];
                out[i] = n.parent.map_or(glam::Mat4::IDENTITY, |p| out[p as usize])
                    * glam::Mat4::from_cols_array(&n.transform);
                if !out[i].is_finite()
                    || !out[i].inverse().is_finite()
                    || n.transform[3] != 0.
                    || n.transform[7] != 0.
                    || n.transform[11] != 0.
                    || n.transform[15] != 1.
                {
                    return Err(format!(
                        "model node `{}` ({i}): singular or non-affine transform",
                        n.name
                    ));
                }
                done[i] = true;
            }
        }
        Ok(out)
    }
}
/// Per-name delivery state, outside simulation saves and hashes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AssetState {
    Pending,
    Loaded,
    Failed(String),
}
#[derive(Default, Clone)]
pub(crate) struct Assets {
    pub models: BTreeMap<String, Arc<Model>>,
    pub states: BTreeMap<String, AssetState>,
    pub declared: BTreeSet<String>,
    pub required: BTreeSet<String>,
    pub requested: BTreeSet<String>,
    pub prepared: BTreeSet<String>,
}
impl Assets {
    pub fn ready(&self) -> bool {
        self.required
            .iter()
            .all(|n| self.states.get(n) == Some(&AssetState::Loaded))
    }
    pub fn request(&mut self, name: &str) {
        self.states.entry(name.into()).or_insert_with(|| {
            if asset_name(name) {
                AssetState::Pending
            } else {
                AssetState::Failed(format!("asset `{name}`: invalid asset name"))
            }
        });
    }
    pub fn state_json(&self) -> String {
        let rows: Vec<_> = self
            .states
            .iter()
            .map(|(name, state)| {
                let (state, reason) = match state {
                    AssetState::Pending => ("Pending", String::new()),
                    AssetState::Loaded => ("Loaded", String::new()),
                    AssetState::Failed(reason) => (
                        "Failed",
                        format!(",\"reason\":{}", crate::values::quote(reason)),
                    ),
                };
                format!(
                    "{{\"name\":{},\"state\":{}{reason}}}",
                    crate::values::quote(name),
                    crate::values::quote(state)
                )
            })
            .collect();
        format!("[{}]", rows.join(","))
    }
}
impl crate::World {
    /// Only declarations are visible to simulation. Cosmetic arrival cannot change this read.
    pub fn model(&self, name: &str) -> Option<&Model> {
        self.assets
            .declared
            .contains(name)
            .then(|| self.assets.models.get(name))
            .flatten()
            .map(AsRef::as_ref)
    }
    /// Outstanding declared assets (the agent additionally lists presentation requests).
    pub fn loading(&self) -> impl Iterator<Item = &str> {
        self.assets
            .required
            .iter()
            .filter(|n| self.assets.states.get(*n) == Some(&AssetState::Pending))
            .map(String::as_str)
    }
}

fn valid_bounds(b: &[f32; 6]) -> bool {
    b.iter().all(|v| v.is_finite()) && (0..3).all(|i| b[i] <= b[i + 3])
}
impl TextureData {
    pub fn validate(&self) -> Result<(), String> {
        if self.width == 0 || self.height == 0 || self.width > 2048 || self.height > 2048 {
            return Err("texture dimensions must be 1..=2048".into());
        }
        let (mut w, mut h) = (self.width, self.height);
        if self.mips.len() != (32 - w.max(h).leading_zeros()) as usize {
            return Err("incomplete mip chain".into());
        }
        for mip in &self.mips {
            if mip.len() as u64 != u64::from(w) * u64::from(h) * 4 {
                return Err("invalid mip byte count".into());
            }
            w = (w / 2).max(1);
            h = (h / 2).max(1);
        }
        Ok(())
    }
}

/// Authored bounds for a presentation-only model, saved with its entity.
#[derive(Default, Clone, crate::Component)]
pub struct ModelBounds(pub [f32; 6]);
impl crate::Mesh {
    /// Attach deterministic local bounds to a mesh bundle, independent of delivery.
    pub fn bounds(self, bounds: [f32; 6]) -> (Self, ModelBounds) {
        assert!(
            valid_bounds(&bounds),
            "mesh bounds must be finite and ordered"
        );
        (self, ModelBounds(bounds))
    }
}
