use super::*;

#[test]
fn fresh_membership_tracks_first_poses_recycling_and_tick_boundaries() {
    let mut w = World::new(60, 0);
    let a = w.spawn(());
    let b = w.spawn(());
    let c = w.spawn(());
    w.begin_tick();
    assert!([a, b, c].into_iter().all(|e| !w.is_fresh(e)));
    w.insert(b, Transform::default());
    for _ in 0..8 {
        w.teleport(c, Transform::default());
    }
    assert_eq!(w.fresh(), [b, c]);
    w.despawn(b);
    assert!(w.is_fresh(b));
    assert!(!w.is_fresh(Entity {
        index: b.index,
        generation: b.generation + 1,
    }));
    let replacement = w.spawn(Transform::default());
    assert_eq!(replacement.index(), b.index());
    w.teleport(a, Transform::default());
    w.teleport(replacement, Transform::default());
    assert_eq!(w.fresh(), [b, c, replacement, a]);
    for e in [a, b, c, replacement, Entity::default()] {
        assert_eq!(w.is_fresh(e), w.fresh().contains(&e));
    }
    let saved = w.save();
    let hash = w.hash();
    w.begin_tick();
    assert!([a, b, c, replacement].into_iter().all(|e| !w.is_fresh(e)));
    w.teleport(replacement, Transform::default());
    assert_eq!(w.fresh(), [replacement]);
    assert!(!w.is_fresh(b));
    assert_eq!(w.save(), saved);
    assert_eq!(w.hash(), hash);
    w.load(&saved).unwrap();
    assert!(w.fresh().is_empty());
    assert!([a, b, c, replacement].into_iter().all(|e| !w.is_fresh(e)));
    assert_eq!(w.save(), saved);
}

#[test]
fn fresh_flag_uses_slot_padding_and_stays_out_of_saved_data() {
    #[derive(Default, Data)]
    struct PreviousSlot {
        generation: u32,
        alive: bool,
        name: Option<String>,
    }
    assert_eq!(size_of::<Slot>(), size_of::<PreviousSlot>());
    let before = PreviousSlot {
        generation: 7,
        alive: true,
        name: Some("hero".into()),
    };
    let after = Slot {
        generation: 7,
        alive: true,
        name: Some("hero".into()),
        fresh: true,
    };
    let bytes = bin::to_vec(&before);
    assert_eq!(bytes, bin::to_vec(&after));
    assert!(!bin::from_slice::<Slot>(&bytes).unwrap().fresh);
}

#[test]
fn registration_links_only_declared_storage_kinds() {
    #[derive(Default, crate::Component)]
    struct Both {
        n: u32,
    }
    impl crate::Resource for Both {
        const NAME: &'static str = "Both";
    }
    let mut source = World::new(60, 0);
    source.spawn(Both { n: 9 });
    let components = source.save();
    let mut target = World::new(60, 0);
    target.register_resource::<Both>();
    assert_eq!(target.load(&components).unwrap_err().to_string(),
        "World: `Both` is registered as a resource; call world.register::<Both>() in Game::register to load components");
    target.register::<Both>();
    target.load(&components).unwrap();
    assert_eq!(target.save(), components);
    source.insert_resource(Both { n: 7 });
    let both = source.save();
    let mut components_only = World::new(60, 0);
    components_only.register::<Both>();
    assert_eq!(components_only.load(&both).unwrap_err().to_string(),
        "World: `Both` is registered as a component; call world.register_resource::<Both>() in Game::register to load resources");
    target.load(&both).unwrap();
    assert_eq!(target.save(), both);
    assert_eq!(target.resource::<Both>().n, 7);
}
use crate::{Component, Transform, Vec3};

