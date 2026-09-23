//! The cubes bench: one ordinary entity per moving cube, at 60 ticks/second.
use exact_game::*;

#[derive(Default, Args)]
pub struct CubesArgs {
    pub n: u32,
}
#[derive(Default, Component)]
pub struct Spin {
    pub step: Quat,
}
#[derive(Default, Component)]
pub struct Orbit {
    pub radius: f32,
    pub height: f32,
    pub period: f32,
}
pub struct Cubes;
impl Game for Cubes {
    const ID: &'static str = "bench-cubes";
    type Args = CubesArgs;
    fn setup(w: &mut World, args: &CubesArgs) {
        // The smallest cube holding n, in integers: f64::cbrt rounds per host.
        let side = (0..).find(|s: &u32| u64::from(*s).pow(3) >= u64::from(args.n)).unwrap();
        let half = side.saturating_sub(1) as f32 * 0.5;
        for i in 0..args.n {
            // Twins compute these fractions in double precision, including at 500k.
            let f = |k: f64| ((i as f64 * k).fract() - 0.5) as f32;
            let axis = Vec3::new(f(0.3719), f(0.7331), f(0.1913)).normalize_or(Vec3::Y);
            let [r, g, b] = hue((i as f64 * 0.61803).fract() as f32);
            w.spawn((
                Transform::at(
                    (i % side) as f32 * 2.0 - half * 2.0,
                    (i / side % side) as f32 * 2.0 - half * 2.0,
                    (i / (side * side)) as f32 * 2.0 - half * 2.0,
                ),
                Mesh::cube(1.0),
                Material::rgb(r, g, b).rough(0.6),
                Spin {
                    step: Quat::from_axis_angle(axis, (0.5 + (i % 7) as f32 * 0.25) / 60.0),
                },
            ));
        }
        let radius = 1.8 * side as f32 + 6.0;
        w.spawn_named(
            "camera",
            (
                Transform::at(radius, radius * 0.35, 0.0).looking_at(Vec3::ZERO, Vec3::Y),
                Camera {
                    far: radius * 4.0,
                    ..Default::default()
                },
                Orbit {
                    radius,
                    height: radius * 0.35,
                    period: 20.0,
                },
            ),
        );
        w.spawn_named(
            "sun",
            (
                Transform::at(0.38, 0.77, 0.51).looking_at(Vec3::ZERO, Vec3::Y),
                DirectionalLight {
                    illuminance: std::f32::consts::PI / 0.0003,
                    shadows: false,
                    ..Default::default()
                },
            ),
        );
        w.insert_resource(Environment {
            background: Some([0.05, 0.06, 0.09]),
            zenith: [0.35, 0.38, 0.45],
            horizon: [0.35, 0.38, 0.45],
            ground: [0.35, 0.38, 0.45],
            ambient: 1.0,
            sun_disc: 0.0,
            bloom: None,
            fog: None,
            ..Default::default()
        });
    }
    fn tick(w: &mut World, _: &Input, _: &CubesArgs) {
        for (spin, mut t) in w.query::<(&Spin, &mut Transform)>() {
            t.rotation = (spin.step * t.rotation).normalize();
        }
        let seconds = (w.tick() + 1) as f32 / 60.0;
        for (orbit, mut t) in w.query::<(&Orbit, &mut Transform)>() {
            let angle = seconds * std::f32::consts::TAU / orbit.period;
            *t = Transform::at(
                math::cos(angle) * orbit.radius,
                orbit.height,
                math::sin(angle) * orbit.radius,
            )
            .looking_at(Vec3::ZERO, Vec3::Y);
        }
    }
}
// three.js setHSL(h, 0.6, 0.6), in its linear working color space.
fn hue(h: f32) -> [f32; 3] {
    let channel = |offset: f32| {
        let k = (offset + h * 12.0) % 12.0;
        0.6 - 0.24 * (k - 3.0).min(9.0 - k).clamp(-1.0, 1.0)
    };
    [channel(0.0), channel(8.0), channel(4.0)]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scene_and_tick_match_the_bench() {
        let mut sim = Sim::<Cubes>::from_values(&[Value::Number(27.0)]).unwrap();
        assert_eq!(sim.world().query::<(&Mesh, &Spin)>().iter().count(), 27);
        assert!(
            !sim.world()
                .query::<(&DirectionalLight,)>()
                .one()
                .unwrap()
                .0
                .shadows
        );
        assert_eq!(sim.world().query::<(&Camera,)>().one().unwrap().0.far, 45.6);
        let before = sim
            .world()
            .query::<(&Spin, &Transform)>()
            .iter()
            .next()
            .unwrap()
            .1
             .1
            .rotation;
        sim.run(1000.0);
        assert_ne!(
            sim.world()
                .query::<(&Spin, &Transform)>()
                .iter()
                .next()
                .unwrap()
                .1
                 .1
                .rotation,
            before
        );
        assert!((hue(0.0)[0] - 0.84).abs() < 1e-6);
    }
}
