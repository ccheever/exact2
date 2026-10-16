use crate::Vertex;
use exact_gpu::wgpu;
use std::sync::OnceLock;

/// Packed shader pieces (build.rs strips comments, keeping line numbers). Each
/// assembled module is a list of these, never an edited string.
macro_rules! piece {
    ($name:literal) => {
        include_str!(concat!(env!("OUT_DIR"), "/", $name, ".wgsl"))
    };
}
const FRAME: &str = piece!("frame");
const TRANSFORM: &str = piece!("transform");
const SHADOW_SAMPLE: &str = piece!("shadow_sample");
const NO_SHADOW_SAMPLE: &str = piece!("no_shadow_sample");
const IBL: &str = piece!("ibl");
const FORWARD: &str = piece!("forward");
const LIGHTS: &str = piece!("lights");
const FADE: &str = piece!("fade");
const TONE: &str = piece!("tone");
const MODEL: &str = piece!("model");
/// The environment prefilter, a standalone module (ibl.rs).
pub(crate) const ENVIRONMENT: &str = piece!("environment");

/// `early`: with the forced-early-depth entry points (`forward_early.wgsl`), which
/// need the device's SHADER_EARLY_DEPTH_TEST.
fn primitive_sources(early: bool) -> [String; 5] {
    let forward_early = if early { piece!("forward_early") } else { "" };
    [
        [
            FRAME,
            TRANSFORM,
            SHADOW_SAMPLE,
            IBL,
            FADE,
            LIGHTS,
            TONE,
            FORWARD,
            forward_early,
        ]
        .concat(),
        [FRAME, TRANSFORM, FADE, piece!("shadow")].concat(),
        [FRAME, TONE, piece!("sky")].concat(),
        [FRAME, TONE, piece!("tonemap")].concat(),
        [FRAME, piece!("bloom")].concat(),
    ]
}

/// The PBR module with the forced-early-depth entry points.
#[cfg(test)]
pub(crate) fn early_forward_source() -> String {
    primitive_sources(true)[0].clone()
}

#[cfg(test)]
pub(crate) fn shader_sources() -> [String; 9] {
    let [a, b, c, d, e] = primitive_sources(false);
    let models = [model_source(false), model_source(true)];
    let [f, g] = models;
    [a, b, c, d, e, f, g, cull_source(), ENVIRONMENT.into()]
}

/// Frustum culling reuses the scene's transform bindings (0-5) in compute.
pub(crate) fn cull_source() -> String {
    [FRAME, TRANSFORM, piece!("cull")].concat()
}

pub(crate) fn model_source(shadow: bool) -> String {
    // Separate entry points leave the primitive path free of texture bindings/samples.
    // Group 1 is the cascade camera in shadow passes, so sampling is stubbed there.
    let (sample, tail) = if shadow {
        (NO_SHADOW_SAMPLE, piece!("model_shadow"))
    } else {
        (SHADOW_SAMPLE, "")
    };
    [
        FRAME, TRANSFORM, sample, IBL, FADE, LIGHTS, TONE, FORWARD, MODEL, tail,
    ]
    .concat()
}

/// Pipelines every renderer creates at construction: eight forward, two shadow,
/// sky, two tone, three bloom, three culling compute and the environment prefilter.
pub(crate) const STARTUP_PIPELINES: u64 = 20;

/// Forward variant bits beyond the eight made at construction (shadowed, fogged,
/// mirrored); [`Pipelines::forward_variant`] makes the rest on first use.
pub(crate) const LEAN: usize = 8;
pub(crate) const EARLY: usize = 16;
pub(crate) const DIRECT: usize = 32;
pub(crate) const UNIFORM: usize = 64;