#[test]
fn loading_moves_asset_ownership_only_after_all_validation_succeeds() {
    use crate::asset::AssetState;
    let mut world = World::new(60, 0);
    world.spawn_named("retained", Transform::default());
    world.assets.request("pending.model");
    world
        .assets
        .models
        .insert("ready.model".into(), crate::asset::Model::default().into());
    world
        .assets
        .states
        .insert("ready.model".into(), AssetState::Loaded);
    world.assets.declared.insert("ready.model".into());
    world.assets.required.insert("ready.model".into());
    world.assets.requested.insert("pending.model".into());
    world.assets.prepared.insert("ready.model".into());
    world.assets.redelivery.insert("pending.model".into());
    world
        .assets
        .set_dependencies("ready.model", vec!["ready.tex".into()]);
    world.assets.retired.push("retired.model".into());
    let saved = world.save();
    let names: Vec<_> = world.assets.states.keys().map(|n| n.as_ptr()).collect();
    let check = |world: &World| {
        assert_eq!(world.save(), saved);
        assert_eq!(
            world
                .assets
                .states
                .keys()
                .map(|n| n.as_ptr())
                .collect::<Vec<_>>(),
            names
        );
        assert!(world.model("ready.model").is_some());
        assert_eq!(world.loading().collect::<Vec<_>>(), Vec::<&str>::new());
        assert!(world.assets.requested.contains("pending.model"));
        assert!(world.assets.prepared.contains("ready.model"));
        assert!(world.assets.redelivery.contains("pending.model"));
        assert_eq!(
            world.assets.dependencies.get("ready.model").unwrap(),
            &["ready.tex"]
        );
        assert_eq!(world.assets.retired, ["retired.model"]);
    };
    let mut trailing = saved.clone();
    trailing.push(0);
    let mut cyclic = World::new(60, 0);
    let entity = cyclic.spawn(Transform::default());
    cyclic.insert(entity, Parent(entity));
    world.register::<Parent>();
    for invalid in [
        b"bad".to_vec(),
        saved[..saved.len() - 1].to_vec(),
        trailing,
        cyclic.save(),
    ] {
        assert!(world.load(&invalid).is_err());
        check(&world);
    }
    world.load(&saved).unwrap();
    check(&world);
    let retained =
        std::sync::Arc::downgrade(&world.assets.models.get("ready.model").unwrap().model);
    let mut branch = world.assets.clone();
    branch
        .states
        .insert("ready.model".into(), AssetState::Pending);
    branch.models.remove("ready.model");
    assert_eq!(
        world.assets.states.get("ready.model"),
        Some(&AssetState::Loaded)
    );
    assert!(world.model("ready.model").is_some());
    drop(world);
    assert!(
        retained.upgrade().is_none(),
        "retiring the final owner releases its model"
    );
}

#[derive(Default, Component)]
struct Velocity(Vec3);
fn churn(w: &mut World, ticks: u32) {
    for _ in 0..ticks {
        if w.rng().chance(0.6) {
            let x = w.rng().range(-10.0..10.0);
            w.spawn_named(
                "particle",
                (Transform::at(x, 0.0, 0.0), Velocity(Vec3::new(1.0, x, 0.0))),
            );
        }
        let dt = w.dt();
        for (_, (t, v)) in w.query::<(&mut Transform, &Velocity)>().iter() {
            t.position += v.0 * dt;
        }
        if w.rng().chance(0.3) {
            let entities: Vec<_> = w.entities().collect();
            let selected = w.rng().pick(&entities).copied();
            if let Some(e) = selected {
                w.despawn(e);
            }
        }
        w.step_clock();
    }
}
#[test]
fn replay_equivalence() {
    let mut straight = World::new(60, 42);
    churn(&mut straight, 600);
    let mut first = World::new(60, 42);
    churn(&mut first, 300);
    let mut restored = World::new(60, 0);
    restored.register::<Transform>().register::<Velocity>();
    restored.load(&first.save()).unwrap();
    assert_eq!(first.hash(), restored.hash());
    churn(&mut restored, 300);
    assert_eq!(straight.tick(), 600);
    assert_eq!(straight.hash(), restored.hash());
    assert_eq!(straight.save(), restored.save());
}
#[test]
fn hierarchy_and_fresh_tick() {
    let mut w = World::new(60, 0);
    let child = w.spawn((Transform::at(1.0, 0.0, 0.0),));
    let middle = w.spawn((Transform::at(0.0, 2.0, 0.0),));
    let root = w.spawn((Transform::at(0.0, 0.0, 3.0),));
    w.insert(child, Parent(middle));
    w.insert(middle, Parent(root));
    w.propagate();
    let old = w.global(child).unwrap();
    assert_eq!(old.translation, Vec3::new(1.0, 2.0, 3.0).into());
    let hash = w.hash();
    w.propagate();
    assert_eq!(hash, w.hash());
    w.step_clock();
    w.get_mut::<Transform>(root).unwrap().position.x = 10.0;
    w.propagate();
    assert_eq!(w.global(child).unwrap().translation.x, 11.0);
    w.propagate();
    assert_eq!(w.global(child).unwrap().translation.x, 11.0);
    w.begin_tick();
    assert!(w.fresh().is_empty());
    w.teleport(root, Transform::at(100.0, 0.0, 0.0));
    assert_eq!(w.fresh(), [root]);
    w.propagate();
    assert_eq!(w.global(child).unwrap().translation.x, 101.0);
    assert!(w.despawn(root));
    assert_eq!(w.len(), 2);
    w.reap_orphans();
    assert!(w.is_empty());
}

#[test]
fn save_entity_limit_is_checked_before_reserving_slots() {
    // A columnar entity table: no index runs or names, one empty shape, and a row
    // count past the limit, which must refuse before any slot is reserved.
    let mut table = vec![0, 0, 1, 0];
    let mut n = crate::data::MAX_LOAD_ENTITIES as u64 + 1;
    while n >= 128 {
        table.push(n as u8 | 128);
        n >>= 7;
    }
    table.push(n as u8);
    let mut out = bin::Encoder::default();
    out.begin_struct();
    out.field("state");
    out.begin_struct();
    out.field("slots");
    out.bytes(crate::data::Bulk::U8(&table));
    let mut bytes = MAGIC.to_vec();
    bytes.extend(out.finish());
    let mut w = World::new(60, 0);
    let before = w.save();
    let err = w.load(&bytes).unwrap_err().to_string();
    assert!(err.contains("slots") && err.contains("limit"), "{err}");
    assert_eq!(w.save(), before);
}

