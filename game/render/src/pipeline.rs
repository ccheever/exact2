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
pub(crate) const OCCLUSION: &str = piece!("occlusion");
pub(crate) const SHADOW_SEED: &str = piece!("shadow_seed");

/// The occlusion pass's depth-only vertex shader (occlusion.rs).
pub(crate) fn occluder_source() -> String {
    [FRAME, TRANSFORM, piece!("occluder")].concat()
}
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

/// frame.wgsl's declaration of the surface colour's float, and its half
/// precision form, which caps a specular peak at 2048 to keep it finite.
const SHADING_F32: &str = "alias shading = f32; const SHADING_LIMIT = 3.0e38;";
const SHADING_F16: &str = "alias shading = f16; const SHADING_LIMIT = 2048.0;";
/// A forward module with its surface colour arithmetic in half precision, for
/// a device with SHADER_F16: a mobile GPU runs it at twice the rate in half the
/// registers. Positions, depth, directions and shadows stay f32 (frame.wgsl).
pub(crate) fn half_precision(source: &str) -> String {
    assert!(
        source.contains(SHADING_F32),
        "a forward module declares `shading`"
    );
    format!("enable f16;\n{}", source.replace(SHADING_F32, SHADING_F16))
}

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
    primitive_forward_source(true)
}
#[cfg(test)]
pub(crate) fn primitive_forward_source(early: bool) -> String {
    primitive_sources(early)[0].clone()
}

#[cfg(test)]
pub(crate) fn shader_sources() -> [String; 11] {
    let [a, b, c, d, e] = primitive_sources(false);
    let models = [model_source(false), model_source(true)];
    let [f, g] = models;
    [
        a,
        b,
        c,
        d,
        e,
        f,
        g,
        cull_source(),
        ENVIRONMENT.into(),
        outline_source(),
        SHADOW_SEED.into(),
    ]
}

/// Frustum culling reuses the scene's transform bindings (0-5) in compute.
pub(crate) fn cull_source() -> String {
    [FRAME, TRANSFORM, piece!("cull")].concat()
}

