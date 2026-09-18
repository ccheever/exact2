use super::*;
use exact_game_render::{Bloom, Fog, Shadows};

fn shadow_scene(gpu: &Gpu, depth: f32) -> (Renderer, FrameInput<'static>, Vec<Batch>) {
    let mut r = Renderer::new(&gpu.device, &gpu.queue, wgpu::TextureFormat::Rgba8Unorm);
    let (v, i) = shapes::plane();
    let plane = r.add_mesh(&v, &i);
    let (v, i) = shapes::cube();
    let cube = r.add_mesh(&v, &i);
    r.write_transforms_both(
        0,
        &transform(
            Vec3::new(0.0, 0.0, -40.0),
            Quat::IDENTITY,
            Vec3::splat(200.0),
        ),
    )
    .unwrap();
    r.write_transforms_both(
        1,
        &transform(Vec3::new(0.0, 0.5, -depth), Quat::IDENTITY, Vec3::ONE),
    )
    .unwrap();
    r.write_materials(0, &material([0.5; 3], 0.0).repeat(2))
        .unwrap();
    let batches = vec![Batch::new(plane, 0..1), Batch::new(cube, 1..2)];
    r.set_batches(&batches, &[0, 1]).unwrap();
    let mut f = frame();
    // With near=2 the practical splits are 10.75, 25.72, 60 m: 5/25/55
    // exercise distinct cascades (25 m lies in the second/third overlap).
    f.camera_position = Vec3::new(0.0, 3.0, 0.0);
    f.view = view::look_at_mat4(f.camera_position, f.camera_position - Vec3::Z, Vec3::Y);
    f.proj = directx::perspective(80f32.to_radians(), 1.0, 2.0, 150.0);
    f.environment = Environment {
        background: None,
        zenith: [0.2; 3],
        horizon: [0.2; 3],
        ground: [0.2; 3],
        ambient: 0.5,
        sun_disc: 0.0,
        fog: None,
        ..f.environment
    };
    f.sun = Some(Sun {
        direction: Vec3::new(1.0, -2.0, 0.3),
        color: Vec3::ONE,
        illuminance: 2.0,
        shadows: Some(Shadows::default()),
    });
    (r, f, batches)
}

fn at_world(p: &fixture::Pixels, f: &FrameInput<'_>, world: Vec3) -> u8 {
    let (x, y) = project(f, world, p);
    p.at(x, y)[0]
}

#[test]
fn sun_shadow_ratio_acne_and_subdegree_stability() {
    let Some(gpu) = gpu() else {
        return;
    };
    let (mut r, mut f, mut batches) = shadow_scene(&gpu, 5.0);
    let texture = target(&gpu, (1024, 1024), wgpu::TextureFormat::Rgba8Unorm);
    let shadow = render(&gpu, &mut r, &texture, &f);
    shadow.save("shadow-cube");
    let probe = Vec3::new(0.78, 0.0, -4.9);
    let dark = at_world(&shadow, &f, probe);
    f.sun.as_mut().unwrap().shadows = None;
    let lit = render(&gpu, &mut r, &texture, &f);
    f.sun.as_mut().unwrap().illuminance = 0.0;
    let ambient = render(&gpu, &mut r, &texture, &f);
    let expected = at_world(&ambient, &f, probe);
    let bright = at_world(&lit, &f, probe);
    eprintln!("shadow: ambient={expected}, shadow={dark}, sun+ambient={bright}");
    assert!(dark.abs_diff(expected) <= 6 && bright > dark + 60);
    let mut patch = Vec::new();
    for z in 0..20 {
        for x in 0..20 {
            let world = Vec3::new(-1.8 + x as f32 * 0.02, 0.0, -5.0 + z as f32 * 0.02);
            let a = at_world(&shadow, &f, world);
            let b = at_world(&lit, &f, world);
            assert!(a.abs_diff(b) <= 1, "acne at {world:?}: {a} vs {b}");
            patch.push(f32::from(a));
        }
    }
    let mean = patch.iter().sum::<f32>() / patch.len() as f32;
    let variance = patch.iter().map(|x| (x - mean).powi(2)).sum::<f32>() / patch.len() as f32;
    assert!(variance < 2.0, "lit patch variance {variance}");
    f.sun.as_mut().unwrap().illuminance = 2.0;
    f.sun.as_mut().unwrap().shadows = Some(Shadows::default());
    // World-space scan across the outer shadow edge, excluding the cube silhouette.
    let edge = |pixels: &fixture::Pixels, frame: &FrameInput<'_>| {
        (0..150)
            .map(|i| 0.7 + i as f32 * 0.004)
            .find(|&x| {
                at_world(pixels, frame, Vec3::new(x, 0.0, -4.9))
                    > ((u16::from(bright) + u16::from(expected)) / 2) as u8
            })
            .expect("shadow edge")
    };
    let before = edge(&shadow, &f);
    f.view = view::look_at_mat4(
        f.camera_position,
        f.camera_position + Quat::from_rotation_y(0.05f32.to_radians()) * -Vec3::Z,
        Vec3::Y,
    );
    let yaw = render(&gpu, &mut r, &texture, &f);
    yaw.save("shadow-yaw");
    let after = edge(&yaw, &f);
    eprintln!("world shadow edge {before:.4} -> {after:.4}, lit variance {variance:.4}");
    assert!((before - after).abs() <= 0.018, "shadow swims");
    batches[1].casts_shadows = false;
    r.set_batches(&batches, &[0, 1]).unwrap();
    let disabled = render(&gpu, &mut r, &texture, &f);
    assert!(at_world(&disabled, &f, probe) > dark + 60);
    // Moving casters must use the very same prev/current interpolation as forward.
    batches[1].casts_shadows = true;
    r.set_batches(&batches, &[0, 1]).unwrap();
    r.begin_tick();
    r.write_transforms(
        1,
        &transform(Vec3::new(2.0, 0.5, -5.0), Quat::IDENTITY, Vec3::ONE),
    )
    .unwrap();
    f.alpha = 0.5;
    let interpolated = render(&gpu, &mut r, &texture, &f);
    r.write_transforms_both(
        1,
        &transform(Vec3::new(1.0, 0.5, -5.0), Quat::IDENTITY, Vec3::ONE),
    )
    .unwrap();
    assert_eq!(interpolated.data, render(&gpu, &mut r, &texture, &f).data);
}