pub(crate) struct Pipelines {
    pub models: Option<ModelPipelines>,
    pub forward: [wgpu::RenderPipeline; 8],
    variants: [OnceLock<wgpu::RenderPipeline>; 128],
    sky_direct: OnceLock<wgpu::RenderPipeline>,
    pbr: wgpu::ShaderModule,
    sky_module: wgpu::ShaderModule,
    format: wgpu::TextureFormat,
    /// The device forces early depth tests: the EARLY variants exist.
    pub early: bool,
    pub shadow: [wgpu::RenderPipeline; 2],
    pub sky: wgpu::RenderPipeline,
    pub tone: [wgpu::RenderPipeline; 2],
    pub bloom: [wgpu::RenderPipeline; 2],
    pub bright: wgpu::ComputePipeline,
    pub bright_target_layout: wgpu::BindGroupLayout,
    pub scene_layout: wgpu::BindGroupLayout,
    pub tone_layout: wgpu::BindGroupLayout,
    pub shadow_layout: wgpu::BindGroupLayout,
    pub camera_layout: wgpu::BindGroupLayout,
    pub bloom_layout: wgpu::BindGroupLayout,
}

impl Pipelines {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let buffer = |binding, ty, visibility| wgpu::BindGroupLayoutEntry {
            binding,
            visibility,
            ty: wgpu::BindingType::Buffer {
                ty,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let uniform = buffer(
            0,
            wgpu::BufferBindingType::Uniform,
            wgpu::ShaderStages::VERTEX_FRAGMENT,
        );
        let storage = |binding, visibility| {
            buffer(
                binding,
                wgpu::BufferBindingType::Storage { read_only: true },
                visibility,
            )
        };
        let texture = |binding, sample_type, view_dimension| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type,
                view_dimension,
                multisampled: false,
            },
            count: None,
        };
        let sampler = |binding, kind| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(kind),
            count: None,
        };
        let layout = |label, entries: &[_]| {
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some(label),
                entries,
            })
        };
        let scene_layout = layout(
            "game scene",
            &[
                uniform,
                storage(1, wgpu::ShaderStages::VERTEX),
                storage(2, wgpu::ShaderStages::VERTEX),
                storage(3, wgpu::ShaderStages::VERTEX_FRAGMENT),
                // Retained draws bind offset zero; culled draws bind their region.
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: true,
                        min_binding_size: None,
                    },
                    count: None,
                },
                storage(5, wgpu::ShaderStages::VERTEX),
                // The environment's prefiltered radiance (ibl.wgsl).
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::Cube,
                        multisampled: false,
                    },
                    count: None,
                },
                sampler(7, wgpu::SamplerBindingType::Filtering),
                // Clustered local lights (lights.wgsl).
                storage(8, wgpu::ShaderStages::FRAGMENT),
                // Per-slot screen-door fade (fade.wgsl `faded`).
                storage(9, wgpu::ShaderStages::FRAGMENT),
                // An authored environment map drawn as the visible sky (sky.wgsl).
                texture(
                    10,
                    wgpu::TextureSampleType::Float { filterable: true },
                    wgpu::TextureViewDimension::D2,
                ),
                sampler(11, wgpu::SamplerBindingType::Filtering),
            ],
        );
        let tone_layout = layout(
            "game tonemap",
            &[
                uniform,
                texture(
                    1,
                    wgpu::TextureSampleType::Float { filterable: false },
                    wgpu::TextureViewDimension::D2,
                ),
            ],
        );
        // Group 1 of every forward pipeline: sun cascades, the comparison
        // sampler and local light shadows (placeholders when absent).
        let shadow_layout = layout(
            "game shadow sample",
            &[
                texture(
                    0,
                    wgpu::TextureSampleType::Depth,
                    wgpu::TextureViewDimension::D2Array,
                ),
                sampler(1, wgpu::SamplerBindingType::Comparison),
                texture(
                    2,
                    wgpu::TextureSampleType::Depth,
                    wgpu::TextureViewDimension::D2Array,
                ),
            ],
        );
        let camera_layout = layout("game cascade camera", &[uniform]);
        let bloom_layout = layout(
            "game bloom",
            &[
                wgpu::BindGroupLayoutEntry {
                    visibility: wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::COMPUTE,
                    ..uniform
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: std::num::NonZeroU64::new(16),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    visibility: wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::COMPUTE,
                    ..texture(
                        1,
                        wgpu::TextureSampleType::Float { filterable: true },
                        wgpu::TextureViewDimension::D2,
                    )
                },
                sampler(2, wgpu::SamplerBindingType::Filtering),
            ],
        );
        let early = device
            .features()
            .contains(wgpu::Features::SHADER_EARLY_DEPTH_TEST);
        let sources = primitive_sources(early);
        let shaders: [_; 5] = std::array::from_fn(|i| {
            device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(
                    [
                        "game PBR",
                        "game shadow",
                        "game sky",
                        "game ACES",
                        "game bloom",
                    ][i],
                ),
                source: wgpu::ShaderSource::Wgsl(sources[i].as_str().into()),
            })
        });
        let vertex_layout = vertex_layout();
        let forward = std::array::from_fn(|i| {
            let shadow = i & 1 != 0;
            let layouts = [Some(&scene_layout), Some(&shadow_layout)];
            make_pipeline(
                device,
                &shaders[0],
                "game forward",
                "vs",
                Some(if shadow { "fs_shadow" } else { "fs" }),
                &layouts,
                &vertex_layout,
                scene_depth(true, wgpu::CompareFunction::Less, Default::default()),
                4,
                wgpu::TextureFormat::Rgba16Float,
                None,
                &[("FOG", if i & 2 != 0 { 1.0 } else { 0.0 })],
                i & 4 != 0,
            )
        });
        // Depth only, with a fragment stage that drops Opacity-faded texels so
        // primitive shadows dither and vanish with the primitive.
        let shadow = std::array::from_fn(|i| {
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("game shadow"),
                bind_group_layouts: &[Some(&scene_layout), Some(&camera_layout)],
                immediate_size: 0,
            });
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("game shadow"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shaders[1],
                    entry_point: Some("vs_shadow"),
                    buffers: &vertex_layout,
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shaders[1],
                    entry_point: Some("fs_shadow"),
                    targets: &[],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    front_face: if i != 0 {
                        wgpu::FrontFace::Cw
                    } else {
                        wgpu::FrontFace::Ccw
                    },
                    cull_mode: Some(wgpu::Face::Back),
                    ..Default::default()
                },
                depth_stencil: scene_depth(true, wgpu::CompareFunction::Less, Default::default()),
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            })
        });
        let sky = sky_pipeline(device, &shaders[2], &scene_layout, None);
        let tone = std::array::from_fn(|i| {
            let layouts = [Some(&tone_layout), Some(&bloom_layout)];
            make_pipeline(
                device,
                &shaders[3],
                "game ACES",
                "vs",
                Some(if i == 0 { "fs" } else { "fs_bloom" }),
                &layouts[..i + 1],
                &[],
                None,
                1,
                format,
                None,
                &[("ENCODE_SRGB", if format.is_srgb() { 0.0 } else { 1.0 })],
                false,
            )
        });
        let additive = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::One,
            operation: wgpu::BlendOperation::Add,
        };
        let bloom = std::array::from_fn(|i| {
            make_pipeline(
                device,
                &shaders[4],
                "game bloom",
                "vs",
                Some(["down", "up"][i]),
                &[Some(&bloom_layout)],
                &[],
                None,
                1,
                wgpu::TextureFormat::Rgba16Float,
                (i == 1).then_some(wgpu::BlendState {
                    color: additive,
                    alpha: additive,
                }),
                &[],
                false,
            )
        });
        let bright_target_layout = layout(
            "game bloom bright target",
            &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::StorageTexture {
                    access: wgpu::StorageTextureAccess::WriteOnly,
                    format: wgpu::TextureFormat::Rgba16Float,
                    view_dimension: wgpu::TextureViewDimension::D2,
                },
                count: None,
            }],
        );
        let bright = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("game bloom bright"),
            layout: Some(
                &device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("game bloom bright"),
                    bind_group_layouts: &[Some(&bloom_layout), Some(&bright_target_layout)],
                    immediate_size: 0,
                }),
            ),
            module: &shaders[4],
            entry_point: Some("bright_tile"),
            compilation_options: Default::default(),
            cache: None,
        });
        let [pbr, _, sky_module, _, _] = shaders;
        Self {
            models: None,
            forward,
            variants: std::array::from_fn(|_| OnceLock::new()),
            sky_direct: OnceLock::new(),
            pbr,
            sky_module,
            format,
            early,
            shadow,
            sky,
            tone,
            bloom,
            bright,
            bright_target_layout,
            scene_layout,
            tone_layout,
            shadow_layout,
            camera_layout,
            bloom_layout,
        }
    }
}

