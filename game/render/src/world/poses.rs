//! Pose uploads: the two transform histories, parented and offset globals
//! patched into their pages, histories reset where a slot's occupant changed,
//! and the loaded models' poses.
use super::upload::page_len;
use super::{offsets, scene, Feed, Pass, Writes};
use crate::RenderError;
use exact_game::{Parent, Transform, World, PAGE};

impl Feed {
    /// Swap the history roles and write this tick's poses.
    pub(super) fn upload_poses(
        &mut self,
        w: &World,
        r: &mut impl Writes,
        pass: &Pass,
    ) -> Result<(), RenderError> {
        let Pass {
            initial,
            old,
            next,
            moved,
            parent_changed,
            offset_change,
        } = *pass;
        r.begin_tick();
        self.current = 1 - self.current;
        // Parented global poses: every one when the hierarchy changed, else
        // only those in blocks whose poses changed since the last pass, and
        // the subtrees under offsets whose content changed. Membership too:
        // a parented entity can gain its Transform later.
        let rebuilt = initial
            || parent_changed
            || offset_change == offsets::Change::Rows
            || next.membership != old.membership
            || self.override_cursor.is_none();
        if rebuilt {
            self.overrides.clear();
            for (e, _) in w.query::<(&Parent, &Transform)>().iter() {
                if let Some(t) = scene::pose(w, e) {
                    self.overrides.push((e, floats(t)));
                }
            }
            // Unparented entities drawn at an offset are patched the same way.
            let parented = self.overrides.len();
            for (e, _) in w
                .query::<(&exact_game::Offset, &Transform)>()
                .without::<Parent>()
                .iter()
            {
                if let Some(t) = scene::pose(w, e) {
                    self.overrides.push((e, floats(t)));
                }
            }
            if self.overrides.len() > parented {
                self.overrides.sort_by_key(|(e, _)| e.index());
            }
            self.override_pages.clear();
            for (i, (e, _)) in self.overrides.iter().enumerate() {
                let page = e.index() as usize / PAGE;
                if self.override_pages.len() <= page {
                    self.override_pages.resize(page + 1, i..i);
                }
                self.override_pages[page].end = i + 1;
            }
            self.offsets.index(w);
            self.offsets.subtrees(w);
        } else {
            changed_blocks(w, self.override_cursor, &mut self.changed_blocks);
            for &block in &self.changed_blocks {
                let range = self
                    .override_pages
                    .get(block as usize / PAGE)
                    .cloned()
                    .unwrap_or_default();
                for (e, pose) in &mut self.overrides[range] {
                    if let Some(t) = scene::pose(w, *e) {
                        *pose = floats(t);
                    }
                }
            }
            // An offset moves only its own drawn subtree, all of it overridden.
            self.offsets.subtrees(w);
            for e in &self.offsets.moved {
                if let Ok(i) = self
                    .overrides
                    .binary_search_by_key(&e.index(), |(o, _)| o.index())
                {
                    if let Some(t) = scene::pose(w, *e) {
                        self.overrides[i].1 = floats(t);
                    }
                }
            }
        }
        self.override_cursor = Some(w.pose_cursor());
        // This history buffer was last written at its cursor: a parented
        // page needs patching only if a pose in it changed since then.
        changed_blocks(
            w,
            self.buffer_cursors[self.current],
            &mut self.changed_blocks,
        );
        if parent_changed {
            // Pages patched before, and those patched now, are rewritten.
            for e in self
                .parents
                .iter()
                .chain(self.overrides.iter().map(|(e, _)| e))
            {
                self.transforms[self.current].invalidate(e.index() as usize / PAGE);
            }
        }
        // Pages an offset moved, now or since this buffer was last written.
        for page in self.offsets.take_pending(self.current) {
            self.transforms[self.current].invalidate(page);
        }
        let pages = w.pages::<Transform>();
        let mut run = 0;
        self.scratch.clear();
        for page in pages.iter() {
            let len = page_len(page.first, r.max_slots()) * 10;
            let index = page.first as usize / PAGE;
            let range = self.override_pages.get(index).cloned().unwrap_or_default();
            let parented = !range.is_empty();
            let posed = parented && self.changed_blocks.binary_search(&page.first).is_ok();
            if !posed && !self.transforms[self.current].needs_check(index, page.generation) {
                continue;
            }
            let mut values = &page.floats()[..len];
            if parented {
                self.page_scratch[..len].copy_from_slice(values);
                for (e, pose) in &self.overrides[range] {
                    let at = (e.index() - page.first) as usize * 10;
                    self.page_scratch[at..at + 10].copy_from_slice(pose);
                }
                values = &self.page_scratch[..len];
            }
            if self.transforms[self.current].dirty(index, page.generation, values, true) {
                if !self.scratch.is_empty() && run + (self.scratch.len() / 10) as u32 != page.first
                {
                    r.transforms(run, &self.scratch, initial)?;
                    self.scratch.clear();
                }
                if self.scratch.is_empty() {
                    run = page.first;
                }
                self.scratch.extend_from_slice(values);
            }
        }
        self.buffer_cursors[self.current] = Some(w.pose_cursor());
        if initial {
            self.buffer_cursors = [self.buffer_cursors[self.current]; 2];
            let (a, b) = self.transforms.split_at_mut(1);
            if self.current == 0 {
                b[0].clone_from(&a[0]);
            } else {
                a[0].clone_from(&b[0]);
            }
        }
        if !self.scratch.is_empty() {
            r.transforms(run, &self.scratch, initial)?;
        }
        // A slot whose occupant changed since the last swap has no history of
        // its own: an entity spawned in an earlier tick of a multi-tick
        // advance is no longer fresh, and its slot's history holds the
        // previous occupant (or nothing). It starts from its current pose.
        if initial || next.live != old.live {
            for e in w.entities() {
                let slot = e.index() as usize;
                if self.occupants.len() <= slot {
                    self.occupants.resize(slot + 1, 0);
                }
                let occupant = e.generation().wrapping_add(1);
                if std::mem::replace(&mut self.occupants[slot], occupant) == occupant
                    || initial
                    || w.is_fresh(e)
                {
                    continue;
                }
                if let Some(t) = scene::pose(w, e) {
                    r.previous(e.index(), &floats(t))?;
                    self.transforms[1 - self.current].invalidate(slot / PAGE);
                }
            }
        }
        if !initial {
            for &e in w.fresh() {
                if let Some(t) = scene::pose(w, e) {
                    r.previous(e.index(), &floats(t))?;
                    self.transforms[1 - self.current].invalidate(e.index() as usize / PAGE);
                }
            }
            for &(e, t) in self
                .overrides
                .iter()
                .take(if parent_changed || !w.fresh().is_empty() {
                    usize::MAX
                } else {
                    0
                })
            {
                if !w.is_fresh(e) && scene::snap(w, e, parent_changed) {
                    r.previous(e.index(), &t)?;
                    self.transforms[1 - self.current].invalidate(e.index() as usize / PAGE);
                }
            }
            if parent_changed {
                for &e in &self.parents {
                    if !w.has::<Parent>(e) {
                        if let Some(t) = scene::pose(w, e) {
                            r.previous(e.index(), &floats(t))?;
                            self.transforms[1 - self.current].invalidate(e.index() as usize / PAGE);
                        }
                    }
                }
            }
        }
        if rebuilt {
            self.parents.clear();
            self.parents.extend(self.overrides.iter().map(|(e, _)| *e));
        }
        self.history_pending = moved && !initial;
        Ok(())
    }
    /// The poses of model instances whose blocks moved since the last pose step.
    pub(super) fn upload_model_poses(
        &mut self,
        w: &World,
        r: &mut impl Writes,
        pass: &Pass,
        batches: bool,
    ) {
        let Pass {
            initial,
            parent_changed,
            ..
        } = *pass;
        // Static instances cost nothing, parented or not: only blocks whose
        // local or propagated poses changed since the last pose step.
        changed_blocks(w, self.model_cursor, &mut self.changed_blocks);
        self.model_cursor = Some(w.pose_cursor());
        self.changed_pages.clear();
        self.changed_pages
            .extend(self.changed_blocks.iter().map(|&b| b as usize / PAGE));
        // An offset moves drawn model poses without a simulated pose write:
        // its subtree's pages, not every instance.
        let offset_pages = self.offsets.moved.iter().map(|e| e.index() as usize / PAGE);
        if offset_pages.len() > 0 {
            self.changed_pages.extend(offset_pages);
            self.changed_pages.sort_unstable();
            self.changed_pages.dedup();
        }
        let moved = if initial || parent_changed || batches {
            crate::models::Moved::All
        } else {
            crate::models::Moved::Pages {
                pages: &self.changed_pages,
            }
        };
        r.model_poses(w, &self.assets.entities, initial || parent_changed, moved);
    }
}
/// First entity index of each block whose poses changed since `cursor`, in
/// order; every block without a cursor.
fn changed_blocks(w: &World, cursor: Option<exact_game::PoseCursor>, out: &mut Vec<u32>) {
    out.clear();
    match cursor {
        Some(cursor) => out.extend(w.poses_changed_since(cursor)),
        None => out.extend(w.pages::<Transform>().iter().map(|p| p.first)),
    }
}
pub(crate) fn floats(t: Transform) -> [f32; 10] {
    [
        t.position.x,
        t.position.y,
        t.position.z,
        t.rotation.x,
        t.rotation.y,
        t.rotation.z,
        t.rotation.w,
        t.scale.x,
        t.scale.y,
        t.scale.z,
    ]
}
