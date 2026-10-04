//! Tick uploads and retained scene selection. Frames never walk entity storage.
use crate::{shapes, Batch, MeshId, RenderError, Vertex};
use exact_game::{Material, Mesh, Parent, Transform, ViewModel, Visible, World, PAGE};
use std::collections::BTreeMap;

pub(crate) mod assets;
pub(crate) mod scene;
mod upload;
pub(crate) use scene::snap as trace_snap;
use scene::Scene;

// The same feed algorithm runs against the GPU and the recording test backend.
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
            self.models.loaded.get(name).filter(|m| m.active).map(|m| {
                (
                    m.nodes.as_slice(),
                    m.names.as_slice(),
                    m.merged.as_slice(),
                    m.members.as_slice(),
                )
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
        let part_looks = &mut self.models.part_looks;
        part_looks.0.clear();
        part_looks.0.extend_from_slice(bases);
        part_looks.1.clear();
        part_looks.1.extend_from_slice(looks);
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
            for (_, look) in w.query::<&exact_game::ParticleLook>().iter() {
                if !look.texture.is_empty() {
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

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Shape {
    Box,
    Sphere,
    Cylinder,
    Plane,
    Capsule,
}
impl Shape {
    fn of(mesh: &Mesh) -> Result<Self, RenderError> {
        Ok(match mesh {
            Mesh::Box { .. } => Self::Box,
            Mesh::Sphere { .. } => Self::Sphere,
            Mesh::Cylinder { .. } => Self::Cylinder,
            Mesh::Plane { .. } => Self::Plane,
            Mesh::Capsule { .. } => Self::Capsule,
            Mesh::Asset(name) => {
                return Err(RenderError::Scene(format!(
                    "Mesh.Asset({name}): asset meshes are not implemented"
                )))
            }
        })
    }
    fn geometry(self) -> (Vec<Vertex>, Vec<u32>) {
        let (mut vertices, indices) = match self {
            Self::Box => shapes::cube(),
            Self::Sphere => shapes::sphere(24),
            Self::Cylinder => shapes::cylinder(24),
            Self::Plane => shapes::plane(),
            Self::Capsule => shapes::capsule(0.5, 2.0, 24),
        };
        let half = vertices.len() / 2;
        for (i, v) in vertices.iter_mut().enumerate() {
            v.uv = if self == Self::Capsule {
                let sign = if i < half { 1.0 } else { -1.0 };
                v.position[1] -= sign * 0.5;
                [sign, 1.0]
            } else {
                [0.0; 2]
            };
        }
        (vertices, indices)
    }
}
fn dimensions(mesh: &Mesh) -> [f32; 3] {
    match mesh {
        Mesh::Box { size } => size.to_array(),
        Mesh::Sphere { radius } => [2.0 * radius; 3],
        Mesh::Cylinder { radius, height } => [2.0 * radius, *height, 2.0 * radius],
        Mesh::Plane { width, depth } => [*width, 1.0, *depth],
        Mesh::Capsule { radius, height } => [2.0 * radius, height * 0.5 - radius, 2.0 * radius],
        Mesh::Asset(_) => [1.0; 3], // Refused before any upload.
    }
}
struct Group {
    mesh: MeshId,
    viewmodel: bool,
    slots: Vec<u32>,
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
struct Versions {
    assets: u64,
    transform: u64,
    parent: u64,
    material: u64,
    glow: u64,
    mesh: u64,
    visible: u64,
    viewmodel: u64,
    opacity: u64,
    node_materials: u64,
    lod: u64,
    // An animated rig moves its socket followers' subtrees without a Transform write.
    pose: u64,
    // Presentation offsets (exact_game::Offset) patch drawn poses like parents do.
    offset: u64,
    live: u64,
    membership: u64,
}
/// A content digest of every `NodeMaterials` row, by slot.
fn node_looks_digest(w: &World) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for (e, looks) in w.query::<&exact_game::NodeMaterials>().iter() {
        e.index().hash(&mut h);
        for l in &looks.0 {
            l.node.hash(&mut h);
            l.color.map(f32::to_bits).hash(&mut h);
            l.emissive.map(f32::to_bits).hash(&mut h);
        }
    }
    h.finish()
}
/// Content of every presentation offset: present rewrites the rows each tick,
/// so their revision moves even when no offset changed.
fn offsets(w: &World) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for (e, o) in w.query::<&exact_game::Offset>().iter() {
        let t = o.0;
        let words = [t.position.to_array(), t.scale.to_array()]
            .into_iter()
            .flatten()
            .chain(t.rotation.to_array())
            .map(f32::to_bits)
            .chain([e.index(), e.generation()]);
        for word in words {
            h = (h ^ u64::from(word)).wrapping_mul(0x100_0000_01b3);
        }
    }
    h
}
impl Versions {
    fn of(w: &World, assets: u64) -> Self {
        Self {
            assets,
            transform: w.revision::<Transform>(),
            parent: w.revision::<Parent>(),
            material: w.revision::<Material>(),
            glow: w.revision::<exact_game::Glow>(),
            mesh: w.revision::<Mesh>(),
            visible: w.revision::<Visible>(),
            viewmodel: w.revision::<ViewModel>(),
            opacity: w.revision::<exact_game::Opacity>(),
            node_materials: w.revision::<exact_game::NodeMaterials>(),
            lod: w.revision::<exact_game::ModelLod>(),
            pose: w.revision::<exact_game::Pose>(),
            offset: offsets(w),
            live: w.entities_revision(),
            membership: w.membership::<Transform>(),
        }
    }
}

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
    // Presentation looks are rebuilt every tick: rebatch only when their content
    // changes, not their revision.
    node_looks: u64,
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
            node_looks: 0,
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
        let moved = initial
            || next.transform != old.transform
            || next.parent != old.parent
            || next.pose != old.pose
            || next.offset != old.offset;
        let material = initial
            || next.material != old.material
            || next.glow != old.glow
            || next.membership != old.membership
            || next.mesh != old.mesh;
        let looks_changed = (initial || next.node_materials != old.node_materials) && {
            let digest = node_looks_digest(w);
            std::mem::replace(&mut self.node_looks, digest) != digest || initial
        };
        let batches = initial
            || next.assets != old.assets
            || next.mesh != old.mesh
            || next.visible != old.visible
            || next.viewmodel != old.viewmodel
            || looks_changed
            || next.lod != old.lod
            || next.live != old.live
            || next.membership != old.membership;
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
        let offset_changed = next.offset != old.offset;
        if moved || (self.history_pending && w.tick() != self.tick) {
            r.begin_tick();
            self.current = 1 - self.current;
            // Parented global poses: every one when the hierarchy changed, else
            // only those in blocks whose poses changed since the last pass.
            // Membership too: a parented entity can gain its Transform later.
            let rebuilt = initial
                || parent_changed
                || offset_changed
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
            }
            self.override_cursor = Some(w.pose_cursor());
            // This history buffer was last written at its cursor: a parented
            // page needs patching only if a pose in it changed since then.
            changed_blocks(
                w,
                self.buffer_cursors[self.current],
                &mut self.changed_blocks,
            );
            if parent_changed || offset_changed {
                // Pages patched before, and those patched now, are rewritten.
                for e in self
                    .parents
                    .iter()
                    .chain(self.overrides.iter().map(|(e, _)| e))
                {
                    self.transforms[self.current].invalidate(e.index() as usize / PAGE);
                }
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
                    if !self.scratch.is_empty()
                        && run + (self.scratch.len() / 10) as u32 != page.first
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
                for &(e, t) in
                    self.overrides
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
                                self.transforms[1 - self.current]
                                    .invalidate(e.index() as usize / PAGE);
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
        }
        if batches {
            self.dimensions.fill([1.0; 3]);
            for group in &mut self.groups {
                group.slots.clear();
            }
            for (e, (mesh, _)) in w.query::<(&Mesh, &Transform)>().iter() {
                mesh.validate().map_err(RenderError::scene)?;
                if matches!(mesh, Mesh::Asset(_)) {
                    continue;
                }
                let shape = Shape::of(mesh)?;
                let slot = e.index() as usize;
                if self.dimensions.len() <= slot {
                    self.dimensions.resize(slot + 1, [1.0; 3]);
                }
                self.dimensions[slot] = dimensions(mesh);
                if w.get::<Visible>(e).is_some_and(|v| !v.0) {
                    continue;
                }
                let viewmodel = w.has::<ViewModel>(e);
                let group = *self.shapes.entry((shape, viewmodel)).or_insert_with(|| {
                    let (v, i) = shape.geometry();
                    let index = self.groups.len();
                    self.groups.push(Group {
                        mesh: r.mesh(&v, &i),
                        viewmodel,
                        slots: Vec::new(),
                    });
                    index
                });
                self.groups[group].slots.push(e.index());
            }
            self.batches.clear();
            self.slots.clear();
            for group in &self.groups {
                if group.slots.is_empty() {
                    continue;
                }
                let start = self.slots.len() as u32;
                self.slots.extend_from_slice(&group.slots);
                let mut batch = Batch::new(group.mesh, start..self.slots.len() as u32);
                if group.viewmodel {
                    batch = batch.viewmodel();
                }
                self.batches.push(batch);
            }
        }
        if material {
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
                    if let Some(p) = page
                        .as_ref()
                        .filter(|p| p.mask()[i / 64] & (1 << (i % 64)) != 0)
                    {
                        let record = &p.floats()[i * 10..i * 10 + 10];
                        out[..9].copy_from_slice(&record[..9]);
                        out[3] = grid_alpha(record[3], record[9]);
                    } else {
                        out.copy_from_slice(&default);
                    }
                    out[9..12].copy_from_slice(
                        self.dimensions.get(first as usize + i).unwrap_or(&[1.0; 3]),
                    );
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
        }
        if batches {
            if next.assets != 0 || !self.assets.records.is_empty() {
                self.assets
                    .batches(w, r, &mut self.batches, &mut self.slots)?;
            }
            r.batches(&self.batches, &self.slots)?;
        }
        if !self.assets.records.is_empty() && (moved || batches || self.tick != w.tick()) {
            // Static instances cost nothing, parented or not: only blocks whose
            // local or propagated poses changed since the last pose step.
            changed_blocks(w, self.model_cursor, &mut self.changed_blocks);
            self.model_cursor = Some(w.pose_cursor());
            self.changed_pages.clear();
            self.changed_pages
                .extend(self.changed_blocks.iter().map(|&b| b as usize / PAGE));
            let moved = if initial || parent_changed || batches {
                crate::models::Moved::All
            } else {
                crate::models::Moved::Pages {
                    pages: &self.changed_pages,
                }
            };
            r.model_poses(w, &self.assets.entities, initial || parent_changed, moved);
        }
        if initial || next.opacity != old.opacity || next.live != old.live {
            self.fades_next.clear();
            for (e, o) in w.query::<&exact_game::Opacity>().iter() {
                self.fades_next.push((e.index(), o.0));
            }
            if initial || self.fades_next != self.fades {
                std::mem::swap(&mut self.fades, &mut self.fades_next);
                r.opacity(&self.fades);
            }
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
            self.glows.clear();
            for (entity, glow) in w.query::<&exact_game::Glow>().iter() {
                let mut values =
                    material_floats(w.get::<Material>(entity).map(|m| *m).unwrap_or_default());
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
                    model: w
                        .get::<Mesh>(entity)
                        .is_some_and(|m| matches!(*m, Mesh::Asset(_))),
                });
            }
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
/// First entity index of each block whose poses changed since `cursor`, in
/// order; every block without a cursor.
fn changed_blocks(w: &World, cursor: Option<exact_game::PoseCursor>, out: &mut Vec<u32>) {
    out.clear();
    match cursor {
        Some(cursor) => out.extend(w.poses_changed_since(cursor)),
        None => out.extend(w.pages::<Transform>().iter().map(|p| p.first)),
    }
}
fn page_len(first: u32, limit: u32) -> usize {
    (limit.saturating_sub(first) as usize).min(PAGE)
}
fn check_page(
    first: u32,
    mask: &[u64],
    limit: u32,
    arena: &'static str,
) -> Result<(), RenderError> {
    if let Some((word, bits)) = mask.iter().enumerate().rev().find(|(_, bits)| **bits != 0) {
        let slot = u64::from(first) + (word * 64 + 63 - bits.leading_zeros() as usize) as u64;
        if slot >= u64::from(limit) {
            return Err(RenderError::Capacity {
                arena,
                slot,
                limit: u64::from(limit),
            });
        }
    }
    Ok(())
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
fn grid_alpha(alpha: f32, spacing: f32) -> f32 {
    if spacing > 0.0 {
        -spacing
    } else {
        alpha.max(0.0)
    }
}
fn material_floats(m: Material) -> [f32; 12] {
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

#[cfg(test)]
pub(crate) mod tests;
