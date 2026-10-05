//! Presentation replacements for what the simulation draws: a mesh, a light,
//! a camera's sky. `Game::present` writes them from saved causes; the renderer
//! draws them in place of the simulated `Mesh`, `ModelLod` and `Material`, the
//! entity's light, and the `Environment` and `AmbientOcclusion` resources. A
//! look that only changes these (a model per growth stage, a day and night, a
//! weather's fog) never moves a save, a hash or a pin.
// @ref llp/1046.008-game-presentation-seams.plan.md#amendment-2026-10-05-presentation-swaps
use crate::{
    AmbientOcclusion, DirectionalLight, Environment, Material, Mesh, ModelLod, PointLight,
    SpotLight,
};

/// What this entity draws, in place of its `Mesh` and `ModelLod` (an entity
/// without a `Mesh` draws it too): a model per growth stage, one look's model
/// of a prop another look draws differently. `lod` replaces the entity's
/// `ModelLod` with the mesh (levels belong to a model; `None` draws no levels);
/// `material`, when set, replaces its `Material`. Pose, `Visible`, `Opacity`,
/// `Tint`, `NodeMaterials` and `MaterialOverrides` apply to it as to the
/// simulated mesh. Picking, physics, animation, sockets and saved asset
/// checks keep the simulated `Mesh`; a model named here is requested like one
/// a `Mesh` names. Presentation state: write it from `Game::present`.
#[derive(Clone, Debug, Default, PartialEq, crate::Presentation)]
pub struct DrawnMesh {
    /// The drawn mesh or model.
    pub mesh: Mesh,
    /// Its levels of detail; `None` draws only `mesh`.
    pub lod: Option<ModelLod>,
    /// Its material; `None` keeps the entity's.
    pub material: Option<Material>,
}
impl DrawnMesh {
    /// Draw this mesh, with no levels and the entity's own material.
    pub fn new(mesh: Mesh) -> Self {
        Self {
            mesh,
            lod: None,
            material: None,
        }
    }
    /// Draw a baked or generated model by name.
    pub fn model(name: impl Into<String>) -> Self {
        Self::new(Mesh::asset(name))
    }
    /// With these levels of detail.
    pub fn lod(mut self, lod: ModelLod) -> Self {
        self.lod = Some(lod);
        self
    }
    /// With this material in place of the entity's.
    pub fn material(mut self, material: Material) -> Self {
        self.material = Some(material);
        self
    }
}

/// The light this entity casts as drawn, in place of any simulated
/// `DirectionalLight`, `PointLight` or `SpotLight` it carries (an entity with
/// none casts it too): a sun that follows a day, a moon, a lantern lit at dusk.
/// It is posed like a simulated light, `Offset` included, so `present` aims a
/// sun with an `Offset`. The first two drawn directional lights in entity order
/// are the sun and the fill, simulated or drawn; `Visible`, `Lit` and
/// `LightShadows` (simulation) apply as they do to a simulated light.
/// Presentation state: write it from `Game::present`.
#[derive(Clone, Copy, Debug, PartialEq, crate::Presentation)]
pub enum DrawnLight {
    /// Parallel rays along the entity's −Z.
    Directional(DirectionalLight),
    /// An omnidirectional light at the entity.
    Point(PointLight),
    /// A cone along the entity's −Z.
    Spot(SpotLight),
}
impl Default for DrawnLight {
    fn default() -> Self {
        Self::Directional(DirectionalLight::default())
    }
}

/// A camera's own sky and post effects: frames this camera draws use them in
/// place of the `Environment` and `AmbientOcclusion` resources (`None` turns
/// occlusion off), so `present` can move a sky with the time of day or a
/// weather's fog without a simulated write. `EnvironmentMap` stays the
/// resource's. Presentation state, on the camera entity: write it from
/// `Game::present`.
#[derive(Clone, Copy, Debug, Default, PartialEq, crate::Presentation)]
pub struct DrawnEnvironment {
    /// The sky, fog, exposure and bloom drawn.
    pub environment: Environment,
    /// Screen-space ambient occlusion drawn; `None` draws none.
    pub ambient_occlusion: Option<AmbientOcclusion>,
}
