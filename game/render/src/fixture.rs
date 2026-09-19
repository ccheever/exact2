//! CPU recording sink for consumer tests of the production primitive feed.
use crate::{world::Writes, Batch, Feed, MeshId, RenderError, Vertex};
use exact_game::{Entity, Vec3, World};
#[derive(Debug, PartialEq)]
pub(crate) enum Call {
    Begin,
    Transform(u32, usize, bool),
    Material(u32, usize),
    Previous(u32, usize),
    Batches,
}
/// In-memory primitive uploads, using the same feed and slot limit as renderer tests.
pub struct Recording {
    pub(crate) calls: Vec<Call>,
    pub(crate) current: Vec<f32>,
    pub(crate) previous: Vec<f32>,
    /// Twelve material floats per entity slot.
    pub materials: Vec<f32>,
    pub(crate) slots: Vec<u32>,
    pub(crate) batches: Vec<Batch>,
    /// Primitive mesh uploads in allocation order.
    pub meshes: Vec<(Vec<Vertex>, Vec<u32>)>,
    pub(crate) limit: u32,
    pub(crate) record: bool,
}
impl Default for Recording {
    fn default() -> Self {
        Self {
            calls: Vec::new(),
            current: Vec::new(),
            previous: Vec::new(),
            materials: Vec::new(),
            slots: Vec::new(),
            batches: Vec::new(),
            meshes: Vec::new(),
            limit: 1_000_000,
            record: true,
        }
    }
}
impl Recording {
    fn call(&mut self, call: Call) {
        if self.record {
            self.calls.push(call);
        }
    }
    /// Translation from the current or previous tick upload for a recorded entity.
    pub fn position(&self, e: Entity, previous: bool) -> Vec3 {
        let v = if previous {
            &self.previous
        } else {
            &self.current
        };
        Vec3::from_slice(&v[e.index() as usize * 10..][..3])
    }
}
impl Writes for Recording {
    fn max_slots(&self) -> u32 {
        self.limit
    }
    fn begin_tick(&mut self) {
        self.call(Call::Begin);
        std::mem::swap(&mut self.current, &mut self.previous);
    }
    fn transforms(&mut self, first: u32, values: &[f32], both: bool) -> Result<(), RenderError> {
        self.call(Call::Transform(first, values.len(), both));
        let start = first as usize * 10;
        let end = start + values.len();
        if end > self.current.len() {
            self.current.resize(end, 0.0);
        }
        self.current[start..end].copy_from_slice(values);
        if both {
            if end > self.previous.len() {
                self.previous.resize(end, 0.0);
            }
            self.previous[start..end].copy_from_slice(values);
        }
        Ok(())
    }
    fn previous(&mut self, first: u32, values: &[f32]) -> Result<(), RenderError> {
        self.call(Call::Previous(first, values.len()));
        let start = first as usize * 10;
        let end = start + values.len();
        if self.previous.len() < end {
            self.previous.resize(end, 0.);
        }
        self.previous[start..end].copy_from_slice(values);
        Ok(())
    }
    fn materials(&mut self, first: u32, values: &[f32]) -> Result<(), RenderError> {
        self.call(Call::Material(first, values.len()));
        let start = first as usize * 12;
        let end = start + values.len();
        if end > self.materials.len() {
            self.materials.resize(end, 0.0);
        }
        self.materials[start..end].copy_from_slice(values);
        Ok(())
    }
    fn mesh(&mut self, v: &[Vertex], i: &[u32]) -> MeshId {
        let id = MeshId(self.meshes.len());
        self.meshes.push((v.to_vec(), i.to_vec()));
        id
    }
    fn batches(&mut self, b: &[Batch], s: &[u32]) -> Result<(), RenderError> {
        self.call(Call::Batches);
        self.batches.clear();
        self.batches.extend_from_slice(b);
        self.slots.clear();
        self.slots.extend_from_slice(s);
        Ok(())
    }
}

impl Recording {
    /// Avoid retaining per-write call traces during timing loops.
    pub fn without_trace(mut self) -> Self {
        self.record = false;
        self
    }
    /// Run the production primitive feed; refuse beyond the 1,000,000-slot limit.
    pub fn feed(&mut self, feed: &mut Feed, world: &World) -> Result<(), RenderError> {
        feed.feed_to(world, self)
    }
}
