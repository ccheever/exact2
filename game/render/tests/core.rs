#![cfg(not(target_arch = "wasm32"))]

use exact_game_render::{shapes, Batch, Environment, FrameInput, PointLightInput, Renderer, Sun};
use exact_gpu::{fixture, wgpu, Gpu};
use glam::camera::rh::{proj::directx, view};
use glam::{Quat, Vec3};

mod support;
use support::*;
mod effects;
mod review;
mod timing;

#[test]
fn lit_scene_and_output_transfer() {
    let Some(gpu) = gpu() else {
        return;
    };
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let mut renderer = Renderer::new(&gpu.device, &gpu.queue, format);
    let mut batches = Vec::new();
    for (slot, (vertices, indices)) in [
        shapes::plane(),
        shapes::cube(),
        shapes::sphere(32),
        shapes::capsule(0.4, 1.8, 32),
    ]
    .into_iter()
    .enumerate()
    {
        let mesh = renderer.add_mesh(&vertices, &indices);
        batches.push(Batch {
            mesh,
            casts_shadows: true,
            slots: slot as u32..slot as u32 + 1,
        });
    }
    for (slot, (p, s, color)) in [
        (Vec3::ZERO, Vec3::new(10.0, 1.0, 8.0), [0.35, 0.38, 0.4]),
        (Vec3::new(-1.5, 0.5, 0.0), Vec3::ONE, [0.65; 3]),
        (
            Vec3::new(0.0, 0.55, 0.0),
            Vec3::splat(1.1),
            [0.75, 0.3, 0.08],
        ),
        (Vec3::new(1.6, 0.9, 0.0), Vec3::ONE, [0.65; 3]),
    ]
    .into_iter()
    .enumerate()
    {
        renderer
            .write_transforms_both(slot as u32, &transform(p, Quat::IDENTITY, s))
            .unwrap();
        renderer
            .write_materials(slot as u32, &material(color, 0.0))
            .unwrap();
    }
    renderer.set_batches(&batches, &[0, 1, 2, 3]).unwrap();
    let mut frame = frame();
    frame.camera_position = Vec3::new(5.0, 4.0, 8.0);
    frame.view = view::look_at_mat4(frame.camera_position, Vec3::new(0.0, 0.6, 0.0), Vec3::Y);
    frame.proj = directx::perspective(45.0_f32.to_radians(), 1.5, 0.1, 100.0);
    frame.sun = Some(Sun {
        direction: Vec3::new(1.0, -2.0, -3.0),
        color: Vec3::ONE,
        shadows: None,
        illuminance: 3.0,
    });
    frame.environment = Environment {
        background: None,
        zenith: [0.12, 0.18, 0.28],
        ground: [0.04, 0.025, 0.02],
        ambient: 0.35,
        horizon: [0.12, 0.18, 0.28],
        sun_disc: 0.0,
        fog: None,
        ..frame.environment
    };
    let texture = target(&gpu, (600, 400), format);
    let unlit_point = render(&gpu, &mut renderer, &texture, &frame);
    let points = [PointLightInput {
        position: Vec3::new(1.6, 1.2, 1.4),
        color: Vec3::new(0.02, 0.1, 1.0),
        intensity: 12.0,
        range: 3.5,
    }];
    frame.points = &points;
    let pixels = render(&gpu, &mut renderer, &texture, &frame);
    pixels.save("lit-scene");
    let sky = pixels.at(5, 5);
    for (actual, expected) in sky[..3]
        .iter()
        .zip(sky_color(&frame, 5, 5, 600, 400).map(tone))
    {
        assert!(actual.abs_diff(expected) <= 2, "sky {sky:?}");
    }
    assert!(pixels.count(|p| p != sky) > 30_000);
    let front = project(&frame, Vec3::new(-1.5, 0.55, 0.501), &pixels);
    let side = project(&frame, Vec3::new(-0.999, 0.55, 0.0), &pixels);
    let lit = pixels.at(front.0, front.1);
    let dark = pixels.at(side.0, side.1);
    eprintln!("cube lit={lit:?}, shadowed={dark:?}, sky={sky:?}");
    assert!(u32::from(lit[0]) > u32::from(dark[0]) + 50);
    let near = project(&frame, Vec3::new(1.6, 1.0, 0.4), &pixels);
    let blue = pixels.at(near.0, near.1);
    let neutral = unlit_point.at(near.0, near.1);
    eprintln!("point nearby: {neutral:?} -> {blue:?}");
    assert!(
        i32::from(blue[2]) - i32::from(neutral[2])
            > i32::from(blue[0]) - i32::from(neutral[0]) + 15
    );

    // An sRGB target must encode exactly once, matching the non-sRGB shader path.
    let srgb_format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let mut srgb = Renderer::new(&gpu.device, &gpu.queue, srgb_format);
    srgb.set_batches(&[], &[]).unwrap();
    let background = render(
        &gpu,
        &mut srgb,
        &target(&gpu, (32, 24), srgb_format),
        &frame,
    );
    assert!(background.at(5, 5)[..3]
        .iter()
        .zip(sky_color(&frame, 5, 5, 32, 24).map(tone))
        .all(|(a, b)| a.abs_diff(b) <= 2));
}

