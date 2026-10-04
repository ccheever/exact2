use super::*;
use crate::{DrawInstance, MaterialId, RENDER_SLOT_BASE};
// Geometry, material, mirrored, viewmodel, and the level of detail.
type GroupKey = (MeshId, MaterialId, bool, bool, u8);
#[derive(Default)]
pub(super) struct Assets {
    pub records: Vec<DrawInstance>,
    pub entities: Vec<exact_game::Entity>,
    groups: BTreeMap<GroupKey, Vec<u32>>,
    // Per record, 1 + its first merged part look (0: none), and those looks.
    part_bases: Vec<u32>,
    part_looks: Vec<[f32; 8]>,
    // Per record, its level of detail; and the `ModelLod` entities.
    levels: Vec<u8>,
    lods: Vec<crate::lod::Lod>,
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
        self.part_bases.clear();
        self.part_looks.clear();
        self.levels.clear();
        self.lods.clear();
        for group in self.groups.values_mut() {
            group.clear();
        }
        for (entity, (mesh, _)) in w.query::<(&Mesh, &Transform)>().iter() {
            let Mesh::Asset(name) = mesh else { continue };
            if w.get::<Visible>(entity).is_some_and(|v| !v.0) {
                continue;
            }
            let looks = w.get::<exact_game::NodeMaterials>(entity);
            let overrides = w.get::<exact_game::MaterialOverrides>(entity);
            let viewmodel = w.has::<exact_game::ViewModel>(entity);
            // The entity's own model, then each coarser level; the renderer
            // draws one of them per frame.
            let lod = w.get::<exact_game::ModelLod>(entity);
            if let Some(lod) = &lod {
                crate::lod::validate(lod)
                    .map_err(|e| RenderError::scene(format!("entity {}: {e}", entity.index())))?;
            }
            let count = 1 + lod.as_ref().map_or(0, |l| l.levels.len());
            let first = self.records.len();
            let mut present = 0u8;
            let mut drawn = false;
            for level in 0..count {
                let model = match (&lod, level) {
                    (Some(lod), 1..) => lod.levels[level - 1].model.as_str(),
                    _ => name.as_str(),
                };
                let Some(draws) = r.model(model) else {
                    continue;
                };
                let names = draws.names;
                // A custom vertex shader sees node-local positions: never merged.
                let custom = draws.merged.iter().any(|n| draws.custom.contains(&n.1));
                let (nodes, members) = if draws.merged.is_empty() || custom {
                    (draws.nodes, None)
                } else {
                    (draws.merged, Some((draws.members, draws.starts)))
                };
                // A material override is one record's tint and glow: every part
                // of a record shares its material.
                let material_look = |material: MaterialId| {
                    let index = draws.materials.iter().position(|&m| m == material)?;
                    let o = overrides
                        .as_ref()?
                        .0
                        .iter()
                        .find(|o| o.material as usize == index)?;
                    let base = draws.bases[index];
                    let tint = o.color.map_or([1.; 4], |c| {
                        std::array::from_fn(|i| if base[i] == 0. { 1. } else { c[i] / base[i] })
                    });
                    Some((tint, o.emissive))
                };
                drawn |= !nodes.is_empty();
                if !nodes.is_empty() {
                    present |= 1 << level;
                }
                for (i, &(geometry, material, local, skin)) in nodes.iter().enumerate() {
                    let slot = RENDER_SLOT_BASE + self.records.len() as u32;
                    let parts = members.map_or(std::slice::from_ref(&names[i]), |m| &m.0[i]);
                    let find = |node: &String| {
                        looks
                            .as_ref()
                            .and_then(|l| l.0.iter().find(|m| m.node == *node))
                    };
                    let mut look = None;
                    let mut base = 0;
                    if parts.len() == 1 {
                        look = find(&parts[0]);
                    } else if parts.iter().any(|p| find(p).is_some()) {
                        // A merged draw finds each vertex's part by its vertex range.
                        let starts = &members.unwrap().1[i];
                        base = self.part_looks.len() as u32 + 1;
                        self.part_looks
                            .extend(parts.iter().zip(starts).map(|(p, &start)| {
                                let (c, e) =
                                    find(p).map_or(([1.; 4], [0.; 3]), |l| (l.color, l.emissive));
                                [
                                    c[0],
                                    c[1],
                                    c[2],
                                    c[3],
                                    e[0],
                                    e[1],
                                    e[2],
                                    f32::from_bits(start),
                                ]
                            }));
                        let mut end = [0.; 8];
                        end[7] = f32::from_bits(u32::MAX);
                        self.part_looks.push(end);
                    }
                    self.part_bases.push(base);
                    self.levels.push(level as u8);
                    let (mut tint, mut glow) =
                        look.map_or(([1.; 4], [0.; 3]), |l| (l.color, l.emissive));
                    if let Some((t, g)) = material_look(material) {
                        tint = std::array::from_fn(|i| tint[i] * t[i]);
                        glow = std::array::from_fn(|i| glow[i] + g[i]);
                    }
                    self.records.push(DrawInstance {
                        data: 0,
                        transform: entity.index(),
                        geometry,
                        material,
                        local,
                        skin,
                        tint,
                        glow,
                    });
                    self.groups
                        .entry((
                            geometry,
                            material,
                            local.determinant() < 0.,
                            viewmodel,
                            level as u8,
                        ))
                        .or_default()
                        .push(slot);
                }
            }
            if let (Some(lod), true) = (&lod, drawn) {
                let mut starts = [0.; crate::lod::MAX_LEVELS];
                for (start, l) in starts[1..].iter_mut().zip(&lod.levels) {
                    *start = l.distance;
                }
                self.lods.push(crate::lod::Lod {
                    slot: entity.index(),
                    record: first as u32,
                    count: (self.records.len() - first) as u32,
                    starts,
                    levels: count as u8,
                    hide: lod.hide.unwrap_or(f32::INFINITY),
                    present,
                });
            }
            if drawn {
                self.entities.push(entity);
            }
        }
        r.part_looks(&self.part_bases, &self.part_looks);
        r.levels(&self.levels, &self.lods);
        r.instances(&self.records)?;
        for (&(mesh, _, _, viewmodel, level), list) in &self.groups {
            if list.is_empty() {
                continue;
            }
            let start = slots.len() as u32;
            slots.extend(list);
            let mut batch = Batch::new(mesh, start..slots.len() as u32);
            batch.level = level;
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
