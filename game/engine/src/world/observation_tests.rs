// The pre-cache observation path is retained as a test oracle.
use super::*;

impl World {
    fn observe_uncached(&self, out: &mut Observation, full: bool) {
        let mut full = full.then(crate::hash::Hasher::default);
        if let Some(w) = &mut full {
            w.begin_struct();
            w.field("state");
            self.state.write(w);
            w.field("rng");
        }
        let rng = self.rng.get().unwrap();
        let rng_hash = match &mut full {
            Some(w) => w.with_observation(&*rng),
            None => crate::hash::of(&*rng),
        };
        out.entries.clear();
        let ambient = self.storage::<crate::Ambient>();
        for e in self.entities() {
            if ambient.is_some_and(|s| s.has(e.index() as usize)) {
                continue;
            }
            out.entries.push((0, "exists", e, 0));
        }
        for (e, _) in self.query::<&crate::Parent>().iter() {
            if ambient.is_some_and(|s| s.has(e.index() as usize)) {
                continue;
            }
            if let Some(pose) = self.global(e) {
                out.entries
                    .push((1, "global", e, crate::hash::of(&pose.to_cols_array())));
            }
        }
        if let Some(w) = &mut full {
            w.field("components");
            w.begin_struct();
        }
        for (&name, storage) in &self.components {
            out.scratch.clear();
            if let Some(w) = &mut full {
                w.key(name);
            }
            storage.snapshot_uncached(ambient, &mut out.scratch, full.as_mut(), &|i| {
                self.entity_at(i)
            });
            out.entries.extend(out.scratch.iter().map(|&(i, hash)| {
                (
                    2,
                    name,
                    Entity {
                        index: i as u32,
                        generation: self.state.slots[i].generation,
                    },
                    hash,
                )
            }));
        }
        if let Some(mut w) = full.take() {
            w.end_struct();
            // Leave resources lazy: observing rest must never snapshot ambient executors.
            *self.hash_prefix.borrow_mut() = Some((self.mutation_epoch(), w));
        }
        // Keep the dedicated RNG in a separate observation category so names remain sorted.
        out.entries.push((3, "Rng", SINGLETON, rng_hash));
        for (&name, storage) in &self.resources {
            if self.registry[name].ambient {
                continue;
            }
            out.scratch.clear();
            storage.snapshot_uncached(None, &mut out.scratch, None, &|_| SINGLETON);
            out.entries.extend(
                out.scratch
                    .iter()
                    .map(|&(_, hash)| (4, name, SINGLETON, hash)),
            );
        }
    }
}

#[derive(Default, crate::Component)]
struct Payload {
    text: String,
    number: f64,
}
#[derive(Default, crate::Resource)]
struct Count(u32);

#[test]
fn in_place_read_discards_every_observation_cache() {
    let mut w = World::new(60, 7);
    let old = w.spawn_named("old", crate::Transform::default());
    w.insert_resource(Count(1));
    let mut before = Observation::default();
    w.observe_with_hash(&mut before, true);
    let id = w.id();
    let mut source = World::new(60, 19);
    let dead = source.spawn(());
    source.despawn(dead);
    let new = source.spawn_named("new", crate::Transform::at(9.0, 0.0, 0.0));
    source.insert_resource(Count(2));
    assert_eq!(old.index(), new.index());
    assert_ne!(old, new);
    let bytes = source.save();
    let mut decoder = bin::Decoder::for_load(&bytes[MAGIC.len()..], None);
    w.read(&mut decoder).unwrap();
    decoder.finish().unwrap();
    assert_eq!(w.id(), id);
    assert!(!w.contains(old));
    assert_eq!(w.get::<crate::Transform>(new).unwrap().position.x, 9.0);
    assert_eq!(w.resource::<Count>().0, 2);
    for full in [false, true] {
        let mut cached = Observation::default();
        let mut oracle = Observation::default();
        w.observe_with_hash(&mut cached, full);
        w.observe_uncached(&mut oracle, full);
        assert_eq!(cached.entries, oracle.entries);
        assert_ne!(cached.entries, before.entries);
        assert_eq!(w.hash(), source.hash());
    }
    // A smaller, sparse save must also clear old alive bits and absent columns.
    source.despawn(new);
    let bytes = source.save();
    w.read(&mut bin::Decoder::for_load(&bytes[MAGIC.len()..], None))
        .unwrap();
    assert_eq!(w.entities().count(), 0);
    let mut cached = Observation::default();
    let mut oracle = Observation::default();
    w.observe(&mut cached);
    w.observe_uncached(&mut oracle, false);
    assert_eq!(cached.entries, oracle.entries);
    assert_eq!(w.save(), bytes);
}
#[derive(Default, crate::Data)]
struct AmbientResource(u32);
impl crate::Resource for AmbientResource {
    const NAME: &'static str = "AmbientResource";
    const AMBIENT: bool = true;
}