fn silhouette(pixels: &fixture::Pixels) -> (f32, u32) {
    let mut total = 0u64;
    let mut count = 0u64;
    let mut min = pixels.width;
    let mut max = 0;
    for y in 0..pixels.height {
        for x in 0..pixels.width {
            if pixels.at(x, y)[0] > 100 {
                total += u64::from(x);
                count += 1;
                min = min.min(x);
                max = max.max(x);
            }
        }
    }
    assert!(count > 0);
    (total as f32 / count as f32, max - min + 1)
}

#[test]
fn interpolation_teleport_untouched_and_growth() {
    let Some(gpu) = gpu() else {
        return;
    };
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let mut renderer = Renderer::new(&gpu.device, &gpu.queue, format);
    let (v, i) = shapes::cube();
    let cube = renderer.add_mesh(&v, &i);
    renderer
        .write_transforms_both(
            7,
            &transform(Vec3::new(-2.0, 0.0, 0.0), Quat::IDENTITY, Vec3::ONE),
        )
        .unwrap();
    renderer
        .write_materials(7, &material([0.5; 3], 1.0))
        .unwrap();
    renderer
        .set_batches(
            &[Batch {
                mesh: cube,
                casts_shadows: true,
                slots: 1..2,
            }],
            &[7, 7],
        )
        .unwrap();
    renderer.begin_tick();
    renderer
        .write_transforms(
            7,
            &transform(Vec3::new(2.0, 0.0, 0.0), Quat::IDENTITY, Vec3::ONE),
        )
        .unwrap();
    let target = target(&gpu, (256, 160), format);
    let mut frame = frame();
    for (alpha, expected, name) in [
        (0.0, 63.5, "interpolation-left"),
        (0.5, 127.5, "interpolation-middle"),
        (1.0, 191.5, "interpolation-right"),
    ] {
        frame.alpha = alpha;
        let pixels = render(&gpu, &mut renderer, &target, &frame);
        pixels.save(name);
        assert!((silhouette(&pixels).0 - expected).abs() < 1.0);
    }
    renderer
        .write_transforms_both(7, &transform(Vec3::ZERO, Quat::IDENTITY, Vec3::ONE))
        .unwrap();
    renderer.begin_tick();
    renderer
        .write_transforms(
            7,
            &transform(
                Vec3::ZERO,
                Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
                Vec3::ONE,
            ),
        )
        .unwrap();
    frame.alpha = 0.5;
    let rotated = render(&gpu, &mut renderer, &target, &frame);
    rotated.save("rotation-eighth-turn");
    assert!(silhouette(&rotated).1 > 42);
    renderer
        .write_transforms_both(
            7,
            &transform(
                Vec3::ZERO,
                Quat::from_rotation_y(std::f32::consts::FRAC_PI_4),
                Vec3::ONE,
            ),
        )
        .unwrap();
    let reference = render(&gpu, &mut renderer, &target, &frame);
    assert_eq!(
        rotated.data, reference.data,
        "nlerp quarter turn midpoint must be an eighth turn"
    );
    // Antipodal quaternion representations denote the same rotation.
    renderer.begin_tick();
    renderer
        .write_transforms(
            7,
            &transform(
                Vec3::ZERO,
                -Quat::from_rotation_y(std::f32::consts::FRAC_PI_4),
                Vec3::ONE,
            ),
        )
        .unwrap();
    assert_eq!(
        reference.data,
        render(&gpu, &mut renderer, &target, &frame).data
    );

    renderer
        .write_transforms_both(
            7,
            &transform(Vec3::new(1.0, 0.0, 0.0), Quat::IDENTITY, Vec3::ONE),
        )
        .unwrap();
    for tick in 0..3 {
        if tick != 0 {
            renderer.begin_tick();
        }
        for alpha in [0.0, 0.5, 1.0] {
            frame.alpha = alpha;
            assert!(
                (silhouette(&render(&gpu, &mut renderer, &target, &frame)).0 - 159.5).abs() < 1.0
            );
        }
    }
    // Grow every storage arena and both geometry arenas, after uploading live data.
    renderer
        .write_transforms_both(
            1_000_000,
            &transform(Vec3::new(-1.0, 0.0, 0.0), Quat::IDENTITY, Vec3::ONE),
        )
        .unwrap();
    renderer
        .write_materials(1_000_000, &material([0.5; 3], 1.0))
        .unwrap();
    let (v, i) = shapes::sphere(64);
    let sphere = renderer.add_mesh(&v, &i);
    let mut slots = vec![7; 100];
    slots[99] = 1_000_000;
    renderer
        .set_batches(
            &[
                Batch {
                    mesh: cube,
                    casts_shadows: true,
                    slots: 0..1,
                },
                Batch {
                    mesh: sphere,
                    casts_shadows: true,
                    slots: 99..100,
                },
            ],
            &slots,
        )
        .unwrap();
    renderer.begin_tick();
    for alpha in [0.0, 0.5, 1.0] {
        frame.alpha = alpha;
        let pixels = render(&gpu, &mut renderer, &target, &frame);
        assert!(pixels.at(160, 80)[0] > 200 && pixels.at(96, 80)[0] > 200);
        pixels.save("millionth-slot-growth");
    }
}

