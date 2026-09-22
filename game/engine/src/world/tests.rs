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
        .dependencies
        .insert("ready.model".into(), vec!["ready.tex".into()]);
    world.assets.retired.push("retired.model".into());
    world.assets.refusal = Some(("refused.model".into(), "reason".into()));
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
        assert_eq!(world.assets.refusal.as_ref().unwrap().1, "reason");
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
    let mut out = bin::Encoder::default();
    out.begin_struct();
    out.field("state");
    out.begin_struct();
    out.field("slots");
    out.begin_seq(crate::data::MAX_LOAD_ENTITIES + 1);
    let mut bytes = MAGIC.to_vec();
    bytes.extend(out.finish());
    // Enough input to satisfy the codec's minimum byte count, without constructing entities.
    bytes.resize(bytes.len() + crate::data::MAX_LOAD_ENTITIES + 1, 0);
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