pub(crate) struct ModelPipelines {
    pub material: wgpu::BindGroupLayout,
    pub instance: wgpu::BindGroupLayout,
    pub forward: [Option<wgpu::RenderPipeline>; 32],
    pub shadow: [Option<wgpu::RenderPipeline>; 4],
    shaders: [wgpu::ShaderModule; 2],
    layouts: [wgpu::PipelineLayout; 3],
}
impl Pipelines {
    pub fn prepare_model(&mut self, device: &wgpu::Device, model: &exact_game::asset::Model) {
        let family = self.models.get_or_insert_with(|| {
            let material = crate::model_pipeline::material_layout(device);
            let instance = crate::model_pipeline::instance_layout(device);
            let shaders = std::array::from_fn(|i| {
                device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("game model shader"),
                    source: wgpu::ShaderSource::Wgsl(model_source(i == 1).into()),
                })
            });
            let layouts = std::array::from_fn(|i| {
                device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("game model pipeline layout"),
                    bind_group_layouts: &[
                        Some(&self.scene_layout),
                        Some(if i < 2 {
                            &self.shadow_layout
                        } else {
                            &self.camera_layout
                        }),
                        Some(&material),
                        Some(&instance),
                    ],
                    immediate_size: 0,
                })
            });
            ModelPipelines {
                material,
                instance,
                forward: std::array::from_fn(|_| None),
                shadow: std::array::from_fn(|_| None),
                shaders,
                layouts,
            }
        });
        for node in &model.nodes {
            let Some(mesh) = node.mesh else { continue };
            let m = &model.materials[model.meshes[mesh as usize].material as usize];
            for mirrored in 0..2 {
                let base = 4 * usize::from(m.double_sided)
                    + 8 * usize::from(m.alpha_mode == exact_game::asset::AlphaMode::Blend)
                    + 16 * mirrored;
                for effect in 0..4 {
                    let i = base + effect;
                    family.forward[i].get_or_insert_with(|| {
                        crate::model_pipeline::pipeline(
                            device,
                            &family.shaders[0],
                            &family.layouts[i & 1],
                            i,
                            false,
                        )
                    });
                }
                if m.alpha_mode != exact_game::asset::AlphaMode::Blend {
                    let i = usize::from(m.double_sided) + mirrored * 2;
                    family.shadow[i].get_or_insert_with(|| {
                        crate::model_pipeline::pipeline(
                            device,
                            &family.shaders[1],
                            &family.layouts[2],
                            base,
                            true,
                        )
                    });
                }
            }
        }
    }
}

