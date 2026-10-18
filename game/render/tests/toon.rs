//! Pixel oracles for the optional toon look and scene parity settings.
#![cfg(not(target_arch = "wasm32"))]
#[path = "fixture/device.rs"]
mod test_device;
use exact_game::{asset::*, *};
use exact_game_render::{Feed, FrameInput, Renderer, Sun};
use exact_gpu::{fixture, wgpu, Gpu};
use glam::camera::rh::{proj::directx, view};

fn sphere(outline: Option<(f32, f32, f32)>, rim: f32) -> Model {
    let (v, indices) = exact_game_render::shapes::sphere(64);
    let normals: Vec<f32> = v.iter().flat_map(|v| v.normal).collect();
    Model {
        meshes: vec![MeshData {
            positions: v.iter().flat_map(|v| v.position).collect(),
            normals: normals.clone(),
            uvs: v.iter().flat_map(|v| v.uv).collect(),
            indices,
            outline: outline.map(|(width, min_pixels, reach)| MeshOutline {
                width,
                min_pixels,
                color: [1., 0., 0.],
                normals,
                reach: vec![reach; v.len()],
            }),
            bounds: [-0.5, -0.5, -0.5, 0.5, 0.5, 0.5],
            ..Default::default()
        }],
        materials: vec![MaterialData {
            base_color: [0.25; 4],
            metallic: 0.,
            roughness: 1.,
            emissive: [0., 0., 0.5],
            rim,
            ..Default::default()
        }],
        nodes: vec![Node {
            mesh: Some(0),
            ..Default::default()
        }],
        bounds: [-0.5, -0.5, -0.5, 0.5, 0.5, 0.5],
        ..Default::default()
    }
}
fn frame() -> FrameInput<'static> {
    FrameInput {
        view: view::look_at_mat4(Vec3::new(0., 0., 5.), Vec3::ZERO, Vec3::Y),
        proj: directx::orthographic(-2., 2., -2., 2., 0.1, 100.),
        camera_position: Vec3::new(0., 0., 5.),
        sun: None,
        environment: Environment {
            background: Some([0.; 3]),
            ambient: 0.,
            fog: None,
            bloom: None,
            tone_mapping: ToneMapping::None,
            ..Default::default()
        },
        ..Default::default()
    }
}
fn render(
    gpu: &Gpu,
    model: Model,
    poses: &[Transform],
    f: &FrameInput<'_>,
    samples: SceneSamples,
    ground: bool,
) -> (fixture::Pixels, exact_game_render::Stats) {
    let (pixels, stats, _) = measured(gpu, model, poses, f, samples, ground);
    (pixels, stats)
}
fn measured(
    gpu: &Gpu,
    model: Model,
    poses: &[Transform],
    f: &FrameInput<'_>,
    samples: SceneSamples,
    ground: bool,
) -> (fixture::Pixels, exact_game_render::Stats, [u64; 4]) {
    measured_mode(gpu, model, poses, f, samples, ground, (false, true))
}
fn measured_mode(
    gpu: &Gpu,
    model: Model,
    poses: &[Transform],
    f: &FrameInput<'_>,
    samples: SceneSamples,
    ground: bool,
    (prepass, direct): (bool, bool),
) -> (fixture::Pixels, exact_game_render::Stats, [u64; 4]) {
    let mut w = World::new(60, 1);
    let mesh = w.generated_model("toon.model", model).unwrap();
    for pose in poses {
        w.spawn((*pose, mesh.clone()));
    }
    if ground {
        w.spawn((
            Transform {
                position: Vec3::new(0., 0., -0.8),
                scale: Vec3::new(4., 4., 0.05),
                ..Default::default()
            },
            Mesh::cube(1.),
            Material::rgb(0.5, 0.5, 0.5),
        ));
    }
    let mut r = Renderer::configured(
        &gpu.device,
        &gpu.queue,
        wgpu::TextureFormat::Rgba8Unorm,
        samples,
        prepass,
    );
    r.set_direct(direct);
    r.prepare_model("toon.model", w.model("toon.model").unwrap())
        .unwrap();
    w.propagate();
    Feed::default().feed(&w, &mut r).unwrap();
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("toon pixel oracle"),
        size: wgpu::Extent3d {
            width: 128,
            height: 128,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    r.count_culled(true);
    let stats = r.draw(&texture.create_view(&Default::default()), (128, 128), f);
    let pixels = fixture::read(gpu, &texture).unwrap();
    gpu.device
        .poll(wgpu::PollType::wait_indefinitely())
        .unwrap();
    (pixels, stats, r.culled().unwrap())
}
fn red(p: [u8; 4]) -> bool {
    p[0] > 180 && p[1] < 30 && p[2] < 30
}
fn blue(p: [u8; 4]) -> bool {
    p[2] > 100 && p[0] < 30
}
fn edge(p: &fixture::Pixels, predicate: fn([u8; 4]) -> bool) -> u32 {
    (64..128).filter(|&x| predicate(p.at(x, 64))).max().unwrap()
}
fn srgb(v: f32) -> i32 {
    ((if v <= 0.0031308 {
        v * 12.92
    } else {
        1.055 * v.powf(1. / 2.4) - 0.055
    }) * 255.)
        .round() as i32
}

#[test]
fn hull_width_reach_and_css_pixel_floor_at_two_depths() {
    let Some(gpu) = test_device::device_or_skip(fixture::device()) else {
        return;
    };
    let f = frame();
    let plain = render(
        &gpu,
        sphere(None, 0.),
        &[Transform::default()],
        &f,
        SceneSamples::One,
        false,
    )
    .0;
    let (hull, stats) = render(
        &gpu,
        sphere(Some((0.125, 0., 1.)), 0.),
        &[Transform::default()],
        &f,
        SceneSamples::One,
        false,
    );
    let width = edge(&hull, red) - edge(&plain, blue);
    assert!((3..=5).contains(&width), "0.125m at 32px/m: {width}px");
    assert_eq!(
        hull.at(64, 64),
        plain.at(64, 64),
        "hull depth-tests behind the surface"
    );
    assert_eq!(stats.draws, 3, "surface, hull, tone");
    let capped = render(
        &gpu,
        sphere(Some((0.5, 0., 0.063)), 0.),
        &[Transform::default()],
        &f,
        SceneSamples::One,
        false,
    )
    .0;
    let width = edge(&capped, red) - edge(&plain, blue);
    assert!(
        (1..=3).contains(&width),
        "reach caps 0.5m width at 0.063m: {width}px"
    );
    for depth in [5., 10.] {
        let mut f = frame();
        f.proj = directx::perspective(60f32.to_radians(), 1., 0.1, 100.);
        f.camera_position.z = depth;
        f.view = view::look_at_mat4(f.camera_position, Vec3::ZERO, Vec3::Y);
        // 128 physical pixels over 64 CSS pixels: a 2 CSS-pixel floor is 4 pixels.
        f.viewport_css_height = Some(64.);
        let plain = render(
            &gpu,
            sphere(None, 0.),
            &[Transform::default()],
            &f,
            SceneSamples::One,
            false,
        )
        .0;
        let hull = render(
            &gpu,
            sphere(Some((0.001, 2., 1.)), 0.),
            &[Transform::default()],
            &f,
            SceneSamples::One,
            false,
        )
        .0;
        let width = edge(&hull, red) - edge(&plain, blue);
        assert!(
            (3..=5).contains(&width),
            "depth {depth}, CSS floor: {width}px"
        );
    }
}
#[test]
fn hull_instances_mirrors_and_rigid_skin_follow_the_surface() {
    let Some(gpu) = test_device::device_or_skip(fixture::device()) else {
        return;
    };
    let poses = [
        Transform::at(-0.8, 0., 0.),
        Transform {
            position: Vec3::new(0.8, 0., 0.),
            scale: Vec3::new(-1., 1., 1.),
            ..Default::default()
        },
    ];
    let model = sphere(Some((0.125, 0., 1.)), 0.);
    let rigid = render(
        &gpu,
        model.clone(),
        &poses,
        &frame(),
        SceneSamples::One,
        false,
    )
    .0;
    let joint = glam::Mat4::from_scale_rotation_translation(
        Vec3::new(0.8, 1.1, 0.7),
        glam::Quat::from_rotation_z(0.3),
        Vec3::new(0., 0.1, 0.),
    );
    let mut transformed = model.clone();
    transformed.nodes[0].transform = joint.to_cols_array();
    let transformed = render(
        &gpu,
        transformed,
        &poses,
        &frame(),
        SceneSamples::One,
        false,
    )
    .0;
    let mut skinned = model;
    let count = skinned.meshes[0].positions.len() / 3;
    skinned.meshes[0].joints = vec![0; count * 4];
    skinned.meshes[0].weights = [1., 0., 0., 0.].repeat(count);
    skinned.nodes[0].skin = Some(0);
    skinned.nodes.push(Node {
        transform: joint.to_cols_array(),
        ..Default::default()
    });
    skinned.skins = vec![Skin {
        joints: vec![1],
        inverse_binds: glam::Mat4::IDENTITY.to_cols_array().to_vec(),
        ..Default::default()
    }];
    let skin = render(&gpu, skinned, &poses, &frame(), SceneSamples::One, false).0;
    for (cx, left, right) in [(38, 18, 57), (89, 70, 109)] {
        assert!(blue(rigid.at(cx, 64)));
        assert!(red(rigid.at(left, 64)));
        assert!(red(rigid.at(right, 64)));
    }
    for y in 0..128 {
        for x in 0..128 {
            assert_eq!(
                transformed.at(x, y),
                skin.at(x, y),
                "rigid weight-one skin at {x},{y}"
            );
        }
    }
}
#[test]
fn nonuniform_node_hull_matches_forward_offset_geometry() {
    let Some(gpu) = test_device::device_or_skip(fixture::device()) else {
        return;
    };
    let node = glam::Mat4::from_scale_rotation_translation(
        Vec3::new(2.8, 0.65, 1.),
        Quat::from_rotation_z(0.4),
        Vec3::ZERO,
    );
    for reach in [0.08, 1.] {
        let mut model = sphere(Some((0.2, 0., reach)), 0.);
        model.nodes[0].transform = node.to_cols_array();
        let actual = render(
            &gpu,
            model.clone(),
            &[Transform::default()],
            &frame(),
            SceneSamples::One,
            false,
        )
        .0;
        // Independent forward-offset oracle: transform the offset forward,
        // clamp its length, and render the resulting back faces as ordinary geometry.
        let mut hull = model.meshes[0].clone();
        hull.outline = None;
        hull.material = 1;
        for (p, n) in hull
            .positions
            .chunks_exact_mut(3)
            .zip(hull.normals.chunks_exact(3))
        {
            let offset = node.transform_vector3(Vec3::from_slice(n) * reach);
            let world = node.transform_point3(Vec3::from_slice(p))
                + offset.normalize() * 0.2f32.min(offset.length());
            p.copy_from_slice(&world.to_array());
        }
        for triangle in hull.indices.chunks_exact_mut(3) {
            triangle.swap(0, 2);
        }
        hull.bounds = [-2., -2., -2., 2., 2., 2.];
        model.meshes[0].outline = None;
        model.meshes.push(hull);
        model.nodes.push(Node {
            mesh: Some(1),
            ..Default::default()
        });
        model.materials.push(MaterialData {
            base_color: [0., 0., 0., 1.],
            emissive: [1., 0., 0.],
            ..Default::default()
        });
        let expected = render(
            &gpu,
            model,
            &[Transform::default()],
            &frame(),
            SceneSamples::One,
            false,
        )
        .0;
        let mut differences = 0;
        let mut outline_pixels = 0;
        for y in 0..128 {
            for x in 0..128 {
                differences += usize::from(red(actual.at(x, y)) != red(expected.at(x, y)));
                outline_pixels += usize::from(red(expected.at(x, y)));
            }
        }
        assert!(outline_pixels > 100, "oracle must expose the hull");
        assert!(
            differences <= 4,
            "reach {reach}: {differences} silhouette pixels differ"
        );
    }
}

#[test]
fn hull_never_expands_the_shadow_and_is_fogged() {
    let Some(gpu) = test_device::device_or_skip(fixture::device()) else {
        return;
    };
    let mut f = frame();
    f.sun = Some(Sun {
        direction: Vec3::new(0., -1., -1.).normalize(),
        illuminance: 3.,
        shadows: Some(Default::default()),
        ..Default::default()
    });
    let plain = render(
        &gpu,
        sphere(None, 0.),
        &[Transform::default()],
        &f,
        SceneSamples::One,
        true,
    )
    .0;
    let hull = render(
        &gpu,
        sphere(Some((0.125, 0., 1.)), 0.),
        &[Transform::default()],
        &f,
        SceneSamples::One,
        true,
    )
    .0;
    f.sun.as_mut().unwrap().shadows = None;
    let unshadowed = render(
        &gpu,
        sphere(None, 0.),
        &[Transform::default()],
        &f,
        SceneSamples::One,
        true,
    )
    .0;
    let mut dark = 0;
    for y in 90..118 {
        for x in 42..86 {
            assert_eq!(
                plain.at(x, y),
                hull.at(x, y),
                "outline changed receiver shadow at {x},{y}"
            );
            dark +=
                usize::from(i32::from(plain.at(x, y)[0]) + 10 < i32::from(unshadowed.at(x, y)[0]));
        }
    }
    assert!(
        dark > 30,
        "receiver crop must include a real shadow: {dark}"
    );
    f = frame();
    f.environment.fog = Some(Fog {
        color: Some([0., 1., 0.]),
        ..Fog::linear(0., 1.)
    });
    let fogged = render(
        &gpu,
        sphere(Some((0.125, 0., 1.)), 0.),
        &[Transform::default()],
        &f,
        SceneSamples::One,
        false,
    )
    .0;
    assert_eq!(fogged.at(82, 64), [0, 255, 0, 255]);
}
#[test]
fn rim_linear_fog_output_transfer_and_single_sample_edges() {
    let Some(gpu) = test_device::device_or_skip(fixture::device()) else {
        return;
    };
    let f = frame();
    let plain = render(
        &gpu,
        sphere(None, 0.),
        &[Transform::default()],
        &f,
        SceneSamples::One,
        false,
    )
    .0;
    let rim = render(
        &gpu,
        sphere(None, 1.),
        &[Transform::default()],
        &f,
        SceneSamples::One,
        false,
    )
    .0;
    assert!(
        rim.at(78, 64)[0] > plain.at(78, 64)[0] + 60,
        "rim lights grazing normals"
    );
    assert!(rim.at(64, 64)[0] < 5, "no head-on rim");
    let mut model = sphere(None, 0.);
    model.materials[0].emissive = [0.25; 3];
    let linear = render(
        &gpu,
        model.clone(),
        &[Transform::default()],
        &f,
        SceneSamples::One,
        false,
    )
    .0;
    assert!((i32::from(linear.at(64, 64)[0]) - srgb(0.25)).abs() <= 2);
    let mut aces = frame();
    aces.environment.tone_mapping = ToneMapping::Aces;
    let curved = render(
        &gpu,
        model.clone(),
        &[Transform::default()],
        &aces,
        SceneSamples::One,
        false,
    )
    .0;
    assert!(curved.at(64, 64)[0] > linear.at(64, 64)[0] + 20);
    let mut fog = frame();
    fog.environment.fog = Some(Fog {
        color: Some([0.75; 3]),
        ..Fog::linear(2., 7.)
    });
    // Sphere's front centre is at view depth 4.5: halfway from 2 to 7.
    let mid = render(
        &gpu,
        model.clone(),
        &[Transform::default()],
        &fog,
        SceneSamples::One,
        false,
    )
    .0;
    assert!((i32::from(mid.at(64, 64)[0]) - srgb(0.5)).abs() <= 2);
    fog.environment.fog = Some(Fog {
        color: Some([0.75; 3]),
        ..Fog::linear(6., 7.)
    });
    let near = render(
        &gpu,
        model.clone(),
        &[Transform::default()],
        &fog,
        SceneSamples::One,
        false,
    )
    .0;
    assert_eq!(near.at(64, 64), linear.at(64, 64));
    let aa = render(
        &gpu,
        model,
        &[Transform::default()],
        &f,
        SceneSamples::Four,
        false,
    )
    .0;
    let intermediate = |p: &fixture::Pixels| {
        (40..88)
            .flat_map(|y| (40..88).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                let v = p.at(x, y)[0];
                v > 5 && v < 130
            })
            .count()
    };
    assert_eq!(
        intermediate(&linear),
        0,
        "1x has no fractional edge coverage"
    );
    assert!(
        intermediate(&aa) > 20,
        "4x retains fractional edge coverage"
    );
}

