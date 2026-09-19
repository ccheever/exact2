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

#[test]
fn named_bindings_use_args_defaults_and_preserve_atomic_live_restart_and_restore() {
    use exact_game::{Args, Transform};
    use exact_gpu::{Module, Registry};
    #[derive(Args)]
    struct Options {
        seed: u64,
        #[live]
        paused: bool,
        #[restart]
        restart: bool,
        offset: f32,
    }
    impl Default for Options {
        fn default() -> Self {
            Self {
                seed: 7,
                paused: false,
                restart: false,
                offset: 2.0,
            }
        }
    }
    struct Named;
    impl Game for Named {
        const ID: &'static str = "named-binding";
        type Args = Options;
        fn setup(w: &mut World, args: &Options) {
            w.reseed(args.seed);
            w.spawn_named("player", Transform::at(args.offset, 0., 0.));
        }
        fn tick(w: &mut World, _: &Input, _: &Options) {
            w.get_mut::<Transform>("player").unwrap().position.x += 1.;
        }
        fn paused(args: &Options) -> bool {
            args.paused
        }
    }
    static REGISTRY: Registry = Registry {
        surfaces: &[("world", 4, || Box::<WorldSurface<Named>>::default())],
        shaders: &[],
    };
    let mut module = Module::new(&REGISTRY);
    module.set_seekable(true);
    let named = module.create_headless("world").unwrap();
    let positional = module.create_headless("world").unwrap();
    assert!(module.bind_json(named, r#"{"offset":3,"seed":9}"#, Some(0.)));
    assert!(module.bind_json(positional, "[9,false,false,3]", Some(0.)));
    assert_eq!(
        module.carry(named).unwrap(),
        module.carry(positional).unwrap()
    );
    // Invalid names/JSON/types may not seek to the supplied future clock.
    let before = module.carry(named).unwrap();
    for (json, message) in [
        (r#"{"seed":9,"typo":true}"#, "typo"),
        (r#"{"seed":9,"seed":10}"#, "duplicate"),
        (r#"{"":9}"#, "empty"),
        (r#"{"seed":9,}"#, "number"),
        (r#"{"seed":9} trailing"#, "trailing"),
        (r#"{"paused":3}"#, "paused"),
        (r#"{"offset":1e309}"#, "offset"),
    ] {
        assert!(!module.bind_json(named, json, Some(1000.)), "{json}");
        assert!(module.take_error().contains(message), "{json}");
        assert_eq!(module.carry(named).unwrap(), before, "{json}");
    }
    // Identical binding seeks with old inputs; reordered names use the same path.
    for (named_json, positional_json, at) in [
        (
            r#"{"paused":true,"seed":9,"offset":3}"#,
            "[9,true,false,3]",
            100.,
        ),
        (
            r#"{"offset":3,"seed":9,"paused":true}"#,
            "[9,true,false,3]",
            200.,
        ),
        (r#"{"seed":9,"offset":3}"#, "[9,false,false,3]", 300.),
        (
            r#"{"restart":true,"offset":3,"seed":9}"#,
            "[9,false,true,3]",
            400.,
        ),
        (
            r#"{"seed":9,"offset":3,"restart":false}"#,
            "[9,false,false,3]",
            500.,
        ),
        (r#"{}"#, "[7,false,false,2]", 600.),
        (r#"{}"#, "[]", 700.),
        (r#"{"seed":9}"#, "[9]", 800.),
        (r#"{}"#, "[]", 900.),
    ] {
        assert!(
            module.bind_json(named, named_json, Some(at)),
            "{}",
            module.take_error()
        );
        assert!(
            module.bind_json(positional, positional_json, Some(at)),
            "{}",
            module.take_error()
        );
        assert_eq!(
            module.carry(named).unwrap(),
            module.carry(positional).unwrap()
        );
    }
    let saved = module.carry(named).unwrap().unwrap();
    for mode in [Restore::Open, Restore::Carry] {
        let fresh = module.create_headless("world").unwrap();
        assert!(module.bind_json(fresh, r#"{"seed":7}"#, None));
        assert!(module.restore(fresh, &saved, mode));
        assert_eq!(module.carry(fresh).unwrap(), Some(saved.clone()));
    }
}

#[test]
fn r13_beacons_world_empty_binds_seed_zero_paused_false_restart_false() {
    use exact_game::Args;
    use exact_gpu::{Module, Registry};
    static REGISTRY: Registry = Registry {
        surfaces: &[("world", 3, || {
            Box::<WorldSurface<beacons_logic::Beacons>>::default()
        })],
        shaders: &[],
    };
    let mut module = Module::new(&REGISTRY);
    module.set_seekable(true);
    let empty = module.create_headless("world").unwrap();
    let full = module.create_headless("world").unwrap();
    assert_eq!(
        beacons_logic::Options::default().values(),
        vec![
            exact_game::Value::Number(0.),
            exact_game::Value::Bool(false),
            exact_game::Value::Bool(false)
        ]
    );
    assert!(module.bind_json(empty, "{}", None));
    assert!(module.bind_json(full, "[0,false,false]", None));
    assert_eq!(module.carry(empty).unwrap(), module.carry(full).unwrap());
}