#[test]
fn randomized_cached_observations_equal_the_original_path() {
    use crate::{Ambient, Parent, Transform};
    for seed in [7, 103, 9001] {
        let mut script = Rng::new(seed);
        let mut w = World::new(60, seed);
        w.insert_resource(Count::default());
        w.insert_resource(AmbientResource::default());
        let mut entities = Vec::new();
        for i in 0..(storage::PAGE * 2 + 17) {
            let e = w.spawn_named(
                format!("entity-{i}"),
                (Transform::at(i as f32, 0.0, 0.0), Payload::default()),
            );
            if i % 3 == 1 {
                w.insert(e, Parent(entities[i - 1]));
            }
            if i % 7 == 0 {
                w.insert(e, Ambient);
            }
            entities.push(e);
        }
        w.propagate();
        let mut saved = w.save();
        let mut previous = Observation::default();
        w.observe_uncached(&mut previous, false);
        let mut coverage = [0; 16];
        for tick in 0..192 {
            w.begin_tick();
            for _ in 0..4 {
                let live: Vec<_> = w.entities().collect();
                let e = *script.pick(&live).unwrap();
                let action = script.range(0u32..coverage.len() as u32) as usize;
                let before_bytes = w.save();
                let before_generation = w.presentation_generation;
                let before_epoch = w.mutation_epoch();
                match action {
                    0 => {
                        w.spawn_named("new", (Transform::default(), Payload::default()));
                    }
                    1 => {
                        w.despawn(e);
                    }
                    2 => {
                        let old = w.spawn_named("reused", Transform::default());
                        w.despawn(old);
                        let new = w.spawn_named("reused", Transform::default());
                        assert_eq!(old.index(), new.index());
                        assert_ne!(old, new);
                    }
                    3 => {
                        if let Some(mut t) = w.get_mut::<Transform>(e) {
                            t.position.x += script.range(-4.0..4.0);
                        }
                    }
                    4 => {
                        drop(w.get_mut::<Transform>(e));
                    }
                    5 => {
                        let parent = *script.pick(&live).unwrap();
                        if parent.index() < e.index() {
                            w.insert(e, Parent(parent));
                        }
                    }
                    6 => {
                        w.remove::<Parent>(e);
                    }
                    7 => {
                        w.resource_mut::<Count>().0 = script.next_u32();
                    }
                    8 => {
                        w.resource_mut::<AmbientResource>().0 = script.next_u32();
                    }
                    9 => {
                        w.rng().next_u32();
                    }
                    10 => {
                        if w.has::<Ambient>(e) {
                            w.remove::<Ambient>(e);
                        } else {
                            w.insert(e, Ambient);
                        }
                    }
                    11 => {
                        w.insert(
                            e,
                            Payload {
                                text: format!("value-{}", script.next_u32()),
                                number: if tick % 3 == 0 { f64::NAN } else { -0.0 },
                            },
                        );
                    }
                    12 => {
                        w.remove::<Transform>(e);
                    }
                    13 => {
                        w.load(&saved).unwrap();
                    }
                    14 => {
                        w.presentation_generation += 1;
                    }
                    _ => {
                        for (_, t) in w.query::<&mut Transform>().iter().take(3) {
                            t.position.y += 0.25;
                        }
                    }
                }
                // An unwritten lease deliberately changes only the observation epoch.
                let mutated = if action == 4 {
                    w.mutation_epoch() != before_epoch
                } else {
                    w.save() != before_bytes || w.presentation_generation != before_generation
                };
                coverage[action] += usize::from(mutated);
            }
            if tick % 67 == 0 {
                w.id = WorldId(std::rc::Rc::new(()));
            }
            w.reap_orphans();
            w.step_clock();
            // Both before and after propagation: caching must preserve stale
            // global poses before propagate, then include descendant changes.
            for propagated in [false, true] {
                if propagated {
                    w.propagate();
                }
                for full in [false, true] {
                    let epoch = w.mutation_epoch();
                    let bytes = w.save();
                    let mut old = Observation::default();
                    let mut cached = Observation::default();
                    w.observe_uncached(&mut old, full);
                    w.observe_with_hash(&mut cached, full);
                    assert_eq!(
                        cached.entries, old.entries,
                        "seed={seed} tick={tick} propagated={propagated} full={full}"
                    );
                    assert_eq!(w.mutation_epoch(), epoch);
                    assert_eq!(w.save(), bytes);
                    let mut canonical = crate::hash::Hasher::default();
                    w.write(&mut canonical, false);
                    w.hash_cache.set(None);
                    assert_eq!(w.hash(), canonical.finish());
                    w.compare(&previous, &old);
                    let verdict = w.observation;
                    let reasons = w.changing();
                    w.compare(&previous, &cached);
                    assert!(w.observation == verdict);
                    assert_eq!(w.changing(), reasons);
                    if full && propagated {
                        previous = cached;
                    }
                }
            }
            if tick % 19 == 0 {
                saved = w.save();
            }
        }
        assert!(coverage.into_iter().all(|count| count > 0));
    }
}