#[test]
fn hull_visibility_outside_original_bounds_and_single_sample_ssao() {
    let Some(gpu) = test_device::device_or_skip(fixture::device()) else {
        return;
    };
    let f = frame();
    let offscreen = render(
        &gpu,
        sphere(Some((0.75, 0., 1.)), 0.),
        &[Transform::at(3., 0., 0.)],
        &f,
        SceneSamples::One,
        false,
    )
    .0;
    assert!(
        red(offscreen.at(125, 64)),
        "hull still reaches the viewport when the surface's original bounding sphere does not"
    );
    let mut ao = frame();
    ao.ambient_occlusion = Some(AmbientOcclusion::default());
    let plain = render(
        &gpu,
        sphere(None, 0.),
        &[Transform::default()],
        &f,
        SceneSamples::One,
        true,
    )
    .0;
    let contact = render(
        &gpu,
        sphere(None, 0.),
        &[Transform::default()],
        &ao,
        SceneSamples::One,
        true,
    )
    .0;
    assert!(
        contact.at(64, 64)[2] > 100,
        "single-sample depth is readable by SSAO"
    );
    for y in 56..72 {
        for x in 56..72 {
            assert!(contact.at(x, y)[2] <= plain.at(x, y)[2] + 1);
        }
    }
}

#[test]
fn outline_pipeline_draws_with_exactly_the_advertised_storage_limit() {
    let Some(mut gpu) = test_device::device_or_skip(fixture::device()) else {
        return;
    };
    (gpu.device, gpu.queue) =
        exact_gpu::block_on(gpu.adapter.request_device(&wgpu::DeviceDescriptor {
            required_limits: wgpu::Limits {
                max_storage_buffers_per_shader_stage: exact_game_render::STORAGE_BINDINGS,
                ..Default::default()
            },
            ..Default::default()
        }))
        .unwrap();
    assert_eq!(
        gpu.device.limits().max_storage_buffers_per_shader_stage,
        exact_game_render::STORAGE_BINDINGS
    );
    let (pixels, stats) = render(
        &gpu,
        sphere(Some((0.125, 0., 1.)), 0.),
        &[Transform::default()],
        &frame(),
        SceneSamples::One,
        false,
    );
    assert!(red(pixels.at(82, 64)));
    assert_eq!(stats.draws, 3);
}

