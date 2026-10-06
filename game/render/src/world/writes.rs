//! The renderer writes a feed makes, behind one trait: the GPU renderer and the
//! recording test backend run the same feed algorithm.
use super::scene;
use crate::{Batch, MeshId, RenderError, Vertex};
use exact_game::World;

pub(crate) trait Writes {
    fn attachments(
        &mut self,
        _: &mut scene::Attachments,
        _: &World,
        _: bool,
        _: bool,
        _: bool,
        _: bool,
    ) {
    }
    /// A loaded model's nodes, their names, and its merged draw list (empty when
    /// nothing merges).
    fn model(&self, _: &str) -> Option<crate::models::Draws<'_>> {
        None
    }
    fn assets_revision(&self) -> u64 {
        0
    }
    fn instances(&mut self, _: &[crate::DrawInstance]) -> Result<(), RenderError> {
        Ok(())
    }
    fn opacity(&mut self, _: &[(u32, f32)]) {}
    /// Per record, 1 + its first merged part look (0: none), and those looks;
    /// set before `instances`.
    fn part_looks(&mut self, _: &[u32], _: &[[f32; 8]]) {}
    /// Each record's level of detail and the `ModelLod` entities; set before
    /// `instances`.
    fn levels(&mut self, _: &[u8], _: &[crate::lod::Lod]) {}
    /// Rewrite records `first..` looks in place (tint and glow), and part looks
    /// `part_first..` (starts relative to their meshes).
    fn patch_looks(&mut self, _: usize, _: &[crate::DrawInstance], _: usize, _: &[[f32; 8]]) {}
    fn model_poses(
        &mut self,
        _: &World,
        _: &[exact_game::Entity],
        _: bool,
        _: crate::models::Moved<'_>,
    ) {
    }
    fn quads(&mut self, _: &World, _: bool, _: bool, _: bool) -> Result<(), RenderError> {
        Ok(())
    }
    fn max_slots(&self) -> u32;
    fn begin_tick(&mut self);
    fn transforms(&mut self, first: u32, floats: &[f32], both: bool) -> Result<(), RenderError>;
    fn previous(&mut self, first: u32, floats: &[f32]) -> Result<(), RenderError>;
    fn materials(&mut self, first: u32, floats: &[f32]) -> Result<(), RenderError>;
    fn mesh(&mut self, vertices: &[Vertex], indices: &[u32]) -> MeshId;
    fn batches(&mut self, batches: &[Batch], slots: &[u32]) -> Result<(), RenderError>;
}
impl<const ASSETS: bool> Writes for crate::renderer::RendererWithAssets<ASSETS> {
    fn attachments(
        &mut self,
        a: &mut scene::Attachments,
        w: &World,
        initial: bool,
        tick: bool,
        parent: bool,
        models: bool,
    ) {
        if ASSETS {
            if models {
                a.model_digests.clear();
                a.model_digests.extend(
                    self.models
                        .loaded
                        .iter()
                        .map(|(name, model)| (name.clone(), model.digest)),
                );
            }
            a.feed(w, initial, tick, parent, models);
        }
    }
    fn model(&self, name: &str) -> Option<crate::models::Draws<'_>> {
        if ASSETS {
            self.models
                .loaded
                .get(name)
                .filter(|m| m.active)
                .map(|m| crate::models::Draws {
                    nodes: &m.nodes,
                    names: &m.names,
                    merged: &m.merged,
                    members: &m.members,
                    starts: &m.starts,
                    materials: &m.materials,
                    custom: &self.models.custom,
                })
        } else {
            None
        }
    }
    fn assets_revision(&self) -> u64 {
        self.models.revision
    }
    fn opacity(&mut self, values: &[(u32, f32)]) {
        if self.lights.set_opacity(&self.device, &self.queue, values) {
            self.rebind();
        }
    }
    fn part_looks(&mut self, bases: &[u32], looks: &[[f32; 8]]) {
        let part_looks = &mut self.models.pending_looks;
        part_looks.0.clear();
        part_looks.0.extend_from_slice(bases);
        part_looks.1.clear();
        part_looks.1.extend_from_slice(looks);
    }
    fn patch_looks(
        &mut self,
        first: usize,
        records: &[crate::DrawInstance],
        part_first: usize,
        looks: &[[f32; 8]],
    ) {
        if ASSETS {
            self.patch_draw_looks(first, records, part_first, looks);
        }
    }
    fn levels(&mut self, records: &[u8], lods: &[crate::lod::Lod]) {
        self.levels.set(records, lods);
    }
    fn instances(&mut self, records: &[crate::DrawInstance]) -> Result<(), RenderError> {
        if ASSETS {
            self.set_draw_instances(records)
        } else {
            Ok(())
        }
    }
    fn model_poses(
        &mut self,
        w: &World,
        entities: &[exact_game::Entity],
        initial: bool,
        moved: crate::models::Moved<'_>,
    ) {
        if ASSETS {
            self.model_poses(w, entities, initial, moved);
        }
    }
    fn quads(
        &mut self,
        w: &World,
        initial: bool,
        next_tick: bool,
        parent_changed: bool,
    ) -> Result<(), RenderError> {
        if ASSETS {
            for (_, sprite) in w.query::<&exact_game::Sprite>().iter() {
                self.sprite_texture(&sprite.texture);
            }
            for (e, look) in w.query::<&exact_game::ParticleLook>().iter() {
                if !look.texture.is_empty() && w.is_visible(e) {
                    self.sprite_texture(&look.texture);
                }
            }
        }
        self.quads
            .feed::<ASSETS>(w, initial, next_tick, parent_changed)?;
        self.quads.prepare(&self.device, &self.queue);
        Ok(())
    }
    fn max_slots(&self) -> u32 {
        self.max_slots()
    }
    fn begin_tick(&mut self) {
        self.begin_tick();
    }
    fn transforms(&mut self, first: u32, floats: &[f32], both: bool) -> Result<(), RenderError> {
        if both {
            self.write_transforms_both(first, floats)
        } else {
            self.write_transforms(first, floats)
        }
    }
    fn previous(&mut self, first: u32, floats: &[f32]) -> Result<(), RenderError> {
        self.write_previous_transforms(first, floats)
    }
    fn materials(&mut self, first: u32, floats: &[f32]) -> Result<(), RenderError> {
        self.write_materials(first, floats)
    }
    fn mesh(&mut self, v: &[Vertex], i: &[u32]) -> MeshId {
        self.add_mesh(v, i)
    }
    fn batches(&mut self, batches: &[Batch], slots: &[u32]) -> Result<(), RenderError> {
        self.set_batches(batches, slots)
    }
}
