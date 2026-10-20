//! Water (`exact_game::Water`): the surface, a full-screen draw in the forward
//! pass after the sky (`shaders/water.wgsl`); caustics, a factor on the sun's
//! direct term in the forward shaders (`caustics.wgsl`); and shafts, an HDR
//! pass before bloom over the forward pass's depth (`shafts.wgsl`). All three
//! read the frame uniform's water words, so turning an effect off clears a
//! flag and never selects another pipeline. Without water the words are zero,
//! no pipeline is made and no pass runs.
use crate::buffers::Targets;
use crate::FrameInput;
use exact_gpu::wgpu;

/// The frame uniform's water words (`frame.wgsl`'s `water` to `water_drift`).
pub(crate) const WORDS: std::ops::Range<usize> = 192..216;
const SURFACE: u32 = 1;
const CAUSTICS: u32 = 2;
const SHAFTS: u32 = 4;
/// The second wave's wavelength as a share of the first's (`wave_normal`).
const SECOND: f64 = 0.61;
/// The two caustic layers' drift, metres a second for each metre a second of
/// wave speed, and their directions; the second layer is 1.27 times finer.
const DRIFT: f64 = 0.25;
const LAYERS: [([f64; 2], f64); 2] = [([0.6, 0.8], 1.), ([-0.87, 0.49], 1.27)];
/// Cells of the shafts' pattern per wavelength (shafts.wgsl `shaft_source`).
const SHAFT_FREQUENCY: f64 = 3.;

/// Which of the water's effects the renderer may draw: both by default.
/// Clearing one turns it off at present time and leaves the saved `Water`
/// as it is (LLP 1139.003's lowest rung).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WaterEffects {
    /// Caustics on lit surfaces under the water.
    pub caustics: bool,
    /// Light shafts seen from below the surface.
    pub shafts: bool,
}
impl Default for WaterEffects {
    fn default() -> Self {
        Self {
            caustics: true,
            shafts: true,
        }
    }
}

/// Whether this frame draws shafts: the camera is below a surface whose
/// shafts are on and allowed, with a sun to scatter.
pub(crate) fn shafts(frame: &FrameInput<'_>, effects: WaterEffects) -> bool {
    frame.water.is_some_and(|w| {
        effects.shafts
            && w.shafts.is_some_and(|s| s.intensity > 0. && s.length > 0.)
            && frame.camera_position.y < w.height
            && frame.sun.is_some_and(|s| s.illuminance > 0.)
    })
}

/// The word holding where the world layer's depth starts (`depth_split`),
/// written once the cull has ordered the frame.
pub(crate) const SPLIT: usize = 195;

/// Fill the water words for `frame` but [`SPLIT`]. Every phase and offset is
/// computed here in f64 from the displayed time and wrapped at its pattern's
/// period, so a long session loses no precision. An effect `effects` clears
/// loses only its flag: the other words stay, so nothing else moves.
pub(crate) fn write(data: &mut [f32], frame: &FrameInput<'_>, effects: WaterEffects) {
    let Some(water) = frame.water else {
        return;
    };
    let t = frame.seconds;
    let waves = water.waves;
    let wavelength = f64::from(waves.wavelength.max(0.01));
    let speed = f64::from(waves.speed);
    let wrap = |x: f64, period: f64| (x.rem_euclid(period)) as f32;
    let tau = std::f64::consts::TAU;
    let mut flags = SURFACE;
    if water.caustics.is_some() && effects.caustics {
        flags |= CAUSTICS;
    }
    if shafts(frame, effects) {
        flags |= SHAFTS;
    }
    let caustics = water.caustics.unwrap_or(exact_game::Caustics {
        strength: 0.,
        scale: 1.,
    });
    let shafts = water.shafts.unwrap_or(exact_game::Shafts {
        intensity: 0.,
        length: 0.,
    });
    // Deep-water waves travel at a speed proportional to √wavelength.
    let phase = |length: f64, speed: f64| wrap(tau * speed * t / length, tau);
    let drift = |cell: f64, period: f64| {
        LAYERS.map(|(direction, fineness)| {
            let cells = DRIFT * speed * t * fineness / cell;
            [direction[0], direction[1]].map(|d| wrap(d * cells, period))
        })
    };
    // The caustic net's layers in cells, wrapped at its four-cell tile; the
    // shafts' in wavelengths, at their pattern's 32 (shafts.wgsl).
    let [a, b] = drift(f64::from(caustics.scale.max(0.01)), 4.);
    let [c, d] = drift(wavelength / SHAFT_FREQUENCY, 32.);
    let words = [
        water.height,
        flags as f32,
        water.clarity,
        0.,
        water.color[0],
        water.color[1],
        water.color[2],
        waves.amplitude.max(0.),
        wavelength as f32,
        phase(wavelength, speed),
        phase(wavelength * SECOND, speed * SECOND.sqrt()),
        caustics.strength.clamp(0., 1.),
        caustics.scale.max(0.01),
        shafts.intensity.max(0.),
        shafts.length.max(0.),
        0.,
        a[0],
        a[1],
        b[0],
        b[1],
        c[0],
        c[1],
        d[0],
        d[1],
    ];
    data[WORDS].copy_from_slice(&words);
}

