#![cfg(not(target_arch = "wasm32"))]
// A procedurally rigged humanoid (holding a socketed sword) and a quadruped walk in
// place; offscreen frames show both skinned bodies and their limbs moving.
// EXACT_GPU_OUT=<dir> writes rig-walk-a.ppm and rig-walk-b.ppm for review.
#[path = "fixture/device.rs"]
mod test_device;
use exact_game::animation;
use exact_game::rig::{self, Gait, Rig};
use exact_game::*;
use exact_game_render::{ModelPresentation, WorldSurface};
use exact_gpu::{fixture, Frame, Surface};

struct Walkers;
impl Game for Walkers {
    const ID: &'static str = "rig-walkers";
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        w.insert_resource(Environment {
            background: Some([0.55, 0.7, 0.85]),
            bloom: None,
            fog: None,
            ..Default::default()
        });
        let hero = Rig::humanoid(1.8);
        let hero = w
            .generated_model(
                "hero.model",
                hero.model([hero.idle("idle"), hero.walk("walk", Gait::walk(1.4))]),
            )
            .unwrap();
        let dog = Rig::quadruped(1.4);
        let dog = w
            .generated_model(
                "dog.model",
                dog.model([dog.idle("idle"), dog.walk("walk", Gait::walk(1.2))]),
            )
            .unwrap();
        // Side by side along their walking direction (+Z), seen in profile.
        for (name, mesh, z, speed) in [("hero", hero, -1.1, 1.4), ("dog", dog, 1.1, 1.2)] {
            let mut a = Animator::new([rig::locomotion("move", "idle", [(speed, "walk")])]);
            rig::drive(&mut a, "move", speed);
            w.spawn_named(name, (Transform::at(0., 0., z), mesh, a));
        }
        w.spawn((
            Transform::default(),
            Mesh::cuboid(Vec3::new(0.035, 0.035, 0.8)),
            Material::rgb(0.85, 0.85, 0.9),
            SocketFollow::new("hero", "hand_r").offset(Transform::at(0., -0.06, 0.38)),
        ));
        w.spawn((
            Transform::default(),
            Mesh::plane(12., 12.),
            Material::rgb(0.3, 0.45, 0.25),
        ));
        w.spawn((
            Transform::at(4.2, 1.1, 0.).looking_at(Vec3::new(0., 0.8, 0.), Vec3::Y),
            Camera::default(),
        ));
        w.spawn((
            Transform::at(4., 6., 2.).looking_at(Vec3::ZERO, Vec3::Y),
            DirectionalLight::default(),
        ));
    }
    fn tick(w: &mut World, _: &Input, _: &()) {
        animation::step(w);
    }
}

#[test]
fn procedural_rigs_render_skinned_and_their_limbs_move() {
    let Some(gpu) = test_device::device_or_skip(fixture::device()) else {
        return;
    };
    let mut surface = WorldSurface::<Walkers, ModelPresentation, true>::default();
    surface.bind(&[], None).unwrap();
    surface.device_ready(exact_gpu::wgpu::Features::empty());
    assert!(
        surface.assets().requests.is_empty(),
        "generated models need no delivery"
    );
    let mut frame = Frame {
        width: 480.,
        height: 270.,
        scale: 1.,
        now_ms: 0.,
        seekable: true,
        period_ms: 0.,
        children_generation: 0,
        shader_generation: 0,
    };
    let mut shots = Vec::new();
    for (ms, name) in [(1000., "rig-walk-a"), (1300., "rig-walk-b")] {
        frame.now_ms = ms;
        surface.prepare_assets(
            &gpu.device,
            &gpu.queue,
            exact_gpu::wgpu::TextureFormat::Rgba8Unorm,
        );
        let (pixels, _) = fixture::render(&gpu, &mut surface, &frame).unwrap();
        assert!(surface.error().is_none(), "{:?}", surface.error());
        pixels.save(name);
        shots.push(pixels);
    }
    // Skin and cloth colours, not sky or grass, cover each figure's half.
    let figure = |p: [u8; 4]| {
        let (r, g, b) = (i32::from(p[0]), i32::from(p[1]), i32::from(p[2]));
        !(b > r + 25 && b > g) && !(g > r + 20 && g > b + 10)
    };
    let (w, h) = (480, 270);
    // Looking along -X, screen right is -Z: the humanoid is right, the dog left.
    for (left, label) in [(false, "humanoid"), (true, "quadruped")] {
        let mut count = 0;
        for y in 0..h {
            for x in 0..w {
                if (x < w / 2) == left && figure(shots[0].at(x, y)) {
                    count += 1;
                }
            }
        }
        assert!(count > 400, "{label} half has {count} figure pixels");
    }
    let changed = (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .filter(|&(x, y)| shots[0].at(x, y) != shots[1].at(x, y))
        .count();
    assert!(
        changed > 300,
        "walking must move limbs: {changed} pixels changed"
    );
}