#[test]
fn all_cascades_and_shadow_distance() {
    let Some(gpu) = gpu() else {
        return;
    };
    let (mut r, mut f, _) = shadow_scene(&gpu, 5.0);
    let texture = target(&gpu, (1536, 1536), wgpu::TextureFormat::Rgba8Unorm);
    for depth in [5.0, 25.0, 55.0, 70.0] {
        r.write_transforms_both(
            1,
            &transform(Vec3::new(0.0, 0.5, -depth), Quat::IDENTITY, Vec3::ONE),
        )
        .unwrap();
        f.sun.as_mut().unwrap().shadows = Some(Shadows::default());
        let shadow = render(&gpu, &mut r, &texture, &f);
        shadow.save(&format!("shadow-depth-{depth}"));
        f.sun.as_mut().unwrap().shadows = None;
        let lit = render(&gpu, &mut r, &texture, &f);
        // Compare the projected ground shadow ROI: at long distances it is only
        // a few pixels high; a single rounded probe would test MSAA placement.
        let mut largest = 0;
        for x in 0..10 {
            for z in 0..10 {
                let p = Vec3::new(0.6 + x as f32 * 0.045, 0.0, -depth - 0.3 + z as f32 * 0.08);
                largest =
                    largest.max(at_world(&lit, &f, p).saturating_sub(at_world(&shadow, &f, p)));
            }
        }
        eprintln!("shadow depth {depth}: maximum darkening {largest}");
        if depth < 60.0 {
            assert!(largest > 20);
        } else {
            assert!(largest <= 1);
        }
    }
    // Enable, change cascade count, resize and disable on the same renderer.
    for count in [1, 2, 3] {
        f.sun.as_mut().unwrap().shadows = Some(Shadows {
            cascades: count,
            ..Default::default()
        });
        render(
            &gpu,
            &mut r,
            &target(&gpu, (64, 48), wgpu::TextureFormat::Rgba8Unorm),
            &f,
        );
    }
}

#[test]
fn bloom_spreads_light_and_leaves_subthreshold_identical() {
    let Some(gpu) = gpu() else {
        return;
    };
    let mut r = Renderer::new(&gpu.device, &gpu.queue, wgpu::TextureFormat::Rgba8Unorm);
    let (v, i) = shapes::sphere(48);
    let sphere = r.add_mesh(&v, &i);
    r.write_transforms_both(0, &transform(Vec3::ZERO, Quat::IDENTITY, Vec3::ONE))
        .unwrap();
    r.write_materials(0, &material([0.0; 3], 20.0)).unwrap();
    r.set_batches(&[Batch::new(sphere, 0..1)], &[0]).unwrap();
    let texture = target(&gpu, (512, 320), wgpu::TextureFormat::Rgba8Unorm);
    let mut f = frame();
    let off = render(&gpu, &mut r, &texture, &f);
    off.save("bloom-off");
    f.environment.bloom = Some(Bloom::default());
    let on = render(&gpu, &mut r, &texture, &f);
    on.save("bloom-on");
    let outside = (0..off.data.len() / 4)
        .filter(|&i| off.data[i * 4] == 0 && on.data[i * 4] > 5)
        .count();
    eprintln!("bloom: {outside} pixels outside silhouette exceed 5/255");
    assert!(outside > 1000);
    assert!(on.at(298, 160)[0] > 10 && off.at(298, 160)[0] == 0);
    r.write_materials(0, &material([0.0; 3], 0.95)).unwrap();
    let low_on = render(&gpu, &mut r, &texture, &f);
    f.environment.bloom = None;
    let low_off = render(&gpu, &mut r, &texture, &f);
    assert!(low_on
        .data
        .iter()
        .zip(&low_off.data)
        .all(|(a, b)| a.abs_diff(*b) <= 1));
    assert_eq!(off.data, {
        r.write_materials(0, &material([0.0; 3], 20.0)).unwrap();
        render(&gpu, &mut r, &texture, &f).data
    });
    f.environment.bloom = Some(Bloom::default());
    for size in [(1, 1), (17, 11), (255, 129)] {
        render(
            &gpu,
            &mut r,
            &target(&gpu, size, wgpu::TextureFormat::Rgba8Unorm),
            &f,
        );
    }
}

