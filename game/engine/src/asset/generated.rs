use super::{AssetState, AssetStore, Assets, MaterialData, MeshData, Model, Node};
use crate::{Data, DataError, Mesh, Reader, World, Writer};
use std::collections::BTreeMap;

// Keep the geometry/identity executor out of primitive-only worlds.
#[derive(Clone, Copy)]
pub(super) struct IdentityCodec {
    write: fn(&Assets, &mut dyn Writer),
    read: fn(&Assets, &mut dyn Reader) -> Result<(), DataError>,
    restart: fn(&mut AssetStore),
}
impl AssetStore {
    pub(crate) fn restart_generated(&mut self) {
        if let Some(codec) = self.identity {
            (codec.restart)(self);
        }
    }
    pub(crate) fn identify(&mut self, name: &str, digest: u64) {
        self.identities.insert(name.into(), digest);
        self.identity = Some(&IdentityCodec {
            write: |assets, w| {
                w.field("assetIdentity");
                assets.identities.write(w);
            },
            read: |assets, r| {
                let mut saved = BTreeMap::<String, u64>::new();
                saved.read(r)?;
                for name in saved.keys().chain(assets.identities.keys()) {
                    if saved.get(name) != assets.identities.get(name) {
                        return Err(DataError::new(format!("restore refused: asset `{name}` identity differs; recreate from the original level and generator")));
                    }
                }
                Ok(())
            },
            restart: |assets| {
                // Only generated models carry identities; delivered models and
                // levels persist. Retirement also invalidates presentation caches.
                let names: Vec<_> = assets
                    .identities
                    .keys()
                    .filter(|name| name.ends_with(".model"))
                    .cloned()
                    .collect();
                for name in names {
                    assets.identities.remove(&name);
                    assets.declared.remove(&name);
                    assets.retire_name(name);
                }
                if assets.identities.is_empty() {
                    assets.identity = None;
                }
            },
        });
    }
    pub(crate) fn write_identity(&self, w: &mut dyn Writer) {
        if let Some(codec) = self.identity {
            (codec.write)(self, w);
        }
    }
    pub(crate) fn require_identity(&self, seen: bool) -> Result<(), DataError> {
        if !seen {
            if let Some(name) = self.identities.keys().next() {
                return Err(DataError::new(format!(
                    "restore refused: asset `{name}` identity is missing"
                )));
            }
        }
        Ok(())
    }
    pub(crate) fn read_identity(&self, r: &mut dyn Reader) -> Result<(), DataError> {
        if let Some(codec) = self.identity {
            return (codec.read)(self, r);
        }
        r.begin_struct()?;
        if let Some(name) = r.field()? {
            return Err(DataError::new(format!(
                "restore refused: asset `{name}` was not reconstructed before restore"
            )));
        }
        Ok(())
    }
}
impl MaterialData {
    /// An untextured opaque surface, white until vertex colours or the entity's
    /// `Material` tint it: what code-made parts want. (`Default` is glTF's
    /// material, which is fully metallic.)
    pub fn surface(metallic: f32, roughness: f32) -> Self {
        Self {
            metallic,
            roughness,
            ..Default::default()
        }
    }
}
impl Model {
    /// A model of parts for [`World::generated_model`], each a mesh drawn with
    /// its own material: code-made art that is glossy, metallic or emissive per
    /// part, prepared and shaded as a baked model's materials are. Part `i` is
    /// mesh, material and node `i` (a `MaterialOverride` addresses it as
    /// material `i`); the bounds cover every part.
    pub fn parts(parts: impl IntoIterator<Item = (MeshData, MaterialData)>) -> Self {
        let mut model = Model::default();
        for (i, (mut mesh, material)) in parts.into_iter().enumerate() {
            let b = mesh.bounds;
            model.bounds = if i == 0 {
                b
            } else {
                let a = model.bounds;
                std::array::from_fn(|k| {
                    if k < 3 {
                        a[k].min(b[k])
                    } else {
                        a[k].max(b[k])
                    }
                })
            };
            mesh.material = i as u32;
            model.meshes.push(mesh);
            model.materials.push(material);
            model.nodes.push(Node {
                mesh: Some(i as u32),
                ..Default::default()
            });
        }
        model
    }
}
impl World {
    /// Register immutable CPU geometry during setup; entities retain only its name.
    /// Reconstruct from the same declared level/seed before restoring a save. Saves
    /// retain its content identity, never vertices; changed generators refuse by name.
    /// Repeating an identical registration reuses the existing shared allocation.
    /// The mesh draws with [`MaterialData::surface`]`(0., 1.)`, matte and not
    /// metal; [`Model::parts`] gives each part its own material.
    pub fn generated(&mut self, name: &str, mesh: MeshData) -> Result<Mesh, String> {
        self.generated_model(name, Model::parts([(mesh, MaterialData::surface(0., 1.))]))
    }
    /// `generated` for a whole model: several meshes, nodes and materials, which
    /// may sample textures by name (`model.textures`, indexed by each material's
    /// texture slots) — shared `.tex` assets such as `art/textures/soil.png`, which
    /// are requested as the model's dependencies. The model is drawable once they
    /// arrive; declare them in `Game::ASSETS` to have them before setup.
    /// Skinned rigs and clips ([`crate::rig`]) register the same way.
    pub fn generated_model(&mut self, name: &str, model: Model) -> Result<Mesh, String> {
        self.sim_writes(format_args!("generated `{name}`"));
        if self.tick() != 0 {
            return Err(format!("generated `{name}`: register during setup"));
        }
        if self.assets.drawn_generated.contains(name) {
            return Err(format!(
                "generated `{name}`: Game::present made this name (Present::generated_model); a model the simulation names takes another"
            ));
        }
        self.check_generated(name, &model)?;
        let digest = crate::hash::of(&model);
        if let Some(prior) = self.assets.models.get(name) {
            if self.assets.identities.get(name) != Some(&digest)
                || crate::hash::of(prior.model.as_ref()) != digest
            {
                return Err(format!(
                    "generated `{name}`: immutable name already registered with different content"
                ));
            }
            return Ok(Mesh::asset(name));
        }
        for texture in &model.textures {
            self.assets.required.insert(texture.clone());
        }
        self.assets.identify(name, digest);
        self.assets.declared.insert(name.into());
        self.install_generated(name, model);
        Ok(Mesh::asset(name))
    }
    /// `Present::generated_model`: made by `make` the first time a present
    /// names it, then kept; nothing a tick reads or a save holds names it.
    pub(crate) fn drawn_generated_model(
        &mut self,
        name: &str,
        make: impl FnOnce() -> Model,
    ) -> Result<Mesh, String> {
        if self.assets.drawn_generated.contains(name) {
            return Ok(Mesh::asset(name));
        }
        if self.assets.identities.contains_key(name) {
            return Err(format!(
                "generated `{name}`: Game::setup registered this name for the simulation; a model only present draws takes another"
            ));
        }
        let model = make();
        self.check_generated(name, &model)?;
        self.assets.drawn_generated.insert(name.into());
        self.install_generated(name, model);
        Ok(Mesh::asset(name))
    }
    fn check_generated(&self, name: &str, model: &Model) -> Result<(), String> {
        if !super::asset_name(name) || !name.ends_with(".model") {
            return Err(format!("generated `{name}`: expected a .model asset name"));
        }
        // Refused here, not when the declared bytes land on it (which a hostless
        // test never sees): one name, one source.
        if let Some(declaration) = self.assets.declared_by.and_then(|of| of(name)) {
            return Err(format!(
                "generated `{name}`: the name is declared in {declaration}, whose delivered bytes would replace this model; give the generated model another name"
            ));
        }
        model
            .validate()
            .map_err(|e| format!("generated `{name}`: {e}"))
    }
    /// Install a generated model, Loaded once its textures (requested here) are.
    fn install_generated(&mut self, name: &str, model: Model) {
        self.assets.request(name);
        for texture in &model.textures {
            self.assets.request(texture);
        }
        let ready = model
            .textures
            .iter()
            .all(|t| self.assets.states.get(t) == Some(&AssetState::Loaded));
        let model_textures = model.textures.clone();
        self.assets.models.insert(name.into(), model.into());
        self.assets.set_dependencies(name, model_textures);
        let state = if ready {
            AssetState::Loaded
        } else {
            AssetState::Pending
        };
        self.assets.states.insert(name.into(), state);
    }
}
