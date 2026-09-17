//! Tick uploads and retained scene selection. Frames never walk entity storage.
use crate::{shapes, Batch, MeshId, RenderError, Renderer, Rewrite, Vertex};
use exact_game::{Material, Mesh, Parent, Transform, Visible, World, PAGE};
use std::collections::BTreeMap;

mod scene;
mod upload;
use scene::Scene;

// The same feed algorithm runs against the GPU and the recording test backend.
pub(crate) trait Writes {
    fn max_slots(&self) -> u32;
    fn begin_tick(&mut self, rewrite: Rewrite);
    fn transforms(&mut self, first: u32, floats: &[f32], both: bool) -> Result<(), RenderError>;
    fn previous(&mut self, first: u32, floats: &[f32]) -> Result<(), RenderError>;
    fn materials(&mut self, first: u32, floats: &[f32]) -> Result<(), RenderError>;
    fn mesh(&mut self, vertices: &[Vertex], indices: &[u32]) -> MeshId;
    fn batches(&mut self, batches: &[Batch], slots: &[u32]) -> Result<(), RenderError>;
}
impl Writes for Renderer {
    fn max_slots(&self) -> u32 {
        self.max_slots()
    }
    fn begin_tick(&mut self, rewrite: Rewrite) {
        self.begin_tick(rewrite);
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
    Cube,
    Sphere,
    Cylinder,
    Plane(u32),
    Capsule(u32, u32),
}
impl Shape {
    fn of(mesh: &Mesh) -> Self {
        match mesh {
            Mesh::Cube | Mesh::Asset(_) => Self::Cube,
            Mesh::Sphere => Self::Sphere,
            Mesh::Cylinder => Self::Cylinder,
            Mesh::Plane { size } => Self::Plane(size.to_bits()),
            Mesh::Capsule { radius, height } => Self::Capsule(radius.to_bits(), height.to_bits()),
        }
    }
    fn geometry(self) -> (Vec<Vertex>, Vec<u32>) {
        let (mut vertices, indices) = match self {
            Self::Cube => shapes::cube(),
            Self::Sphere => shapes::sphere(24),
            Self::Cylinder => shapes::cylinder(24),
            Self::Plane(_) => shapes::plane(),
            Self::Capsule(r, h) => {
                let r = f32::from_bits(r);
                let h = f32::from_bits(h);
                // Invalid author geometry has a visible fallback, never a shape panic.
                if r.is_finite()
                    && r > 0.0
                    && h.is_finite()
                    && h >= 0.0
                    && (h + 2.0 * r).is_finite()
                {
                    shapes::capsule(r, h + 2.0 * r, 24)
                } else {
                    shapes::cube()
                }
            }
        };
        for v in &mut vertices {
            match self {
                Self::Sphere => v.position.iter_mut().for_each(|x| *x *= 2.0),
                Self::Cylinder => {
                    v.position[0] *= 2.0;
                    v.position[2] *= 2.0;
                }
                Self::Plane(bits) => {
                    let size = f32::from_bits(bits);
                    let size = if size.is_finite() { size.abs() } else { 1.0 };
                    v.position[0] *= size;
                    v.position[2] *= size;
                }
                _ => {}
            }
        }
        (vertices, indices)
    }
}
struct Group {
    mesh: MeshId,
    slots: Vec<u32>,
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
struct Versions {
    transform: u64,
    parent: u64,
    material: u64,
    mesh: u64,
    visible: u64,
    live: u64,
    membership: u64,
}
impl Versions {
    fn of(w: &World) -> Self {
        Self {
            transform: w.revision::<Transform>(),
            parent: w.revision::<Parent>(),
            material: w.revision::<Material>(),
            mesh: w.revision::<Mesh>(),
            visible: w.revision::<Visible>(),
            live: w.entities_revision(),
            membership: w.membership::<Transform>(),
        }
    }
}

/// Persistent bridge from one world to one renderer. World loads invalidate it
/// automatically; reset it when replacing a World with an unrelated instance.
/// Page hashes select coalesced uploads; parented poses are patched before hashing.
/// Decomposed globals are exact TRS for uniform ancestor scale; shear is approximated.
/// Storage, batches and meshes grow only when scene structure changes.
pub struct Feed {
    versions: Option<Versions>,
    tick: u64,
    history_pending: bool,
    groups: Vec<Group>,
    shapes: BTreeMap<Shape, usize>,
    batches: Vec<Batch>,
    slots: Vec<u32>,
    material_page: Box<[f32; PAGE * 12]>,
    transform_page: Box<[f32; PAGE * 10]>,
    scratch: Vec<f32>,
    transforms: [upload::Pages; 2],
    materials: upload::Pages,
    current: usize,
    generation: u64,
    parents: Vec<exact_game::Entity>,
    overrides: Vec<(exact_game::Entity, [f32; 10])>,
    scene: Scene,
}
impl Default for Feed {
    fn default() -> Self {
        Self {
            versions: None,
            tick: 0,
            history_pending: false,
            groups: Vec::new(),
            shapes: BTreeMap::new(),
            batches: Vec::new(),
            slots: Vec::new(),
            material_page: Box::new([0.0; PAGE * 12]),
            transform_page: Box::new([0.0; PAGE * 10]),
            scratch: Vec::new(),
            transforms: Default::default(),
            materials: Default::default(),
            current: 0,
            generation: 0,
            parents: Vec::new(),
            overrides: Vec::new(),
            scene: Scene::default(),
        }
    }
}
impl Feed {
    /// Forget world history after replacement, retaining registered geometry and scratch.
    pub fn reset(&mut self) {
        self.versions = None;
        self.tick = 0;
        self.history_pending = false;
        self.scene.reset();
        for buffer in &mut self.transforms {
            buffer.reset();
        }
        self.materials.reset();
        self.parents.clear();
    }

