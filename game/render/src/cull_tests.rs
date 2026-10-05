//! Culling changes no pixels: each frame renders culled and again with every item
//! kept, bit for bit, while the culled frame draws fewer instances in every view.
use crate::WorldSurface;
use exact_game::*;
use exact_gpu::{fixture, Frame, Surface};

#[path = "../../bake/tests/samples.rs"]
mod samples;

/// BoxTextured's colour texture, named by its content at bake.
fn box_texture() -> String {
    static NAME: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    NAME.get_or_init(|| samples::sample("BoxTextured").textures[0].clone())
        .clone()
}

#[derive(Default, Component)]
struct Drift {
    velocity: Vec3,
    spin: Quat,
    grow: f32,
}
#[derive(Default, Args)]
struct FieldArgs {
    /// Orthographic camera instead of perspective.
    pub ortho: bool,
}
fn random(seed: &mut u64) -> f32 {
    *seed = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    (*seed >> 40) as f32 / 16777216.
}
fn drift(w: &mut World) {
    let dt = w.dt();
    let seconds = w.tick_end().seconds();
    for (d, mut t) in w.query::<(&Drift, &mut Transform)>() {
        // Metres per tick, a full turn in a few ticks and pulsing scale: frames
        // between ticks interpolate far from both endpoints.
        t.position += d.velocity * dt;
        t.rotation = (d.spin * t.rotation).normalize();
        t.scale = Vec3::splat(1. + d.grow * math::sin(seconds * 9.));
    }
    let yaw = seconds * 3.;
    let mut camera = w.require_mut::<Transform>("camera");
    camera.rotation = Quat::from_rotation_y(yaw) * Quat::from_rotation_x(-0.15);
}
fn sun_and_camera(w: &mut World, camera: Camera) {
    w.spawn_named("camera", (Transform::at(0., 2., 0.), camera));
    // A low sun: tall casters behind the camera throw shadows into its view.
    w.spawn((
        Transform::at(-30., 9., 12.).looking_at(Vec3::ZERO, Vec3::Y),
        DirectionalLight::default(),
    ));
    w.spawn((
        Transform::at(3., 2., -6.),
        PointLight {
            color: [1., 0.6, 0.2],
            intensity: 40. / crate::PHOTOMETRIC_SCALE,
            range: 12.,
        },
    ));
}

struct Field;
impl Game for Field {
    const ID: &'static str = "cull-field";
    type Args = FieldArgs;
    fn setup(w: &mut World, args: &FieldArgs) {
        let mut seed = 7;
        let mut next = || random(&mut seed);
        w.spawn((
            Transform::default(),
            Mesh::plane(260., 260.),
            Material::grid([0.2, 0.24, 0.22], 2.),
        ));
        for i in 0..1500 {
            let position = Vec3::new(next() * 220. - 110., next() * 4., next() * 220. - 110.);
            let mesh = match i % 5 {
                0 => Mesh::cuboid(Vec3::new(0.5 + next(), 0.5 + 3. * next(), 0.5 + next())),
                1 => Mesh::sphere(0.3 + next()),
                2 => Mesh::cylinder(0.3 + 0.4 * next(), 1. + 2. * next()),
                3 => Mesh::capsule(0.2 + 0.3 * next(), 1.5 + 2. * next()),
                _ => Mesh::plane(1. + next(), 1. + next()),
            };
            let rotation =
                Quat::from_euler(glam::EulerRot::XYZ, next() * 6., next() * 6., next() * 6.);
            let scale = Vec3::new(0.5 + next(), 0.5 + 2. * next(), 0.5 + next());
            let e = w.spawn((
                Transform {
                    position,
                    rotation,
                    scale,
                },
                mesh,
                Material::rgb(next(), next(), next()),
            ));
            if i % 6 == 0 {
                let velocity = Vec3::new(next() - 0.5, 0., next() - 0.5) * 400.;
                w.insert(
                    e,
                    Drift {
                        velocity,
                        spin: Quat::from_rotation_y(1.3),
                        grow: 0.5,
                    },
                );
            }
            if i % 97 == 0 {
                w.spawn((
                    Transform::at(0., 2., 0.),
                    Mesh::sphere(0.4),
                    Material::rgb(1., 1., 0.),
                    Parent(e),
                ));
            }
            if i % 211 == 0 {
                w.insert(e, Visible(false));
            }
        }
        for i in 0..24 {
            let angle = i as f32 * std::f32::consts::TAU / 24.;
            w.spawn((
                Transform::at(math::cos(angle) * 14., 12., math::sin(angle) * 14.),
                Mesh::cuboid(Vec3::new(1., 24., 1.)),
                Material::rgb(0.7, 0.7, 0.68),
            ));
        }
        // Fountains all around: most emitters are off screen at any moment.
        for i in 0..12 {
            let angle = i as f32 * std::f32::consts::TAU / 12.;
            let mut e = Emitter::sparks().rate(120.).lifetime(0.8).seed(i);
            e.speed = 6.;
            e.shape = exact_game::emitter::Shape::Sphere(0.5);
            e.additive = i % 2 == 0;
            let fountain = w.spawn((
                Transform::at(math::cos(angle) * 9., 0.5, math::sin(angle) * 9.),
                e,
            ));
            if i % 4 == 0 {
                w.insert(
                    fountain,
                    Drift {
                        velocity: Vec3::new(math::sin(angle), 0., math::cos(angle)) * 120.,
                        spin: Quat::from_rotation_z(0.9),
                        grow: 0.3,
                    },
                );
            }
        }
        let camera = if args.ortho {
            Camera::orthographic(30.)
        } else {
            Camera {
                far: 120.,
                ..Default::default()
            }
        };
        sun_and_camera(w, camera);
    }
    fn tick(w: &mut World, _: &Input, _: &FieldArgs) {
        emitter::step(w);
        drift(w);
    }
}

