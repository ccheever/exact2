use super::*;
use std::cell::Cell;

struct Recording {
    nodes: Vec<crate::models::ModelNode>,
    names: Vec<String>,
    members: Vec<Vec<String>>,
    starts: Vec<Vec<u32>>,
    materials: Vec<MaterialId>,
    custom: std::collections::BTreeSet<MaterialId>,
    revision: u64,
    lookups: Cell<usize>,
}
impl Default for Recording {
    fn default() -> Self {
        Self {
            nodes: vec![
                (MeshId(0), MaterialId(0), glam::Mat4::IDENTITY, Some(2)),
                (
                    MeshId(1),
                    MaterialId(1),
                    glam::Mat4::from_scale(glam::Vec3::new(-1., 1., 1.)),
                    None,
                ),
            ],
            names: vec!["a".into(), "c".into()],
            members: vec![vec!["a".into(), "b".into()], vec!["c".into()]],
            starts: vec![vec![0, 3], vec![0]],
            materials: vec![MaterialId(0), MaterialId(1)],
            custom: Default::default(),
            revision: 1,
            lookups: Cell::new(0),
        }
    }
}
impl Writes for Recording {
    fn model(&self, name: &str) -> Option<crate::models::Draws<'_>> {
        self.lookups.set(self.lookups.get() + 1);
        (name != "missing").then_some(crate::models::Draws {
            nodes: &self.nodes,
            names: &self.names,
            merged: &self.nodes,
            members: &self.members,
            starts: &self.starts,
            parts: &[],
            materials: &self.materials,
            custom: &self.custom,
        })
    }
    fn assets_revision(&self) -> u64 {
        self.revision
    }
    fn max_slots(&self) -> u32 {
        100_000
    }
    fn begin_tick(&mut self) {}
    fn transforms(&mut self, _: u32, _: &[f32], _: bool) -> Result<(), RenderError> {
        Ok(())
    }
    fn previous(&mut self, _: u32, _: &[f32]) -> Result<(), RenderError> {
        Ok(())
    }
    fn materials(&mut self, _: u32, _: &[f32]) -> Result<(), RenderError> {
        Ok(())
    }
    fn mesh(&mut self, _: &[Vertex], _: &[u32]) -> MeshId {
        unreachable!()
    }
    fn batches(&mut self, _: &[Batch], _: &[u32]) -> Result<(), RenderError> {
        Ok(())
    }
}

fn feed(assets: &mut Assets, w: &World, r: &mut Recording) -> (Vec<Batch>, Vec<u32>) {
    let drawn: Vec<_> = w
        .query::<(&Mesh, &Transform)>()
        .iter()
        .map(|(entity, _)| (entity, w.is_visible(entity), false))
        .collect();
    let (mut batches, mut slots) = (Vec::new(), Vec::new());
    assets
        .batches(w, r, &drawn, &mut batches, &mut slots)
        .unwrap();
    (batches, slots)
}

fn compare(assets: &mut Assets, w: &World, r: &mut Recording, resolves: bool) {
    r.lookups.set(0);
    let actual = feed(assets, w, r);
    assert_eq!(
        r.lookups.get() != 0,
        resolves,
        "survivors need no model resolution"
    );
    let mut fresh = Assets::default();
    let expected = feed(&mut fresh, w, r);
    assert_eq!(format!("{:?}", actual.0), format!("{:?}", expected.0));
    assert_eq!(actual.1, expected.1);
    assert_eq!(
        format!("{:?}", assets.records),
        format!("{:?}", fresh.records)
    );
    assert_eq!(assets.entities, fresh.entities);
    assert_eq!(assets.part_bases, fresh.part_bases);
    let bits = |rows: &[crate::models::PartLook]| {
        rows.iter().map(|r| r.map(f32::to_bits)).collect::<Vec<_>>()
    };
    assert_eq!(bits(&assets.part_looks), bits(&fresh.part_looks));
    assert_eq!(assets.levels, fresh.levels);
    assert_eq!(assets.lods, fresh.lods);
    assert_eq!(assets.digests, fresh.digests);
    let ranges = |a: &Assets| {
        a.ranges
            .values()
            .map(|r| (r.entity, r.first, r.count, r.part_first, r.part_count))
            .collect::<Vec<_>>()
    };
    assert_eq!(ranges(assets), ranges(&fresh));
    assert_eq!(
        assets.structure.as_ref().unwrap().models,
        fresh.structure.unwrap().models
    );
}

