//! Material uploads: authored materials, presentation swaps (`DrawnMesh::material`),
//! tints, primitive dimensions and grid spacing, packed twelve floats a slot; and
//! the frame-time glow inputs over them.
use super::upload::page_len;
use super::{shown, Feed, Pass, Writes};
use crate::RenderError;
use exact_game::{DrawnMesh, Material, Mesh, Transform, World, PAGE};

/// A primitive's material floats under its `Tint`: base colour (not a grid's
/// spacing) multiplied, emission added.
fn tinted(out: &mut [f32], t: &exact_game::Tint) {
    for c in 0..3 {
        out[c] *= t.color[c];
        out[6 + c] += t.emissive[c];
    }
    if out[3] >= 0. {
        out[3] *= t.color[3];
    }
}
impl Feed {
    pub(super) fn upload_materials(
        &mut self,
        w: &World,
        r: &mut impl Writes,
        pass: &Pass,
    ) -> Result<(), RenderError> {
        let Pass {
            initial, old, next, ..
        } = *pass;
        if initial || next.tint != old.tint {
            self.tints = w
                .query::<&exact_game::Tint>()
                .iter()
                .filter(|(e, _)| w.is_visible(*e))
                .map(|(e, t)| (e.index(), *t))
                .collect();
        }
        if initial || next.drawn != old.drawn {
            self.swapped = w
                .query::<&DrawnMesh>()
                .iter()
                .filter_map(|(e, d)| d.material.map(|m| (e.index(), m)))
                .collect();
        }
        // Frame-time Glow writes bypass page fingerprints. Restore authored
        // values when a tween disappears, including model emission. Retargeting
        // an existing tween needs only its next frame-time write.
        if next.glow != old.glow {
            let live: std::collections::BTreeSet<_> = w
                .query::<&exact_game::Glow>()
                .iter()
                .map(|(entity, _)| entity.index())
                .collect();
            for glow in &self.glows {
                if !live.contains(&glow.slot) {
                    self.materials.invalidate(glow.slot as usize / PAGE);
                }
            }
        }
        let transforms = w.pages::<Transform>();
        let materials = w.pages::<Material>();
        let mut tp = transforms.iter().peekable();
        let mut mp = materials.iter().peekable();
        let mut run = 0;
        self.scratch.clear();
        while tp.peek().is_some() || mp.peek().is_some() {
            let first = tp
                .peek()
                .map_or(u32::MAX, |p| p.first)
                .min(mp.peek().map_or(u32::MAX, |p| p.first));
            if tp.peek().is_some_and(|p| p.first == first) {
                tp.next();
            }
            let page = if mp.peek().is_some_and(|p| p.first == first) {
                mp.next()
            } else {
                None
            };
            let index = first as usize / PAGE;
            let generation = page.as_ref().map_or(0, |p| p.generation);
            if !initial
                && next.mesh == old.mesh
                && next.glow == old.glow
                && next.tint == old.tint
                && next.drawn == old.drawn
                && next.membership == old.membership
                && !self.materials.needs_check(index, generation)
            {
                continue;
            }
            let len = page_len(first, r.max_slots());
            let default = material_floats(Material::default());
            for (i, out) in self.page_scratch[..len * 12]
                .chunks_exact_mut(12)
                .enumerate()
            {
                if let Some(m) = self.swapped.get(&(first + i as u32)) {
                    out.copy_from_slice(&material_floats(*m));
                } else if let Some(p) = page
                    .as_ref()
                    .filter(|p| p.mask()[i / 64] & (1 << (i % 64)) != 0)
                {
                    let record = &p.floats()[i * 10..i * 10 + 10];
                    out[..9].copy_from_slice(&record[..9]);
                    out[3] = grid_alpha(record[3], record[9]);
                } else {
                    out.copy_from_slice(&default);
                }
                out[9..12]
                    .copy_from_slice(self.dimensions.get(first as usize + i).unwrap_or(&[1.0; 3]));
                if let Some(t) = self.tints.get(&(first + i as u32)) {
                    tinted(out, t);
                }
            }
            let values = &self.page_scratch[..len * 12];
            if self.materials.dirty(index, generation, values, true) {
                if !self.scratch.is_empty() && run + (self.scratch.len() / 12) as u32 != first {
                    r.materials(run, &self.scratch)?;
                    self.scratch.clear();
                }
                if self.scratch.is_empty() {
                    run = first;
                }
                self.scratch.extend_from_slice(values);
            }
        }
        if !self.scratch.is_empty() {
            r.materials(run, &self.scratch)?;
        }
        Ok(())
    }
    /// Each glowing entity's material at rest, for frame-time tweens.
    pub(super) fn glow_inputs(&mut self, w: &World) {
        self.glows.clear();
        for (entity, glow) in w.query::<&exact_game::Glow>().iter() {
            let material = self.swapped.get(&entity.index()).copied();
            let mut values = material_floats(
                material
                    .or_else(|| w.get::<Material>(entity).map(|m| *m))
                    .unwrap_or_default(),
            );
            if let Some(t) = w.get::<exact_game::Tint>(entity) {
                tinted(&mut values, &t);
            }
            values[9..12].copy_from_slice(
                self.dimensions
                    .get(entity.index() as usize)
                    .unwrap_or(&[1.; 3]),
            );
            self.glows.push(crate::GlowInput {
                slot: entity.index(),
                material: values,
                tween: glow.0.clone(),
                hz: w.hz(),
                model: shown(w, entity, |m| matches!(m, Mesh::Asset(_))).unwrap_or(false),
            });
        }
    }
}
fn grid_alpha(alpha: f32, spacing: f32) -> f32 {
    if spacing > 0.0 {
        -spacing
    } else {
        alpha.max(0.0)
    }
}
pub(super) fn material_floats(m: Material) -> [f32; 12] {
    [
        m.color[0],
        m.color[1],
        m.color[2],
        grid_alpha(m.color[3], m.grid_spacing),
        m.metallic,
        m.roughness,
        m.emissive[0],
        m.emissive[1],
        m.emissive[2],
        1.0,
        1.0,
        1.0,
    ]
}