struct Herd;
impl Game for Herd {
    const ID: &'static str = "cull-herd";
    const ASSETS: &'static [&'static str] = &["fox.model", "box.model", "glass.model"];
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        let b = w.model("fox.model").unwrap().bounds;
        let fox = 1.2 / (b[4] - b[1]);
        let mut seed = 11;
        let mut next = || random(&mut seed);
        w.spawn((
            Transform::default(),
            Mesh::plane(120., 120.),
            Material::grid([0.2, 0.24, 0.22], 2.),
        ));
        let mut first = None;
        for i in 0..36 {
            let angle = i as f32 * std::f32::consts::TAU / 36.;
            let radius = 6. + 14. * next();
            let e = w.spawn((
                Transform::at(math::cos(angle) * radius, 0., math::sin(angle) * radius)
                    .with_scale(fox),
                Mesh::asset("fox.model"),
                Animator::new([State::clip("run", ["Run", "Walk", "Survey"][i % 3])]),
            ));
            first.get_or_insert(e);
        }
        w.spawn((
            Transform::default(),
            Mesh::sphere(0.12),
            Material::rgb(1., 0.65, 0.1),
            SocketFollow::new(first.unwrap(), "b_Head_05").offset(Transform::at(0., 0.2, 0.)),
        ));
        for i in 0..80 {
            let position = Vec3::new(next() * 60. - 30., 0.5 + next() * 3., next() * 60. - 30.);
            let e = w.spawn((
                Transform {
                    position,
                    rotation: Quat::from_rotation_y(next() * 6.),
                    scale: Vec3::new(0.5 + next(), 0.5 + next(), 0.5 + next()),
                },
                Mesh::asset("box.model"),
            ));
            if i % 3 == 0 {
                w.insert(
                    e,
                    Drift {
                        velocity: Vec3::new(next() - 0.5, 0., next() - 0.5) * 300.,
                        spin: Quat::from_rotation_x(1.1),
                        grow: 0.4,
                    },
                );
            }
            // Blended models and sprites are ordered and culled on the CPU.
            let at = Vec3::new(next() * 50. - 25., 1. + next() * 2., next() * 50. - 25.);
            if i % 2 == 0 {
                w.spawn((Transform::at(at.x, at.y, at.z), Mesh::asset("glass.model")));
            } else {
                let mut sprite = Sprite::new(box_texture(), [1.5, 1.]);
                sprite.alpha = [
                    asset::AlphaMode::Opaque,
                    asset::AlphaMode::Mask,
                    asset::AlphaMode::Blend,
                ][i % 3];
                w.spawn((Transform::at(at.x, at.y, at.z), sprite));
            }
        }
        sun_and_camera(w, Camera::default());
        w.insert_resource(Environment {
            fog: None,
            ..Default::default()
        });
    }
    fn tick(w: &mut World, _: &Input, _: &()) {
        animation::step(w);
        drift(w);
    }
}

fn gpu() -> Option<exact_gpu::Gpu> {
    crate::test_device::device_or_skip(fixture::device())
}