/// The water's pipelines, made on a frame that first needs each.
#[derive(Default)]
pub(crate) struct Passes {
    surface: Option<wgpu::RenderPipeline>,
    shafts: Option<Shafts>,
}

struct Shafts {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    /// Over the current targets' depth; dropped when they are replaced.
    bind: Option<wgpu::BindGroup>,
}

impl Passes {
    /// Make what this frame draws: the surface's pipeline with water, the
    /// shafts' and their bindings over `targets` when `shafts`.
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        pipelines: &crate::pipeline::Pipelines,
        uniform: &wgpu::Buffer,
        targets: &Targets,
        water: bool,
        shafts: bool,
    ) {
        if water && self.surface.is_none() {
            self.surface = Some(surface_pipeline(device, pipelines));
        }
        if !shafts {
            return;
        }
        let made = self
            .shafts
            .get_or_insert_with(|| shafts_pipeline(device, pipelines, targets.samples));
        made.bind.get_or_insert_with(|| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("game water shafts"),
                layout: &made.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: uniform.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&targets.depth),
                    },
                ],
            })
        });
    }

    /// The targets were replaced: the shafts' binding reads the old depth.
    pub fn targets_replaced(&mut self) {
        if let Some(shafts) = &mut self.shafts {
            shafts.bind = None;
        }
    }

    /// Draw the surface into the forward pass, with the scene group bound.
    pub fn draw_surface(&self, pass: &mut wgpu::RenderPass<'_>, scene: &wgpu::BindGroup) {
        pass.set_pipeline(self.surface.as_ref().expect("prepared with water"));
        pass.set_bind_group(0, scene, &[0]);
        pass.draw(0..3, 0..1);
    }

    /// Add the shafts to the resolved HDR colour.
    pub fn encode_shafts(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        color: &wgpu::TextureView,
        size: (u32, u32),
    ) {
        let shafts = self.shafts.as_ref().expect("prepared with shafts");
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("game water shafts"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: color,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        crate::renderer::viewport(&mut pass, size);
        pass.set_pipeline(&shafts.pipeline);
        pass.set_bind_group(0, shafts.bind.as_ref().unwrap(), &[]);
        pass.draw(0..3, 0..1);
    }
}

fn surface_pipeline(
    device: &wgpu::Device,
    pipelines: &crate::pipeline::Pipelines,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("game water surface"),
        source: wgpu::ShaderSource::Wgsl(crate::pipeline::water_source().into()),
    });
    // Premultiplied: opaque from below, the sky's reflection over the floor from above.
    let over = wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::One,
        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
        operation: wgpu::BlendOperation::Add,
    };
    let keep = wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::Zero,
        dst_factor: wgpu::BlendFactor::One,
        operation: wgpu::BlendOperation::Add,
    };
    crate::pipeline::make_pipeline(
        device,
        pipelines.cache.handle.as_ref(),
        &shader,
        "game water surface",
        "sky_vs",
        Some("water_fs"),
        &[Some(&pipelines.scene_layout)],
        &[],
        crate::pipeline::scene_depth(true, wgpu::CompareFunction::Less, Default::default()),
        pipelines.samples,
        wgpu::TextureFormat::Rgba16Float,
        Some(wgpu::BlendState {
            color: over,
            alpha: keep,
        }),
        &[],
        false,
        true,
    )
}

fn shafts_pipeline(
    device: &wgpu::Device,
    pipelines: &crate::pipeline::Pipelines,
    samples: u32,
) -> Shafts {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("game water shafts"),
        source: wgpu::ShaderSource::Wgsl(
            crate::pipeline::depth_source(&crate::pipeline::shafts_source(), samples).into(),
        ),
    });
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("game water shafts"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Depth,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: samples > 1,
                },
                count: None,
            },
        ],
    });
    // Scattered light adds to what the forward pass resolved.
    let add = wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::One,
        dst_factor: wgpu::BlendFactor::One,
        operation: wgpu::BlendOperation::Add,
    };
    let pipeline = crate::pipeline::make_pipeline(
        device,
        pipelines.cache.handle.as_ref(),
        &shader,
        "game water shafts",
        "vs",
        Some("fs"),
        &[Some(&layout)],
        &[],
        None,
        1,
        wgpu::TextureFormat::Rgba16Float,
        Some(wgpu::BlendState {
            color: add,
            alpha: add,
        }),
        &[],
        false,
        true,
    );
    Shafts {
        pipeline,
        layout,
        bind: None,
    }
}