#[test]
fn huge_reach_does_not_retain_offscreen_surfaces_or_shadow_casters() {
    let Some(gpu) = test_device::device_or_skip(fixture::device()) else {
        return;
    };
    let poses = [
        Transform::default(),
        Transform::at(5., 0., 0.),
        Transform::at(100., 0., 0.),
    ];
    for perspective in [false, true] {
        let mut f = frame();
        if perspective {
            f.proj = directx::perspective(60f32.to_radians(), 1., 0.1, 100.);
        }
        f.sun = Some(Sun {
            shadows: Some(Default::default()),
            ..Default::default()
        });
        let plain = measured(&gpu, sphere(None, 0.), &poses, &f, SceneSamples::One, false).2;
        let hull = measured(
            &gpu,
            sphere(Some((0.01, 1.1, 500_000.)), 0.),
            &poses,
            &f,
            SceneSamples::One,
            false,
        )
        .2;
        assert_eq!(plain[0], 1);
        assert_eq!(
            hull, plain,
            "reach only caps displacement; it does not enlarge caster bounds"
        );
    }
    let clipped = measured(
        &gpu,
        sphere(Some((100., 0., 0.001)), 0.),
        &[Transform::at(5., 0., 0.)],
        &frame(),
        SceneSamples::One,
        false,
    )
    .2;
    assert_eq!(
        clipped[0], 0,
        "transformed reach caps a large requested width"
    );
}