thread_local! {
    static WRITES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}
#[derive(Default)]
struct Measured(u32);
impl crate::Component for Measured {
    const NAME: &'static str = "Measured";
}
impl crate::Data for Measured {
    fn write(&self, w: &mut dyn Writer) {
        WRITES.set(WRITES.get() + 1);
        self.0.write(w);
    }
    fn read(&mut self, r: &mut dyn Reader) -> Result<(), DataError> {
        self.0.read(r)
    }
}
fn writes(w: &World, sample: &mut Observation) -> usize {
    WRITES.set(0);
    w.observe(sample);
    WRITES.get()
}

#[test]
fn only_dirty_pages_are_hashed_and_context_changes_reset_them() {
    let mut w = World::new(60, 0);
    let entities: Vec<_> = (0..storage::PAGE * 2 + 1)
        .map(|_| w.spawn(Measured(0)))
        .collect();
    let mut sample = Observation::default();
    assert_eq!(writes(&w, &mut sample), entities.len());
    assert_eq!(writes(&w, &mut sample), 0);
    drop(w.get_mut::<Measured>(entities[0]));
    assert_eq!(writes(&w, &mut sample), storage::PAGE);
    w.get_mut::<Measured>(*entities.last().unwrap()).unwrap().0 = 1;
    assert_eq!(writes(&w, &mut sample), 1);
    w.insert(entities[0], crate::Ambient);
    assert_eq!(writes(&w, &mut sample), storage::PAGE - 1);
    assert_eq!(writes(&w, &mut sample), 0);
    w.presentation_generation += 1;
    assert_eq!(writes(&w, &mut sample), entities.len() - 1);
    w.id = WorldId(std::rc::Rc::new(()));
    assert_eq!(writes(&w, &mut sample), entities.len() - 1);
    let bytes = w.save();
    w.load(&bytes).unwrap();
    assert_eq!(writes(&w, &mut sample), entities.len() - 1);
    // Free an entire page, then allocate it again with new incarnations.
    for &e in &entities[storage::PAGE..storage::PAGE * 2] {
        w.despawn(e);
    }
    assert_eq!(writes(&w, &mut sample), 0);
    for _ in 0..storage::PAGE {
        w.spawn(Measured(0));
    }
    assert_eq!(writes(&w, &mut sample), storage::PAGE);
    assert_eq!(writes(&w, &mut sample), 0);
    let guard = w.get_mut::<Measured>(entities[1]).unwrap();
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| w.observe(&mut sample))).is_err()
    );
    drop(guard);
    assert_eq!(writes(&w, &mut sample), storage::PAGE - 1);
}

