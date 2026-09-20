//! These modules deliberately import only the public crate surface.
use exact_world::*;

mod reload {
    use exact_world::*;
    pub fn edit(
        world: &World,
        entity: Option<Entity>,
        name: &str,
        patch: &impl Data,
    ) -> Result<Candidate, DataError> {
        let mut candidate = world.candidate()?;
        candidate.edit(entity, name, &bin::to_vec(patch)?)?;
        Ok(candidate)
    }
}
mod adapter {
    use exact_world::*;
    pub fn inspect(world: &World, entity: Option<Entity>) -> Result<String, DataError> {
        let mut out = json::Encoder::default();
        world.visit(entity, &mut out)?;
        out.finish()
    }
    pub fn work(world: &World) -> (Readiness, Option<bool>) {
        (world.readiness(), world.observation())
    }
}
#[derive(Default, Component)]
struct Controller {
    speed: u32,
    lives: u32,
}
#[derive(Default, Resource)]
struct Assets {
    loaded: bool,
}
#[derive(Default, Component)]
struct Blob(Vec<u32>);
#[derive(Default, Data)]
struct Speed {
    speed: u32,
}

#[test]
fn external_reload_edits_erased_candidates_and_adapter_visits_both_roles() {
    let mut world = World::new(60, 0);
    world.register::<Controller>().unwrap();
    world.register_resource::<Assets>().unwrap();
    world.insert_resource(Assets { loaded: false }).unwrap();
    let player = world.spawn(Controller { speed: 3, lives: 7 }).unwrap();
    world.work("assets", Work::Pending).unwrap();
    let original = world.save().unwrap();
    let candidate = reload::edit(&world, Some(player), "Controller", &Speed { speed: 9 }).unwrap();
    assert_eq!(world.save().unwrap(), original);
    assert_eq!(
        candidate.world().get::<Controller>(player).unwrap().speed,
        9
    );
    assert_eq!(
        candidate.world().get::<Controller>(player).unwrap().lives,
        7
    );
    candidate.commit(&mut world).unwrap();
    assert!(adapter::inspect(&world, Some(player))
        .unwrap()
        .contains("\"speed\":9"));
    assert_eq!(
        adapter::inspect(&world, None).unwrap(),
        "{\"Assets\":{\"loaded\":false}}"
    );
    assert!(matches!(
        adapter::work(&world),
        (Readiness::Pending(_), None)
    ));
    let candidate = reload::edit(&world, None, "Assets", &Assets { loaded: true }).unwrap();
    candidate.commit(&mut world).unwrap();
    assert!(world.resource::<Assets>().loaded);
    let sample = world.sample().unwrap();
    assert_eq!(sample.components, 1);
    assert!(sample.bytes > 0 && sample.hash != 0);
    assert!(adapter::inspect(&world, Some(Entity::default())).is_err());
}

#[test]
fn failed_candidate_edits_and_ownership_cycles_never_change_live_world() {
    let mut w = World::new(60, 0);
    w.register::<Controller>().unwrap();
    let parent = w.spawn(()).unwrap();
    let child = w.spawn(Controller::default()).unwrap();
    w.set_parent(child, Some(parent)).unwrap();
    let original = w.save().unwrap();
    let mut candidate = w.candidate().unwrap();
    let mut patch = bin::to_vec(&Speed { speed: 10 }).unwrap();
    patch.pop();
    assert!(candidate.edit(Some(child), "Controller", &patch).is_err());
    assert!(candidate
        .edit(
            Some(child),
            "Controller",
            &bin::to_vec(&Speed::default()).unwrap()
        )
        .is_err());
    assert!(candidate.commit(&mut w).is_err());
    assert_eq!(w.save().unwrap(), original);
    #[derive(Default, Data)]
    struct ParentPatch(Entity);
    assert!(reload::edit(&w, Some(child), "Parent", &ParentPatch(child)).is_err());
    assert_eq!(w.save().unwrap(), original);
}