// Draw each time culled, then with every item kept; pixels must be identical.
// Returns the camera and cascade instance totals of each culled frame, and how
// many quads (sprites, particles) the culled frames skipped in all.
fn identical<G: Game, P: crate::Executor, const ASSETS: bool>(
    gpu: &exact_gpu::Gpu,
    surface: &mut WorldSurface<G, P, ASSETS>,
    times: &[f64],
    name: &str,
) -> (Vec<[u64; 4]>, u64) {
    let mut totals = Vec::new();
    let mut skipped = 0;
    for &now_ms in times {
        let frame = Frame {
            width: 320.,
            height: 180.,
            scale: 1.,
            now_ms,
            seekable: true,
            period_ms: 0.,
            children_generation: 0,
            shader_generation: 0,
            headroom: 1.0,
        };
        if let Some(r) = surface.renderer_for_test() {
            r.count_culled(true);
        }
        let (culled, _) = fixture::render(gpu, surface, &frame).unwrap();
        assert!(surface.error().is_none(), "{:?}", surface.error());
        let r = surface.renderer_for_test().unwrap();
        gpu.device
            .poll(exact_gpu::wgpu::PollType::wait_indefinitely())
            .unwrap();
        if let Some(counts) = r.culled() {
            totals.push(counts);
        }
        let quads = r.quads.instances();
        r.cull.keep_all = true;
        let (all, _) = fixture::render(gpu, surface, &frame).unwrap();
        let r = surface.renderer_for_test().unwrap();
        r.cull.keep_all = false;
        assert!(quads <= r.quads.instances());
        skipped += r.quads.instances() - quads;
        // Drain the kept frame's readback so the next culled frame copies its own.
        gpu.device
            .poll(exact_gpu::wgpu::PollType::wait_indefinitely())
            .unwrap();
        r.culled();
        if culled.data != all.data {
            culled.save(&format!("{name}-{now_ms}-culled"));
            all.save(&format!("{name}-{now_ms}-all"));
            let differing = culled
                .data
                .chunks_exact(4)
                .zip(all.data.chunks_exact(4))
                .filter(|(a, b)| a != b)
                .count();
            panic!("{name} at {now_ms} ms: culling changed {differing} pixels");
        }
    }
    (totals, skipped)
}

const TIMES: [f64; 6] = [0., 25., 58.3, 100., 108.4, 500.];

#[test]
fn culling_changes_no_pixels_in_a_moving_primitive_field() {
    let Some(gpu) = gpu() else { return };
    for ortho in [false, true] {
        let mut surface = WorldSurface::<Field>::default();
        surface.bind(&[Value::Bool(ortho)], None).unwrap();
        let (totals, skipped) = identical(&gpu, &mut surface, &TIMES, "field");
        eprintln!("field ortho={ortho}: {skipped} particles skipped");
        assert!(skipped > 0);
        let items = surface.renderer_for_test().unwrap().slot_list.len() as u64;
        assert!(totals.len() >= TIMES.len() - 1, "{totals:?}");
        for views in &totals {
            eprintln!("field ortho={ortho}: {items} items, per view {views:?}");
            assert!(views[0] > 0 && views[0] < items / 2, "{views:?} of {items}");
            // The shadow cascades keep casters towards the sun, but not the field.
            assert!(views[1] > 0 && views[1] < items, "{views:?} of {items}");
            assert!(views[3] < items, "{views:?} of {items}");
        }
    }
}

#[test]
fn culling_changes_no_pixels_with_animated_skins_models_and_sockets() {
    let Some(gpu) = gpu() else { return };
    let mut surface = WorldSurface::<Herd, crate::ModelExecutor, true>::default();
    surface.device_ready(exact_gpu::wgpu::Features::empty());
    surface.bind(&[], None).unwrap();
    let fox = samples::sample("Fox");
    let crate_box = samples::sample("BoxTextured");
    let mut glass = crate_box.clone();
    for material in &mut glass.materials {
        material.alpha_mode = asset::AlphaMode::Blend;
        material.base_color[3] = 0.5;
    }
    surface.asset("fox.model", Ok(&bin::to_vec(&fox)));
    surface.asset("box.model", Ok(&bin::to_vec(&crate_box)));
    surface.asset("glass.model", Ok(&bin::to_vec(&glass)));
    let format = exact_gpu::wgpu::TextureFormat::Rgba8Unorm;
    surface.prepare_assets(&gpu.device, &gpu.queue, format);
    for texture in surface.assets().requests {
        let path = samples::cache().join(&texture);
        surface.asset(&texture, Ok(&std::fs::read(path).unwrap()));
    }
    surface.prepare_assets(&gpu.device, &gpu.queue, format);
    let (totals, skipped) = identical(&gpu, &mut surface, &TIMES, "herd");
    eprintln!("herd: {skipped} sprites skipped");
    assert!(skipped > 0);
    let items = surface.renderer_for_test().unwrap().slot_list.len() as u64;
    for views in &totals {
        eprintln!("herd: {items} items, per view {views:?}");
        assert!(views[0] > 0 && views[0] < items, "{views:?} of {items}");
    }
}

