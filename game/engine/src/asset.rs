//! Baked, renderer-neutral assets. Runtime reads only the engine's binary codec.
//! Geometry and mip chains (RGBA8 or GPU block formats) are ready to upload;
//! skins/clips are data, not playback.
#![allow(missing_docs)]
use crate::Data;
#[path = "../../../gpu/src/asset_name.rs"]
mod names;
pub use names::asset_name;
use std::{collections::BTreeSet, mem::ManuallyDrop, rc::Rc, sync::Arc};

mod build;
mod generated;
pub(crate) mod level;
mod map;
mod sound;
pub use build::{across, MeshBuilder};
pub use level::{Level, LevelValue};
pub use sound::{SoundAsset, SoundData, SOUND_BYTE_BUDGET, SOUND_RATES};
/// Renderer-neutral pose records and rig geometry.
pub mod pose;

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
    /// Optional linear RGBA vertex colours; empty means white.
    pub colors: Vec<f32>,
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
/// A complete mip chain in one texel encoding. Block formats are uploaded
/// as baked: the module carries no transcoder.
#[derive(Data, Default, Clone, Debug)]
pub struct TextureData {
    pub width: u32,
    pub height: u32,
    pub mips: Vec<Vec<u8>>,
    pub srgb: bool,
    pub wrap: [Wrap; 2],
    pub filter: [Filter; 3],
    pub format: TextureFormat,
}
/// How every mip of a `.tex` record is encoded. Block formats use 4×4 blocks;
/// a record's base dimensions are whole blocks (WebGPU's rule).
/// @ref llp/1046.003-game-engine-as-built.explainer.md#compressed-textures-2026-09-23
#[derive(Data, Default, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum TextureFormat {
    /// Four bytes per texel; every device samples it.
    #[default]
    Rgba8,
    /// One linear channel (R), 8 bytes per block.
    Bc4,
    /// Two linear channels (RG), 16 bytes per block.
    Bc5,
    /// RGBA, sRGB or linear, 16 bytes per block.
    Bc7,
    /// RGBA, sRGB or linear, 16 bytes per 4×4 block.
    Astc4x4,
}
/// Which per-device payload a texture request names. The baker writes one
/// file per family beside the authored `.tex` name; a device fetches one.
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextureFamily {
    /// The authored name: RGBA8, the fallback every device and host reads.
    #[default]
    Rgba8,
    /// `name.bc.tex`: BC4/BC5/BC7 (WebGPU `texture-compression-bc`).
    Bc,
    /// `name.astc.tex`: ASTC 4×4 (WebGPU `texture-compression-astc`).
    Astc,
}
/// RGBA8 payloads stop at 2048²; block formats reach 4096² in the same bytes.
pub const RGBA8_TEXTURE_LIMIT: u32 = 2048;
/// The largest block-compressed texture edge.
pub const BLOCK_TEXTURE_LIMIT: u32 = 4096;
impl TextureFormat {
    /// Every format, in declaration order (`format as usize` indexes it).
    pub const ALL: [Self; 5] = [Self::Rgba8, Self::Bc4, Self::Bc5, Self::Bc7, Self::Astc4x4];
    /// The format's name, as errors and `state.world.gpu.textures` report it.
    pub fn name(self) -> &'static str {
        ["Rgba8", "Bc4", "Bc5", "Bc7", "Astc4x4"][self as usize]
    }
    /// Texel edge of one block and its bytes (RGBA8 is a one-texel block).
    pub fn block(self) -> (u32, usize) {
        match self {
            Self::Rgba8 => (1, 4),
            Self::Bc4 => (4, 8),
            Self::Bc5 | Self::Bc7 | Self::Astc4x4 => (4, 16),
        }
    }
    /// The payload family that carries this format.
    pub fn family(self) -> TextureFamily {
        match self {
            Self::Rgba8 => TextureFamily::Rgba8,
            Self::Bc4 | Self::Bc5 | Self::Bc7 => TextureFamily::Bc,
            Self::Astc4x4 => TextureFamily::Astc,
        }
    }
    /// Bytes of one mip level of `width` × `height` texels.
    pub fn level_bytes(self, width: u32, height: u32) -> u64 {
        let (edge, bytes) = self.block();
        u64::from(width.div_ceil(edge)) * u64::from(height.div_ceil(edge)) * bytes as u64
    }
    /// The largest base edge a record of this format may carry.
    pub fn limit(self) -> u32 {
        if self == Self::Rgba8 {
            RGBA8_TEXTURE_LIMIT
        } else {
            BLOCK_TEXTURE_LIMIT
        }
    }
}
impl TextureFamily {
    /// The family's name, as `state.world.gpu.textureFamily` reports it.
    pub fn label(self) -> &'static str {
        ["Rgba8", "Bc", "Astc"][self as usize]
    }
    /// The file a device of this family requests for the authored `.tex`
    /// name; RGBA8 is the authored name itself. Other names have no variants.
    pub fn name(self, texture: &str) -> String {
        match (self, texture.strip_suffix(".tex")) {
            (Self::Bc, Some(stem)) => format!("{stem}.bc.tex"),
            (Self::Astc, Some(stem)) => format!("{stem}.astc.tex"),
            _ => texture.into(),
        }
    }
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
    /// Seconds and event name; absent extras leave this empty.
    pub markers: Vec<(f32, String)>,
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
        if self.textures.len() > 64 {
            return fail("at most 64 textures are allowed");
        }
        let mut used = BTreeSet::new();
        for material in self
            .nodes
            .iter()
            .filter_map(|node| node.mesh)
            .filter_map(|mesh| self.meshes.get(mesh as usize))
            .filter_map(|mesh| self.materials.get(mesh.material as usize))
        {
            used.extend(
                [
                    material.base_color_texture,
                    material.normal_texture,
                    material.metallic_roughness_texture,
                    material.emissive_texture,
                    material.occlusion_texture,
                ]
                .into_iter()
                .flatten(),
            );
        }
        for (index, name) in self.textures.iter().enumerate() {
            if !used.contains(&(index as u32)) {
                return Err(format!("model: unused texture `{name}`"));
            }
        }
        for m in &self.meshes {
            let n = m.positions.len() / 3;
            if n == 0
                || m.positions.len() != n * 3
                || m.normals.len() != n * 3
                || m.uvs.len() != n * 2
                || (!m.colors.is_empty() && m.colors.len() != n * 4)
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
                .chain(&m.weights)
                .chain(&m.colors)
                .any(|v| !v.is_finite())
            {
                return fail("non-finite vertex");
            }
        }
        for mesh in &self.meshes {
            if mesh
                .weights
                .chunks_exact(4)
                .any(|w| w.iter().any(|v| *v < 0.) || (w.iter().sum::<f32>() - 1.).abs() > 1e-4)
            {
                return fail("skin weights must be nonnegative and normalized");
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
        if (!self.skins.is_empty() || !self.clips.is_empty()) && self.nodes.len() > 256 {
            return fail("animated models support at most 256 imported nodes");
        }
        for s in &self.skins {
            if s.joints.is_empty()
                || s.joints.len() > 256
                || s.inverse_binds.len() != s.joints.len() * 16
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
            if clip.markers.iter().any(|(t, name)| {
                !t.is_finite() || *t < 0. || *t > clip.duration() || name.is_empty()
            }) {
                return Err(format!("model clip `{}`: invalid marker", clip.name));
            }
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
        let mut state = vec![0_u8; self.nodes.len()];
        let mut chain = Vec::new();
        for start in 0..self.nodes.len() {
            let mut at = start;
            loop {
                if state[at] == 2 {
                    break;
                }
                if state[at] == 1 {
                    return Err(format!("model node {at}: parent cycle"));
                }
                let n = &self.nodes[at];
                if n.transform.iter().any(|v| !v.is_finite())
                    || n.mesh.is_some_and(|i| i as usize >= self.meshes.len())
                    || n.skin.is_some_and(|i| i as usize >= self.skins.len())
                {
                    return Err(format!("model node {at}: invalid transform, mesh or skin"));
                }
                state[at] = 1;
                chain.push(at);
                match n.parent {
                    Some(i) if (i as usize) < self.nodes.len() => at = i as usize,
                    Some(_) => return Err(format!("model node {at}: invalid parent")),
                    None => break,
                }
            }
            while let Some(i) = chain.pop() {
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
                state[i] = 2;
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
    pub models: map::AssetMap<ModelAsset>,
    pub identities: std::collections::BTreeMap<String, u64>,
    pub levels: map::AssetMap<Arc<LevelValue>>,
    pub sounds: map::AssetMap<Arc<SoundAsset>>,
    pub states: map::AssetMap<AssetState>,
    pub declared: BTreeSet<String>,
    /// Generated models only `Game::present` draws (`Present::generated_model`):
    /// made the first time a present names them, never saved, never retired.
    pub drawn_generated: BTreeSet<String>,
    pub required: BTreeSet<String>,
    pub requested: BTreeSet<String>,
    pub prepared: BTreeSet<String>,
    pub redelivery: BTreeSet<String>,
    /// Names an entity's mesh or sprite showed when requests were last taken:
    /// shown `Game::STREAMED` models are fetched and prepared first.
    pub shown: BTreeSet<String>,
    pub dependencies: map::AssetMap<Vec<String>>,
    /// Each texture's models, `dependencies` reversed: a delivery finishes
    /// only the models it concerns. Write both through `set_dependencies`.
    pub dependents: map::AssetMap<Vec<String>>,
    pub retired: Vec<String>,
}

// The delivery owner is installed by the first asset mutation. Primitive worlds
// borrow an immutable empty view without linking the model owner's clone/drop.
#[derive(Default, Clone)]
pub(crate) struct AssetStore {
    owner: Option<ManuallyDrop<Rc<Assets>>>,
    release: Option<fn(ManuallyDrop<Rc<Assets>>)>,
    identity: Option<&'static generated::IdentityCodec>,
    /// Which of the game's declarations (`Game::ASSETS`, `STREAMED`, `LEVELS`)
    /// names an asset; installed by `Sim` before setup, so a generated model
    /// cannot take a name whose delivered bytes would land on it.
    pub(crate) declared_by: Option<fn(&str) -> Option<&'static str>>,
}
impl Drop for AssetStore {
    fn drop(&mut self) {
        if let Some(owner) = self.owner.take() {
            self.release.expect("asset owner release")(owner);
        }
    }
}
impl std::ops::Deref for AssetStore {
    type Target = Assets;
    fn deref(&self) -> &Assets {
        static EMPTY: Assets = Assets {
            models: map::AssetMap::EMPTY,
            identities: std::collections::BTreeMap::new(),
            levels: map::AssetMap::EMPTY,
            sounds: map::AssetMap::EMPTY,
            states: map::AssetMap::EMPTY,
            declared: BTreeSet::new(),
            drawn_generated: BTreeSet::new(),
            required: BTreeSet::new(),
            requested: BTreeSet::new(),
            prepared: BTreeSet::new(),
            redelivery: BTreeSet::new(),
            shown: BTreeSet::new(),
            dependencies: map::AssetMap::EMPTY,
            dependents: map::AssetMap::EMPTY,
            retired: Vec::new(),
        };
        self.owner.as_ref().map_or(&EMPTY, |owner| owner.as_ref())
    }
}
impl std::ops::DerefMut for AssetStore {
    fn deref_mut(&mut self) -> &mut Assets {
        if self.owner.is_none() {
            // The callback owns destruction; ManuallyDrop keeps that executor
            // out of primitive worlds. No raw pointers or unsafe drops are used.
            self.owner = Some(ManuallyDrop::new(Rc::new(Assets::default())));
            self.release = Some(|owner| drop(ManuallyDrop::into_inner(owner)));
            self.identity = None;
        }
        Rc::make_mut(self.owner.as_mut().unwrap())
    }
}
#[derive(Clone)]
pub(crate) struct ModelAsset {
    pub model: Arc<Model>,
    pub bounds: [f32; 6],
}
impl From<Model> for ModelAsset {
    fn from(model: Model) -> Self {
        Self {
            bounds: pose::animated_bounds(&model),
            model: Arc::new(model),
        }
    }
}
impl Assets {
    /// A model's textures, and the reverse index with them.
    pub fn set_dependencies(&mut self, model: &str, textures: Vec<String>) {
        self.remove_dependencies(model);
        for texture in &textures {
            match self.dependents.get_mut(texture) {
                Some(models) if !models.iter().any(|m| m == model) => models.push(model.into()),
                Some(_) => {}
                None => self.dependents.insert(texture.clone(), vec![model.into()]),
            }
        }
        self.dependencies.insert(model.into(), textures);
    }
    pub fn remove_dependencies(&mut self, model: &str) {
        let Some(textures) = self.dependencies.get(model).cloned() else {
            return;
        };
        for texture in textures {
            if let Some(models) = self.dependents.get_mut(&texture) {
                models.retain(|m| m != model);
                if models.is_empty() {
                    self.dependents.remove(&texture);
                }
            }
        }
        self.dependencies.remove(model);
    }
    pub fn ready(&self) -> bool {
        self.required
            .iter()
            .all(|n| self.states.get(n).is_none_or(|s| *s == AssetState::Loaded))
    }
    pub fn request(&mut self, name: &str) {
        if !self.states.contains_key(name) {
            let state = if asset_name(name) {
                AssetState::Pending
            } else {
                AssetState::Failed(format!("asset `{name}`: invalid asset name"))
            };
            self.states.insert(name.into(), state);
        }
    }
    pub fn retire(&mut self, roots: &BTreeSet<String>) {
        let mut live = roots.clone();
        live.extend(self.identities.keys().cloned());
        live.extend(self.drawn_generated.iter().cloned());
        for name in roots.iter().chain(&self.drawn_generated) {
            if let Some(deps) = self.dependencies.get(name) {
                live.extend(deps.iter().cloned());
            }
        }
        let removed: Vec<_> = self
            .states
            .keys()
            .filter(|n| !live.contains(*n))
            .cloned()
            .collect();
        for name in removed {
            self.retire_name(name);
        }
    }
    pub(super) fn retire_name(&mut self, name: String) {
        self.states.remove(&name);
        if !self.declared.contains(&name) {
            self.models.remove(&name);
            self.remove_dependencies(&name);
        }
        self.requested.remove(&name);
        self.prepared.remove(&name);
        self.redelivery.remove(&name);
        self.retired.push(name);
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
                let value = self
                    .levels
                    .get(name)
                    .map_or(String::new(), |v| format!(",\"value\":{}", v.text));
                format!(
                    "{{\"name\":{},\"state\":{}{reason}{value}}}",
                    crate::values::quote(name),
                    crate::values::quote(state)
                )
            })
            .collect();
        format!("[{}]", rows.join(","))
    }
}
impl crate::World {
    /// Delivery stamp for presentation caches; excluded from simulation state.
    pub fn model_revision(&self) -> u64 {
        self.assets.models.revision()
    }

    /// Only declarations are visible to simulation. Cosmetic arrival cannot change this read.
    pub fn model(&self, name: &str) -> Option<&Model> {
        self.model_asset(name).map(|asset| asset.model.as_ref())
    }
    pub(crate) fn model_asset(&self, name: &str) -> Option<&ModelAsset> {
        self.assets
            .declared
            .contains(name)
            .then(|| self.assets.models.get(name))
            .flatten()
    }
    /// Outstanding declared assets (the agent additionally lists presentation requests).
    pub fn loading(&self) -> impl Iterator<Item = &str> {
        self.assets
            .required
            .iter()
            .filter(|n| self.assets.states.get(n) == Some(&AssetState::Pending))
            .map(String::as_str)
    }
}

fn valid_bounds(b: &[f32; 6]) -> bool {
    b.iter().all(|v| v.is_finite()) && (0..3).all(|i| b[i] <= b[i + 3])
}
impl TextureData {
    pub fn validate(&self) -> Result<(), String> {
        let (format, limit) = (self.format, self.format.limit());
        if self.width == 0 || self.height == 0 || self.width > limit || self.height > limit {
            return Err(format!(
                "{} texture dimensions must be 1..={limit}",
                format.name()
            ));
        }
        let (edge, _) = format.block();
        if !self.width.is_multiple_of(edge) || !self.height.is_multiple_of(edge) {
            return Err(format!(
                "{} texture {}x{} is not whole {edge}x{edge} blocks",
                format.name(),
                self.width,
                self.height
            ));
        }
        if self.srgb && matches!(format, TextureFormat::Bc4 | TextureFormat::Bc5) {
            return Err(format!("{} textures are linear", format.name()));
        }
        let (mut w, mut h) = (self.width, self.height);
        if self.mips.len() != (32 - w.max(h).leading_zeros()) as usize {
            return Err("incomplete mip chain".into());
        }
        for mip in &self.mips {
            if mip.len() as u64 != format.level_bytes(w, h) {
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

/// Validated content delivered to the name/dependency gate.
pub enum Content {
    Model(Model),
    Texture(TextureData),
    /// JSON is typed by the game declaration when it crosses the barrier.
    Level(String),
    /// Baked 16-bit PCM; decoded in primitive and model modules alike.
    Sound(SoundData),
}
impl Content {
    pub fn decode<const MODELS: bool>(name: &str, bytes: &[u8]) -> Result<Self, String> {
        if !asset_name(name) {
            return Err("invalid asset name".into());
        }
        if bytes.len() > 64 * 1024 * 1024 {
            return Err("exceeds 64 MiB".into());
        }
        if name.ends_with(".level.json") {
            Ok(Self::Level(
                std::str::from_utf8(bytes)
                    .map_err(|e| e.to_string())?
                    .into(),
            ))
        } else if name.ends_with(".sound") {
            let sound: SoundData = crate::bin::from_slice(bytes).map_err(|e| e.to_string())?;
            sound.validate()?;
            Ok(Self::Sound(sound))
        } else if MODELS && name.ends_with(".tex") {
            let texture: TextureData = crate::bin::from_slice(bytes).map_err(|e| e.to_string())?;
            texture.validate()?;
            Ok(Self::Texture(texture))
        } else if MODELS && name.ends_with(".model") {
            let model: Model = crate::bin::from_slice(bytes).map_err(|e| e.to_string())?;
            model.validate()?;
            Ok(Self::Model(model))
        } else {
            Err("expected .model, .tex, .sound or .level.json".into())
        }
    }
}
/// Whether a module without model support still receives deliveries: a declared
/// JSON level or declared `.sound` assets.
pub fn delivers_without_models<G: crate::Game>() -> bool {
    !G::LEVELS.is_empty() || G::ASSETS.iter().any(|name| name.ends_with(".sound"))
}
impl<G: crate::Game> crate::Sim<G> {
    /// Headless model decoder. Primitive surfaces never link this adapter.
    pub fn asset(&mut self, name: &str, bytes: Option<&[u8]>) -> Result<(), String> {
        self.deliver_asset(
            name,
            bytes
                .ok_or_else(|| "missing file".to_owned())
                .and_then(|b| Content::decode::<true>(name, b)),
        )
    }
}

#[cfg(test)]
mod texture_tests {
    use super::*;
    fn chain(format: TextureFormat, width: u32, height: u32) -> TextureData {
        let (mut w, mut h, mut mips) = (width, height, Vec::new());
        loop {
            mips.push(vec![0; format.level_bytes(w, h) as usize]);
            if w == 1 && h == 1 {
                break;
            }
            (w, h) = ((w / 2).max(1), (h / 2).max(1));
        }
        TextureData {
            width,
            height,
            mips,
            format,
            ..Default::default()
        }
    }
    #[test]
    fn block_records_are_whole_blocks_linear_where_single_purpose_and_4096_at_most() {
        chain(TextureFormat::Bc7, 4096, 8).validate().unwrap();
        chain(TextureFormat::Astc4x4, 12, 4).validate().unwrap();
        // Levels below one block still occupy a whole block.
        assert_eq!(chain(TextureFormat::Bc4, 8, 4).mips[3].len(), 8);
        let refused = |t: TextureData| t.validate().unwrap_err();
        assert!(refused(chain(TextureFormat::Rgba8, 4096, 4)).contains("1..=2048"));
        assert!(refused(chain(TextureFormat::Bc7, 8192, 4)).contains("1..=4096"));
        assert!(refused(chain(TextureFormat::Bc5, 6, 4)).contains("4x4 blocks"));
        let mut srgb = chain(TextureFormat::Bc5, 4, 4);
        srgb.srgb = true;
        assert!(refused(srgb).contains("linear"));
        let mut short = chain(TextureFormat::Astc4x4, 8, 8);
        short.mips[1].pop();
        assert!(refused(short).contains("byte count"));
    }
    #[test]
    fn families_name_their_files_beside_the_authored_texture() {
        assert_eq!(TextureFamily::Rgba8.name("fox/0.tex"), "fox/0.tex");
        assert_eq!(TextureFamily::Bc.name("fox/0.tex"), "fox/0.bc.tex");
        assert_eq!(TextureFamily::Astc.name("fox/0.tex"), "fox/0.astc.tex");
        assert_eq!(TextureFamily::Bc.name("fox.model"), "fox.model");
        assert_eq!(TextureFormat::Bc4.family(), TextureFamily::Bc);
        assert_eq!(TextureFormat::Astc4x4.family(), TextureFamily::Astc);
    }
}