#[test]
fn sky_gradient_sun_disc_and_height_fog() {
    let Some(gpu) = gpu() else {
        return;
    };
    let mut r = Renderer::new(&gpu.device, &gpu.queue, wgpu::TextureFormat::Rgba8Unorm);
    let mut f = frame();
    f.camera_position = Vec3::ZERO;
    f.view = glam::Mat4::IDENTITY;
    f.proj = directx::perspective(170f32.to_radians(), 1.0, 0.1, 100.0);
    f.environment = Environment {
        fog: None,
        bloom: f.environment.bloom,
        exposure: f.environment.exposure,
        ..Environment::default()
    };
    f.environment.sun_disc = 0.0;
    let texture = target(&gpu, (512, 512), wgpu::TextureFormat::Rgba8Unorm);
    let sky = render(&gpu, &mut r, &texture, &f);
    sky.save("sky-gradient");
    for (y, expected) in [
        (0, f.environment.zenith),
        (255, f.environment.horizon),
        (511, f.environment.ground),
    ] {
        for (a, b) in sky.at(256, y)[..3].iter().zip(expected.map(tone)) {
            assert!(a.abs_diff(b) < 4, "sky row {y}: {a} vs {b}");
        }
    }
    f.sun = Some(Sun {
        direction: Vec3::Z,
        color: Vec3::ONE,
        illuminance: 5.0,
        shadows: None,
    });
    f.environment.sun_disc = 0.04;
    let disc = render(&gpu, &mut r, &texture, &f);
    disc.save("sky-sun");
    assert!(disc.at(256, 256)[0] > sky.at(256, 256)[0] + 20);
    let (v, i) = shapes::cube();
    let cube = r.add_mesh(&v, &i);
    r.write_materials(0, &material([0.0; 3], 0.02).repeat(2))
        .unwrap();
    r.write_transforms_both(
        0,
        &transform(Vec3::new(-0.8, 0.0, -2.0), Quat::IDENTITY, Vec3::ONE * 0.6),
    )
    .unwrap();
    r.write_transforms_both(
        1,
        &transform(
            Vec3::new(12.0, 0.0, -40.0),
            Quat::IDENTITY,
            Vec3::ONE * 10.0,
        ),
    )
    .unwrap();
    r.set_batches(&[Batch::new(cube, 0..2)], &[0, 1]).unwrap();
    f.proj = directx::perspective(70f32.to_radians(), 1.0, 0.1, 100.0);
    f.sun = None;
    f.environment.sun_disc = 0.0;
    let clear = render(&gpu, &mut r, &texture, &f);
    f.environment.fog = Some(Fog {
        density: 0.02,
        height_falloff: 0.3,
        color: None,
    });
    let foggy = render(&gpu, &mut r, &texture, &f);
    foggy.save("fog-near-far");
    let near = Vec3::new(-0.8, 0.0, -1.7);
    let far = Vec3::new(12.0, 0.0, -35.0);
    let near_change = at_world(&foggy, &f, near) - at_world(&clear, &f, near);
    let far_change = at_world(&foggy, &f, far) - at_world(&clear, &f, far);
    eprintln!("fog: near change {near_change}, far change {far_change}");
    assert!(far_change > near_change * 3 && near_change < 40);
    // Raise eye and objects together; density should fall sharply at altitude.
    f.camera_position.y = 20.0;
    f.view = view::look_at_mat4(f.camera_position, f.camera_position - Vec3::Z, Vec3::Y);
    r.write_transforms_both(
        0,
        &transform(Vec3::new(-0.8, 20.0, -2.0), Quat::IDENTITY, Vec3::ONE * 0.6),
    )
    .unwrap();
    r.write_transforms_both(
        1,
        &transform(
            Vec3::new(12.0, 20.0, -40.0),
            Quat::IDENTITY,
            Vec3::ONE * 10.0,
        ),
    )
    .unwrap();
    let high = render(&gpu, &mut r, &texture, &f);
    assert!(at_world(&high, &f, far + Vec3::Y * 20.0).abs_diff(tone(0.02)) <= 3);
}

pub(super) mod shadow_quality;