#[test]
fn nonuniform_scale_matches_baked_normal_matrix() {
    let Some(gpu) = gpu() else {
        return;
    };
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let mut renderer = Renderer::new(&gpu.device, &gpu.queue, format);
    let positions = [
        Vec3::new(-1.0, -0.8, -0.5),
        Vec3::new(1.0, -0.8, 0.5),
        Vec3::new(0.0, 0.8, 0.0),
    ];
    let normal = (positions[1] - positions[0])
        .cross(positions[2] - positions[0])
        .normalize();
    let vertices = positions.map(|p| exact_game_render::Vertex {
        position: p.to_array(),
        normal: normal.to_array(),
        uv: [0.0; 2],
    });
    let rotation = Quat::from_rotation_y(0.3) * Quat::from_rotation_z(0.4);
    let scale = Vec3::new(1.2, 0.7, 2.5);
    let baked = vertices.map(|v| exact_game_render::Vertex {
        position: (rotation * (Vec3::from_array(v.position) * scale)).to_array(),
        normal: (rotation * (normal / scale)).normalize().to_array(),
        uv: v.uv,
    });
    let source = renderer.add_mesh(&vertices, &[0, 1, 2]);
    let reference = renderer.add_mesh(&baked, &[0, 1, 2]);
    renderer
        .write_materials(0, &material([0.6; 3], 0.0))
        .unwrap();
    renderer
        .write_transforms_both(0, &transform(Vec3::ZERO, rotation, scale))
        .unwrap();
    renderer
        .set_batches(
            &[Batch {
                mesh: source,
                casts_shadows: true,
                slots: 0..1,
            }],
            &[0],
        )
        .unwrap();
    let mut frame = frame();
    frame.environment = Environment {
        background: None,
        zenith: [0.3, 0.4, 0.5],
        ground: [0.02; 3],
        ambient: 1.0,
        horizon: [0.16, 0.21, 0.26],
        sun_disc: 0.0,
        fog: None,
        ..frame.environment
    };
    frame.sun = Some(Sun {
        direction: Vec3::new(1.0, -1.0, -2.0),
        color: Vec3::ONE,
        shadows: None,
        illuminance: 3.0,
    });
    let target = target(&gpu, (256, 160), format);
    let actual = render(&gpu, &mut renderer, &target, &frame);
    renderer
        .write_transforms_both(0, &transform(Vec3::ZERO, Quat::IDENTITY, Vec3::ONE))
        .unwrap();
    renderer
        .set_batches(
            &[Batch {
                mesh: reference,
                casts_shadows: true,
                slots: 0..1,
            }],
            &[0],
        )
        .unwrap();
    let expected = render(&gpu, &mut renderer, &target, &frame);
    assert!(actual.at(128, 80)[0] > 150);
    let mean_error = actual
        .data
        .iter()
        .zip(&expected.data)
        .map(|(a, b)| f64::from(a.abs_diff(*b)))
        .sum::<f64>()
        / actual.data.len() as f64;
    assert!(mean_error < 0.1, "normal matrix pixel error: {mean_error}");
}

