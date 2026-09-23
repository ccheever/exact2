//! Environment lighting: prefiltered specular radiance and SH9 diffuse irradiance,
//! derived from the environment's procedural sky only when that sky changes.
//! @ref llp/1046.003-game-engine-as-built.explainer.md#culling-and-environment-lighting-2026-09-23
//!
//! Split-sum specular samples a small cube map whose mips hold GGX-prefiltered
//! radiance (roughness = mip / (MIPS - 1)); forward and model shaders scale it by
//! Karis's analytic environment BRDF (`ibl.wgsl`). Diffuse uses nine SH coefficients
//! in the frame uniform. The sun is excluded: it is a direct light. `Environment`'s
//! flat `background` stays independent of lighting, as the hemisphere was.
use crate::buffers::bytes;
use exact_gpu::wgpu;
use glam::Vec3;

/// Cube face size and mip count: mip `m` is roughness `m / (MIPS - 1)`.
pub(crate) const SIZE: u32 = 32;
pub(crate) const MIPS: u32 = 6;
const STRIDE: u64 = 256;

pub(crate) struct EnvironmentLight {
    /// Cube view over every mip, sampled by forward and model shaders.
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    faces: Vec<wgpu::TextureView>,
    pipeline: wgpu::RenderPipeline,
    uniform: wgpu::Buffer,
    bind: wgpu::BindGroup,
    key: Option<[u32; 9]>,
    pending: bool,
    /// SH9 irradiance / π, premultiplied for the shader's polynomial basis.
    pub irradiance: [f32; 36],
    /// Completed prefilters; the sky is reprojected only when it changes.
    pub updates: u64,
}

impl EnvironmentLight {
    pub fn new(device: &wgpu::Device, directions: bool) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("game environment radiance"),
            size: wgpu::Extent3d {
                width: SIZE,
                height: SIZE,
                depth_or_array_layers: 6,
            },
            mip_level_count: MIPS,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::Cube),
            ..Default::default()
        });
        let faces = (0..MIPS * 6)
            .map(|i| {
                texture.create_view(&wgpu::TextureViewDescriptor {
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    base_mip_level: i / 6,
                    mip_level_count: Some(1),
                    base_array_layer: i % 6,
                    array_layer_count: Some(1),
                    ..Default::default()
                })
            })
            .collect();
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("game environment"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("game environment prefilter"),
            size: STRIDE * u64::from(MIPS) * 6,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("game environment prefilter"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: std::num::NonZeroU64::new(64),
                },
                count: None,
            }],
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("game environment prefilter"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &uniform,
                    offset: 0,
                    size: std::num::NonZeroU64::new(64),
                }),
            }],
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("game environment prefilter"),
            source: wgpu::ShaderSource::Wgsl(crate::pipeline::ENVIRONMENT.into()),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("game environment prefilter"),
            layout: Some(
                &device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("game environment prefilter"),
                    bind_group_layouts: &[Some(&layout)],
                    immediate_size: 0,
                }),
            ),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                targets: &[Some(wgpu::TextureFormat::Rgba16Float.into())],
                compilation_options: wgpu::PipelineCompilationOptions {
                    constants: &[("DIRECTIONS", if directions { 1.0 } else { 0.0 })],
                    ..Default::default()
                },
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        Self {
            view,
            sampler,
            faces,
            pipeline,
            uniform,
            bind,
            key: None,
            pending: false,
            irradiance: [0.; 36],
            updates: 0,
        }
    }

    /// Project a changed sky and queue its prefilter for the next `encode`.
    pub fn prepare(&mut self, queue: &wgpu::Queue, environment: &exact_game::Environment) {
        let colors = [environment.zenith, environment.horizon, environment.ground];
        let key: [u32; 9] = std::array::from_fn(|i| colors[i / 3][i % 3].to_bits());
        if self.key == Some(key) {
            return;
        }
        self.key = Some(key);
        let sky = Sky::of(environment);
        self.irradiance = sky.irradiance();
        let mut words = [0f32; (STRIDE as usize / 4) * (MIPS as usize) * 6];
        for mip in 0..MIPS {
            for face in 0..6 {
                let at = (mip * 6 + face) as usize * STRIDE as usize / 4;
                for (i, color) in colors.iter().enumerate() {
                    words[at + i * 4..at + i * 4 + 3].copy_from_slice(color);
                }
                words[at + 12..at + 15].copy_from_slice(&[
                    face as f32,
                    mip as f32 / (MIPS - 1) as f32,
                    (SIZE >> mip).max(1) as f32,
                ]);
            }
        }
        queue.write_buffer(&self.uniform, 0, bytes(&words));
        self.pending = true;
    }

    /// Render the pending prefilter: one small pass per face and mip.
    pub fn encode(&mut self, encoder: &mut wgpu::CommandEncoder) {
        if !std::mem::take(&mut self.pending) {
            return;
        }
        for (i, face) in self.faces.iter().enumerate() {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("game environment prefilter"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: face,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind, &[(i as u64 * STRIDE) as u32]);
            pass.draw(0..3, 0..1);
        }
        self.updates += 1;
    }
}

