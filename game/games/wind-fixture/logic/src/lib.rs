//! A field of reeds the render hooks sway (render/), under a sky they paint.
//! The simulation never moves a reed: the wind is presentation only.
use exact_game::*;

/// How hard the wind blows: presentation state, rebuilt by `present` at every
/// tick, restore and paranoid rebuild, never saved or hashed. The render hooks
/// read it; the simulation cannot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Presentation)]
pub struct Gust(pub f32);

/// Reeds per side of the square field.
pub const SIDE: usize = 12;

pub struct WindGame;
impl Game for WindGame {
    const ID: &'static str = "wind-fixture";
    const HZ: u32 = 60;
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        w.spawn_named("wind", Transform::default());
        let reed = w.generated("reed.model", reed()).unwrap();
        for i in 0..SIDE * SIDE {
            let (x, z) = ((i % SIDE) as f32, (i / SIDE) as f32);
            let at = Vec3::new(x - SIDE as f32 / 2., 0., z - SIDE as f32 / 2.);
            w.spawn((
                Transform {
                    rotation: Quat::from_rotation_y(x * 0.7 + z * 1.3),
                    ..Transform::at(at.x, at.y, at.z)
                },
                reed.clone(),
            ));
        }
        w.spawn((
            Transform::at(0., -0.05, 0.),
            Mesh::cuboid(Vec3::new(SIDE as f32 + 2., 0.1, SIDE as f32 + 2.)),
            Material::rgb(0.25, 0.2, 0.12),
        ));
        w.spawn_named(
            "camera",
            (
                Transform::at(0., 4., 12.).looking_at(Vec3::new(0., 1., 0.), Vec3::Y),
                Camera::default(),
            ),
        );
        w.spawn((
            Transform::default().looking_at(Vec3::new(-1., -2., -1.), Vec3::Y),
            DirectionalLight::default(),
        ));
    }
    fn tick(_: &mut World, _: &Input, _: &()) {}
    fn present(w: &mut exact_game::Present<'_>, _: &()) {
        // Slow swells from the tick, with a flutter from the tick-seeded
        // presentation RNG: the same tick always shows the same gust.
        let t = w.tick() as f32 / Self::HZ as f32;
        let flutter = w.rng(1).range(-0.05..0.05);
        let wind = w.named("wind").unwrap();
        w.insert(
            wind,
            Gust(0.75 + 0.25 * math::sin(t * 0.9) * math::sin(t * 0.37) + flutter),
        );
    }
}

/// One reed: a tapered, double-sided blade two units tall. The hooks bend it
/// by height, so its base stays planted.
fn reed() -> asset::MeshData {
    let (w, h) = (0.08, 2.);
    asset::MeshData {
        positions: vec![-w, 0., 0., w, 0., 0., w * 0.2, h, 0., -w * 0.2, h, 0.],
        normals: [0., 0., 1.].repeat(4),
        uvs: vec![0., 1., 1., 1., 1., 0., 0., 0.],
        colors: [0.3, 0.55, 0.2, 1.].repeat(4),
        indices: vec![0, 1, 2, 0, 2, 3],
        bounds: [-w, 0., 0., w, h, 0.],
        ..Default::default()
    }
}
