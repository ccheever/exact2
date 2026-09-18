//! Baked, renderer-neutral assets. Runtime reads only the engine's binary codec.
//! Geometry and RGBA8 mip chains are ready to upload; skins/clips are data, not playback.
#![allow(missing_docs)]
use crate::Data;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

#[derive(Data, Default, Clone, Debug)]
pub struct Model {
    pub meshes: Vec<MeshData>,
    pub materials: Vec<MaterialData>,
    pub textures: Vec<TextureData>,
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
    pub filter: Filter,
}
#[derive(Data, Default, Clone, Copy, Debug)]
pub enum Wrap {
    #[default]
    Repeat,
    Clamp,
    Mirror,
}
#[derive(Data, Default, Clone, Copy, Debug)]
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
        for m in &self.materials {
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
        for t in &self.textures {
            if t.width == 0 || t.height == 0 || t.width > 16384 || t.height > 16384 {
                return fail("invalid texture dimensions");
            }
            let (mut w, mut h) = (t.width, t.height);
            if t.mips.len() != (32 - w.max(h).leading_zeros()) as usize {
                return fail("incomplete mip chain");
            }
            for mip in &t.mips {
                if mip.len() as u64 != u64::from(w) * u64::from(h) * 4 {
                    return fail("invalid mip byte count");
                }
                w = (w / 2).max(1);
                h = (h / 2).max(1);
            }
        }
        self.offsets()?;
        for s in &self.skins {
            if s.inverse_binds.len() != s.joints.len() * 16
                || s.joints.iter().any(|&i| i as usize >= self.nodes.len())
            {
                return fail("invalid skin joints/inverse binds");
            }
        }
        if self.bounds.iter().any(|v| !v.is_finite()) {
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
                done[i] = true;
            }
        }
        Ok(out)
    }
}
/// Runtime-owned immutable cache. Excluded from simulation saves and hashes.
#[derive(Default, Clone)]
pub(crate) struct Assets {
    pub models: BTreeMap<String, Arc<Model>>,
    pub pending: BTreeSet<String>,
    pub requested: BTreeSet<String>,
    pub failed: BTreeMap<String, String>,
    pub revision: u64,
}

impl crate::World {
    /// An arrived model. Declared assets are available before setup runs.
    pub fn model(&self, name: &str) -> Option<&Model> {
        self.assets.models.get(name).map(AsRef::as_ref)
    }
    /// Asset cache revision for retained render feeds.
    pub fn assets_revision(&self) -> u64 {
        self.assets.revision
    }
    /// Outstanding model names, including later first-sight requests.
    pub fn loading(&self) -> impl Iterator<Item = &str> {
        self.assets.pending.iter().map(String::as_str)
    }
}