/// The procedural sky as a function of direction: frame.wgsl's `environment()`.
pub(crate) struct Sky {
    zenith: Vec3,
    horizon: Vec3,
    ground: Vec3,
}
impl Sky {
    pub fn of(e: &exact_game::Environment) -> Self {
        Self {
            zenith: Vec3::from_array(e.zenith),
            horizon: Vec3::from_array(e.horizon),
            ground: Vec3::from_array(e.ground),
        }
    }
    pub fn radiance(&self, d: Vec3) -> Vec3 {
        let pole = if d.y >= 0. { self.zenith } else { self.ground };
        self.horizon.lerp(pole, d.y.abs())
    }
    /// SH9 of the radiance, convolved with the clamped cosine and divided by π:
    /// a Lambertian surface's outgoing radiance per unit albedo. Midpoint
    /// quadrature over the sphere; the basis constants are folded in, so the shader
    /// evaluates `c0 + c1 y + c2 z + c3 x + c4 xy + c5 yz + c6 (3z² - 1) + c7 xz + c8 (x² - y²)`.
    pub fn irradiance(&self) -> [f32; 36] {
        const THETA: usize = 64;
        const PHI: usize = 128;
        let mut sh = [Vec3::ZERO; 9];
        for i in 0..THETA {
            let theta = (i as f32 + 0.5) * std::f32::consts::PI / THETA as f32;
            let weight = theta.sin()
                * (std::f32::consts::PI / THETA as f32)
                * (std::f32::consts::TAU / PHI as f32);
            for j in 0..PHI {
                let phi = (j as f32 + 0.5) * std::f32::consts::TAU / PHI as f32;
                let d = Vec3::new(
                    theta.sin() * phi.cos(),
                    theta.cos(),
                    theta.sin() * phi.sin(),
                );
                let l = self.radiance(d) * weight;
                for (k, y) in basis(d).iter().enumerate() {
                    sh[k] += l * *y;
                }
            }
        }
        // Lambert's cosine lobe per band (π, 2π/3, π/4), divided by π, times
        // each basis function's constant for the shader's bare polynomials.
        const BAND: [f32; 9] = [1., 2. / 3., 2. / 3., 2. / 3., 0.25, 0.25, 0.25, 0.25, 0.25];
        const CONSTANT: [f32; 9] = [
            0.282095, 0.488603, 0.488603, 0.488603, 1.092548, 1.092548, 0.315392, 1.092548,
            0.546274,
        ];
        let mut out = [0.; 36];
        for k in 0..9 {
            out[k * 4..k * 4 + 3].copy_from_slice(&(sh[k] * BAND[k] * CONSTANT[k]).to_array());
        }
        out
    }
}
// Real SH basis, band order: 00, 1-1 (y), 10 (z), 11 (x), 2-2 (xy), 2-1 (yz),
// 20 (3z²-1), 21 (xz), 22 (x²-y²).
fn basis(d: Vec3) -> [f32; 9] {
    [
        0.282095,
        0.488603 * d.y,
        0.488603 * d.z,
        0.488603 * d.x,
        1.092548 * d.x * d.y,
        1.092548 * d.y * d.z,
        0.315392 * (3. * d.z * d.z - 1.),
        1.092548 * d.x * d.z,
        0.546274 * (d.x * d.x - d.y * d.y),
    ]
}
/// The shader's irradiance polynomial, for tests.
#[cfg(test)]
pub(crate) fn evaluate(c: &[f32; 36], n: Vec3) -> Vec3 {
    let k = |i: usize| Vec3::from_slice(&c[i * 4..i * 4 + 3]);
    (k(0)
        + k(1) * n.y
        + k(2) * n.z
        + k(3) * n.x
        + k(4) * (n.x * n.y)
        + k(5) * (n.y * n.z)
        + k(6) * (3. * n.z * n.z - 1.)
        + k(7) * (n.x * n.z)
        + k(8) * (n.x * n.x - n.y * n.y))
        .max(Vec3::ZERO)
}

#[cfg(test)]
mod tests {
    use super::*;
    use exact_game::Environment;

