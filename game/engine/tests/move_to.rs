use exact_game::*;
#[derive(Default, Args)]
struct Settings {
    blocked: bool,
    #[live]
    paused: bool,
}
struct Walker;
impl Game for Walker {
    type Args = Settings;
    const ID: &'static str = "move-to";
    fn setup(w: &mut World, _: &Settings) {
        let parent = w.spawn(Transform::at(100., 0., 100.));
        w.spawn_named("walker", (Transform::default(), Parent(parent)));
        w.spawn_named("wall", (Transform::at(102., 0., 100.), Mesh::cube(2.)));
    }
    fn actions() -> Actions {
        Actions::new().stick("move", Stick::keys("KeyW", "KeyS", "KeyA", "KeyD"))
    }
    fn paused(args: &Settings) -> bool {
        args.paused
    }
    fn tick(w: &mut World, input: &Input, args: &Settings) {
        if !args.blocked {
            let delta = input.stick("move") / 60. * 4.5;
            w.get_mut::<Transform>("walker").unwrap().position += Vec3::new(delta.x, 0., -delta.y);
        }
    }
}
#[test]
fn move_to_uses_global_coordinates_and_releases_on_success_stall_and_refusal() {
    let mut s = Sim::<Walker>::new(Settings::default()).unwrap();
    let target = Vec3::new(101., 0., 102.);
    s.move_to("walker", target, 0.18, 4.5).unwrap();
    assert!(s.global_position("walker").unwrap().distance(target) <= 0.18);
    assert!(s.agent(r#"{"op":"state"}"#).contains(r#""forwarded":[]"#));
    let mut s = Sim::<Walker>::new(Settings {
        blocked: true,
        paused: false,
    })
    .unwrap();
    let error = s
        .move_to("walker", Vec3::new(104., 0., 100.), 0.18, 4.5)
        .unwrap_err();
    assert!(
        error.contains("160 bursts")
            && error.contains("wall")
            && error.contains("nearestClearSide"),
        "{error}"
    );
    assert_eq!(s.world().tick(), 1760);
    assert!(s.agent(r#"{"op":"state"}"#).contains(r#""forwarded":[]"#));
    let before = s.world().tick();
    assert!(s.move_to("walker", target, 0., 4.5).is_err());
    assert_eq!(s.world().tick(), before);
    s.bind(&[Value::Bool(true), Value::Bool(true)], None)
        .unwrap();
    assert!(s
        .move_to("walker", target, 0.18, 4.5)
        .unwrap_err()
        .contains("clock refused"));
    assert!(s.agent(r#"{"op":"state"}"#).contains(r#""forwarded":[]"#));
}