#[cfg(test)]
pub(crate) fn model_source(shadow: bool) -> String {
    model_sources(shadow, false)
}
pub(crate) fn model_sources(shadow: bool, early: bool) -> String {
    // Separate entry points leave the primitive path free of texture bindings/samples.
    // Group 1 is the cascade camera in shadow passes, so sampling is stubbed there.
    let (sample, tail) = if shadow {
        (NO_SHADOW_SAMPLE, piece!("model_shadow"))
    } else {
        (SHADOW_SAMPLE, "")
    };
    [
        FRAME,
        TRANSFORM,
        sample,
        IBL,
        FADE,
        LIGHTS,
        TONE,
        FORWARD,
        piece!("model_vertex"),
        MODEL,
        tail,
        if early && !shadow {
            piece!("model_early")
        } else {
            ""
        },
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
pub(crate) const SINGLE_CASCADE: usize = 128;
pub(crate) const PCF_SOFT: usize = 256;
pub(crate) const PREPASS: usize = 512;
/// Model forward variant bits beyond the primitive ones they share (shadowed 1,
/// fogged 2, PCF-soft and prepass): double-sided 4, blended 8, mirrored 16,
/// opaque without coverage fading 32, untextured 64, one cascade 128, and direct
/// (`MODEL_DIRECT`, tonemapped into the target's format).
pub(crate) const MODEL_DIRECT: usize = 1024;
/// A model build key's uniform-driven fallback, never an index.
pub(crate) const MODEL_GENERAL: usize = 8192;
pub(crate) const MODEL_BILINEAR: usize = 4096;
pub(crate) const BILINEAR: usize = 1024;
const PRIMITIVE_GENERAL: usize = 2048;
/// Fill or local lights are present; omitted light code frees fragment registers.
pub(crate) const MODEL_LIGHTS: usize = 2048;

pub(crate) struct Pipelines {
    pub cache: exact_gpu::PipelineCache,
    pub samples: u32,
    pub depth_prepass: bool,
    /// The device has SHADER_F16: forward modules shade in half precision.
    pub half: bool,
    pub matte_models: bool,
    pub models: Option<ModelPipelines>,
    pub forward: [wgpu::RenderPipeline; 8],
    variants: std::sync::Arc<[OnceLock<wgpu::RenderPipeline>; 2048]>,
    primitive_fallback: [OnceLock<wgpu::RenderPipeline>; 16],
    #[cfg(not(target_arch = "wasm32"))]
    primitive_workers: Specializations,
    camera_depth: [OnceLock<wgpu::RenderPipeline>; 8],
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
    #[cfg(test)]
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        Self::with_samples(device, format, 4)
    }
    #[cfg(test)]
    pub fn with_samples(device: &wgpu::Device, format: wgpu::TextureFormat, samples: u32) -> Self {
        Self::configured(device, format, samples, false)
    }
    pub fn configured(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        samples: u32,
        depth_prepass: bool,
    ) -> Self {
        let cache = exact_gpu::PipelineCache::new(device);
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
        let half = device.features().contains(wgpu::Features::SHADER_F16);
        let pbr_source = if half {
            half_precision(&sources[0])
        } else {
            sources[0].clone()
        };
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
                source: wgpu::ShaderSource::Wgsl(
                    if i == 0 { &pbr_source } else { &sources[i] }
                        .as_str()
                        .into(),
                ),
            })
        });
        let vertex_layout = vertex_layout();
        let forward = std::array::from_fn(|i| {
            let shadow = i & 1 != 0;
            let layouts = [Some(&scene_layout), Some(&shadow_layout)];
            make_pipeline(
                device,
                cache.handle.as_ref(),
                &shaders[0],
                "game forward",
                "vs",
                Some(if shadow { "fs_shadow" } else { "fs" }),
                &layouts,
                &vertex_layout,
                scene_depth(true, wgpu::CompareFunction::Less, Default::default()),
                samples,
                wgpu::TextureFormat::Rgba16Float,
                None,
                &[
                    ("FOG", if i & 2 != 0 { 1.0 } else { 0.0 }),
                    ("SHADOW_FILTER", 0.),
                ],
                i & 4 != 0,
                true,
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
                cache: cache.handle.as_ref(),
            })
        });
        let sky = sky_pipeline(
            device,
            cache.handle.as_ref(),
            &shaders[2],
            &scene_layout,
            None,
            samples,
        );
        let tone = std::array::from_fn(|i| {
            let layouts = [Some(&tone_layout), Some(&bloom_layout)];
            make_pipeline(
                device,
                cache.handle.as_ref(),
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
                true,
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
                cache.handle.as_ref(),
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
                true,
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
            cache: cache.handle.as_ref(),
        });
        let [pbr, _, sky_module, _, _] = shaders;
        Self {
            cache,
            samples,
            depth_prepass,
            matte_models: false,
            models: None,
            forward,
            variants: std::sync::Arc::new(std::array::from_fn(|_| OnceLock::new())),
            primitive_fallback: std::array::from_fn(|_| OnceLock::new()),
            #[cfg(not(target_arch = "wasm32"))]
            primitive_workers: Specializations::default(),
            camera_depth: std::array::from_fn(|_| OnceLock::new()),
            sky_direct: OnceLock::new(),
            pbr,
            sky_module,
            format,
            early,
            half,
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
    pub cache: Option<wgpu::PipelineCache>,
    pub material: wgpu::BindGroupLayout,
    pub instance: wgpu::BindGroupLayout,
    pub forward: std::sync::Arc<[OnceLock<wgpu::RenderPipeline>; MODEL_GENERAL]>,
    pub fallback: [OnceLock<wgpu::RenderPipeline>; 2048],
    #[cfg(not(target_arch = "wasm32"))]
    workers: Specializations,
    pub camera_depth: [OnceLock<wgpu::RenderPipeline>; 16],
    pub shadow: [OnceLock<wgpu::RenderPipeline>; 8],
    shaders: [wgpu::ShaderModule; 2],
    layouts: [wgpu::PipelineLayout; 3],
    /// Opaque depth programs have no fragment stage and so read no material:
    /// their layouts (camera depth, then shadow casting) take an empty group 2,
    /// bound once per pass, where the others take each draw's material.
    depth_layouts: [wgpu::PipelineLayout; 2],
    no_material: wgpu::BindGroup,
}
impl Pipelines {
    pub fn reap_specializations(&self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.primitive_workers.reap(false);
            if let Some(models) = &self.models {
                models.workers.reap(false);
            }
        }
    }

    #[cfg(all(test, not(target_arch = "wasm32")))]
    pub fn wait_specializations(&self) {
        self.primitive_workers.reap(true);
        if let Some(models) = &self.models {
            models.workers.reap(true);
        }
    }

    #[cfg(test)]
    pub fn wait_forward(&self, key: usize) {
        self.variants[key].wait();
    }

    /// The output format direct frames draw in.
    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    pub fn variant_count(&self) -> u64 {
        self.variants.iter().filter(|p| p.get().is_some()).count() as u64
            + self
                .primitive_fallback
                .iter()
                .filter(|p| p.get().is_some())
                .count() as u64
            + u64::from(self.sky_direct.get().is_some())
            + self
                .camera_depth
                .iter()
                .filter(|p| p.get().is_some())
                .count() as u64
    }

    pub fn prepare_model(&mut self, device: &wgpu::Device) {
        self.models.get_or_insert_with(|| {
            let material = crate::model_pipeline::material_layout(device);
            let instance = crate::model_pipeline::instance_layout(device);
            let shaders = std::array::from_fn(|i| {
                device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("game model shader"),
                    source: wgpu::ShaderSource::Wgsl({
                        let source = model_sources(i == 1, self.early);
                        // The forward module shades; the shadow module only casts.
                        if i == 0 && self.half {
                            half_precision(&source).into()
                        } else {
                            source.into()
                        }
                    }),
                })
            });
            let layout = |camera: &wgpu::BindGroupLayout, material| {
                device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("game model pipeline layout"),
                    bind_group_layouts: &[
                        Some(&self.scene_layout),
                        Some(camera),
                        Some(material),
                        Some(&instance),
                    ],
                    immediate_size: 0,
                })
            };
            let layouts = std::array::from_fn(|i| {
                layout(
                    if i < 2 {
                        &self.shadow_layout
                    } else {
                        &self.camera_layout
                    },
                    &material,
                )
            });
            let empty = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("game no material"),
                entries: &[],
            });
            let depth_layouts = [
                layout(&self.shadow_layout, &empty),
                layout(&self.camera_layout, &empty),
            ];
            let no_material = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("game no material"),
                layout: &empty,
                entries: &[],
            });
            ModelPipelines {
                cache: self.cache.handle.clone(),
                material,
                instance,
                forward: std::sync::Arc::new(std::array::from_fn(|_| OnceLock::new())),
                fallback: std::array::from_fn(|_| OnceLock::new()),
                #[cfg(not(target_arch = "wasm32"))]
                workers: Specializations::default(),
                camera_depth: std::array::from_fn(|_| OnceLock::new()),
                shadow: std::array::from_fn(|_| OnceLock::new()),
                shaders,
                layouts,
                depth_layouts,
                no_material,
            }
        });
    }

    pub fn model_variant(
        &self,
        device: &wgpu::Device,
        key: usize,
        shadow: bool,
    ) -> &wgpu::RenderPipeline {
        let family = self.models.as_ref().unwrap();
        let make = |key, shadow| {
            crate::model_pipeline::pipeline(
                device,
                self.cache.handle.as_ref(),
                &family.shaders[usize::from(shadow)],
                match (shadow, key & 32 != 0) {
                    (true, true) => &family.depth_layouts[1],
                    (true, false) => &family.layouts[2],
                    (false, _) => &family.layouts[0],
                },
                key,
                shadow,
                (self.samples, self.format),
                (self.early, key & PREPASS != 0, false, self.matte_models),
            )
        };
        if shadow {
            let index = usize::from(key & 4 != 0) + 2 * usize::from(key & 16 != 0);
            // Coverage is also the fallback for a later Opacity change. These
            // depth-only programs are small; no lighting cross-product exists.
            let coverage = family.shadow[index].get_or_init(|| make(key & !32, true));
            return if key & 32 == 0 {
                coverage
            } else {
                family.shadow[index + 4].get_or_init(|| make(key, true))
            };
        }
        let slot = &family.forward[key];
        if let Some(pipeline) = slot.get() {
            return pipeline;
        }
        {
            let class = key & (4 | 8 | 16 | 64 | PREPASS | MODEL_DIRECT);
            #[cfg(target_arch = "wasm32")]
            let first = family.fallback[class].get().is_none();
            // Uniform-driven opaque and coverage programs handle live settings.
            // The opaque fallback must retain early depth while a worker specializes it.
            let coverage =
                family.fallback[class].get_or_init(|| make(class | MODEL_GENERAL | 3, false));
            let opaque = (class & 8 == 0).then(|| {
                family.fallback[class | 32]
                    .get_or_init(|| make(class | MODEL_GENERAL | 3 | 32, false))
            });
            let fallback = if key & 32 != 0 {
                opaque.unwrap()
            } else {
                coverage
            };
            #[cfg(target_arch = "wasm32")]
            if first {
                return slot.get_or_init(|| make(key, false));
            }
            #[cfg(not(target_arch = "wasm32"))]
            {
                let slots = family.forward.clone();
                let device = device.clone();
                let shader = family.shaders[0].clone();
                let layout = family.layouts[0].clone();
                let cache = self.cache.handle.clone();
                let (target, early) = ((self.samples, self.format), self.early);
                let matte = self.matte_models;
                family.workers.start(key, move || {
                    slots[key].get_or_init(|| {
                        crate::model_pipeline::pipeline(
                            &device,
                            cache.as_ref(),
                            &shader,
                            &layout,
                            key,
                            false,
                            target,
                            (early, key & PREPASS != 0, false, matte),
                        )
                    });
                });
            }
            fallback
        }
    }
}

