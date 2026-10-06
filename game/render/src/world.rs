//! Tick uploads and retained scene selection. Frames never walk entity storage.
//! A feed's concerns live in sub-modules: poses and offsets (`poses`,
//! `offsets`), primitive batches (`primitives`), materials and glows
//! (`materials`), presentation swaps (`shown`), fades (`opacity`), loaded
//! models (`assets`), camera, lights and attachments (`scene`).
use crate::{Batch, RenderError};
use exact_game::{Material, Transform, World, PAGE};
use std::collections::BTreeMap;

pub(crate) mod assets;
mod materials;
mod offsets;
mod opacity;
mod poses;
mod primitives;
pub(crate) mod scene;
mod shown;
mod upload;
mod versions;
mod writes;
#[cfg(test)]
pub(crate) use poses::floats;
use primitives::{Group, Shape};
pub(crate) use scene::snap as trace_snap;
use scene::Scene;
pub(crate) use shown::{each_shown, shown};
use upload::check_page;
use versions::{Pass, Versions};
pub(crate) use writes::Writes;

/// Persistent bridge from one world to one renderer. World loads invalidate it
/// automatically; reset it when replacing a World with an unrelated instance.
/// Replacement invalidates world-derived records, never renderer asset residency.
/// Page hashes select coalesced uploads; parented poses are patched before hashing.
/// Decomposed globals are exact TRS for uniform ancestor scale; shear is approximated.
/// Storage, batches and meshes grow only when scene structure changes.
pub struct Feed {
    assets: assets::Assets,
    versions: Option<Versions>,
    tick: u64,
    history_pending: bool,
    groups: Vec<Group>,
    shapes: BTreeMap<(Shape, bool), usize>,
    batches: Vec<Batch>,
    slots: Vec<u32>,
    page_scratch: Box<[f32; PAGE * 12]>,
    dimensions: Vec<[f32; 3]>,
    scratch: Vec<f32>,
    transforms: [upload::Pages; 2],
    materials: upload::Pages,
    current: usize,
    generation: u64,
    parents: Vec<exact_game::Entity>,
    overrides: Vec<(exact_game::Entity, [f32; 10])>,
    // Each Transform page's range in `overrides`, rebuilt with them.
    override_pages: Vec<std::ops::Range<usize>>,
    // Each slot's occupant generation + 1 at the last history swap (0: none).
    occupants: Vec<u32>,
    // Pose cursors: the parented overrides', each transform history buffer's,
    // and the model poses' last step. None reports every block.
    override_cursor: Option<exact_game::PoseCursor>,
    buffer_cursors: [Option<exact_game::PoseCursor>; 2],
    model_cursor: Option<exact_game::PoseCursor>,
    // Scratch: entity-index blocks whose poses changed since a cursor.
    changed_blocks: Vec<u32>,
    changed_pages: Vec<usize>,
    fades: Vec<(u32, f32)>,
    fades_next: Vec<(u32, f32)>,
    // Each tinted slot's Tint, rebuilt when their content changes.
    tints: BTreeMap<u32, exact_game::Tint>,
    // Presentation offsets that changed, and the drawn subtrees they moved.
    offsets: offsets::Offsets,
    // Each slot's presentation material (`DrawnMesh::material`), likewise.
    swapped: BTreeMap<u32, Material>,
    scene: Scene,
    glows: Vec<crate::GlowInput>,
}
impl Default for Feed {
    fn default() -> Self {
        Self {
            assets: Default::default(),
            versions: None,
            tick: 0,
            history_pending: false,
            groups: Vec::new(),
            shapes: BTreeMap::new(),
            batches: Vec::new(),
            slots: Vec::new(),
            page_scratch: Box::new([0.0; PAGE * 12]),
            dimensions: Vec::new(),
            scratch: Vec::new(),
            transforms: Default::default(),
            materials: Default::default(),
            current: 0,
            generation: 0,
            parents: Vec::new(),
            overrides: Vec::new(),
            override_pages: Vec::new(),
            occupants: Vec::new(),
            override_cursor: None,
            buffer_cursors: [None; 2],
            model_cursor: None,
            changed_blocks: Vec::new(),
            changed_pages: Vec::new(),
            fades: Vec::new(),
            fades_next: Vec::new(),
            tints: BTreeMap::new(),
            offsets: Default::default(),
            swapped: BTreeMap::new(),
            scene: Scene::default(),
            glows: Vec::new(),
        }
    }
}
impl Feed {
    #[cfg(test)]
    pub(crate) fn attachment_diagnostics(&self) -> &scene::AttachmentDiagnostics {
        &self.scene.attachments.diagnostics
    }
    pub(crate) fn share_attachment_diagnostics(
        &mut self,
        diagnostics: scene::AttachmentDiagnostics,
    ) {
        self.scene.attachments.diagnostics = diagnostics;
    }
    /// Forget world history after replacement, retaining registered geometry and scratch.
    pub fn reset(&mut self) {
        self.versions = None;
        self.tick = 0;
        self.history_pending = false;
        self.scene.reset();
        self.glows.clear();
        for buffer in &mut self.transforms {
            buffer.reset();
        }
        self.materials.reset();
        self.parents.clear();
        self.override_pages.clear();
        self.occupants.clear();
        self.override_cursor = None;
        self.buffer_cursors = [None; 2];
        self.model_cursor = None;
        self.offsets.reset();
        self.assets.records.clear();
        self.assets.entities.clear();
    }