#[test]
fn compatible_outlined_parts_merge_surface_hull_and_caster_draws() {
    let Some(gpu) = test_device::device_or_skip(fixture::device()) else {
        return;
    };
    let mut base = sphere(Some((0.025, 1.1, 0.1)), 0.);
    let (vertices, indices) = exact_game_render::shapes::sphere(8);
    let mesh = &mut base.meshes[0];
    mesh.positions = vertices.iter().flat_map(|v| v.position).collect();
    mesh.normals = vertices.iter().flat_map(|v| v.normal).collect();
    mesh.uvs = vertices.iter().flat_map(|v| v.uv).collect();
    mesh.indices = indices;
    mesh.outline.as_mut().unwrap().normals = mesh.normals.clone();
    mesh.outline.as_mut().unwrap().reach = vec![0.1; vertices.len()];
    base.meshes = vec![base.meshes[0].clone(); 64];
    base.nodes = (0..64)
        .map(|i| Node {
            name: format!("part{i}"),
            mesh: Some(i),
            transform: glam::Mat4::from_scale_rotation_translation(
                Vec3::new(0.35, 0.18, 0.25),
                Quat::from_rotation_z(i as f32 * 0.13),
                Vec3::new((i % 8) as f32 * 0.45 - 1.6, (i / 8) as f32 * 0.45 - 1.6, 0.),
            )
            .to_cols_array(),
            ..Default::default()
        })
        .collect();
    let mut f = frame();
    f.sun = Some(Sun {
        shadows: Some(exact_game_render::Shadows {
            cascades: 1,
            ..Default::default()
        }),
        ..Default::default()
    });
    let poses = [Transform {
        scale: Vec3::new(1.1, 0.9, 1.),
        ..Default::default()
    }];
    for animated in [false, true] {
        let mut merged = base.clone();
        if animated {
            merged.clips.push(Clip {
                name: "bind".into(),
                ..Default::default()
            });
        }
        let mut separate = merged.clone();
        // Identical materials with distinct IDs force the pre-fix draw list.
        separate.materials = vec![separate.materials[0].clone(); 64];
        for (i, mesh) in separate.meshes.iter_mut().enumerate() {
            mesh.material = i as u32;
        }
        let (before, before_stats, _) =
            measured(&gpu, separate, &poses, &f, SceneSamples::One, false);
        let (after, after_stats, _) =
            measured(&gpu, merged.clone(), &poses, &f, SceneSamples::One, false);
        assert_eq!(
            before_stats.draws, 193,
            "64 surfaces + 64 hulls + 64 casters + tone"
        );
        assert_eq!(
            after_stats.draws, 4,
            "one surface + one hull + one caster + tone"
        );
        println!("outlined parts (animated={animated}): surface 64 -> 1, hull 64 -> 1, caster per cascade 64 -> 1; total {} -> {}", before_stats.draws, after_stats.draws);
        let changed = before
            .data
            .chunks_exact(4)
            .zip(after.data.chunks_exact(4))
            .filter(|(a, b)| a.iter().zip(*b).any(|(a, b)| a.abs_diff(*b) > 8))
            .count();
        assert!(
            changed <= 8,
            "merged normals/reach/palettes changed {changed} pixels"
        );
        merged.meshes[0].outline.as_mut().unwrap().width *= 2.;
        let split = measured(&gpu, merged, &poses, &f, SceneSamples::One, false).1;
        assert_eq!(
            split.draws, 7,
            "different hull settings retain separate draws"
        );
    }
}