impl Pipelines {
    /// Forward variant `key`: shadowed (1), fogged (2), mirrored (4), lean (no
    /// fill or local lights, no grid material), early (depth tested before shading), direct
    /// (tonemapped into the target's format) and uniform (a one-colour sky's
    /// irradiance). The first eight are made at construction; the rest on first use.
    pub fn forward_variant(&self, device: &wgpu::Device, key: usize) -> &wgpu::RenderPipeline {
        let class = key & (4 | DIRECT | PREPASS);
        let index = 2
            * (usize::from(class & 4 != 0)
                + 2 * usize::from(class & DIRECT != 0)
                + 4 * usize::from(class & PREPASS != 0));
        if key < 8 && self.primitive_fallback[index].get().is_some() {
            return &self.forward[key];
        }
        let slot = &self.variants[key];
        if let Some(pipeline) = slot.get() {
            return pipeline;
        }
        let device = device.clone();
        let cache = self.cache.handle.clone();
        let shader = self.pbr.clone();
        let scene_layout = self.scene_layout.clone();
        let shadow_layout = self.shadow_layout.clone();
        let (samples, format, supports_early) = (self.samples, self.format, self.early);
        let build = move |key| {
            let general = key & PRIMITIVE_GENERAL != 0;
            let early = key & EARLY != 0;
            assert!(
                !early || supports_early,
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
                &device,
                cache.as_ref(),
                &shader,
                "game forward",
                "vs",
                Some(entry),
                &[Some(&scene_layout), Some(&shadow_layout)],
                &vertex_layout(),
                scene_depth(
                    true,
                    if key & PREPASS != 0 {
                        wgpu::CompareFunction::Equal
                    } else {
                        wgpu::CompareFunction::Less
                    },
                    Default::default(),
                ),
                samples,
                if direct {
                    format
                } else {
                    wgpu::TextureFormat::Rgba16Float
                },
                None,
                &[
                    ("FOG", flag(2)),
                    ("DEPTH_PREPASS", flag(PREPASS)),
                    ("FADES", 1.0 - flag(EARLY)),
                    ("LIGHTS", 1.0 - flag(LEAN)),
                    ("GRID", 1.0 - flag(LEAN)),
                    ("DIRECT", flag(DIRECT)),
                    ("UNIFORM_AMBIENT", flag(UNIFORM)),
                    ("SINGLE_CASCADE", flag(SINGLE_CASCADE)),
                    (
                        "SHADOW_FILTER",
                        if general {
                            2.
                        } else if key & BILINEAR != 0 {
                            3.
                        } else {
                            flag(PCF_SOFT)
                        },
                    ),
                    ("ENCODE_SRGB", if format.is_srgb() { 0.0 } else { 1.0 }),
                ],
                key & 4 != 0,
                true,
            )
        };
        {
            #[cfg(target_arch = "wasm32")]
            let first = self.primitive_fallback[index].get().is_none();
            let coverage =
                self.primitive_fallback[index].get_or_init(|| build(class | PRIMITIVE_GENERAL | 3));
            let opaque = self.early.then(|| {
                self.primitive_fallback[index + 1]
                    .get_or_init(|| build(class | PRIMITIVE_GENERAL | 3 | EARLY))
            });
            let fallback = if key & EARLY != 0 {
                opaque.unwrap()
            } else {
                coverage
            };
            if key < 8 {
                return &self.forward[key];
            }
            #[cfg(target_arch = "wasm32")]
            if first {
                return slot.get_or_init(|| build(key));
            }
            #[cfg(not(target_arch = "wasm32"))]
            {
                let slots = self.variants.clone();
                self.primitive_workers.start(key, move || {
                    slots[key].get_or_init(|| build(key));
                });
            }
            fallback
        }
    }

