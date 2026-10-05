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
impl World {
    /// Register immutable CPU geometry during setup; entities retain only its name.
    /// Reconstruct from the same declared level/seed before restoring a save. Saves
    /// retain its content identity, never vertices; changed generators refuse by name.
    /// Repeating an identical registration reuses the existing shared allocation.
    pub fn generated(&mut self, name: &str, mesh: MeshData) -> Result<Mesh, String> {
        self.sim_writes(format_args!("generated `{name}`"));
        if self.tick() != 0 {
            return Err(format!("generated `{name}`: register during setup"));
        }
        if !super::asset_name(name) || !name.ends_with(".model") {
            return Err(format!("generated `{name}`: expected a .model asset name"));
        }
        let model = Model {
            bounds: mesh.bounds,
            meshes: vec![mesh],
            materials: vec![MaterialData {
                metallic: 0.,
                ..Default::default()
            }],
            nodes: vec![Node {
                mesh: Some(0),
                ..Default::default()
            }],
            ..Default::default()
        };
        model
            .validate()
            .map_err(|e| format!("generated `{name}`: {e}"))?;
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
        self.assets.request(name);
        self.assets.identify(name, digest);
        self.assets.declared.insert(name.into());
        self.assets.models.insert(name.into(), model.into());
        self.assets.dependencies.insert(name.into(), Vec::new());
        self.assets.states.insert(name.into(), AssetState::Loaded);
        Ok(Mesh::asset(name))
    }
}