    fn sky(zenith: [f32; 3], horizon: [f32; 3], ground: [f32; 3]) -> Environment {
        Environment {
            zenith,
            horizon,
            ground,
            ..Default::default()
        }
    }
    // (1/π)∫ L(ω) max(0, n·ω) dω on a finer grid than the projection's.
    fn cosine_integral(sky: &Sky, n: Vec3) -> Vec3 {
        let (rows, columns) = (256, 512);
        let mut sum = Vec3::ZERO;
        for i in 0..rows {
            let theta = (i as f32 + 0.5) * std::f32::consts::PI / rows as f32;
            let weight = theta.sin() * std::f32::consts::PI / rows as f32 * std::f32::consts::TAU
                / columns as f32;
            for j in 0..columns {
                let phi = (j as f32 + 0.5) * std::f32::consts::TAU / columns as f32;
                let d = Vec3::new(
                    theta.sin() * phi.cos(),
                    theta.cos(),
                    theta.sin() * phi.sin(),
                );
                sum += sky.radiance(d) * n.dot(d).max(0.) * weight;
            }
        }
        sum / std::f32::consts::PI
    }
    const NORMALS: [[f32; 3]; 7] = [
        [0., 1., 0.],
        [0., -1., 0.],
        [1., 0., 0.],
        [0., 0., -1.],
        [0.6, 0.8, 0.],
        [-0.36, -0.48, 0.8],
        [0.577, 0.577, 0.577],
    ];

    #[test]
    fn a_uniform_sky_irradiates_its_own_radiance() {
        let c = [0.3, 0.5, 0.7];
        let sh = Sky::of(&sky(c, c, c)).irradiance();
        for n in NORMALS {
            let e = evaluate(&sh, Vec3::from_array(n).normalize());
            assert!(e.abs_diff_eq(Vec3::from_array(c), 2e-3), "{n:?}: {e}");
        }
    }

    #[test]
    fn sh9_irradiance_matches_the_cosine_integral_of_the_default_sky() {
        let s = Sky::of(&Environment::default());
        let sh = s.irradiance();
        for n in NORMALS {
            let n = Vec3::from_array(n).normalize();
            let (projected, exact) = (evaluate(&sh, n), cosine_integral(&s, n));
            // SH9 keeps the low frequencies only; the sky's horizon kink costs a little.
            assert!(
                projected.abs_diff_eq(exact, 0.012),
                "{n}: SH {projected} vs {exact}"
            );
        }
    }

