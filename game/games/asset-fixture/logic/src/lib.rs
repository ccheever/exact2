use exact_game::*;
use exact_game_physics::{self as physics, Collider, Shape};

#[derive(Default, Data)]
pub struct Island {
    pub seed: u64,
    pub lanterns: Vec<Vec3>,
    pub crate_position: Vec3,
    pub sign: String,
}

pub struct AssetFixture;
impl Game for AssetFixture {
    const ID: &'static str = "asset-fixture";
    const ASSETS: &'static [&'static str] = &["crate.model"];
    const LEVELS: &'static [asset::Level] = &[asset::Level::of::<Island>("island.level.json")];
    type Args = ();
    fn register(w: &mut World, _: &std::collections::BTreeMap<&str, Value>) {
        physics::register(w);
    }
    fn setup(w: &mut World, _: &()) {
        w.insert_resource(Environment {
            background: Some([0.15, 0.22, 0.3]),
            bloom: None,
            fog: None,
            ..Default::default()
        });
        assert!(w.model("crate.model").is_some());
        let level = w.level::<Island>("island.level.json").unwrap();
        w.reseed(level.seed);
        let (mut terrain, shape) = island(level.seed);
        for p in terrain.positions.chunks_exact(3) {
            terrain.colors.extend(if p[1] < 1. {
                [0.12, 0.4, 0.08, 1.]
            } else {
                [0.6, 0.45, 0.2, 1.]
            });
        }
        let terrain = w.generated("island.model", terrain).unwrap();
        w.spawn_named(
            "crate",
            (
                Transform::at(
                    level.crate_position.x,
                    level.crate_position.y,
                    level.crate_position.z,
                ),
                Mesh::asset("crate.model"),
            ),
        );
        w.spawn_named(
            "ground",
            (
                Transform::default(),
                terrain,
                Collider {
                    shape,
                    ..Default::default()
                },
            ),
        );
        let (mut rock, _) = Shape::heightfield(
            3,
            3,
            vec![0., 0.2, 0., 0.2, 0.65, 0.2, 0., 0.2, 0.],
            Vec3::new(0.9, 1., 0.8),
        )
        .unwrap();
        rock.colors = [0.3, 0.32, 0.4, 1.].repeat(9);
        let rock = w.generated("rock.model", rock).unwrap();
        for (i, p) in level.lanterns.iter().enumerate() {
            w.spawn_named(
                &format!("rock-{i}"),
                (Transform::at(p.x + 0.65, p.y - 0.3, p.z), rock.clone()),
            );
            w.spawn_named(
                &format!("lantern-{i}"),
                (
                    Transform::at(p.x, p.y, p.z),
                    Mesh::sphere(0.2),
                    Material::rgb(1., 0.7, 0.12),
                ),
            );
        }
        for (name, x, z) in [
            ("terrainA", -4., -4.),
            ("terrainB", 0., 0.),
            ("terrainC", 4., 4.),
        ] {
            let hit = physics::raycast(w, Vec3::new(x, 10., z), -Vec3::Y, 20., u32::MAX).unwrap();
            w.publish(name, 10. - hit.distance);
        }
        let contact = physics::sweep(
            w,
            &Shape::Sphere { radius: 0.2 },
            Transform::at(-4., 10., 0.),
            Vec3::new(0., -20., 0.),
            u32::MAX,
        )
        .unwrap();
        w.publish("slopeContact", 10. - contact.distance);
        w.publish("sign", level.sign.clone());
        w.spawn_named(
            "camera",
            (
                Transform::at(-9., 9., 12.).looking_at(Vec3::new(0., 0.6, 0.), Vec3::Y),
                Camera::default(),
            ),
        );
        w.publish("ready", true);
    }
    fn tick(w: &mut World, _: &Input, _: &()) {
        w.publish("tick", w.tick() as u32);
    }
}

/// Asymmetric seeded rows, with a gentle slope, a steep rise and an upper ledge.
pub fn island(seed: u64) -> (asset::MeshData, Shape) {
    let heights = (0..16)
        .map(|i| {
            [0., 0.5, 2., 2.][i % 4] + ((seed.wrapping_add((i / 4) as u64 * 3)) % 5) as f32 * 0.125
        })
        .collect();
    Shape::heightfield(4, 4, heights, Vec3::new(12., 1., 12.)).unwrap()
}