    pub fn camera_depth(
        &self,
        device: &wgpu::Device,
        mirrored: bool,
        fading: bool,
        direct: bool,
    ) -> &wgpu::RenderPipeline {
        self.camera_depth[usize::from(mirrored) + 2 * usize::from(fading) + 4 * usize::from(direct)]
            .get_or_init(|| {
                make_pipeline(
                    device,
                    self.cache.handle.as_ref(),
                    &self.pbr,
                    "camera depth",
                    // Without fading, position only; `fs_depth` reads the slot.
                    if fading { "vs" } else { "vs_depth" },
                    (fading || direct).then_some(if fading {
                        "fs_depth"
                    } else {
                        "camera_depth_empty"
                    }),
                    &[Some(&self.scene_layout), Some(&self.shadow_layout)],
                    &vertex_layout(),
                    scene_depth(true, wgpu::CompareFunction::Less, Default::default()),
                    self.samples,
                    if direct {
                        self.format
                    } else {
                        wgpu::TextureFormat::Depth32Float
                    },
                    None,
                    &[],
                    mirrored,
                    false,
                )
            })
    }
    /// Group 2 of an opaque model's depth program.
    pub fn no_material(&self) -> &wgpu::BindGroup {
        &self.models.as_ref().unwrap().no_material
    }
    pub fn model_camera_depth(&self, device: &wgpu::Device, key: usize) -> &wgpu::RenderPipeline {
        let family = self.models.as_ref().unwrap();
        let index = usize::from(key & 4 != 0)
            + 2 * usize::from(key & 16 != 0)
            + 4 * usize::from(key & 32 != 0)
            + 8 * usize::from(key & MODEL_DIRECT != 0);
        let make = |key: usize| {
            crate::model_pipeline::pipeline(
                device,
                self.cache.handle.as_ref(),
                &family.shaders[0],
                if key & 32 != 0 {
                    &family.depth_layouts[0]
                } else {
                    &family.layouts[0]
                },
                key,
                false,
                (self.samples, self.format),
                (false, false, true, self.matte_models),
            )
        };
        let coverage = family.camera_depth[index & !4].get_or_init(|| make(key & !32));
        if key & 32 == 0 {
            coverage
        } else {
            family.camera_depth[index].get_or_init(|| make(key))
        }
    }
    /// The sky tonemapped into the target, for direct frames.
    pub fn sky_direct(&self, device: &wgpu::Device) -> &wgpu::RenderPipeline {
        self.sky_direct.get_or_init(|| {
            sky_pipeline(
                device,
                self.cache.handle.as_ref(),
                &self.sky_module,
                &self.scene_layout,
                Some(self.format),
                self.samples,
            )
        })
    }
}