    // Sample through texture_cube at chosen directions, as the forward shader does.
    fn sample(
        gpu: &exact_gpu::Gpu,
        light: &EnvironmentLight,
        lod: f32,
        dirs: &[Vec3],
    ) -> Vec<Vec3> {
        let source = format!(
            "@group(0) @binding(0) var cube: texture_cube<f32>;
             @group(0) @binding(1) var linear: sampler;
             @group(0) @binding(2) var<storage, read_write> io: array<vec4<f32>>;
             @compute @workgroup_size(1) fn probe(@builtin(global_invocation_id) id: vec3<u32>) {{
                 io[id.x] = textureSampleLevel(cube, linear, io[id.x].xyz, {lod:.1});
             }}"
        );
        let module = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: None,
                source: wgpu::ShaderSource::Wgsl(source.into()),
            });
        let pipeline = gpu
            .device
            .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: None,
                layout: None,
                module: &module,
                entry_point: Some("probe"),
                compilation_options: Default::default(),
                cache: None,
            });
        let words: Vec<f32> = dirs.iter().flat_map(|d| [d.x, d.y, d.z, 0.]).collect();
        let io = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (words.len() * 4) as u64,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        gpu.queue.write_buffer(&io, 0, bytes(&words));
        let bind = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&light.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&light.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: io.as_entire_binding(),
                },
            ],
        });
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &bind, &[]);
            pass.dispatch_workgroups(dirs.len() as u32, 1, 1);
        }
        gpu.queue.submit([encoder.finish()]);
        crate::skinning::tests::read(gpu, &io, (words.len() * 4) as u64)
            .chunks_exact(16)
            .map(|c| {
                Vec3::from_array(std::array::from_fn(|i| {
                    f32::from_ne_bytes(c[i * 4..i * 4 + 4].try_into().unwrap())
                }))
            })
            .collect()
    }
    fn prefiltered(gpu: &exact_gpu::Gpu, directions: bool, e: &Environment) -> EnvironmentLight {
        let mut light = EnvironmentLight::new(&gpu.device, directions);
        light.prepare(&gpu.queue, e);
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        light.encode(&mut encoder);
        gpu.queue.submit([encoder.finish()]);
        light
    }
    fn directions() -> Vec<Vec3> {
        let mut dirs: Vec<Vec3> = NORMALS
            .iter()
            .map(|n| Vec3::from_array(*n).normalize())
            .collect();
        dirs.extend([Vec3::Z, -Vec3::X, Vec3::new(0.3, -0.2, -0.93).normalize()]);
        dirs
    }

    #[test]
    fn metals_reflect_the_sky_out_of_the_sun_and_it_is_prefiltered_once_per_change() {
        let Some(gpu) = crate::test_device::device_or_skip(exact_gpu::fixture::device()) else {
            return;
        };
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let mut r = crate::Renderer::new(&gpu.device, &gpu.queue, format);
        let (v, i) = crate::shapes::sphere(32);
        let sphere = r.add_mesh(&v, &i);
        r.write_transforms_both(0, &[0., 0., 0., 0., 0., 0., 1., 1., 1., 1.])
            .unwrap();
        // Polished metal: base 0.9, metallic 1, roughness 0.2; unit dimensions.
        r.write_materials(0, &[0.9, 0.9, 0.9, 1., 1., 0.2, 0., 0., 0., 1., 1., 1.])
            .unwrap();
        r.set_batches(&[crate::Batch::new(sphere, 0..1)], &[0])
            .unwrap();
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: 64,
                height: 64,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let camera = Vec3::new(0., 0., 1.6);
        let mut frame = crate::FrameInput {
            view: glam::camera::rh::view::look_at_mat4(camera, Vec3::ZERO, Vec3::Y),
            proj: glam::camera::rh::proj::directx::perspective(0.8, 1., 0.1, 10.),
            camera_position: camera,
            // The sun lights only the far side: the camera sees environment light.
            sun: Some(crate::Sun {
                direction: Vec3::Z,
                shadows: None,
                ..Default::default()
            }),
            ..Default::default()
        };
        frame.environment.fog = None;
        frame.environment.bloom = None;
        let draw = |r: &mut crate::Renderer, f: &crate::FrameInput<'_>| {
            r.draw(&view, (64, 64), f);
            exact_gpu::fixture::read(&gpu, &texture).unwrap()
        };
        let lit = draw(&mut r, &frame);
        lit.save("ibl-backlit-metal");
        let [top, centre, bottom] = [20, 32, 44].map(|y| lit.at(32, y));
        eprintln!("back-lit metal: top {top:?}, centre {centre:?}, bottom {bottom:?}");
        // The default sky: blue zenith above, dark ground below, bright horizon between.
        assert!(
            centre[0] > 60 && centre[1] > 60,
            "metal is not black: {centre:?}"
        );
        assert!(
            top[2] > top[0] && top[2] > bottom[2] + 30,
            "zenith above ground: {top:?} {bottom:?}"
        );
        draw(&mut r, &frame);
        frame.environment.ambient = 0.25;
        let dimmer = draw(&mut r, &frame);
        assert!(
            dimmer.at(32, 32)[0] < centre[0],
            "ambient scales environment light"
        );
        assert_eq!(
            r.environment.updates, 1,
            "an unchanged sky is not prefiltered again"
        );
        frame.environment.ambient = exact_game::Environment::default().ambient;
        frame.environment.zenith = [0.5, 0.1, 0.1];
        let red = draw(&mut r, &frame);
        assert_eq!(r.environment.updates, 2);
        assert!(
            red.at(32, 20)[0] > top[0] + 20,
            "a new sky is reflected at once"
        );
    }

    #[test]
    fn cube_faces_follow_webgpu_orientation_and_mip_zero_is_the_sky() {
        let Some(gpu) = crate::test_device::device_or_skip(exact_gpu::fixture::device()) else {
            return;
        };
        let e = sky([0.1, 0.3, 0.9], [0.8, 0.7, 0.5], [0.05, 0.1, 0.02]);
        // Direction-coloured faces: any swapped or flipped face shows up here.
        let dirs = directions();
        let got = sample(&gpu, &prefiltered(&gpu, true, &e), 0., &dirs);
        for (d, c) in dirs.iter().zip(&got) {
            assert!(
                c.abs_diff_eq(*d * 0.5 + 0.5, 0.03),
                "direction {d}: sampled {c}"
            );
        }
        let light = prefiltered(&gpu, false, &e);
        let s = Sky::of(&e);
        for (d, c) in dirs.iter().zip(sample(&gpu, &light, 0., &dirs)) {
            assert!(
                c.abs_diff_eq(s.radiance(*d), 0.03),
                "direction {d}: {c} vs {}",
                s.radiance(*d)
            );
        }
        // The roughest mip is a wide lobe: between the pole and the horizon, smoothly.
        let rough = sample(&gpu, &light, 5., &[Vec3::Y, -Vec3::Y]);
        assert!(
            rough[0].z < e.zenith[2] && rough[0].z > e.horizon[2],
            "{rough:?}"
        );
        assert!(
            rough[1].x > e.ground[0] && rough[1].x < e.horizon[0],
            "{rough:?}"
        );
    }
}
