//! Glass: the whole app inside the aurora (LLP 1014). The canvas is the
//! page; its children are the interface, captured by the host; the surface
//! renders the sky at a quarter of the resolution and composes the children
//! over it at full resolution in one of three materials — `glass`, `ink`,
//! `crt` — crossfading from the previous children when they change.
//! Inputs, in `canvas surface=glass(material, seed)` order: the material's
//! name and a seed string (the selected station).

use exact_gpu::json::text;
use exact_gpu::wgpu;
use exact_gpu::{Frame, FrameUniform, Surface, SurfaceError, Value};

use crate::shaders::glass::{entry, module, Uniforms, CHILDREN, GROUP_0, PREVIOUS, SKY, SMP, U};
use crate::AuroraSurface;

/// Milliseconds a change of children takes to fade in.
const FADE_MS: f64 = 320.0;
/// A change this soon after the last one is motion, not a change of scene:
/// it shows at once, or an animation under the glass would trail its own
/// previous frame.
const CONTINUOUS_MS: f64 = 100.0;

/// The glass surface.
#[derive(Default)]
pub struct GlassSurface {
    seed: f32,
    material: f32,
    gpu: Option<Gpu>,
    children: Option<wgpu::TextureView>,
    previous: Option<wgpu::TextureView>,
    rebind: bool,
    generation: u32,
    fade_start: f64,
    last_change: f64,
    /// The children texture is new (the first, or a resize): there is no
    /// previous picture to fade from.
    fresh: bool,
}

struct Gpu {
    format: wgpu::TextureFormat,
    /// The shader generation the pipelines were built at (LLP 1030 D8).
    generation: u32,
    sky_pipeline: wgpu::RenderPipeline,
    compose_pipeline: wgpu::RenderPipeline,
    /// Written in place each frame (`FrameUniform`); bind groups per slot.
    uniforms: FrameUniform,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    blank: wgpu::TextureView,
    sky: Option<(u32, u32, wgpu::TextureView)>,
    sky_groups: Vec<wgpu::BindGroup>,
    compose_groups: Vec<wgpu::BindGroup>,
}

impl GlassSurface {
    /// A fresh surface.
    pub fn new() -> GlassSurface {
        GlassSurface::default()
    }

    /// The material's number: `glass` 0, `ink` 1, `crt` 2; anything else
    /// is refused.
    pub fn material_of(name: &str) -> Result<f32, SurfaceError> {
        match name {
            "glass" => Ok(0.0),
            "ink" => Ok(1.0),
            "crt" => Ok(2.0),
            other => Err(SurfaceError(format!(
                "glass: material `{other}` is not glass, ink, or crt"
            ))),
        }
    }
}

impl Surface for GlassSurface {
    fn bind(&mut self, inputs: &[Value], _: Option<f64>) -> Result<(), SurfaceError> {
        let [material, seed] = inputs else {
            return Err(SurfaceError(format!(
                "glass: expected 2 inputs, got {}",
                inputs.len()
            )));
        };
        self.material = GlassSurface::material_of(&text(material, "material")?)?;
        self.seed = AuroraSurface::seed_of(&text(seed, "seed")?);
        Ok(())
    }

    fn children_mode(&self) -> exact_gpu::ChildrenMode {
        exact_gpu::ChildrenMode::Composite { previous: true }
    }

    fn children(
        &mut self,
        texture: Option<&wgpu::TextureView>,
        previous: Option<&wgpu::TextureView>,
    ) {
        self.children = texture.cloned();
        self.previous = previous.cloned();
        self.rebind = true;
        self.fresh = true;
    }

