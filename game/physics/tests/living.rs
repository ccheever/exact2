//! Copyable controller composition: no visual root motion writes the capsule pose.
#[path = "compare.rs"]
mod paranoid;
use exact_game::{
    asset::{Clip, Model, Node, Track},
    motion::{Gravity, Jump, Move},
    *,
};
use exact_game_physics::{self as physics, Body, CapsuleController, Collider, Shape};
struct Living;
impl Game for Living {
    const ID: &'static str = "living-controller";
    const ASSETS: &'static [&'static str] = &["fox.model"];
    type Args = ();
    fn actions() -> Actions {
        Actions::new()
            .stick("move", Stick::wasd())
            .button("jump", &["Space"])
    }
    fn setup(w: &mut World, _: &()) {
        physics::register(w);
        let heights: Vec<_> = (0..49)
            .map(|col| {
                let x = col as f32 * 0.25 - 6.;
                if x < -4. {
                    0.
                } else if x < -2. {
                    (x + 4.) * 0.3
                } else if x < 0. {
                    0.6
                } else if x <= 2. {
                    0.8
                } else {
                    -2.
                }
            })
            .collect();
        w.spawn_named(
            "terrain",
            (
                Transform::default(),
                Collider {
                    shape: Shape::Heightfield {
                        rows: 3,
                        cols: 49,
                        heights: heights.repeat(3),
                        scale: Vec3::new(12., 1., 8.),
                    },
                    ..Default::default()
                },
            ),
        );
        let player = w.spawn_named(
            "player",
            (Transform::at(-5., 0.91, 0.), CapsuleController::default()),
        );
        w.spawn_named(
            "fox",
            (
                Parent(player),
                Ambient,
                Transform {
                    position: Vec3::new(0., -0.9, 0.),
                    rotation: Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
                    scale: Vec3::splat(0.025),
                },
                Mesh::asset("fox.model"),
                Animator::new([State::clip("idle", "Survey")]).motion_root("b_Root_00"),
            ),
        );
        w.spawn_named(
            "crate",
            (
                Transform::at(1.6, 1.3, 0.),
                Collider::default(),
                Body {
                    mass: 10.,
                    ..Default::default()
                },
            ),
        );
        w.spawn_named(
            "sun",
            (Transform::default(), DirectionalLight::default(), Ambient),
        );
        w.insert_resource(Environment::default());
        w.ambient_resource::<Environment>();
        w.derived_publication("remaining");
    }
    fn tick(w: &mut World, input: &Input, _: &()) {
        let dt = w.dt();
        let velocity = {
            let mut c = w.require_mut::<CapsuleController>("player");
            Move {
                speed: 2.,
                accel: 12.,
                brake: 20.,
            }
            .step(&mut c.velocity, input.stick_xz("move"), dt);
            if c.grounded && input.pressed("jump") {
                Jump {
                    height: 1.2,
                    gravity: 9.81,
                }
                .start(&mut c.velocity);
            }
            Gravity(9.81).step(&mut c.velocity, dt);
            c.velocity
        };
        physics::capsule(w, "player").step(velocity);
        physics::step(w);
        // Idle is presentation on the child; discard root motion instead of moving the root.
        let _motion = animation::step(w);
        let end = w.tick_end();
        w.require_mut::<Transform>("sun").rotation = Quat::from_rotation_x(end.seconds() * 0.1);
        w.require_mut::<DirectionalLight>("sun").illuminance =
            if end.seconds() < 60. { 10_000. } else { 0. };
        w.resource_mut::<Environment>().ambient = (1. - end.seconds() / 60.).max(0.);
        w.publish("remaining", (60. - end.seconds()).max(0.));
    }
}
fn sim() -> Sim<Living> {
    let model = Model {
        nodes: vec![
            Node {
                name: "b_Root_00".into(),
                ..Default::default()
            },
            Node {
                name: "animated_child".into(),
                parent: Some(0),
                ..Default::default()
            },
        ],
        clips: vec![Clip {
            name: "Survey".into(),
            tracks: vec![
                Track {
                    node: 0,
                    times: vec![0., 7.],
                    values: vec![0., 0., 0., 0.7, 0., 0.],
                    ..Default::default()
                },
                Track {
                    node: 1,
                    times: vec![0., 7.],
                    values: vec![0., 0., 0., 0., 0.4, 0.],
                    ..Default::default()
                },
            ],
            ..Default::default()
        }],
        ..Default::default()
    };
    let model = bin::to_vec(&model);
    Sim::with_assets((), move |name: &str| match name {
        "fox.model" => Ok(model.clone()),
        _ => Err(name.to_owned()),
    })
    .unwrap()
}
#[test]
fn controller_pushes_crate_down_ledge_then_living_scene_settles() {
    paranoid::compare(sim, |s| {
        s.run(100.);
        s.tap("Space");
        s.run(300.);
        assert!(
            s.local_position("player").unwrap().y > 1.5,
            "Jump must lift the controller"
        );
        s.run(1200.);
        s.key_down("KeyD");
        let mut climbed_slope = false;
        let mut climbed_step = false;
        for _ in 0..45 {
            s.run(100.);
            let p = s.local_position("player").unwrap();
            climbed_slope |= p.x > -2.5 && p.x < -1. && p.y > 1.35;
            climbed_step |= p.x > 0.5 && p.x < 1. && p.y > 1.65;
        }
        assert!(
            climbed_slope && climbed_step,
            "controller must traverse slope and step"
        );
        s.key_up("KeyD");
        let pushed = s.local_position("crate").unwrap();
        assert!(pushed.x > 2.25, "crate not pushed beyond ledge: {pushed:?}");
        assert!(pushed.y < 1., "crate did not fall: {pushed:?}");
        eprintln!(
            "before settle tick={} player={:?} crate={:?} asleep={}",
            s.world().tick(),
            s.local_position("player"),
            pushed,
            s.world().require::<Body>("crate").asleep
        );
        assert!(s.settle(), "{}", s.agent(r#"{"op":"clock","settle":true}"#));
        assert!(physics::quiescent(s.world()));
        let pinned = s.local_position("crate").unwrap();
        let player = s.local_position("player").unwrap();
        assert_eq!(s.world().tick(), 547);
        assert!((pinned - Vec3::new(4.3423862, -1.5000682, 0.061061338)).length() < 0.0001);
        assert!((player - Vec3::new(2.2726393, 1.5475401, -0.0044510923)).length() < 0.0001);
        eprintln!(
            "settled tick={} player={player:?} crate={pinned:?} asleep=true remaining={:?}",
            s.world().tick(),
            s.world().published("remaining")
        );
        assert!(
            (pinned.y + 1.5).abs() < 0.03,
            "crate must rest on lower terrain: {pinned:?}"
        );
        assert_eq!(
            s.world().require::<Transform>("player").rotation,
            Quat::IDENTITY
        );
        assert_eq!(s.world().require::<Transform>("player").scale, Vec3::ONE);
        let fox = s.world().require::<animation::Pose>("fox").local.clone();
        s.run(60_000.);
        assert_eq!(s.local_position("crate").unwrap(), pinned);
        assert_eq!(s.local_position("player").unwrap(), player);
        assert_ne!(s.world().require::<animation::Pose>("fox").local, fox);
        assert_eq!(s.world().require::<DirectionalLight>("sun").illuminance, 0.);
        assert_eq!(s.world().resource::<Environment>().ambient, 0.);
    });
}
