use super::*;

pub(super) fn gpu() -> Option<Gpu> {
    crate::test_device::device_or_skip(exact_gpu::fixture::device())
}

pub(super) fn target(gpu: &Gpu, size: (u32, u32), format: wgpu::TextureFormat) -> wgpu::Texture {
    gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("game test"),
        size: wgpu::Extent3d {
            width: size.0,
            height: size.1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    })
}

pub(super) fn render(
    gpu: &Gpu,
    renderer: &mut Renderer,
    target: &wgpu::Texture,
    frame: &FrameInput<'_>,
) -> fixture::Pixels {
    let stats = renderer.draw(
        &target.create_view(&Default::default()),
        (target.width(), target.height()),
        frame,
    );
    assert!(stats.draws >= 1 && stats.triangles >= 1);
    fixture::read(gpu, target).unwrap()
}

pub(super) fn transform(position: Vec3, rotation: Quat, scale: Vec3) -> [f32; 10] {
    [
        position.x, position.y, position.z, rotation.x, rotation.y, rotation.z, rotation.w,
        scale.x, scale.y, scale.z,
    ]
}

pub(super) fn material(color: [f32; 3], emissive: f32) -> [f32; 12] {
    [
        color[0], color[1], color[2], 1.0, 0.0, 0.65, emissive, emissive, emissive, 1.0, 1.0, 1.0,
    ]
}

pub(super) fn frame() -> FrameInput<'static> {
    FrameInput {
        glows: &[],
        seconds: 0.,
        view: view::look_at_mat4(Vec3::new(0.0, 0.0, 10.0), Vec3::ZERO, Vec3::Y),
        proj: directx::orthographic(-4.0, 4.0, -2.5, 2.5, 0.1, 100.0),
        camera_position: Vec3::new(0.0, 0.0, 10.0),
        alpha: 1.0,
        sun: None,
        fill: None,
        lights: &[],
        lights_dropped: 0,
        environment: Environment {
            background: None,
            zenith: [0.0; 3],
            ground: [0.0; 3],
            ambient: 0.0,
            horizon: [0.0; 3],
            sun_disc: 0.0,
            fog: None,
            exposure: 1.0,
            bloom: None,
        },
        timestamps: None,
        attachments: &[],
    }
}

pub(super) fn tone(x: f32) -> u8 {
    let x = (x * (2.51 * x + 0.03) / (x * (2.43 * x + 0.59) + 0.14)).clamp(0.0, 1.0);
    let srgb = if x <= 0.0031308 {
        12.92 * x
    } else {
        1.055 * x.powf(1.0 / 2.4) - 0.055
    };
    (srgb * 255.0).round() as u8
}

pub(super) fn project(frame: &FrameInput<'_>, point: Vec3, pixels: &fixture::Pixels) -> (u32, u32) {
    let clip = frame.proj * frame.view * point.extend(1.0);
    (
        ((clip.x / clip.w * 0.5 + 0.5) * pixels.width as f32) as u32,
        ((0.5 - clip.y / clip.w * 0.5) * pixels.height as f32) as u32,
    )
}

pub(super) fn sky_color(frame: &FrameInput<'_>, x: u32, y: u32, w: u32, h: u32) -> [f32; 3] {
    let ndc = Vec3::new(
        (x as f32 + 0.5) / w as f32 * 2.0 - 1.0,
        1.0 - (y as f32 + 0.5) / h as f32 * 2.0,
        0.0,
    );
    let inverse = (frame.proj * frame.view).inverse();
    let d = (inverse.project_point3(ndc + Vec3::Z * 0.5) - inverse.project_point3(ndc)).normalize();
    let end = if d.y >= 0.0 {
        frame.environment.zenith
    } else {
        frame.environment.ground
    };
    Vec3::from_array(frame.environment.horizon)
        .lerp(Vec3::from_array(end), d.y.abs())
        .to_array()
}