pub(crate) fn vertex_layout() -> [Option<wgpu::VertexBufferLayout<'static>>; 1] {
    const ATTRIBUTES: [wgpu::VertexAttribute; 4] =
        wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2, 3 => Float32x4];
    [Some(wgpu::VertexBufferLayout {
        array_stride: size_of::<Vertex>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &ATTRIBUTES,
    })]
}

pub(crate) fn scene_depth(
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
    cache: Option<&wgpu::PipelineCache>,
    module: &wgpu::ShaderModule,
    layout: &wgpu::BindGroupLayout,
    direct: Option<wgpu::TextureFormat>,
    samples: u32,
) -> wgpu::RenderPipeline {
    make_pipeline(
        device,
        cache,
        module,
        "game sky",
        "sky_vs",
        Some("sky_fs"),
        &[Some(layout)],
        &[],
        scene_depth(false, wgpu::CompareFunction::LessEqual, Default::default()),
        samples,
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
        true,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn make_pipeline(
    device: &wgpu::Device,
    cache: Option<&wgpu::PipelineCache>,
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
    color_write: bool,
) -> wgpu::RenderPipeline {
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(label),
        bind_group_layouts: layouts,
        immediate_size: 0,
    });
    let targets = [Some(wgpu::ColorTargetState {
        format,
        blend,
        write_mask: if color_write {
            wgpu::ColorWrites::ALL
        } else {
            wgpu::ColorWrites::empty()
        },
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
            targets: if format == wgpu::TextureFormat::Depth32Float {
                &[]
            } else {
                &targets
            },
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
        cache,
    })
}

/// A single-sample depth load uses mip zero where MSAA uses sample zero.
pub(crate) fn depth_source(source: &str, samples: u32) -> String {
    if samples == 1 {
        source
            .replace("texture_depth_multisampled_2d", "texture_depth_2d")
            .replace("textureNumSamples(depth)", "1u")
    } else {
        source.into()
    }
}

pub(crate) fn outline_source() -> String {
    [
        FRAME,
        TRANSFORM,
        FADE,
        TONE,
        piece!("model_vertex"),
        piece!("outline"),
    ]
    .concat()
    .replace(
        "const OUTLINE_OFFSET: bool = false;",
        "const OUTLINE_OFFSET: bool = true;",
    )
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod model_variant_tests {
    use super::*;

    #[test]
    fn first_primitive_draw_uses_fallback_while_worker_specializes() {
        let Some(gpu) = crate::test_device::device_or_skip(exact_gpu::fixture::device()) else {
            return;
        };
        let p = Pipelines::with_samples(&gpu.device, wgpu::TextureFormat::Rgba8Unorm, 1);
        for key in [LEAN | SINGLE_CASCADE, LEAN | SINGLE_CASCADE | 4] {
            let selected = p.forward_variant(&gpu.device, key);
            let index = 2 * usize::from(key & 4 != 0);
            assert_eq!(selected, p.primitive_fallback[index].get().unwrap());
            let specialized = p.variants[key].wait();
            assert_eq!(specialized, p.forward_variant(&gpu.device, key));
        }
    }

    #[test]
    fn model_preparation_compiles_only_requested_variants_and_reuses_them() {
        let Some(gpu) = crate::test_device::device_or_skip(exact_gpu::fixture::device()) else {
            return;
        };
        let mut p = Pipelines::with_samples(&gpu.device, wgpu::TextureFormat::Rgba8Unorm, 1);
        p.prepare_model(&gpu.device);
        let family = p.models.as_ref().unwrap();
        assert!(family.forward.iter().all(|v| v.get().is_none()));
        assert!(family.shadow.iter().all(|v| v.get().is_none()));
        for key in [32 + 64, 3 + 32 + 64 + 128 + 256, 3 + 8 + 16] {
            let selected = p.model_variant(&gpu.device, key, false);
            let class = key & (4 | 8 | 16 | 64 | PREPASS);
            assert_eq!(
                selected,
                family.fallback[class | (key & 32)].get().unwrap(),
                "even the first draw of a family uses its ready fallback"
            );
            let pipeline = family.forward[key].wait();
            assert_eq!(pipeline, p.model_variant(&gpu.device, key, false));
        }
        for key in [0, 4 + 16 + 32] {
            let pipeline = p.model_variant(&gpu.device, key, true).clone();
            assert_eq!(&pipeline, p.model_variant(&gpu.device, key, true));
        }
        assert_eq!(
            family.forward.iter().filter(|v| v.get().is_some()).count(),
            3
        );
        assert_eq!(
            family.shadow.iter().filter(|v| v.get().is_some()).count(),
            3
        );
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Copy, Default)]
struct Attempt {
    running: bool,
    tries: u8,
    retry_frames: u8,
}

#[cfg(not(target_arch = "wasm32"))]
struct Specializations(std::sync::Mutex<Workers>);
#[cfg(not(target_arch = "wasm32"))]
struct Workers {
    attempts: [Attempt; MODEL_GENERAL],
    handles: Vec<(usize, std::thread::JoinHandle<()>)>,
}
#[cfg(not(target_arch = "wasm32"))]
impl Default for Specializations {
    fn default() -> Self {
        Self(std::sync::Mutex::new(Workers {
            attempts: [Attempt::default(); MODEL_GENERAL],
            handles: Vec::new(),
        }))
    }
}
#[cfg(not(target_arch = "wasm32"))]
impl Specializations {
    fn start(&self, key: usize, work: impl FnOnce() + Send + 'static) {
        self.start_with(key, || {
            std::thread::Builder::new()
                .name("exact specialization".into())
                .spawn(work)
        });
    }

    fn start_with(
        &self,
        key: usize,
        spawn: impl FnOnce() -> std::io::Result<std::thread::JoinHandle<()>>,
    ) {
        let mut workers = self.0.lock().unwrap();
        let attempt = &mut workers.attempts[key];
        if attempt.running || attempt.tries == 3 || attempt.retry_frames != 0 {
            return;
        }
        attempt.tries += 1;
        match spawn() {
            Ok(handle) => {
                attempt.running = true;
                workers.handles.push((key, handle));
            }
            Err(error) => {
                attempt.running = false;
                attempt.retry_frames = 60;
                eprintln!("game specialization spawn failed: {error}");
            }
        }
    }

    fn reap(&self, wait: bool) {
        let mut workers = self.0.lock().unwrap();
        for attempt in &mut workers.attempts {
            attempt.retry_frames = attempt.retry_frames.saturating_sub(1);
        }
        let mut i = 0;
        while i < workers.handles.len() {
            if wait || workers.handles[i].1.is_finished() {
                let (key, worker) = workers.handles.swap_remove(i);
                let attempt = &mut workers.attempts[key];
                attempt.running = false;
                if worker.join().is_err() {
                    attempt.retry_frames = 60;
                    eprintln!("game specialization worker panicked for key {key}");
                }
            } else {
                i += 1;
            }
        }
    }
}
#[cfg(not(target_arch = "wasm32"))]
impl Drop for Specializations {
    fn drop(&mut self) {
        // No worker may execute artifact code after gpu_unload returns.
        self.reap(true);
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod worker_tests {
    use super::*;

    #[test]
    fn spawn_failure_and_panic_clear_running_and_retry_with_a_bound() {
        let workers = Specializations::default();
        workers.start_with(17, || Err(std::io::Error::other("injected spawn failure")));
        assert!(!workers.0.lock().unwrap().attempts[17].running);
        workers.start_with(17, || panic!("retry must wait for later frames"));
        for _ in 0..60 {
            workers.reap(false);
        }
        workers.start(17, || panic!("injected worker panic"));
        workers.reap(true);
        assert!(!workers.0.lock().unwrap().attempts[17].running);
        for _ in 0..60 {
            workers.reap(false);
        }
        let result = std::sync::Arc::new(OnceLock::new());
        let output = result.clone();
        workers.start(17, move || {
            output.set(42).unwrap();
        });
        workers.reap(true);
        assert_eq!(result.get(), Some(&42));
        assert!(workers.0.lock().unwrap().handles.is_empty());
        workers.start_with(17, || panic!("three attempts exhaust the key's budget"));
        // A different key still compiles after one key exhausts its budget.
        workers.start(18, || {});
        workers.reap(true);
        assert_eq!(workers.0.lock().unwrap().attempts[18].tries, 1);
    }

    #[test]
    fn frames_reap_completed_workers_without_waiting_for_live_ones() {
        let workers = Specializations::default();
        let (release, wait) = std::sync::mpsc::channel();
        workers.start(1, move || {
            wait.recv().unwrap();
        });
        workers.start_with(1, || panic!("only one worker per key"));
        workers.reap(false);
        assert_eq!(workers.0.lock().unwrap().handles.len(), 1);
        workers.start(2, || {});
        let end = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while !workers.0.lock().unwrap().handles[1].1.is_finished() {
            assert!(std::time::Instant::now() < end);
            std::thread::yield_now();
        }
        workers.reap(false);
        assert_eq!(workers.0.lock().unwrap().handles.len(), 1);
        assert!(!workers.0.lock().unwrap().attempts[2].running);
        release.send(()).unwrap();
        drop(workers);
    }
}