#[test]
fn explicit_observation_and_external_visitors_refuse_large_payloads() {
    let mut w = World::new(60, 0);
    w.register::<Blob>().unwrap();
    let e = w.spawn(Blob(vec![1; 9 * 1024 * 1024])).unwrap();
    assert!(w.sample().unwrap_err().message.contains("budget"));
    assert!(adapter::inspect(&w, Some(e)).is_err());
    w.get_mut::<Blob>(e).unwrap().0.truncate(1);
    let sample = w.sample().unwrap();
    assert_eq!(sample.components, 1);
    assert!(adapter::inspect(&w, Some(e)).unwrap().contains("bytes"));
}

#[test]
fn resource_revisions_are_local_and_candidates_invalidate_replacement() {
    #[derive(Default, Resource)]
    struct Other(u32);
    let mut w = World::new(60, 0);
    w.register_resource::<Assets>().unwrap();
    w.register_resource::<Other>().unwrap();
    assert_eq!(w.resource_revision::<Other>(), None);
    w.insert_resource(Assets::default()).unwrap();
    w.insert_resource(Other::default()).unwrap();
    let a = w.resource_revision::<Assets>().unwrap();
    let b = w.resource_revision::<Other>().unwrap();
    w.resource_mut::<Other>().0 = 9;
    assert_eq!(w.resource_revision::<Assets>(), Some(a));
    assert_ne!(w.resource_revision::<Other>(), Some(b));
    let candidate = reload::edit(&w, None, "Assets", &Assets { loaded: true }).unwrap();
    let replacement = w.replacement();
    candidate.commit(&mut w).unwrap();
    assert_ne!(w.replacement(), replacement);
    assert!(w.resource::<Assets>().loaded);
    assert_eq!(w.resource::<Other>().0, 9);
}

#[test]
fn settle_at_max_entities_counts_only_live_components_and_slots() {
    #[derive(Default, Component)]
    struct A;
    #[derive(Default, Component)]
    struct B;
    #[derive(Default, Component)]
    struct C;
    #[derive(Default, Component)]
    struct D;
    #[derive(Default, Component)]
    struct E(u32);
    struct Maximal;
    impl Game for Maximal {
        const ID: &'static str = "maximal-observation";
        type Args = ();
        fn register(w: &mut World, _: args::SetupArgs<'_, ()>) -> Result<(), DataError> {
            w.register::<A>()?
                .register::<B>()?
                .register::<C>()?
                .register::<D>()?
                .register::<E>()?;
            Ok(())
        }
        fn setup(w: &mut World, _: &()) {
            for _ in 0..MAX_ENTITIES {
                w.spawn(()).unwrap();
            }
            let e = w.entity_at(MAX_ENTITIES - 1).unwrap();
            w.insert(e, A).unwrap();
            w.insert(e, B).unwrap();
            w.insert(e, C).unwrap();
            w.insert(e, D).unwrap();
            w.insert(e, E(7)).unwrap();
        }
        fn tick(_: &mut World, _: &Input, _: &()) {}
    }
    let mut sim = Sim::<Maximal>::new(()).unwrap();
    assert_eq!(sim.world().sample().unwrap().components, 5);
    assert_eq!(sim.settle(1).unwrap(), 1);
    assert!(sim.world().quiescent());
    let hash = sim.world().sample().unwrap().hash;
    let e = sim.world().entity_at(MAX_ENTITIES - 1).unwrap();
    sim.world().get_mut::<E>(e).unwrap().0 += 1;
    assert_ne!(sim.world().sample().unwrap().hash, hash);
    let w = sim.world_mut();
    for slot in (0..MAX_ENTITIES - 1).rev() {
        let e = w.entity_at(slot).unwrap();
        w.insert(e, A).unwrap();
        w.insert(e, B).unwrap();
        w.insert(e, C).unwrap();
        w.insert(e, D).unwrap();
    }
    assert!(w.sample().unwrap_err().message.contains("probe budget")); // 1,000,001
    w.remove::<E>(e).unwrap();
    assert_eq!(w.sample().unwrap().components, MAX_ENTITIES as u64 * 4); // exactly 1,000,000
    assert_eq!(sim.settle(1).unwrap(), 1);
}
