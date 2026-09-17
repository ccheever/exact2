use exact_game::{Args, Clock, Environment, Game, Input, Material, Mesh, Sim, Transform, World};

#[test]
fn renderer_revisions_see_same_tick_edits_without_entering_the_hash() {
    let mut w = World::new(60, 0);
    let e = w.spawn((Transform::default(), Mesh::Cube));
    let hash = w.hash();
    let revision = w.revision::<Transform>();
    let membership = w.membership::<Transform>();
    let live = w.entities_revision();
    drop(w.get_mut::<Transform>(e));
    assert_ne!(revision, w.revision::<Transform>());
    assert_eq!(w.changed::<Transform>(), 0);
    assert_eq!(membership, w.membership::<Transform>());
    assert_eq!(hash, w.hash());
    w.despawn(e);
    let replacement = w.spawn((Transform::default(), Mesh::Cube));
    assert_eq!(e.index(), replacement.index());
    assert_ne!(live, w.entities_revision());
    assert_ne!(membership, w.membership::<Transform>());
}
#[test]
fn whole_page_float_views_match_bytes_and_environment_roundtrips() {
    let mut w = World::new(60, 0);
    w.register_scene();
    let e = w.spawn((Transform::at(1., 2., 3.), Material::rgb(0.2, 0.3, 0.4)));
    let t = w.pages::<Transform>();
    let p = t.iter().next().unwrap();
    assert_eq!(&p.floats()[..3], &[1., 2., 3.]);
    assert_eq!(p.floats().len(), exact_game::PAGE * 10);
    assert!(p.floats()[10..].iter().all(|v| *v == 0.));
    drop(t);
    assert!(w.try_resource::<Environment>().is_none());
    w.insert_resource(Environment::default());
    let bytes = w.save();
    let mut restored = World::new(60, 0);
    restored.register_scene();
    restored.load(&bytes).unwrap();
    assert_eq!(w.hash(), restored.hash());
    assert_eq!(*restored.resource::<Environment>(), Environment::default());
    assert!(restored.contains(e));
}
struct Moves;
impl Game for Moves {
    const ID: &'static str = "observer-test";
    fn setup(w: &mut World, _: &Args) -> Result<(), String> {
        w.spawn(Transform::default());
        Ok(())
    }
    fn tick(w: &mut World, _: &Input) {
        for (_, t) in w.query::<&mut Transform>().iter() {
            t.position.x += 1.;
        }
    }
}
#[test]
fn agent_and_timed_bind_observe_final_propagated_ticks_and_restore_generation() {
    let mut sim = Sim::<Moves>::new(&[]).unwrap();
    sim.advance(0., Clock::Seekable);
    let mut ticks = Vec::new();
    sim.agent_with(r#"{"op":"clock","now":1000}"#, |w, left| {
        if left < 2 {
            ticks.push(w.tick());
        }
    });
    assert_eq!(ticks, [59, 60]);
    ticks.clear();
    sim.bind_with(&[], Some(2000.), |w, left| {
        if left < 2 {
            ticks.push(w.tick());
        }
    })
    .unwrap();
    assert_eq!(ticks, [119, 120]);
    let generation = sim.generation();
    let saved = sim.save();
    sim.restore(&saved).unwrap();
    assert_ne!(generation, sim.generation());
}
