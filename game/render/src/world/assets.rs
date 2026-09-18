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