#[test]
fn singleton_load_claims_inline_allocation_before_factory() {
    use std::sync::atomic::{AtomicBool, Ordering};
    static MADE: AtomicBool = AtomicBool::new(false);
    #[derive(Default, crate::Resource)]
    struct LargeInline {
        #[data(skip)]
        _bytes: [[[u8; 32]; 32]; 32],
    }
    let mut source = World::new(60, 0);
    source.insert_resource(LargeInline::default());
    let bytes = source.save();
    for remaining in [4096, 65536] {
        let mut target = World::new(60, 0);
        target.register_resource::<LargeInline>();
        target
            .registry
            .get_mut("LargeInline")
            .unwrap()
            .make_resource = Some(|name, epoch| {
            MADE.store(true, Ordering::SeqCst);
            storage::make_cell::<LargeInline>(name, epoch)
        });
        MADE.store(false, Ordering::SeqCst);
        let mut reader = bin::Decoder::new(&bytes[MAGIC.len()..]);
        reader
            .claim(crate::data::MAX_LOAD_BYTES - remaining)
            .unwrap();
        let result = target.read(&mut reader);
        if remaining == 4096 {
            let error = result.expect_err("inline singleton bypassed allocation budget");
            assert!(error.to_string().contains("budget"), "{error}");
            assert!(error.to_string().contains("LargeInline"), "{error}");
            assert!(!MADE.load(Ordering::SeqCst), "factory ran before claim");
        } else {
            result.unwrap();
            reader.finish().unwrap();
            assert!(MADE.load(Ordering::SeqCst));
            assert_eq!(source.save(), target.save());
            assert_eq!(source.hash(), target.hash());
        }
    }
}

// The paged hash must equal a from-scratch hash after every kind of write, and a
// seek's observation must see every change a full read would.
#[test]
fn paged_hash_and_observation_follow_every_write_path() {
    #[derive(Default, Clone, Data)]
    struct Score(u32);
    impl Resource for Score {
        const NAME: &'static str = "Score";
    }
    #[derive(Default, Clone, Data)]
    struct Tag(String);
    impl Component for Tag {
        const NAME: &'static str = "Tag";
    }
    let fresh_hash = |w: &World| {
        let mut scratch = w.registered_scratch();
        scratch.load(&w.save()).unwrap();
        scratch.hash()
    };
    let mut w = World::new(60, 4);
    w.insert_resource(Score(0));
    let mut live = vec![];
    for i in 0..1_500 {
        live.push(w.spawn((Transform::at(i as f32, 0., 0.), Tag(format!("t{}", i % 3)))));
    }
    let mut seed = 0x9e37_79b9_7f4a_7c15u64;
    let mut next = move |n: u64| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed % n
    };
    let mut before = Observation::default();
    let mut after = Observation::default();
    for round in 0..400 {
        w.observe(&mut before);
        let pick = live[next(live.len() as u64) as usize];
        let op = next(10);
        match op {
            0 => w.get_mut::<Transform>(pick).unwrap().position.y += 1.0,
            1 => {
                for (_, t) in w.query::<&mut Transform>().iter().take(3) {
                    t.scale.x += 0.5;
                }
            }
            2 => {
                for mut tag in w.query::<&mut Tag>().into_iter().skip(700).take(2) {
                    tag.0.push('!');
                }
            }
            3 => {
                w.despawn(pick);
                live.retain(|e| *e != pick);
                live.push(w.spawn(Tag("new".into())));
            }
            4 => {
                w.insert(pick, crate::Ambient);
            }
            5 => {
                w.remove::<Tag>(pick);
            }
            6 => w.resource_mut::<Score>().0 += 1,
            7 => {
                w.rng().next_u32();
            }
            8 => {
                let _ = w.get_mut::<Tag>(pick);
            }
            _ => w.teleport(pick, Transform::at(-1., -2., -3.)),
        }
        w.propagate();
        assert_eq!(w.hash(), fresh_hash(&w), "round {round} op {op}");
        w.observe(&mut after);
        w.compare(&before, &after);
        let observed = !w.has::<crate::Ambient>(pick);
        if op == 6 || op == 7 || (op == 0 && observed) {
            assert!(
                w.observation == ObservationState::Changing,
                "round {round} op {op}"
            );
        }
        if op == 8 {
            assert!(
                w.observation == ObservationState::Still,
                "round {round}: {:?}",
                w.changing
            );
        }
    }
}