    /// Feed one completed tick. With Sim::advance_with, call only when ticks_left < 2.
    /// Initial feeding initializes both histories, including a world's setup tick.
    pub fn feed<const ASSETS: bool>(
        &mut self,
        world: &World,
        renderer: &mut crate::renderer::RendererWithAssets<ASSETS>,
    ) -> Result<(), RenderError> {
        self.feed_to(world, renderer)
    }
    pub(crate) fn trace_camera(&self, alpha: f32) -> [f64; 3] {
        self.scene.trace_camera(alpha)
    }
    /// Frame inputs: interpolated camera and up to 16 positive tick-end lights,
    /// ordered by tick-end camera distance then entity index. Lit samples frame time.
    /// No world queries; retained attachment chains are composed at this alpha.
    pub fn frame(&mut self, world: &World, alpha: f32, aspect: f32) -> crate::FrameInput<'_> {
        {
            let mut frame = self
                .scene
                .frame(world, alpha, glam::Vec2::new(aspect, 1.), false);
            frame.glows = &self.glows;
            frame
        }
    }
    /// The pointer motion the next frame's camera `MouseLook` turns by: the
    /// simulation's `Sim::unshown_motion` at that frame.
    pub fn unshown_motion(&mut self, motion: glam::Vec2) {
        self.scene.unshown = motion;
    }
    /// Whether the fed camera has a `MouseLook` (else no frame reads the motion).
    pub fn mouse_look(&self) -> bool {
        self.scene.has_look()
    }
    /// Frame projection at the CSS-pixel viewport size, including integer scaling.
    pub fn frame_pixels(
        &mut self,
        world: &World,
        alpha: f32,
        size: (f32, f32),
    ) -> crate::FrameInput<'_> {
        {
            let mut frame = self
                .scene
                .frame(world, alpha, glam::Vec2::new(size.0, size.1), true);
            frame.glows = &self.glows;
            frame
        }
    }
    pub(crate) fn feed_to(&mut self, w: &World, r: &mut impl Writes) -> Result<(), RenderError> {
        if self.generation != w.presentation_generation() {
            self.reset();
            self.generation = w.presentation_generation();
        }
        let next = Versions::of(w, r.assets_revision());
        let initial = self.versions.is_none();
        let old = self.versions.unwrap_or_default();
        // The Offset rows a present changed, and the subtrees they move.
        let offset_change = self.offsets.diff(w);
        let moved = initial
            || next.transform != old.transform
            || next.parent != old.parent
            || next.pose != old.pose
            || offset_change != offsets::Change::None;
        let material = initial
            || next.material != old.material
            || next.tint != old.tint
            || next.glow != old.glow
            || next.membership != old.membership
            || next.mesh != old.mesh
            || next.drawn != old.drawn;
        let looks_changed = next.node_materials != old.node_materials
            || next.material_overrides != old.material_overrides;
        let mut batches = initial
            || next.assets != old.assets
            || next.mesh != old.mesh
            || next.parent != old.parent
            || next.visible != old.visible
            || next.viewmodel != old.viewmodel
            || next.lod != old.lod
            || next.drawn != old.drawn
            || next.live != old.live
            || next.membership != old.membership;
        // Changed looks (a shimmer, a flash) patch their records in place.
        if !batches && looks_changed && !self.assets.patch_looks(w, r)? {
            batches = true;
        }
        // Validate live slots before any history swap. A last partial page is clipped
        // only at the device boundary; absent trailing slots do not refuse a valid world.
        if moved || material || batches {
            for page in w.pages::<Transform>().iter() {
                check_page(page.first, page.mask(), r.max_slots(), "transforms")?;
            }
            for page in w.pages::<Material>().iter() {
                check_page(page.first, page.mask(), r.max_slots(), "materials")?;
            }
        }
        let parent_changed = next.parent != old.parent;
        let pass = Pass {
            initial,
            old,
            next,
            moved,
            parent_changed,
            offset_change,
        };
        if moved || (self.history_pending && w.tick() != self.tick) {
            self.upload_poses(w, r, &pass)?;
        }
        if batches {
            self.primitive_batches(w, r)?;
        }
        if material {
            self.upload_materials(w, r, &pass)?;
        }
        if batches {
            if next.assets != 0 || !self.assets.records.is_empty() {
                self.assets
                    .batches(w, r, &mut self.batches, &mut self.slots)?;
            }
            r.batches(&self.batches, &self.slots)?;
        }
        if !self.assets.records.is_empty() && (moved || batches || self.tick != w.tick()) {
            self.upload_model_poses(w, r, &pass, batches);
        }
        if initial || next.opacity != old.opacity || next.live != old.live || parent_changed {
            self.fades(w, r, initial);
        }
        r.quads(w, initial, self.tick != w.tick(), parent_changed)?;
        r.attachments(
            &mut self.scene.attachments,
            w,
            initial,
            initial || self.tick != w.tick(),
            parent_changed,
            initial || next.assets != old.assets,
        );
        if material {
            self.glow_inputs(w);
        }
        self.scene.feed(
            w,
            initial || self.tick != w.tick(),
            moved,
            next.live != old.live || next.membership != old.membership,
            parent_changed,
        );
        self.tick = w.tick();
        self.versions = Some(next);
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod tests;
