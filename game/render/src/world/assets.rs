use super::*;
use crate::{DrawInstance, MaterialId, RENDER_SLOT_BASE};
use glam::Mat4;
#[derive(Default)]
pub(super) struct Assets {
    loaded: BTreeMap<String, Vec<(MeshId, MaterialId, Mat4)>>,
    pub records: Vec<DrawInstance>,
    pub entities: Vec<exact_game::Entity>,
    groups: BTreeMap<(MeshId, MaterialId), Vec<u32>>,
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
            let Some(model) = w.model(name) else { continue };
            if w.get::<Visible>(entity).is_some_and(|v| !v.0) {
                continue;
            }
            if !self.loaded.contains_key(name) {
                let (meshes, materials) = r.model(model)?;
                let offsets = model.offsets().map_err(RenderError::scene)?;
                let nodes = model
                    .nodes
                    .iter()
                    .zip(offsets)
                    .filter_map(|(n, local)| {
                        n.mesh.map(|m| {
                            (
                                meshes[m as usize],
                                materials[model.meshes[m as usize].material as usize],
                                local,
                            )
                        })
                    })
                    .collect();
                self.loaded.insert(name.clone(), nodes);
            }
            for &(geometry, material, local) in &self.loaded[name] {
                let slot = RENDER_SLOT_BASE + self.records.len() as u32;
                self.entities.push(entity);
                self.records.push(DrawInstance {
                    transform: entity.index(),
                    geometry,
                    material,
                    local,
                });
                self.groups
                    .entry((geometry, material))
                    .or_default()
                    .push(slot);
            }
        }
        r.instances(&self.records)?;
        for (&(mesh, _), list) in &self.groups {
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
