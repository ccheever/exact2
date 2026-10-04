use super::*;
use crate::{DrawInstance, MaterialId, RENDER_SLOT_BASE};
// Geometry, material, mirrored, viewmodel, and the level's distance band bits.
type GroupKey = (MeshId, MaterialId, bool, bool, [u32; 2]);
#[derive(Default)]
pub(super) struct Assets {
    pub records: Vec<DrawInstance>,
    pub entities: Vec<exact_game::Entity>,
    groups: BTreeMap<GroupKey, Vec<u32>>,
}
impl Assets {
    pub fn batches(
        &mut self,
        w: &World,
        r: &mut impl Writes,
        batches: &mut Vec<Batch>,
        slots: &mut Vec<u32>,
    ) -> Result<(), RenderError> {
        self.records.clear();
        self.entities.clear();
        for group in self.groups.values_mut() {
            group.clear();
        }
        for (entity, (mesh, _)) in w.query::<(&Mesh, &Transform)>().iter() {
            let Mesh::Asset(name) = mesh else { continue };
            if w.get::<Visible>(entity).is_some_and(|v| !v.0) {
                continue;
            }
            let looks = w.get::<exact_game::NodeMaterials>(entity);
            let viewmodel = w.has::<exact_game::ViewModel>(entity);
            // The entity's own model, then each coarser level, each in its band.
            let lod = w.get::<exact_game::ModelLod>(entity);
            let mut levels = vec![(name.as_str(), 0.)];
            if let Some(lod) = &lod {
                levels.extend(lod.levels.iter().map(|l| (l.model.as_str(), l.distance)));
            }
            let end = lod.as_ref().and_then(|l| l.hide).unwrap_or(f32::INFINITY);
            let mut drawn = false;
            for (level, &(model, near)) in levels.iter().enumerate() {
                let far = levels.get(level + 1).map_or(end, |l| l.1).min(end);
                if near >= far {
                    continue;
                }
                let Some((nodes, names, merged)) = r.model(model) else {
                    continue;
                };
                // Per-node looks need the unmerged parts.
                let (nodes, names) = match &looks {
                    Some(l) if !l.0.is_empty() => (nodes, names),
                    _ if !merged.is_empty() => (merged, &[][..]),
                    _ => (nodes, names),
                };
                drawn |= !nodes.is_empty();
                let band = [near.max(0.).to_bits(), far.to_bits()];
                for (i, &(geometry, material, local, skin)) in nodes.iter().enumerate() {
                    let slot = RENDER_SLOT_BASE + self.records.len() as u32;
                    let look = looks
                        .as_ref()
                        .zip(names.get(i))
                        .and_then(|(l, node)| l.0.iter().find(|m| m.node == *node));
                    self.records.push(DrawInstance {
                        data: 0,
                        transform: entity.index(),
                        geometry,
                        material,
                        local,
                        skin,
                        tint: look.map_or([1.; 4], |l| l.color),
                        glow: look.map_or([0.; 3], |l| l.emissive),
                    });
                    self.groups
                        .entry((
                            geometry,
                            material,
                            local.determinant() < 0.,
                            viewmodel,
                            band,
                        ))
                        .or_default()
                        .push(slot);
                }
            }
            if drawn {
                self.entities.push(entity);
            }
        }
        r.instances(&self.records)?;
        for (&(mesh, _, _, viewmodel, band), list) in &self.groups {
            if list.is_empty() {
                continue;
            }
            let start = slots.len() as u32;
            slots.extend(list);
            let mut batch = Batch::new(mesh, start..slots.len() as u32);
            batch.distance = band.map(f32::from_bits);
            batches.push(if viewmodel { batch.viewmodel() } else { batch });
        }
        Ok(())
    }
}

/// Device work is diagnostic only: never part of a save or simulation hash.
#[derive(Clone, Copy, Default)]
pub(crate) struct Work {
    pub texture_uploads: u64,
    pub mesh_uploads: u64,
    pub pipeline_creations: u64,
    pub buffer_reallocations: u64,
}
impl Work {
    pub fn plus(self, old: Self) -> Self {
        Self {
            texture_uploads: self.texture_uploads + old.texture_uploads,
            mesh_uploads: self.mesh_uploads + old.mesh_uploads,
            pipeline_creations: self.pipeline_creations + old.pipeline_creations,
            buffer_reallocations: self.buffer_reallocations + old.buffer_reallocations,
        }
    }
    pub fn since(self, old: Self) -> Self {
        Self {
            texture_uploads: self.texture_uploads.saturating_sub(old.texture_uploads),
            mesh_uploads: self.mesh_uploads.saturating_sub(old.mesh_uploads),
            pipeline_creations: self
                .pipeline_creations
                .saturating_sub(old.pipeline_creations),
            buffer_reallocations: self
                .buffer_reallocations
                .saturating_sub(old.buffer_reallocations),
        }
    }
    pub fn json(self) -> String {
        format!("{{\"textureUploads\":{},\"meshUploads\":{},\"pipelineCreations\":{},\"modelSkinBufferReallocations\":{}}}",
            self.texture_uploads, self.mesh_uploads, self.pipeline_creations, self.buffer_reallocations)
    }
}
