use super::*;
use crate::{DrawInstance, MaterialId, RENDER_SLOT_BASE};
#[derive(Default)]
pub(super) struct Assets {
    pub records: Vec<DrawInstance>,
    pub entities: Vec<exact_game::Entity>,
    groups: BTreeMap<(MeshId, MaterialId, bool), Vec<u32>>,
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
            if w.global(entity)
                .is_some_and(|p| p.matrix3.determinant() < 0.)
            {
                return Err(RenderError::Scene(format!(
                    "asset `{name}`: negative-determinant entity transform is unsupported"
                )));
            }
            let Some(nodes) = r.model(name) else { continue };
            if w.get::<Visible>(entity).is_some_and(|v| !v.0) {
                continue;
            }
            for &(geometry, material, local, skin) in nodes {
                let slot = RENDER_SLOT_BASE + self.records.len() as u32;
                self.entities.push(entity);
                self.records.push(DrawInstance {
                    transform: entity.index(),
                    geometry,
                    material,
                    local,
                    skin,
                });
                self.groups
                    .entry((geometry, material, local.determinant() < 0.))
                    .or_default()
                    .push(slot);
            }
        }
        r.instances(&self.records)?;
        for (&(mesh, _, _), list) in &self.groups {
            if list.is_empty() {
                continue;
            }
            let start = slots.len() as u32;
            slots.extend(list);
            batches.push(Batch::new(mesh, start..slots.len() as u32));
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
        format!("{{\"textureUploads\":{},\"meshUploads\":{},\"pipelineCreations\":{},\"bufferReallocations\":{}}}",
            self.texture_uploads, self.mesh_uploads, self.pipeline_creations, self.buffer_reallocations)
    }
}
