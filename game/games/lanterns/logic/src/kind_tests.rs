use super::*;
use exact_game::Sim;

fn game() -> Sim<Lanterns> {
    game_sim(Options {
        scene: bake_scene().content,
        seed: 1_041_003,
        started: true,
        sound: true,
        ..Default::default()
    })
}
fn generations<C: Component>(w: &World) -> Vec<u64> {
    w.pages::<C>().iter().map(|p| p.generation).collect()
}
#[test]
fn idle_tick_does_not_dirty_previously_read_only_columns() {
    let mut sim = game();
    let w = sim.world();
    let before = (
        generations::<Lantern>(w),
        generations::<Player>(w),
        generations::<Parent>(w),
        generations::<Mesh>(w),
        generations::<AudioListener>(w),
        generations::<DirectionalLight>(w),
    );
    sim.run(1000.0 / 60.0);
    assert_eq!(sim.world().tick(), 1);
    let w = sim.world();
    assert_eq!(
        before,
        (
            generations::<Lantern>(w),
            generations::<Player>(w),
            generations::<Parent>(w),
            generations::<Mesh>(w),
            generations::<AudioListener>(w),
            generations::<DirectionalLight>(w)
        )
    );
}
#[test]
fn partial_lantern_without_transform_still_counts_and_updates() {
    let mut sim = game();
    let w = sim.world_mut();
    let e = w.named("lantern-1").unwrap();
    w.remove::<Transform>(e);
    let id = w.bind::<Lamp>(e).unwrap();
    w.edit(id, |l| {
        l.lantern.lit = true;
        l.lantern.glow.set_target(w.now(), 1.0);
    });
    assert_eq!(w.rows::<Lamp>().count(), 12);
    assert_eq!(w.rows::<Lightable>().count(), 11);
    sim.run(500.0);
    assert!(
        sim.world()
            .get::<PointLight>("lantern-1/bulb")
            .unwrap()
            .intensity
            > 0.0
    );
}
#[test]
fn saved_child_replacement_fails_identically_live_and_restored() {
    fn failure(sim: &mut Sim<Lanterns>) -> String {
        let error =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| sim.run(1000.0 / 60.0)))
                .unwrap_err();
        if let Some(s) = error.downcast_ref::<String>() {
            s.clone()
        } else {
            error.downcast_ref::<&str>().unwrap().to_string()
        }
    }
    let mut live = game();
    let w = live.world_mut();
    let e = w.named("lantern-1/bulb").unwrap();
    let parent = w.get::<Parent>(e).unwrap().0;
    w.despawn(e);
    w.spawn_named(
        "lantern-1/bulb",
        (Parent(parent), Material::default(), PointLight::default()),
    );
    let bytes = live.save().unwrap();
    let mut restored = game();
    restored.restore(&bytes).unwrap();
    assert_eq!(live.world().save(), restored.world().save());
    let error = failure(&mut live);
    assert_eq!(error, failure(&mut restored));
    for expected in ["Lamp", "lantern-1", "lantern.bulb", "generation"] {
        assert!(error.contains(expected), "{error}");
    }
    assert_eq!(live.world().save(), restored.world().save());
}
#[test]
fn save_load_every_tick_matches_uninterrupted_with_controller_and_declared_assets() {
    let mut live = game();
    let mut restored = game();
    live.agent(r#"{"op":"clock","owner":"agent","now":0}"#);
    restored.agent(r#"{"op":"clock","owner":"agent","now":0}"#);
    live.key_down("KeyW");
    restored.key_down("KeyW");
    for tick in 0..180 {
        if tick == 45 {
            live.tap("Space");
            restored.tap("Space");
        }
        if tick == 90 {
            live.tap("KeyE");
            restored.tap("KeyE");
        }
        live.agent(r#"{"op":"clock","ticks":1}"#);
        restored.agent(r#"{"op":"clock","ticks":1}"#);
        assert!(
            live.save().unwrap() == restored.save().unwrap(),
            "continuation save mismatch at tick {tick}; world bytes equal: {}; live hash: {:016x}; restored hash: {:016x}",
            live.world().save() == restored.world().save(), live.world().hash(), restored.world().hash()
        );
        let bytes = restored.save().unwrap();
        restored.restore(&bytes).unwrap();
        assert!(
            live.world().save() == restored.world().save(),
            "world mismatch at tick {tick}"
        );
    }
    assert!(live.world().get::<Transform>("player").unwrap().position.z < SPAWN.z - 5.0);
}