#[test]
fn steady_culling_preparation_allocates_nothing() {
    let Some(gpu) = gpu() else { return };
    let mut r = crate::Renderer::new(
        &gpu.device,
        &gpu.queue,
        exact_gpu::wgpu::TextureFormat::Rgba8Unorm,
    );
    let (v, i) = crate::shapes::cube();
    let cube = r.add_mesh(&v, &i);
    let pose = [0., 0., 0., 0., 0., 0., 1., 1., 1., 1.];
    r.write_transforms_both(0, &pose.repeat(64)).unwrap();
    r.write_materials(0, &[1.; 12 * 64]).unwrap();
    let slots: Vec<u32> = (0..64).collect();
    r.set_batches(
        &[
            crate::Batch::new(cube, 0..40),
            crate::Batch::new(cube, 40..64),
        ],
        &slots,
    )
    .unwrap();
    let mut world = World::new(60, 0);
    let entities: Vec<_> = (0..64).map(|_| world.spawn(Transform::default())).collect();
    // A mirrored attachment splits a batch into two winding groups every frame.
    let attachments = [crate::DisplayedAttachment {
        entity: entities[7],
        matrix: glam::Mat4::from_scale(glam::Vec3::new(-1., 1., 1.)),
        pose: Default::default(),
    }];
    let frame = crate::FrameInput {
        attachments: &attachments,
        ..Default::default()
    };
    let texture = gpu
        .device
        .create_texture(&exact_gpu::wgpu::TextureDescriptor {
            label: None,
            size: exact_gpu::wgpu::Extent3d {
                width: 16,
                height: 16,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: exact_gpu::wgpu::TextureDimension::D2,
            format: exact_gpu::wgpu::TextureFormat::Rgba8Unorm,
            usage: exact_gpu::wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
    r.draw(&texture.create_view(&Default::default()), (16, 16), &frame);
    assert_eq!(r.cull.groups.len(), 4);
    assert!(!r.cull.stale());
    let count = crate::world::tests::allocations::count(|| {
        for _ in 0..100 {
            r.cull_groups(&frame, 3);
            assert!(!r.cull.stale());
        }
    });
    assert_eq!(count, 0);
}

#[test]
fn armed_perf_reports_culled_views_and_pass_times_without_hooks() {
    let Some(gpu) = gpu() else { return };
    let mut surface = WorldSurface::<Field>::default();
    surface.bind(&[Value::Bool(false)], None).unwrap();
    let mut frame = Frame {
        width: 320.,
        height: 180.,
        scale: 1.,
        now_ms: 0.,
        seekable: true,
        period_ms: 0.,
        children_generation: 0,
        shader_generation: 0,
        headroom: 1.0,
    };
    fixture::render(&gpu, &mut surface, &frame).unwrap();
    let unarmed = surface.agent("{\"op\":\"state\"}").unwrap();
    assert!(unarmed.contains("\"culled\":null"), "{unarmed}");
    assert!(!unarmed.contains("\"gpuMs\""), "{unarmed}");
    surface.agent("{\"op\":\"state\",\"perf\":true}");
    frame.seekable = false;
    for _ in 0..8 {
        frame.now_ms += 16.;
        fixture::render(&gpu, &mut surface, &frame).unwrap();
    }
    let state = surface.agent("{\"op\":\"state\"}").unwrap();
    assert!(state.contains("\"culled\":{\"camera\":"), "{state}");
    if gpu
        .device
        .features()
        .contains(exact_gpu::wgpu::Features::TIMESTAMP_QUERY)
    {
        assert!(state.contains("\"cull\":{\"p50\""), "{state}");
        assert!(state.contains("\"shadow 0\":{\"p50\""), "{state}");
    }
}

#[test]
fn devices_without_indirect_execution_draw_the_same_pixels_directly() {
    let Some(gpu) = gpu() else { return };
    let mut surface = WorldSurface::<Field>::default();
    surface.bind(&[Value::Bool(false)], None).unwrap();
    let frame = Frame {
        width: 320.,
        height: 180.,
        scale: 1.,
        now_ms: 108.4,
        seekable: true,
        period_ms: 0.,
        children_generation: 0,
        shader_generation: 0,
        headroom: 1.0,
    };
    let (culled, _) = fixture::render(&gpu, &mut surface, &frame).unwrap();
    // As on the iOS simulator: no indirect execution, so every group draws directly.
    let r = surface.renderer_for_test().unwrap();
    r.cull.indirect_execution = false;
    r.cull.epoch += 1;
    let (direct, _) = fixture::render(&gpu, &mut surface, &frame).unwrap();
    assert!(surface.renderer_for_test().unwrap().cull.direct);
    assert!(culled.data == direct.data, "the direct path changed pixels");
}