#[test]
fn removing_models_compacts_parts_lods_skin_records_and_batches_like_a_fresh_feed() {
    let mut w = World::new(60, 0);
    let mut ids = Vec::new();
    for i in 0..12 {
        let e = w.spawn((
            Transform::default(),
            Mesh::asset(if i == 7 { "missing" } else { "model" }),
        ));
        w.insert(
            e,
            exact_game::NodeMaterials(vec![exact_game::NodeMaterial {
                node: if i % 2 == 0 { "a" } else { "c" }.into(),
                color: [i as f32 / 12., 0.5, 0.75, 1.],
                ..Default::default()
            }]),
        );
        if i % 3 == 0 {
            w.insert(
                e,
                exact_game::MaterialOverrides(vec![exact_game::MaterialOverride {
                    material: 0,
                    color: Some([0.5; 4]),
                    emissive: [0.25; 3],
                }]),
            );
            w.insert(
                e,
                exact_game::ModelLod {
                    levels: vec![exact_game::LodLevel {
                        distance: 10.,
                        model: "coarse".into(),
                    }],
                    hide: Some(20.),
                },
            );
        }
        if i % 4 == 0 {
            w.insert(e, ViewModel);
        }
        ids.push(e);
    }
    let (mut assets, mut r) = (Assets::default(), Recording::default());
    compare(&mut assets, &w, &mut r, true);
    for i in [0, 2, 5, 7, 9] {
        w.insert(ids[i], Visible(false));
    }
    compare(&mut assets, &w, &mut r, false);
    w.despawn(ids[11]); // A surviving ordinary model disappears permanently.
    compare(&mut assets, &w, &mut r, false);
    for &e in &ids {
        if w.contains(e) {
            w.insert(e, Visible(false));
        }
    }
    compare(&mut assets, &w, &mut r, false);
    w.remove::<Visible>(ids[6]);
    compare(&mut assets, &w, &mut r, true);
}

#[test]
fn removal_reuse_rejects_changed_models_looks_revisions_and_recycled_entities() {
    let mut w = World::new(60, 0);
    let a = w.spawn((Transform::default(), Mesh::asset("model")));
    let b = w.spawn((Transform::default(), Mesh::asset("model")));
    let c = w.spawn((Transform::default(), Mesh::asset("model")));
    let (mut assets, mut r) = (Assets::default(), Recording::default());
    compare(&mut assets, &w, &mut r, true);
    w.insert(a, Visible(false));
    w.insert(
        b,
        exact_game::NodeMaterials(vec![exact_game::NodeMaterial {
            node: "a".into(),
            ..Default::default()
        }]),
    );
    compare(&mut assets, &w, &mut r, true);
    w.insert(b, Mesh::asset("replacement"));
    compare(&mut assets, &w, &mut r, true);
    w.insert(b, ViewModel);
    compare(&mut assets, &w, &mut r, true);
    w.insert(
        b,
        exact_game::ModelLod {
            levels: vec![],
            hide: Some(10.),
        },
    );
    compare(&mut assets, &w, &mut r, true);
    r.revision += 1;
    compare(&mut assets, &w, &mut r, true);
    w.despawn(c);
    let replacement = w.spawn((Transform::default(), Mesh::asset("model")));
    assert_eq!(replacement.index(), c.index());
    assert_ne!(replacement, c);
    compare(&mut assets, &w, &mut r, true);
}

#[test]
fn sparse_model_removals_keep_entity_order_across_unloaded_models() {
    let mut w = World::new(60, 0);
    let ids: Vec<_> = (0..96)
        .map(|i| {
            w.spawn((
                Transform::default(),
                Mesh::asset(if i % 3 == 0 { "missing" } else { "model" }),
            ))
        })
        .collect();
    let (mut assets, mut r) = (Assets::default(), Recording::default());
    compare(&mut assets, &w, &mut r, true);
    for residue in [1, 4, 0, 3, 2] {
        for (i, &entity) in ids.iter().enumerate() {
            if i % 5 == residue {
                w.insert(entity, Visible(false));
            }
        }
        compare(&mut assets, &w, &mut r, false);
    }
    assert!(assets.records.is_empty());
    assert!(assets.entities.is_empty());
    assert!(assets.ranges.is_empty());
}
