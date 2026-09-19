use exact_game::{
    audio::{Sounds, Synth, Voices},
    Game, Input, Sim, World,
};
use exact_game_render::WorldSurface;
use exact_gpu::{Restore, Surface};
struct Tone;
impl Game for Tone {
    const ID: &'static str = "restore-mode";
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        w.register_audio();
        w.resource_mut::<Sounds>().add("tone", Synth::sine(880.));
    }
    fn tick(_: &mut World, _: &Input, _: &()) {}
}
exact_game_render::module!(Tone, audio);
#[test]
fn carry_overlays_fresh_definitions_open_preserves_saved_and_runtime_names() {
    let mut old = Sim::<Tone>::new(()).unwrap();
    old.world_mut()
        .resource_mut::<Sounds>()
        .add("tone", Synth::sine(220.));
    old.world_mut()
        .resource_mut::<Sounds>()
        .add("runtime", Synth::sine(330.));
    old.world().play("tone").start();
    let bytes = old.save().unwrap();
    for mode in [Restore::Open, Restore::Carry] {
        let mut surface = WorldSurface::<Tone, GameAudio>::default();
        surface.bind(&[], None).unwrap();
        surface.restore(&bytes, mode).unwrap();
        let world = surface.sim().unwrap().world();
        assert_eq!(
            world.resource::<Sounds>().0["tone"].hz,
            if mode == Restore::Carry { 880. } else { 220. }
        );
        assert_eq!(world.resource::<Sounds>().0["runtime"].hz, 330.);
        assert_eq!(world.resource::<Voices>().voices[0].synth.hz, 220.);
    }
}

struct Animated;
impl Game for Animated {
    const ID: &'static str = "carry-animation";
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        w.spawn_named(
            "clip",
            (
                exact_game::Transform::default(),
                exact_game::Animation::play("walk"),
            ),
        );
        w.spawn_named(
            "animator",
            (
                exact_game::Transform::default(),
                exact_game::Animator::new([exact_game::State::clip("idle", "idle")]),
            ),
        );
        w.spawn_named(
            "blend",
            (
                exact_game::Transform::default(),
                exact_game::Blend::across([(0., "idle"), (1., "walk")]),
            ),
        );
    }
    fn tick(_: &mut World, _: &Input, _: &()) {}
}
#[test]
fn carry_adds_animation_components_and_replaces_controller_kinds() {
    for conflict in [false, true] {
        let mut old = Sim::<Animated>::new(()).unwrap();
        for name in ["clip", "animator", "blend"] {
            let e = old.world().named(name).unwrap();
            old.world_mut().remove::<exact_game::Animation>(e);
            old.world_mut().remove::<exact_game::Animator>(e);
            old.world_mut().remove::<exact_game::Blend>(e);
            if conflict {
                if name == "clip" {
                    old.world_mut().insert(
                        e,
                        exact_game::Animator::new([exact_game::State::clip("old", "old")]),
                    );
                } else {
                    old.world_mut()
                        .insert(e, exact_game::Animation::play("old"));
                }
            }
        }
        let bytes = old.save().unwrap();
        for mode in [Restore::Open, Restore::Carry] {
            let mut s =
                WorldSurface::<Animated, exact_game_render::ModelPresentation, true>::default();
            s.bind(&[], None).unwrap();
            s.restore(&bytes, mode).unwrap();
            if mode == Restore::Open {
                assert_eq!(s.carry().unwrap().unwrap(), bytes);
                continue;
            }
            let w = s.sim().unwrap().world();
            assert!(
                w.get::<exact_game::Animation>("clip").is_some(),
                "Carry must add Animation"
            );
            assert!(
                w.get::<exact_game::Animator>("animator").is_some(),
                "Carry must add Animator"
            );
            assert!(
                w.get::<exact_game::Blend>("blend").is_some(),
                "Carry must add Blend"
            );
            assert!(w.get::<exact_game::Animator>("clip").is_none());
            assert!(w.get::<exact_game::Animation>("animator").is_none());
            assert!(w.get::<exact_game::Animation>("blend").is_none());
        }
    }
}
#[test]
fn primitive_pose_refusal_names_the_same_author_knob() {
    let mut sim = Sim::<Animated>::new(()).unwrap();
    let request = r#"{"op":"state","entity":"clip","pose":true}"#;
    let reply = sim.agent(request);
    let error = <() as exact_game_render::Presentation>::inspect(
        sim.world(),
        sim.world().named("clip").unwrap(),
        true,
    )
    .unwrap_err();
    assert_eq!(error, "pose inspection requires game.assets: true");
    assert!(reply.contains(&error), "{reply}");
}