#[test]
fn nonuniform_instances_push_in_the_castle_direction() {
    let Some(gpu) = test_device::device_or_skip(fixture::device()) else {
        return;
    };
    let scale = Vec3::new(2., 0.75, 1.);
    let model = sphere(Some((0.2, 0., 1.)), 0.);
    let actual = render(
        &gpu,
        model.clone(),
        &[Transform {
            scale,
            ..Default::default()
        }],
        &frame(),
        SceneSamples::One,
        false,
    )
    .0;
    let mut reference = model;
    reference.meshes[0].outline = None;
    let mut hull = reference.meshes[0].clone();
    for (p, n) in hull
        .positions
        .chunks_exact_mut(3)
        .zip(hull.normals.chunks_exact(3))
    {
        let expanded =
            Vec3::from_slice(p) * scale + (Vec3::from_slice(n) * scale).normalize() * 0.2;
        p.copy_from_slice(&expanded.to_array());
    }
    for triangle in hull.indices.chunks_exact_mut(3) {
        triangle.swap(1, 2);
    }
    hull.material = 1;
    reference.meshes.push(hull);
    reference.materials.push(MaterialData {
        base_color: [0., 0., 0., 1.],
        emissive: [1., 0., 0.],
        ..Default::default()
    });
    reference.nodes[0].transform = glam::Mat4::from_scale(scale).to_cols_array();
    reference.nodes.push(Node {
        mesh: Some(1),
        ..Default::default()
    });
    let expected = render(
        &gpu,
        reference,
        &[Transform::default()],
        &frame(),
        SceneSamples::One,
        false,
    )
    .0;
    assert_eq!(
        actual.data, expected.data,
        "mat3(instanceMatrix) determines the hull direction"
    );
}