#[test]
fn global_pages_follow_ambient_ancestors_across_page_boundaries() {
    use crate::{Ambient, Parent, Transform};
    let mut w = World::new(60, 0);
    let root = w.spawn_named("root", (Transform::default(), Ambient));
    for _ in 1..storage::PAGE {
        w.spawn(());
    }
    let child = w.spawn_named("child", Parent(root));
    for _ in 1..storage::PAGE {
        w.spawn(());
    }
    w.spawn_named("grandchild", Parent(child));
    w.propagate();
    let mut before = Observation::default();
    let mut after = Observation::default();
    w.observe(&mut before);
    let generations: Vec<_> = (0..3)
        .map(|page| w.hierarchy.page_generation(page))
        .collect();
    w.get_mut::<Transform>(root).unwrap().position.x = 3.0;
    w.observe(&mut after);
    assert_eq!(
        before.entries, after.entries,
        "globals wait for propagation"
    );
    w.propagate();
    assert_ne!(w.hierarchy.page_generation(1), generations[1]);
    assert_ne!(w.hierarchy.page_generation(2), generations[2]);
    w.observe(&mut after);
    w.compare(&before, &after);
    assert_eq!(w.changing(), ["child.global", "grandchild.global"]);
    let mut oracle = Observation::default();
    w.observe_uncached(&mut oracle, false);
    assert_eq!(after.entries, oracle.entries);
    let generations: Vec<_> = (0..3)
        .map(|page| w.hierarchy.page_generation(page))
        .collect();
    w.propagate();
    assert_eq!(
        generations,
        (0..3)
            .map(|page| w.hierarchy.page_generation(page))
            .collect::<Vec<_>>()
    );
}

#[test]
fn newly_available_globals_invalidate_equal_pose_cache_entries() {
    use crate::{Parent, Transform};
    let mut w = World::new(60, 0);
    let root = w.spawn(Transform::default());
    let mut child = w.spawn_named("child", Parent(root));
    w.spawn(Parent(root)); // Keep propagation active while child's edge is absent.
    w.propagate();
    let mut cached = Observation::default();
    w.observe(&mut cached);
    for recycle in [true, false] {
        if recycle {
            let old = child;
            w.despawn(child);
            child = w.spawn_named("child", Parent(root));
            assert_eq!(child.index(), old.index());
        } else {
            w.remove::<Parent>(child);
            w.propagate();
            w.insert(child, Parent(root));
        }
        assert!(w.global(child).is_none());
        w.observe(&mut cached);
        assert!(!cached
            .entries
            .iter()
            .any(|row| row.0 == 1 && row.2 == child));
        w.propagate();
        assert!(w.global(child).is_some());
        w.observe(&mut cached);
        let mut old = Observation::default();
        w.observe_uncached(&mut old, false);
        assert_eq!(cached.entries, old.entries);
        assert!(cached
            .entries
            .iter()
            .any(|row| row.0 == 1 && row.2 == child));
    }
}

#[path = "observation_invalidation.rs"]
mod invalidation;