impl Pipelines {
    /// Forward variant `key`: shadowed (1), fogged (2), mirrored (4), lean (no
    /// fill or local lights, no grid material), early (depth tested before shading), direct
    /// (tonemapped into the target's format) and uniform (a one-colour sky's
    /// irradiance). The first eight are made at construction; the rest on first use.
    pub fn forward_variant(&self, device: &wgpu::Device, key: usize) -> &wgpu::RenderPipeline {
        if key < 8 {
            return &self.forward[key];
        }
        self.variants[key].get_or_init(|| {
            let early = key & EARLY != 0;
            assert!(
                !early || self.early,
                "early forward variant without the device feature"
            );
            let entry = match (key & 1 != 0, early) {
                (false, false) => "fs",
                (true, false) => "fs_shadow",
                (false, true) => "fs_early",
                (true, true) => "fs_shadow_early",
            };
            let flag = |bit| if key & bit != 0 { 1.0 } else { 0.0 };
            let direct = key & DIRECT != 0;
            make_pipeline(
                device,
                &self.pbr,
                "game forward",
                "vs",
                Some(entry),
                &[Some(&self.scene_layout), Some(&self.shadow_layout)],
                &vertex_layout(),
                scene_depth(true, wgpu::CompareFunction::Less, Default::default()),
                4,
                if direct {
                    self.format
                } else {
                    wgpu::TextureFormat::Rgba16Float
                },
                None,
                &[
                    ("FOG", flag(2)),
                    ("FADES", 1.0 - flag(EARLY)),
                    ("LIGHTS", 1.0 - flag(LEAN)),
                    ("GRID", 1.0 - flag(LEAN)),
                    ("DIRECT", flag(DIRECT)),
                    ("UNIFORM_AMBIENT", flag(UNIFORM)),
                    ("ENCODE_SRGB", if self.format.is_srgb() { 0.0 } else { 1.0 }),
                ],
                key & 4 != 0,
            )
        })
    }
    /// The sky tonemapped into the target, for direct frames.
    pub fn sky_direct(&self, device: &wgpu::Device) -> &wgpu::RenderPipeline {
        self.sky_direct.get_or_init(|| {
            sky_pipeline(
                device,
                &self.sky_module,
                &self.scene_layout,
                Some(self.format),
            )
        })
    }
}

