use super::*;
use crate::{Environment, FrameInput, Renderer, ShadowBox, Shadows, Sun};
use exact_game::{asset::*, *};
use exact_gpu::fixture;

#[test]
fn fixed_sun_box_is_independent_of_camera_and_projection() {
    let mut f = FrameInput {
        sun: Some(Sun {
            direction: Vec3::new(1., -2., 0.5),
            color: Vec3::ONE,
            illuminance: 1.,
            shadows: None,
        }),
        ..Default::default()
    };
    let settings = Shadows {
        fixed_box: Some(ShadowBox::default()),
        ..Default::default()
    };
    let a = Cascades::new(&f, settings);
    f.view = glam::camera::rh::view::look_at_mat4(Vec3::new(4., 5., 6.), Vec3::ZERO, Vec3::Y);
    f.proj = glam::camera::rh::proj::directx::perspective(1., 1.7, 0.2, 300.);
    let b = Cascades::new(&f, settings);
    assert_eq!(a.count, 1);
    assert_eq!(a.matrices, b.matrices);
    assert_eq!(a.texels, b.texels);
    f.sun.as_mut().unwrap().direction = Vec3::new(-1., -2., 0.5);
    assert_ne!(a.matrices, Cascades::new(&f, settings).matrices);
}

/// Every layer of the sun's shadow map, as stored.
fn map_depth(gpu: &exact_gpu::Gpu, r: &Renderer) -> Vec<f32> {
    let texture = r.shadows.as_ref().unwrap().view.texture();
    let (side, layers) = (texture.width(), texture.depth_or_array_layers());
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: u64::from(side * side * layers * 4),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::DepthOnly,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(side * 4),
                rows_per_image: Some(side),
            },
        },
        texture.size(),
    );
    gpu.queue.submit([encoder.finish()]);
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, |r| r.unwrap());
    gpu.device
        .poll(wgpu::PollType::wait_indefinitely())
        .unwrap();
    let data = buffer.slice(..).get_mapped_range().unwrap();
    data.chunks(4)
        .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
        .collect()
}