    /// Feed one completed tick. With Sim::advance_with, call only when ticks_left < 2.
    /// Initial feeding initializes both histories, including a world's setup tick.
    pub fn feed(&mut self, world: &World, renderer: &mut Renderer) -> Result<(), RenderError> {
        self.feed_to(world, renderer)
    }
    /// Fixed-size frame inputs, including the interpolated camera and nearest 16 lights.
    /// No world queries, allocation, or entity scans occur here.
    pub fn frame(&mut self, world: &World, alpha: f32, aspect: f32) -> crate::FrameInput<'_> {
        self.scene.frame(world, alpha, aspect)
    }
    pub(crate) fn feed_to(&mut self, w: &World, r: &mut impl Writes) -> Result<(), RenderError> {
        if self.generation != w.presentation_generation() {
            self.reset();
            self.generation = w.presentation_generation();
        }
        let next = Versions::of(w);
        let initial = self.versions.is_none();
        let old = self.versions.unwrap_or_default();
        let moved = initial || next.transform != old.transform || next.parent != old.parent;
        let material =
            initial || next.material != old.material || next.membership != old.membership;
        let batches = initial
            || next.mesh != old.mesh
            || next.visible != old.visible
            || next.live != old.live
            || next.membership != old.membership;
        // Validate live slots before any history swap. A last partial page is clipped
        // only at the device boundary; absent trailing slots do not refuse a valid world.
        if moved || material || batches {
            for page in w.pages::<Transform>().iter() {
                check_page(page.first, page.mask, r.max_slots(), "transforms")?;
            }
            for page in w.pages::<Material>().iter() {
                check_page(page.first, page.mask, r.max_slots(), "materials")?;
            }
        }
        let parent_changed = next.parent != old.parent;
        if moved || (self.history_pending && w.tick() != self.tick) {
            r.begin_tick(Rewrite::All);
            self.current = 1 - self.current;
            self.overrides.clear();
            for (e, _) in w.query::<(&Parent, &Transform)>().iter() {
                if let Some(t) = scene::pose(w, e) {
                    self.overrides.push((e, floats(t)));
                }
            }
            let (a, b) = self.transforms.split_at_mut(1);
            if self.current == 0 {
                a[0].inherit_policy(&b[0]);
            } else {
                b[0].inherit_policy(&a[0]);
            }
            let pages = w.pages::<Transform>();
            let hashing = self.transforms[self.current].start(pages.iter().count(), initial);
            let mut overrides = self.overrides.iter().peekable();
            let mut run = 0;
            self.scratch.clear();
            for page in pages.iter() {
                let len = page_len(page.first, r.max_slots()) * 10;
                let index = page.first as usize / PAGE;
                let mut values = &page.floats()[..len];
                if overrides
                    .peek()
                    .is_some_and(|(e, _)| e.index() < page.first + PAGE as u32)
                {
                    self.transform_page[..len].copy_from_slice(values);
                    while let Some((e, pose)) =
                        overrides.next_if(|(e, _)| e.index() < page.first + PAGE as u32)
                    {
                        let at = (e.index() - page.first) as usize * 10;
                        self.transform_page[at..at + 10].copy_from_slice(pose);
                    }
                    values = &self.transform_page[..len];
                }
                let hash = if hashing { upload::hash(values) } else { 0 };
                if self.transforms[self.current].dirty(index, hash, initial) {
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
                if initial {
                    let other = &mut self.transforms[1 - self.current].hashes;
                    if other.len() <= index {
                        other.resize(index + 1, 0);
                    }
                    other[index] = hash;
                }
            }
            if !self.scratch.is_empty() {
                r.transforms(run, &self.scratch, initial)?;
            }
            self.transforms[self.current].finish();
            if !initial {
                for &e in w.fresh() {
                    if let Some(t) = scene::pose(w, e) {
                        r.previous(e.index(), &floats(t))?;
                        self.transforms[1 - self.current].invalidate(e.index() as usize / PAGE);
                    }
                }
                for &(e, t) in &self.overrides {
                    if !w.fresh().contains(&e) && scene::snap(w, e, parent_changed) {
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
            self.parents.clear();
            self.parents.extend(self.overrides.iter().map(|(e, _)| *e));
            self.history_pending = moved && !initial;
        }
        if material {
            let transforms = w.pages::<Transform>();
            let materials = w.pages::<Material>();
            let hashing = self.materials.start(
                transforms.iter().count().max(materials.iter().count()),
                initial,
            );
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
                let len = page_len(first, r.max_slots());
                let default = material_floats(Material::default());
                for (i, out) in self.material_page[..len * 12]
                    .chunks_exact_mut(12)
                    .enumerate()
                {
                    if let Some(p) = page
                        .as_ref()
                        .filter(|p| p.mask[i / 64] & (1 << (i % 64)) != 0)
                    {
                        out[..9].copy_from_slice(&p.floats()[i * 10..i * 10 + 9]);
                        out[9..].fill(0.0);
                    } else {
                        out.copy_from_slice(&default);
                    }
                }
                let values = &self.material_page[..len * 12];
                let hash = if hashing { upload::hash(values) } else { 0 };
                if self.materials.dirty(first as usize / PAGE, hash, initial) {
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
            self.materials.finish();
        }
        if batches {
            for group in &mut self.groups {
                group.slots.clear();
            }
            for (e, (mesh, _)) in w.query::<(&Mesh, &Transform)>().iter() {
                if w.get::<Visible>(e).is_some_and(|v| !v.0) {
                    continue;
                }
                let shape = Shape::of(mesh);
                let group = *self.shapes.entry(shape).or_insert_with(|| {
                    let (v, i) = shape.geometry();
                    let index = self.groups.len();
                    self.groups.push(Group {
                        mesh: r.mesh(&v, &i),
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
                self.batches
                    .push(Batch::new(group.mesh, start..self.slots.len() as u32));
            }
            r.batches(&self.batches, &self.slots)?;
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
            return Err(RenderError {
                arena,
                slot,
                limit: u64::from(limit),
            });
        }
    }
    Ok(())
}
fn floats(t: Transform) -> [f32; 10] {
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
fn material_floats(m: Material) -> [f32; 12] {
    [
        m.color[0],
        m.color[1],
        m.color[2],
        m.color[3],
        m.metallic,
        m.roughness,
        m.emissive[0],
        m.emissive[1],
        m.emissive[2],
        0.0,
        0.0,
        0.0,
    ]
}

#[cfg(test)]
mod tests;
