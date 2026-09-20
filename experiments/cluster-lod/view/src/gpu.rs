use crate::scene::{Instance, Scene};
use bytemuck::{Pod, Zeroable};
use clod_format::{Reader, Vertex};
use wgpu::util::DeviceExt;

pub const SHADOW_SIZE: u32 = 2048;
pub type Result<T> = std::result::Result<T, String>;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Cluster,
    Naive,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum View {
    Lit,
    Clusters,
    Depth,
    Triangles,
    Instances,
    Overdraw,
    Coverage,
}
impl View {
    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "lit" => Ok(Self::Lit),
            "clusters" => Ok(Self::Clusters),
            "depth" => Ok(Self::Depth),
            "triangles" => Ok(Self::Triangles),
            "instances" => Ok(Self::Instances),
            "overdraw" => Ok(Self::Overdraw),
            "coverage" => Ok(Self::Coverage),
            _ => Err("unknown view".into()),
        }
    }
}
pub struct Baseline {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}
/// Exact portability oracle. Timing is an explicit opt-in, never requested by image tests.
pub fn device_descriptor(timestamps: bool) -> wgpu::DeviceDescriptor<'static> {
    wgpu::DeviceDescriptor {
        required_features: if timestamps {
            wgpu::Features::TIMESTAMP_QUERY
        } else {
            wgpu::Features::empty()
        },
        required_limits: wgpu::Limits::default(),
        ..Default::default()
    }
}
pub async fn request_device(
    timing: bool,
) -> Result<(wgpu::Device, wgpu::Queue, wgpu::AdapterInfo)> {
    if wgpu::Instance::enabled_backend_features().is_empty() {
        return Err("NO ADAPTER: this target has no compiled Metal/WebGPU backend".into());
    }
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::METAL | wgpu::Backends::BROWSER_WEBGPU,
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions::default())
        .await
        .map_err(|e| format!("NO ADAPTER: {e}"))?;
    if timing && !adapter.features().contains(wgpu::Features::TIMESTAMP_QUERY) {
        return Err("time requires TIMESTAMP_QUERY; adapter does not expose timestamps".into());
    }
    let (device, queue) = adapter
        .request_device(&device_descriptor(timing))
        .await
        .map_err(|e| e.to_string())?;
    Ok((device, queue, adapter.get_info()))
}
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct Globals {
    pub vp: [f32; 16],
    pub light_vp: [f32; 16],
    pub eye: [f32; 4],
    pub ground: [f32; 4],
    pub params: [u32; 4],
}
pub(crate) struct PageGpu {
    pub geometry: wgpu::Buffer,
    pub params: wgpu::Buffer,
}
pub(crate) struct ListGpu {
    pub buffer: wgpu::Buffer,
    pub bind: wgpu::BindGroup,
    pub capacity: u64,
}
pub(crate) struct ChunkGpu {
    pub vertices: wgpu::Buffer,
    pub indices: wgpu::Buffer,
    pub count: u32,
}
pub struct Renderer {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub width: u32,
    pub height: u32,
    pub mode: Mode,
    pub culling: bool,
    pub shadows: bool,
    pub(crate) instance_sphere: [f32; 4],
    pub(crate) pages: Vec<PageGpu>,
    pub(crate) compute: Option<crate::compute::Compute>,
    pub(crate) dummy_draws: wgpu::Buffer,
    pub(crate) lists: Vec<ListGpu>,
    pub(crate) shadow_lists: Vec<ListGpu>,
    pub(crate) clusters: wgpu::Buffer,
    pub(crate) instances: wgpu::Buffer,
    pub(crate) chunks: Vec<ChunkGpu>,
    pub(crate) globals: wgpu::Buffer,
    pub(crate) global_bind: wgpu::BindGroup,
    pub(crate) page_layout: wgpu::BindGroupLayout,
    pub(crate) shadow_bind: wgpu::BindGroup,
    pub(crate) color: wgpu::Texture,
    pub(crate) msaa: wgpu::TextureView,
    pub(crate) depth: wgpu::TextureView,
    pub(crate) shadow: wgpu::TextureView,
    pub(crate) shadow_texture: wgpu::Texture,
    pub(crate) main: wgpu::RenderPipeline,
    pub(crate) overdraw: wgpu::RenderPipeline,
    pub(crate) ground: wgpu::RenderPipeline,
    pub(crate) shadow_pipeline: wgpu::RenderPipeline,
    pub(crate) query: Option<wgpu::QuerySet>,
    pub(crate) query_resolve: Option<wgpu::Buffer>,
    pub(crate) static_bytes: u64,
    pub(crate) instance_count: u32,
    pub(crate) max_triangles: u32,
    pub(crate) max_depth: u32,
}
pub(crate) fn buffer(
    device: &wgpu::Device,
    label: &str,
    bytes: &[u8],
    usage: wgpu::BufferUsages,
) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents: bytes,
        usage,
    })
}
fn entry(
    binding: u32,
    ty: wgpu::BufferBindingType,
    visibility: wgpu::ShaderStages,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}