#[test]
fn cached_casters_match_redrawing_after_scene_changes() {
    let Some(gpu) = crate::test_device::device_or_skip(fixture::device()) else {
        return;
    };
    let mut w = World::new(60, 1);
    let (vertices, indices) = crate::shapes::cube();
    let model = Model {
        meshes: vec![MeshData {
            positions: vertices.iter().flat_map(|v| v.position).collect(),
            normals: vertices.iter().flat_map(|v| v.normal).collect(),
            uvs: vertices.iter().flat_map(|v| v.uv).collect(),
            indices,
            bounds: [-0.5, -0.5, -0.5, 0.5, 0.5, 0.5],
            ..Default::default()
        }],
        nodes: vec![Node {
            mesh: Some(0),
            ..Default::default()
        }],
        materials: vec![MaterialData::default()],
        bounds: [-0.5, -0.5, -0.5, 0.5, 0.5, 0.5],
        ..Default::default()
    };
    let mut r = Renderer::configured(
        &gpu.device,
        &gpu.queue,
        wgpu::TextureFormat::Rgba8Unorm,
        SceneSamples::One,
        true,
    );
    for (name, transform) in [
        (
            "floor.model",
            Transform {
                scale: Vec3::new(12., 0.2, 12.),
                ..Transform::at(0., -0.1, 0.)
            },
        ),
        ("box.model", Transform::at(0., 1., 0.)),
    ] {
        let mesh = w.generated_model(name, model.clone()).unwrap();
        w.spawn_named(name, (transform, mesh));
        r.prepare_model(name, &model).unwrap();
    }
    let cube = w.named("box.model").unwrap();
    let mut feed = crate::Feed::default();
    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: None,
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
    let view = target.create_view(&Default::default());
    let mut f = FrameInput {
        view: glam::camera::rh::view::look_at_mat4(Vec3::new(4., 6., 8.), Vec3::ZERO, Vec3::Y),
        proj: glam::camera::rh::proj::directx::orthographic(-5., 5., -5., 5., 0.1, 40.),
        camera_position: Vec3::new(4., 6., 8.),
        sun: Some(Sun {
            direction: Vec3::new(1., -2., 0.3),
            color: Vec3::ONE,
            illuminance: 2.,
            shadows: Some(Shadows {
                resolution: 256,
                cache_static: true,
                fixed_box: Some(ShadowBox {
                    half_width: 10.,
                    ..Default::default()
                }),
                ..Default::default()
            }),
        }),
        environment: Environment {
            fog: None,
            bloom: None,
            sun_disc: 0.,
            ..Default::default()
        },
        ..Default::default()
    };
    for step in 0..8 {
        match step {
            1 => {
                f.camera_position.x += 1.;
                f.view =
                    glam::camera::rh::view::look_at_mat4(f.camera_position, Vec3::ZERO, Vec3::Y);
            }
            2 => {
                w.get_mut::<Transform>(cube).unwrap().position.x = 2.;
            }
            3 => {
                w.insert(cube, Opacity(0.5));
            }
            4 => {
                w.remove::<Opacity>(cube);
                w.insert(cube, Visible(false));
            }
            5 => {
                w.remove::<Visible>(cube);
                f.sun.as_mut().unwrap().direction.x = -1.;
            }
            6 => {
                f.sun.as_mut().unwrap().shadows.as_mut().unwrap().resolution = 512;
            }
            7 => {
                f.sun.as_mut().unwrap().shadows.as_mut().unwrap().fixed_box = None;
            }
            _ => {}
        }
        w.propagate();
        feed.feed(&w, &mut r).unwrap();
        f.sun
            .as_mut()
            .unwrap()
            .shadows
            .as_mut()
            .unwrap()
            .cache_static = true;
        let cold = r.draw(&view, (128, 128), &f);
        let warm = r.draw(&view, (128, 128), &f);
        assert!(
            r.shadows.as_ref().unwrap().cached.is_some(),
            "cache active at {step}"
        );
        assert!(
            warm.draws < cold.draws,
            "cache reuses at {step}: {} vs {}",
            cold.draws,
            warm.draws
        );
        if step == 0 {
            f.camera_position.x += 0.1;
            f.view = glam::camera::rh::view::look_at_mat4(f.camera_position, Vec3::ZERO, Vec3::Y);
            let moved_camera = r.draw(&view, (128, 128), &f);
            assert_eq!(
                warm.draws, moved_camera.draws,
                "camera motion keeps the fixed map"
            );
            assert!(!r.shadows.as_ref().unwrap().cached.as_ref().unwrap().dirty);
        }
        let cached = fixture::read(&gpu, &target).unwrap();
        // The cache copied into the map where a device cannot seed it: the
        // same depth, to the bit, and the same image.
        let seeded = map_depth(&gpu, &r);
        let copy = |r: &mut Renderer, seed| {
            let shadows = r.shadows.as_mut().unwrap();
            shadows.seed = seed;
            shadows.cached = None;
        };
        copy(&mut r, false);
        r.draw(&view, (128, 128), &f);
        r.draw(&view, (128, 128), &f);
        assert!(r.shadows.as_ref().unwrap().cached.is_some());
        let copied = map_depth(&gpu, &r);
        assert!(
            seeded
                .iter()
                .zip(&copied)
                .all(|(a, b)| a.to_bits() == b.to_bits()),
            "step {step}: seeded and copied depth differ"
        );
        assert_eq!(cached.data, fixture::read(&gpu, &target).unwrap().data);
        copy(&mut r, true);
        r.draw(&view, (128, 128), &f);
        r.draw(&view, (128, 128), &f);
        f.sun
            .as_mut()
            .unwrap()
            .shadows
            .as_mut()
            .unwrap()
            .cache_static = false;
        r.draw(&view, (128, 128), &f);
        let normal = fixture::read(&gpu, &target).unwrap();
        for y in 0..128 {
            for x in 0..128 {
                for (a, b) in cached.at(x, y).into_iter().zip(normal.at(x, y)) {
                    assert!(a.abs_diff(b) <= 1, "step {step}, pixel {x},{y}: {a} vs {b}");
                }
            }
        }
        if step == 0 {
            let sun = f.sun.take();
            r.draw(&view, (128, 128), &f);
            let no_sun = fixture::read(&gpu, &target).unwrap();
            f.sun = sun;
            assert!(
                (0..128)
                    .flat_map(|y| (0..128).map(move |x| (x, y)))
                    .filter(|&(x, y)| normal.at(x, y)[0].abs_diff(no_sun.at(x, y)[0]) > 20)
                    .count()
                    > 100
            );
        }
    }
    // An encoded but discarded frame cannot make a replacement cache valid.
    f.sun
        .as_mut()
        .unwrap()
        .shadows
        .as_mut()
        .unwrap()
        .cache_static = true;
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    r.encode_hooked(
        &mut encoder,
        &view,
        (128, 128),
        &f,
        &mut (),
        None,
        Default::default(),
        &Default::default(),
    )
    .unwrap();
    assert!(!r.shadows.as_ref().unwrap().cached.as_ref().unwrap().valid);
    drop(encoder);
    r.draw(&view, (128, 128), &f);
    assert!(r.shadows.as_ref().unwrap().cached.as_ref().unwrap().dirty);
    // Public transform writes bypass Feed's CPU histories; never reuse them.
    f.sun
        .as_mut()
        .unwrap()
        .shadows
        .as_mut()
        .unwrap()
        .cache_static = true;
    r.draw(&view, (128, 128), &f);
    r.write_transforms_both(cube.index(), &[0., 2., 0., 0., 0., 0., 1., 1., 1., 1.])
        .unwrap();
    r.draw(&view, (128, 128), &f);
    assert!(r.shadows.as_ref().unwrap().cached.is_none());
}