#[test]
fn affine_attachment_pixels_match_transformed_vertices_and_detach_cleanly() {
    let gpu = fixture::device().unwrap();
    for sign in [1., -1.] {
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let mut r = Renderer::new(&gpu.device, &gpu.queue, format);
        let vertices = [
            ([-1., -1., 0.], [0., 0.]),
            ([1., -1., 0.], [1., 0.]),
            ([0., 1., 0.], [0.5, 1.]),
        ]
        .map(|(position, _)| exact_game_render::Vertex {
            position,
            normal: [0., 0., 1.],
            uv: [0.; 2],
        });
        let indices = vec![0, 1, 2];
        let mesh = r.add_mesh(&vertices, &indices);
        r.write_transforms_both(0, &transform(Vec3::ZERO, Quat::IDENTITY, Vec3::ONE))
            .unwrap();
        r.write_materials(0, &material([0.6, 0.2, 0.1], 1.))
            .unwrap();
        r.set_batches(
            &[Batch {
                mesh,
                casts_shadows: true,
                slots: 0..1,
            }],
            &[0],
        )
        .unwrap();
        let target = target(&gpu, (256, 160), format);
        let mut f = frame();
        let baseline = render(&gpu, &mut r, &target, &f).data;
        let matrix = glam::Mat4::from_scale_rotation_translation(
            Vec3::new(2. * sign, 1., 1.),
            Quat::IDENTITY,
            Vec3::new(0.3, 0.2, 0.),
        ) * glam::Mat4::from_rotation_z(0.7);
        let (scale, rotation, position) = matrix.to_scale_rotation_translation();
        let mut w = exact_game::World::new(60, 0);
        let entity = w.spawn(());
        let attachments = [exact_game_render::DisplayedAttachment {
            entity,
            matrix,
            pose: exact_game::Transform {
                scale,
                rotation,
                position,
            },
        }];
        f.attachments = &attachments;
        let attached = render(&gpu, &mut r, &target, &f).data;
        assert_ne!(attached, baseline);
        f.attachments = &[];
        assert_eq!(
            render(&gpu, &mut r, &target, &f).data,
            baseline,
            "detaching clears the affine override"
        );
        let transformed: Vec<_> = vertices
            .iter()
            .map(|v| exact_game_render::Vertex {
                position: matrix
                    .transform_point3(Vec3::from_array(v.position))
                    .to_array(),
                normal: matrix
                    .inverse()
                    .transpose()
                    .transform_vector3(Vec3::from_array(v.normal))
                    .to_array(),
                ..*v
            })
            .collect();
        let mut indices = indices;
        if sign < 0. {
            for face in indices.chunks_exact_mut(3) {
                face.swap(0, 2);
            }
        }
        let expected = r.add_mesh(&transformed, &indices);
        r.set_batches(
            &[Batch {
                mesh: expected,
                casts_shadows: true,
                slots: 0..1,
            }],
            &[0],
        )
        .unwrap();
        let expected = render(&gpu, &mut r, &target, &f).data;
        let differences = attached
            .iter()
            .zip(&expected)
            .filter(|(a, b)| a != b)
            .count();
        assert!(attached == expected, "shader must preserve the full affine map: sign {sign}, {differences} different channels");
    }
}

#[test]
fn declared_storage_needs_fit_the_host_device() {
    let Some(gpu) = gpu() else { return };
    let requested = exact_gpu::requested_limits(gpu.adapter.limits());
    assert!(exact_game_render::STORAGE_BINDINGS <= requested.max_storage_buffers_per_shader_stage);
    assert!(
        exact_game_render::STORAGE_BINDINGS
            <= gpu.device.limits().max_storage_buffers_per_shader_stage
    );
}