fn texture(
    device: &wgpu::Device,
    w: u32,
    h: u32,
    samples: u32,
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("offscreen"),
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: samples,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage,
        view_formats: &[],
    })
}
impl Renderer {
    pub fn new(
        device: wgpu::Device,
        queue: wgpu::Queue,
        reader: &Reader<'_>,
        baseline: Option<&[Baseline]>,
        scene: &Scene,
        size: [u32; 2],
        mode: Mode,
    ) -> Result<Self> {
        scene.validate()?;
        let [width, height] = size;
        if width == 0 || height == 0 || width > 8192 || height > 8192 {
            return Err("size must be 1..8192".into());
        }
        let storage = wgpu::BufferUsages::STORAGE;
        let limits = device.limits();
        if std::mem::size_of_val(scene.instances.as_slice()) as u64
            > limits.max_storage_buffer_binding_size
        {
            return Err("instance table exceeds storage binding limit".into());
        }
        if mode == Mode::Naive {
            for b in baseline.ok_or("naive mode needs baseline")? {
                if std::mem::size_of_val(b.vertices.as_slice()) as u64 > limits.max_buffer_size
                    || std::mem::size_of_val(b.indices.as_slice()) as u64 > limits.max_buffer_size
                {
                    return Err("baseline chunk exceeds core buffer limit".into());
                }
            }
        }
        let instances = buffer(
            &device,
            "instances",
            bytemuck::cast_slice::<Instance, u8>(&scene.instances),
            storage,
        );
        let cluster_bytes = bytemuck::cast_slice(reader.clusters);
        if mode == Mode::Cluster
            && cluster_bytes.len() as u64 > limits.max_storage_buffer_binding_size
        {
            return Err("cluster metadata exceeds core binding limit".into());
        }
        let clusters = buffer(
            &device,
            "clusters",
            if mode == Mode::Cluster {
                cluster_bytes
            } else {
                &[0u8; 128]
            },
            storage,
        );
        let dummy_draws = buffer(&device, "CPU draw offsets", &[0u8; 32], storage);
        let page_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("four storage buffers"),
            entries: &[
                entry(
                    0,
                    wgpu::BufferBindingType::Storage { read_only: true },
                    wgpu::ShaderStages::VERTEX,
                ),
                entry(
                    1,
                    wgpu::BufferBindingType::Storage { read_only: true },
                    wgpu::ShaderStages::VERTEX,
                ),
                entry(
                    2,
                    wgpu::BufferBindingType::Storage { read_only: true },
                    wgpu::ShaderStages::VERTEX,
                ),
                entry(
                    3,
                    wgpu::BufferBindingType::Storage { read_only: true },
                    wgpu::ShaderStages::VERTEX,
                ),
                entry(
                    5,
                    wgpu::BufferBindingType::Storage { read_only: true },
                    wgpu::ShaderStages::VERTEX,
                ),
                entry(
                    4,
                    wgpu::BufferBindingType::Uniform,
                    wgpu::ShaderStages::VERTEX,
                ),
            ],
        });
        let global_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("camera"),
            entries: &[entry(
                0,
                wgpu::BufferBindingType::Uniform,
                wgpu::ShaderStages::VERTEX_FRAGMENT,
            )],
        });
        let globals = buffer(
            &device,
            "globals",
            bytemuck::bytes_of(&Globals::zeroed()),
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        );
        let global_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &global_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: globals.as_entire_binding(),
            }],
        });
        let mut static_bytes =
            instances.size() + clusters.size() + globals.size() + dummy_draws.size();
        let mut pages = Vec::new();
        for (id, p) in reader
            .pages
            .iter()
            .enumerate()
            .take(if mode == Mode::Cluster {
                reader.pages.len()
            } else {
                1
            })
        {
            let geometry = buffer(
                &device,
                "baked page",
                if mode == Mode::Cluster {
                    reader.page_bytes(id).expect("page")
                } else {
                    &[0u8; 16]
                },
                storage,
            );
            let params = buffer(
                &device,
                "page offsets",
                bytemuck::cast_slice(&[p.indices_offset, id as u32, 0, 0]),
                wgpu::BufferUsages::UNIFORM,
            );
            static_bytes += geometry.size() + params.size();
            pages.push(PageGpu { geometry, params });
        }
        let mut chunks = Vec::new();
        if mode == Mode::Naive {
            for b in baseline.ok_or("naive mode needs baseline")? {
                let vertices = buffer(
                    &device,
                    "indexed vertices",
                    bytemuck::cast_slice(&b.vertices),
                    wgpu::BufferUsages::VERTEX,
                );
                let indices = buffer(
                    &device,
                    "cache optimized indices",
                    bytemuck::cast_slice(&b.indices),
                    wgpu::BufferUsages::INDEX,
                );
                static_bytes += vertices.size() + indices.size();
                chunks.push(ChunkGpu {
                    vertices,
                    indices,
                    count: b.indices.len() as u32,
                });
            }
        }
        let shadow_tex = texture(
            &device,
            SHADOW_SIZE,
            SHADOW_SIZE,
            1,
            wgpu::TextureFormat::Depth32Float,
            wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
        );
        let shadow = shadow_tex.create_view(&Default::default());
        let shadow_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("shadow sampling"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
            ],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            compare: Some(wgpu::CompareFunction::LessEqual),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let shadow_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &shadow_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&shadow),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[
                Some(&global_layout),
                Some(&page_layout),
                Some(&shadow_layout),
            ],
            immediate_size: 0,
        });
        let depth_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&global_layout), Some(&page_layout)],
            immediate_size: 0,
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("validated WGSL"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/scene.wgsl").into()),
        });
        let pipeline = |vertex: &str, fragment: Option<&str>, indexed: bool, overdraw: bool| {
            let attributes = wgpu::vertex_attr_array![0=>Float32x3,1=>Uint32,2=>Uint32];
            let vertex_layout = Some(wgpu::VertexBufferLayout {
                array_stride: 20,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &attributes,
            });
            let vertices = [vertex_layout];
            let targets = [Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                blend: if overdraw {
                    Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent::REPLACE,
                    })
                } else {
                    None
                },
                write_mask: wgpu::ColorWrites::ALL,
            })];
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(vertex),
                layout: Some(if fragment.is_some() {
                    &layout
                } else {
                    &depth_layout
                }),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some(vertex),
                    compilation_options: Default::default(),
                    buffers: if indexed { &vertices } else { &[] },
                },
                fragment: fragment.map(|entry_point| wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(entry_point),
                    compilation_options: Default::default(),
                    targets: &targets,
                }),
                primitive: wgpu::PrimitiveState {
                    cull_mode: if overdraw {
                        None
                    } else {
                        Some(wgpu::Face::Back)
                    },
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: Some(!overdraw),
                    depth_compare: Some(if overdraw {
                        wgpu::CompareFunction::Always
                    } else if fragment.is_some() {
                        wgpu::CompareFunction::Greater
                    } else {
                        wgpu::CompareFunction::Less
                    }),
                    stencil: Default::default(),
                    bias: if fragment.is_none() {
                        wgpu::DepthBiasState {
                            constant: 2,
                            slope_scale: 2.0,
                            clamp: 0.0,
                        }
                    } else {
                        Default::default()
                    },
                }),
                multisample: wgpu::MultisampleState {
                    count: if fragment.is_some() { 4 } else { 1 },
                    ..Default::default()
                },
                multiview_mask: None,
                cache: None,
            })
        };
        let main = pipeline(
            if mode == Mode::Cluster {
                "pull"
            } else {
                "indexed"
            },
            Some("shade"),
            mode == Mode::Naive,
            false,
        );
        let overdraw = pipeline(
            if mode == Mode::Cluster {
                "pull"
            } else {
                "indexed"
            },
            Some("overdraw"),
            mode == Mode::Naive,
            true,
        );
        let ground = pipeline("ground", Some("shade"), false, false);
        let shadow_pipeline = pipeline(
            if mode == Mode::Cluster {
                "shadow_pull"
            } else {
                "shadow_indexed"
            },
            None,
            mode == Mode::Naive,
            false,
        );
        let color = texture(
            &device,
            width,
            height,
            1,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        );
        let msaa = texture(
            &device,
            width,
            height,
            4,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
        )
        .create_view(&Default::default());
        let depth = texture(
            &device,
            width,
            height,
            4,
            wgpu::TextureFormat::Depth32Float,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
        )
        .create_view(&Default::default());
        static_bytes +=
            width as u64 * height as u64 * 36 + SHADOW_SIZE as u64 * SHADOW_SIZE as u64 * 4;
        let query = device
            .features()
            .contains(wgpu::Features::TIMESTAMP_QUERY)
            .then(|| {
                device.create_query_set(&wgpu::QuerySetDescriptor {
                    label: None,
                    ty: wgpu::QueryType::Timestamp,
                    count: 8,
                })
            });
        let query_resolve = query.as_ref().map(|_| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: 64,
                usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            })
        });
        if query.is_some() {
            static_bytes += 64;
        }
        let lists = pages
            .iter()
            .map(|p| {
                Self::make_list(
                    &device,
                    &page_layout,
                    p,
                    &clusters,
                    &instances,
                    &dummy_draws,
                    8,
                )
            })
            .collect();
        let shadow_lists = pages
            .iter()
            .map(|p| {
                Self::make_list(
                    &device,
                    &page_layout,
                    p,
                    &clusters,
                    &instances,
                    &dummy_draws,
                    8,
                )
            })
            .collect();
        Ok(Self {
            device,
            queue,
            width,
            height,
            mode,
            culling: true,
            shadows: true,
            instance_sphere: crate::select::CandidateIndex::new(reader).sphere,
            pages,
            compute: None,
            dummy_draws,
            lists,
            shadow_lists,
            clusters,
            instances,
            chunks,
            globals,
            global_bind,
            page_layout,
            shadow_bind,
            color,
            msaa,
            depth,
            shadow,
            shadow_texture: shadow_tex,
            main,
            overdraw,
            ground,
            shadow_pipeline,
            query,
            query_resolve,
            static_bytes,
            instance_count: scene.instances.len() as u32,
            max_triangles: reader.header.config.max_triangles,
            max_depth: reader.groups.iter().map(|g| g.depth).max().unwrap_or(0),
        })
    }
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn make_list(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        page: &PageGpu,
        clusters: &wgpu::Buffer,
        instances: &wgpu::Buffer,
        draws: &wgpu::Buffer,
        capacity: u64,
    ) -> ListGpu {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("visible cluster instance pairs"),
            size: capacity,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self::bind_list(device, layout, page, clusters, instances, buffer, draws)
    }
    pub(crate) fn bind_list(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        page: &PageGpu,
        clusters: &wgpu::Buffer,
        instances: &wgpu::Buffer,
        buffer: wgpu::Buffer,
        draws: &wgpu::Buffer,
    ) -> ListGpu {
        let capacity = buffer.size();
        let bindings = [
            &page.geometry,
            clusters,
            &buffer,
            instances,
            &page.params,
            draws,
        ];
        let entries: Vec<_> = bindings
            .iter()
            .enumerate()
            .map(|(binding, b)| wgpu::BindGroupEntry {
                binding: binding as u32,
                resource: b.as_entire_binding(),
            })
            .collect();
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout,
            entries: &entries,
        });
        ListGpu {
            buffer,
            bind,
            capacity,
        }
    }
}