    fn render(
        &mut self,
        frame: &Frame,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        format: wgpu::TextureFormat,
    ) -> bool {
        if self.gpu.as_ref().map(|g| (g.format, g.generation))
            != Some((format, frame.shader_generation))
        {
            self.gpu = Some(build(device, queue, format, frame.shader_generation));
            self.rebind = true;
        }
        let (w, h) = frame.pixels();
        let (sw, sh) = ((w / 4).max(1), (h / 4).max(1));
        let children = self.children.as_ref();
        let previous = self.previous.as_ref();
        let gpu = self.gpu.as_mut().unwrap();
        if gpu.sky.as_ref().map(|(a, b, _)| (*a, *b)) != Some((sw, sh)) {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("sky"),
                size: wgpu::Extent3d {
                    width: sw,
                    height: sh,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            gpu.sky = Some((sw, sh, texture.create_view(&Default::default())));
            self.rebind = true;
        }
        if self.rebind {
            let sky = &gpu.sky.as_ref().unwrap().2;
            gpu.compose_groups = (0..gpu.uniforms.slots())
                .map(|slot| {
                    bind_group(
                        device,
                        &gpu.layout,
                        gpu.uniforms.buffer(slot),
                        sky,
                        children.unwrap_or(&gpu.blank),
                        previous.unwrap_or(children.unwrap_or(&gpu.blank)),
                        &gpu.sampler,
                    )
                })
                .collect();
            self.rebind = false;
        }
        // A change of children after a quiet spell starts a fade; the first
        // upload, a new texture (a resize: its previous is blank), and a
        // change hard on the heels of the last show at once. A change
        // during a fade restarts it from the latest picture — a small jump
        // the CONTINUOUS_MS rule already hides for bursts (declared, LLP
        // 1014.000 §5).
        if frame.children_generation != self.generation {
            let quiet = frame.now_ms - self.last_change >= CONTINUOUS_MS;
            self.fade_start = if self.generation == 0 || !quiet || self.fresh {
                f64::NEG_INFINITY
            } else {
                frame.now_ms
            };
            self.generation = frame.children_generation;
            self.last_change = frame.now_ms;
            self.fresh = false;
        }
        let fade = ((frame.now_ms - self.fade_start) / FADE_MS).clamp(0.0, 1.0) as f32;
        let uniforms = Uniforms {
            time: frame.now_ms as f32,
            width: w as f32,
            height: h as f32,
            seed: self.seed,
            material: self.material,
            fade,
            sky_width: sw as f32,
            sky_height: sh as f32,
        };
        let slot = gpu.uniforms.write(queue, &uniforms.bytes());
        {
            let sky = &gpu.sky.as_ref().unwrap().2;
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("sky"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: sky,
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
            pass.set_pipeline(&gpu.sky_pipeline);
            pass.set_bind_group(0, &gpu.sky_groups[slot], &[]);
            pass.draw(0..3, 0..1);
        }
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("glass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
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
            pass.set_pipeline(&gpu.compose_pipeline);
            pass.set_bind_group(0, &gpu.compose_groups[slot], &[]);
            pass.draw(0..3, 0..1);
        }
        // Lit from the clock, and a fade may be running: another frame, always.
        true
    }
}

fn bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    uniforms: &wgpu::Buffer,
    sky: &wgpu::TextureView,
    children: &wgpu::TextureView,
    previous: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("glass"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: U.binding,
                resource: uniforms.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: SKY.binding,
                resource: wgpu::BindingResource::TextureView(sky),
            },
            wgpu::BindGroupEntry {
                binding: CHILDREN.binding,
                resource: wgpu::BindingResource::TextureView(children),
            },
            wgpu::BindGroupEntry {
                binding: PREVIOUS.binding,
                resource: wgpu::BindingResource::TextureView(previous),
            },
            wgpu::BindGroupEntry {
                binding: SMP.binding,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    })
}

fn pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    layout: &wgpu::PipelineLayout,
    fragment: &'static str,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(fragment),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some(entry::VS),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some(fragment),
            targets: &[Some(format.into())],
            compilation_options: Default::default(),
        }),
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    })
}

fn build(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    format: wgpu::TextureFormat,
    generation: u32,
) -> Gpu {
    let shader = device.create_shader_module(module());
    let layout = device.create_bind_group_layout(&GROUP_0);
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("glass"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });
    let blank_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("blank"),
        size: wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &blank_texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &[0, 0, 0, 0],
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4),
            rows_per_image: None,
        },
        wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
    );
    let blank = blank_texture.create_view(&Default::default());
    let uniforms = FrameUniform::new(device, Uniforms::SIZE, "glass uniforms");
    // The sky pass binds nothing it renders into: blanks all round.
    let each = |slot| {
        bind_group(
            device,
            &layout,
            uniforms.buffer(slot),
            &blank,
            &blank,
            &blank,
            &sampler,
        )
    };
    let sky_groups = (0..uniforms.slots()).map(each).collect();
    let compose_groups = (0..uniforms.slots()).map(each).collect();
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("glass"),
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });
    // The sky texture's format is fixed; the target's is the host's.
    let sky_pipeline = pipeline(
        device,
        &shader,
        &pipeline_layout,
        entry::FS_SKY,
        wgpu::TextureFormat::Rgba8Unorm,
    );
    let compose_pipeline = pipeline(device, &shader, &pipeline_layout, entry::FS_COMPOSE, format);
    Gpu {
        format,
        generation,
        sky_pipeline,
        compose_pipeline,
        uniforms,
        layout,
        sampler,
        blank,
        sky: None,
        sky_groups,
        compose_groups,
    }
}