fn vertex_layout() -> [Option<wgpu::VertexBufferLayout<'static>>; 1] {
    const ATTRIBUTES: [wgpu::VertexAttribute; 4] =
        wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2, 3 => Float32x4];
    [Some(wgpu::VertexBufferLayout {
        array_stride: size_of::<Vertex>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &ATTRIBUTES,
    })]
}

fn scene_depth(
    write: bool,
    compare: wgpu::CompareFunction,
    bias: wgpu::DepthBiasState,
) -> Option<wgpu::DepthStencilState> {
    Some(wgpu::DepthStencilState {
        format: wgpu::TextureFormat::Depth32Float,
        depth_write_enabled: Some(write),
        depth_compare: Some(compare),
        stencil: Default::default(),
        bias,
    })
}

/// The sky into the HDR target, or tonemapped into `direct`'s format.
fn sky_pipeline(
    device: &wgpu::Device,
    module: &wgpu::ShaderModule,
    layout: &wgpu::BindGroupLayout,
    direct: Option<wgpu::TextureFormat>,
) -> wgpu::RenderPipeline {
    make_pipeline(
        device,
        module,
        "game sky",
        "sky_vs",
        Some("sky_fs"),
        &[Some(layout)],
        &[],
        scene_depth(false, wgpu::CompareFunction::LessEqual, Default::default()),
        4,
        direct.unwrap_or(wgpu::TextureFormat::Rgba16Float),
        None,
        &match direct {
            Some(format) => vec![
                ("DIRECT", 1.0),
                ("ENCODE_SRGB", if format.is_srgb() { 0.0 } else { 1.0 }),
            ],
            None => vec![],
        },
        false,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn make_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    label: &str,
    vs: &str,
    fs: Option<&str>,
    layouts: &[Option<&wgpu::BindGroupLayout>],
    buffers: &[Option<wgpu::VertexBufferLayout<'_>>],
    depth_stencil: Option<wgpu::DepthStencilState>,
    samples: u32,
    format: wgpu::TextureFormat,
    blend: Option<wgpu::BlendState>,
    constants: &[(&str, f64)],
    mirrored: bool,
) -> wgpu::RenderPipeline {
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(label),
        bind_group_layouts: layouts,
        immediate_size: 0,
    });
    let targets = [Some(wgpu::ColorTargetState {
        format,
        blend,
        write_mask: wgpu::ColorWrites::ALL,
    })];
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some(vs),
            buffers,
            compilation_options: Default::default(),
        },
        fragment: fs.map(|entry| wgpu::FragmentState {
            module: shader,
            entry_point: Some(entry),
            targets: &targets,
            compilation_options: wgpu::PipelineCompilationOptions {
                constants,
                ..Default::default()
            },
        }),
        primitive: wgpu::PrimitiveState {
            front_face: if mirrored {
                wgpu::FrontFace::Cw
            } else {
                wgpu::FrontFace::Ccw
            },
            cull_mode: if buffers.is_empty() {
                None
            } else {
                Some(wgpu::Face::Back)
            },
            ..Default::default()
        },
        depth_stencil,
        multisample: wgpu::MultisampleState {
            count: samples,
            ..Default::default()
        },
        multiview_mask: None,
        cache: None,
    })
}
