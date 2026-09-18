//! Headless Writes: same feed and arena growth policy, no pretend shader/device work.
use crate::{
    audit::{self, Audit},
    world::Writes,
    Batch, DrawInstance, MaterialId, MeshId, RenderError, Vertex,
};
use exact_game::{asset::Model, World};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) struct Recording {
    pub audit: Audit,
    capacities: [u64; 7],
    mesh_bytes: [u64; 2],
    meshes: usize,
    models: BTreeMap<String, Vec<crate::models::ModelNode>>,
    revision: u64,
    textures: BTreeSet<String>,
}
impl Recording {
    pub fn new(audit: Audit) -> Self {
        let _scope = audit.enter();
        audit::record(audit::VERTEX, "game vertices", 1);
        audit::record(audit::INDEX, "game indices", 1);
        Self {
            audit,
            capacities: [640, 640, 768, 64, 128, 1024, 1024],
            mesh_bytes: [0, 0],
            meshes: 0,
            models: BTreeMap::new(),
            revision: 0,
            textures: BTreeSet::new(),
        }
    }
    fn grow(&mut self, index: usize, needed: u64) {
        if needed <= self.capacities[index] {
            return;
        }
        self.capacities[index] = needed.next_power_of_two();
        let names = [
            "game transforms a",
            "game transforms b",
            "game materials",
            "game slots",
            "game model instances",
            "game vertices",
            "game indices",
        ];
        audit::record(
            if index < 5 {
                audit::GROW
            } else if index == 5 {
                audit::VERTEX
            } else {
                audit::INDEX
            },
            names[index],
            1,
        );
        if matches!(index, 0 | 3 | 4) {
            audit::record(audit::SLOTS, names[index], 1);
        }
    }
    fn slots(&mut self, end: u64) -> Result<(), RenderError> {
        if end > u64::from(self.max_slots()) {
            return Err(RenderError::Capacity {
                arena: "headless slots",
                slot: end - 1,
                limit: u64::from(self.max_slots()),
            });
        }
        self.grow(0, end * 40);
        self.grow(1, end * 40);
        self.grow(2, end * 48);
        Ok(())
    }
    pub fn texture(&mut self, name: &str, data: &exact_game::asset::TextureData) {
        if self.textures.insert(name.into()) {
            audit::record(audit::TEXTURE, name, 1);
            audit::record(
                audit::TEX_BYTES,
                name,
                data.mips.iter().map(|m| m.len() as u64).sum(),
            );
        }
    }
    pub fn prepare(&mut self, name: &str, model: &Model) {
        if self.models.contains_key(name) {
            return;
        }
        let meshes: Vec<_> = model
            .meshes
            .iter()
            .map(|mesh| {
                self.geometry(
                    mesh.positions.len() as u64 / 3 * 40,
                    mesh.indices.len() as u64 * 4,
                )
            })
            .collect();
        let nodes = model
            .nodes
            .iter()
            .zip(model.offsets().expect("validated model"))
            .filter_map(|(node, local)| {
                node.mesh.map(|m| {
                    (
                        meshes[m as usize],
                        MaterialId(model.meshes[m as usize].material as usize),
                        local,
                        node.skin,
                    )
                })
            })
            .collect();
        self.models.insert(name.into(), nodes);
        self.revision += 1;
    }
    fn geometry(&mut self, vertices: u64, indices: u64) -> MeshId {
        self.mesh_bytes[0] += vertices;
        self.mesh_bytes[1] += indices;
        self.grow(5, self.mesh_bytes[0]);
        self.grow(6, self.mesh_bytes[1]);
        audit::record(audit::MESH_BYTES, "mesh", vertices + indices);
        let id = MeshId(self.meshes);
        self.meshes += 1;
        id
    }
    pub fn feed(&mut self, feed: &mut crate::Feed, w: &World) -> Result<(), RenderError> {
        let _scope = self.audit.enter();
        self.audit.tick(w.tick());
        feed.feed_to(w, self)
    }
}
impl Writes for Recording {
    fn max_slots(&self) -> u32 {
        200_000
    }
    fn begin_tick(&mut self) {}
    fn model(&self, name: &str) -> Option<&[crate::models::ModelNode]> {
        self.models.get(name).map(Vec::as_slice)
    }
    fn assets_revision(&self) -> u64 {
        self.revision
    }
    fn instances(&mut self, records: &[DrawInstance]) -> Result<(), RenderError> {
        if records.len() > self.max_slots() as usize {
            return Err(RenderError::Scene(
                "headless draw instances exceed 200000".into(),
            ));
        }
        self.grow(4, records.len() as u64 * 144);
        Ok(())
    }
    fn transforms(&mut self, first: u32, floats: &[f32], _: bool) -> Result<(), RenderError> {
        self.slots(u64::from(first) + floats.len() as u64 / 10)
    }
    fn previous(&mut self, first: u32, floats: &[f32]) -> Result<(), RenderError> {
        self.transforms(first, floats, false)
    }
    fn materials(&mut self, first: u32, floats: &[f32]) -> Result<(), RenderError> {
        self.slots(u64::from(first) + floats.len() as u64 / 12)
    }
    fn mesh(&mut self, v: &[Vertex], i: &[u32]) -> MeshId {
        self.geometry(size_of_val(v) as u64, size_of_val(i) as u64)
    }
    fn textured_mesh(&mut self, v: &[Vertex], i: &[u32], image: &crate::assets::Image) -> MeshId {
        audit::record(audit::TEXTURE, "embedded texture", 1);
        audit::record(
            audit::TEX_BYTES,
            "embedded texture",
            image.rgba.len() as u64,
        );
        self.mesh(v, i)
    }
    fn update_mesh(&mut self, _: MeshId, v: &[Vertex]) {
        audit::streaming(size_of_val(v) as u64);
    }
    fn batches(&mut self, _: &[Batch], slots: &[u32]) -> Result<(), RenderError> {
        if slots.len() > self.max_slots() as usize {
            return Err(RenderError::Scene(
                "headless draw slots exceed 200000".into(),
            ));
        }
        self.grow(3, size_of_val(slots) as u64);
        Ok(())
    }
}
