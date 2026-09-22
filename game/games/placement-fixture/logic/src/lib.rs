use exact_game::*;

#[derive(Default, Args)]
pub struct Options {
    pub plates: u32,
}
#[derive(Default, Data)]
struct Hud {
    lit: bool,
}
#[derive(Default, Resource)]
struct Lamp {
    lit: bool,
}
pub struct SmallGame;
impl Game for SmallGame {
    const ID: &'static str = "placement-fixture";
    const HZ: u32 = 60;
    type Args = Options;
    fn actions() -> Actions {
        Actions::new().button("pull", &[])
    }
    fn setup(w: &mut World, args: &Options) {
        w.insert_resource(Environment {
            background: Some([0.04, 0.08, 0.12]),
            fog: None,
            bloom: None,
            ..Default::default()
        });
        w.insert_resource(Lamp::default());
        if args.plates > 0 {
            w.spawn_named(
                "camera",
                (
                    Transform::at(0., 3., 8.).looking_at(Vec3::new(0., 1., 0.), Vec3::Y),
                    Camera::default(),
                ),
            );
            for i in 0..args.plates.min(40) {
                w.spawn_named(
                    format!("plate-{i}"),
                    (
                        Transform::at((i % 8) as f32 - 3.5, (i / 8) as f32 * 0.6, -2.),
                        Placed::child(i as u16).width(0.7),
                    ),
                );
            }
            return;
        }
        w.spawn((
            Transform::default(),
            Mesh::plane(20., 20.),
            Material::grid([0.12, 0.18, 0.2], 1.),
        ));
        w.spawn((
            Transform::at(0., 0.01, 0.),
            Mesh::plane(2., 20.),
            Material::rgb(0.35, 0.3, 0.2),
        ));
        w.spawn_named(
            "sign",
            (
                Transform::at(-1.8, 1.4, 0.),
                Placed::child("sign").width(1.8).facing(Facing::Fixed),
            ),
        );
        w.spawn_named(
            "cube",
            (
                Transform::at(0., 0.5, 0.),
                Mesh::cube(1.),
                Material::rgb(0.15, 0.5, 0.7),
            ),
        );
        w.spawn_named(
            "name",
            (
                Transform::at(0., 1.2, 0.),
                Parent(w.named("cube").unwrap()),
                Placed::child("name").width(1.2),
            ),
        );
        w.spawn_named(
            "pull",
            (
                Transform::at(1.8, 1.2, 0.),
                Placed::child("pull").width(1.2),
            ),
        );
        w.spawn_named(
            "lamp",
            (
                Transform::at(2.5, 0.5, -1.),
                Mesh::sphere(0.3),
                Material::rgb(0.3, 0.2, 0.1),
            ),
        );
        w.spawn_named(
            "camera",
            (
                Transform::at(0., 3., 8.).looking_at(Vec3::new(0., 1., 0.), Vec3::Y),
                Camera::default(),
            ),
        );
        w.publish_record(&Hud::default());
    }
    fn tick(w: &mut World, input: &Input, args: &Options) {
        if args.plates > 0 {
            return;
        }
        let t = w.tick_end().seconds();
        let (sin, cos) = (math::sin(t * 0.6), math::cos(t * 0.6));
        *w.require_mut::<Transform>("camera") =
            Transform::at(sin * 8., 3., cos * 8.).looking_at(Vec3::new(0., 1., 0.), Vec3::Y);
        w.require_mut::<Transform>("cube").position.x = math::sin(t) * 0.6;
        let lit = w.resource::<Lamp>().lit || input.pressed("pull");
        w.resource_mut::<Lamp>().lit = lit;
        w.require_mut::<Material>("lamp").emissive = if lit { [4., 2., 0.3] } else { [0.; 3] };
        w.publish_record(&Hud { lit });
    }
}