#[test]
fn linear_fog_leaves_the_sky_unchanged_even_with_height_density() {
    let Some(gpu) = test_device::device_or_skip(fixture::device()) else {
        return;
    };
    let mut f = frame();
    f.environment.background = None;
    f.environment.sun_disc = 0.;
    f.proj = directx::perspective(120f32.to_radians(), 1., 0.1, 100.);
    let clear = render(
        &gpu,
        sphere(None, 0.),
        &[Transform::at(1000., 0., 0.)],
        &f,
        SceneSamples::One,
        false,
    )
    .0;
    for density in [Fog::linear(0., 1.).density, 1.] {
        f.environment.fog = Some(Fog {
            density,
            color: Some([0., 1., 0.]),
            ..Fog::linear(0., 1.)
        });
        let foggy = render(
            &gpu,
            sphere(None, 0.),
            &[Transform::at(1000., 0., 0.)],
            &f,
            SceneSamples::One,
            false,
        )
        .0;
        for y in [0, 32, 64, 96, 127] {
            assert_eq!(
                clear.at(64, y),
                foggy.at(64, y),
                "sky row {y}, density {density}"
            );
        }
        assert_ne!(foggy.at(64, 0), foggy.at(64, 127), "retain sky gradient");
    }
}

/// A fading model's hull drops the same dithered texels as its surface; an
/// opaque model's hull, drawn without a discard, is whole.
#[test]
fn fading_hulls_dither_and_opaque_hulls_draw_whole() {
    let Some(gpu) = test_device::device_or_skip(fixture::device()) else {
        return;
    };
    let draw = |opacity: Option<f32>, direct: bool| {
        let mut w = World::new(60, 1);
        let mesh = w
            .generated_model("toon.model", sphere(Some((0.125, 0., 1.)), 0.))
            .unwrap();
        let left = w.spawn((Transform::at(-1., 0., 0.), mesh.clone()));
        w.spawn((Transform::at(1., 0., 0.), mesh));
        if let Some(opacity) = opacity {
            w.insert(left, Opacity(opacity));
        }
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let mut r = Renderer::with_samples(&gpu.device, &gpu.queue, format, SceneSamples::One);
        r.set_direct(direct);
        r.prepare_model("toon.model", w.model("toon.model").unwrap())
            .unwrap();
        w.propagate();
        Feed::default().feed(&w, &mut r).unwrap();
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("fading hull oracle"),
            size: wgpu::Extent3d {
                width: 128,
                height: 128,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        r.draw(
            &texture.create_view(&Default::default()),
            (128, 128),
            &frame(),
        );
        let pixels = fixture::read(&gpu, &texture).unwrap();
        pixels.save(&format!("hull-fade-{opacity:?}-{direct}"));
        pixels
    };
    // Red hull pixels in the left and right halves.
    let hull = |p: &fixture::Pixels| {
        let count = |xs: std::ops::Range<u32>| {
            xs.flat_map(|x| (0..128).map(move |y| (x, y)))
                .filter(|&(x, y)| red(p.at(x, y)))
                .count()
        };
        (count(0..64), count(64..128))
    };
    for direct in [false, true] {
        let whole = draw(None, direct);
        let faded = draw(Some(0.5), direct);
        let (left, right) = hull(&whole);
        assert!(left > 100 && left == right, "{left} {right}");
        let (dithered, opaque) = hull(&faded);
        assert_eq!(opaque, right, "the opaque model's hull is unchanged");
        assert!(
            dithered * 10 > left * 3 && dithered * 10 < left * 7,
            "half the hull's texels drop: {dithered} of {left}"
        );
        for y in 0..128 {
            for x in 64..128 {
                assert_eq!(whole.at(x, y), faded.at(x, y), "({x},{y})");
            }
        }
    }
}

#[test]
fn camera_prepass_preserves_model_coverage_and_hulls() {
    let Some(gpu) = test_device::device_or_skip(fixture::device()) else {
        return;
    };
    for direct in [false, true] {
        for alpha in [AlphaMode::Opaque, AlphaMode::Mask, AlphaMode::Blend] {
            for mirrored in [false, true] {
                let mut model = sphere((alpha == AlphaMode::Opaque).then_some((0.02, 1., 1.)), 0.3);
                model.materials[0].alpha_mode = alpha;
                model.materials[0].base_color[3] = if alpha == AlphaMode::Mask { 0.2 } else { 0.6 };
                let poses = [
                    Transform {
                        scale: Vec3::new(if mirrored { -1. } else { 1. }, 1., 1.),
                        ..Default::default()
                    },
                    Transform {
                        position: Vec3::new(0.3, 0.1, -0.3),
                        ..Default::default()
                    },
                ];
                let f = frame();
                let a = measured_mode(
                    &gpu,
                    model.clone(),
                    &poses,
                    &f,
                    SceneSamples::One,
                    true,
                    (false, direct),
                )
                .0;
                let b = measured_mode(
                    &gpu,
                    model,
                    &poses,
                    &f,
                    SceneSamples::One,
                    true,
                    (true, direct),
                )
                .0;
                let mut changed = 0;
                for y in 0..128 {
                    for x in 0..128 {
                        if a.at(x, y)
                            .into_iter()
                            .zip(b.at(x, y))
                            .any(|(a, b)| a.abs_diff(b) > 2)
                        {
                            changed += 1;
                        }
                    }
                }
                assert!(
                    changed < 24,
                    "direct={direct}, {alpha:?}, mirrored={mirrored}: {changed} changed pixels"
                );
            }
        }
    }
}

#[test]
fn camera_depth_rejects_clear_depth_and_keeps_near_far_fragments() {
    let Some(gpu) = test_device::device_or_skip(fixture::device()) else {
        return;
    };
    let mut f = frame();
    f.view = Mat4::IDENTITY;
    f.proj = Mat4::IDENTITY;
    for z in [0.999, f32::from_bits(1f32.to_bits() - 1), 1., 1.001] {
        let mut model = sphere(None, 0.);
        model.meshes[0] = MeshData {
            positions: vec![-0.8, -0.8, z, 0.8, -0.8, z, 0.8, 0.8, z, -0.8, 0.8, z],
            normals: [0., 0., 1.].repeat(4),
            uvs: vec![0.; 8],
            indices: vec![0, 1, 2, 0, 2, 3],
            bounds: [-0.8, -0.8, z, 0.8, 0.8, z],
            ..Default::default()
        };
        model.bounds = model.meshes[0].bounds;
        let a = measured_mode(
            &gpu,
            model.clone(),
            &[Transform::default()],
            &f,
            SceneSamples::One,
            false,
            (false, true),
        )
        .0;
        let b = measured_mode(
            &gpu,
            model,
            &[Transform::default()],
            &f,
            SceneSamples::One,
            false,
            (true, true),
        )
        .0;
        for y in 0..128 {
            for x in 0..128 {
                assert_eq!(a.at(x, y), b.at(x, y), "depth {z}, ({x},{y})");
            }
        }
        assert_eq!(b.at(64, 64)[2] > 0, z < 1., "depth {z}");
    }
}

#[test]
fn absent_normal_map_is_flat_in_textured_and_untextured_materials() {
    let Some(gpu) = test_device::device_or_skip(fixture::device()) else {
        return;
    };
    let mut model = sphere(None, 0.);
    // Amplify the old RGBA8 neutral-map quantization error.
    model.materials[0].normal_scale = 100.;
    let mut f = frame();
    f.sun = Some(Sun {
        direction: Vec3::new(-1., -0.3, -0.1).normalize(),
        illuminance: 2.,
        color: Vec3::ONE,
        shadows: None,
    });
    let a = render(
        &gpu,
        model.clone(),
        &[Transform::default()],
        &f,
        SceneSamples::One,
        false,
    )
    .0;
    model.materials[0].base_color_texture = Some(0);
    // A pending base texture binds white, but selects the textured pipeline.
    model.textures = vec!["pending-white.tex".into()];
    let b = render(
        &gpu,
        model,
        &[Transform::default()],
        &f,
        SceneSamples::One,
        false,
    )
    .0;
    // Within a level: in half precision the two pipelines round apart.
    for y in 0..128 {
        for x in 0..128 {
            let (p, q) = (a.at(x, y), b.at(x, y));
            assert!(
                p.iter().zip(q).all(|(p, q)| p.abs_diff(q) <= 1),
                "({x},{y}) {p:?} {q:?}"
            );
        }
    }
}
