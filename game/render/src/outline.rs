//! Inverted hulls share forward visibility and skin palettes, never caster draws.
use crate::{buffers::bytes, pipeline};
use exact_gpu::wgpu;

pub(crate) struct Hull {
    pub bind: wgpu::BindGroup,
    uniform: wgpu::Buffer,
    vertices: wgpu::Buffer,
    pub bytes: u64,
    pub width: f32,
    pub min_pixels: f32,
    pub reach: f32,
}
impl Hull {
    pub fn new(
        d: &wgpu::Device,
        q: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
        o: &exact_game::asset::MeshOutline,
        base: u32,
        reach_vectors: Option<&[glam::Vec3]>,
    ) -> Self {
        let uniform = d.create_buffer(&wgpu::BufferDescriptor {
            label: Some("game outline"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        q.write_buffer(
            &uniform,
            0,
            bytes(&[
                o.width,
                o.min_pixels,
                f32::from_bits(base),
                0.,
                o.color[0],
                o.color[1],
                o.color[2],
                1.,
            ]),
        );
        let values: Vec<f32> = o
            .normals
            .chunks_exact(3)
            .zip(&o.reach)
            .enumerate()
            .flat_map(|(i, (n, r))| {
                let reach = reach_vectors.map_or_else(|| glam::Vec3::from_slice(n) * *r, |v| v[i]);
                [n[0], n[1], n[2], reach.x, reach.y, reach.z]
            })
            .collect();
        let reach = values
            .chunks_exact(6)
            .map(|v| glam::Vec3::from_slice(&v[3..]).length())
            .fold(0., f32::max);
        let data = d.create_buffer(&wgpu::BufferDescriptor {
            label: Some("game outline normals and reach"),
            size: values.len() as u64 * 4,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        q.write_buffer(&data, 0, bytes(&values));
        let bind = d.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("game outline"),
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });
        Self {
            bind,
            bytes: data.size() + uniform.size(),
            uniform,
            vertices: data,
            min_pixels: o.min_pixels,
            width: o.width,
            reach,
        }
    }
    pub fn rebase(&self, q: &wgpu::Queue, base: u32) {
        q.write_buffer(&self.uniform, 8, bytes(&[base]));
    }
}
pub(crate) struct Pipelines {
    pub layout: wgpu::BindGroupLayout,
    /// By winding, then fading (with the dither's discard), then direct (tonemapped
    /// into the target's format). The opaque HDR pair is made with the first
    /// outlined model, the rest on first use.
    forward: [std::sync::OnceLock<wgpu::RenderPipeline>; 8],
    shader: wgpu::ShaderModule,
    pipeline_layout: wgpu::PipelineLayout,
}
impl Pipelines {
    pub fn new(d: &wgpu::Device, p: &pipeline::Pipelines) -> Self {
        let layout = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("game outline"),
            entries: &[0].map(|binding| wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }),
        });
        let shader = d.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("game outline"),
            source: wgpu::ShaderSource::Wgsl(pipeline::outline_source().into()),
        });
        let pipeline_layout = d.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("game outline"),
            bind_group_layouts: &[
                Some(&p.scene_layout),
                Some(&p.shadow_layout),
                Some(&layout),
                Some(&p.models.as_ref().unwrap().instance),
            ],
            immediate_size: 0,
        });
        let pipelines = Self {
            layout,
            forward: Default::default(),
            shader,
            pipeline_layout,
        };
        for mirrored in [false, true] {
            pipelines.pipeline(d, p, mirrored, false, false);
        }
        pipelines
    }
    fn pipeline(
        &self,
        d: &wgpu::Device,
        p: &pipeline::Pipelines,
        mirrored: bool,
        fading: bool,
        direct: bool,
    ) -> &wgpu::RenderPipeline {
        let index = usize::from(mirrored) + 2 * usize::from(fading) + 4 * usize::from(direct);
        self.forward[index].get_or_init(|| {
            hull_pipeline(
                d,
                p,
                &self.shader,
                &self.pipeline_layout,
                (mirrored, fading, direct),
            )
        })
    }
    /// Pipelines made, for the residency counters.
    pub fn count(&self) -> usize {
        self.forward.iter().filter(|p| p.get().is_some()).count()
    }
}
/// Front faces culled; `fading` keeps the dither's discard; `direct` tonemaps
/// into the renderer's output format.
fn hull_pipeline(
    d: &wgpu::Device,
    p: &pipeline::Pipelines,
    shader: &wgpu::ShaderModule,
    layout: &wgpu::PipelineLayout,
    (mirrored, fading, direct): (bool, bool, bool),
) -> wgpu::RenderPipeline {
    let format = p.format();
    let constants = [
        ("DIRECT", if direct { 1. } else { 0. }),
        ("ENCODE_SRGB", if format.is_srgb() { 0. } else { 1. }),
        ("FADES", if fading { 1. } else { 0. }),
    ];
    d.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("game outline"),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("outline_vs"),
            compilation_options: Default::default(),
            buffers: &[
                pipeline::vertex_layout()[0].clone(),
                Some(wgpu::VertexBufferLayout {
                    array_stride: 24,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![4 => Float32x3, 5 => Float32x3],
                }),
            ],
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("outline_fs"),
            compilation_options: wgpu::PipelineCompilationOptions {
                constants: &constants,
                ..Default::default()
            },
            targets: &[Some(wgpu::ColorTargetState {
                format: if direct {
                    format
                } else {
                    wgpu::TextureFormat::Rgba16Float
                },
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            cull_mode: Some(wgpu::Face::Front),
            front_face: if mirrored {
                wgpu::FrontFace::Cw
            } else {
                wgpu::FrontFace::Ccw
            },
            ..Default::default()
        },
        depth_stencil: pipeline::scene_depth(true, wgpu::CompareFunction::Less, Default::default()),
        multisample: wgpu::MultisampleState {
            count: p.samples,
            ..Default::default()
        },
        multiview_mask: None,
        cache: p.cache.handle.as_ref(),
    })
}
impl<const ASSETS: bool> crate::renderer::RendererWithAssets<ASSETS> {
    pub(crate) fn add_outline(
        &mut self,
        id: crate::MeshId,
        outline: &exact_game::asset::MeshOutline,
        reach_vectors: Option<&[glam::Vec3]>,
    ) {
        let pipelines = self
            .outlines
            .get_or_insert_with(|| Pipelines::new(&self.device, &self.pipelines));
        self.meshes[id.0].outline = Some(Hull::new(
            &self.device,
            &self.queue,
            &pipelines.layout,
            outline,
            self.meshes[id.0].base_vertex as u32,
            reach_vectors,
        ));
        // The cull's setup marks outlined groups, whose hull draws it writes.
        self.cull.epoch += 1;
    }
    pub(crate) fn draw_outlines(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        size: (u32, u32),
        direct: bool,
    ) -> crate::Stats {
        let Some(pipelines) = &self.outlines else {
            return crate::Stats::default();
        };
        let mut stats = crate::Stats::default();
        pass.set_index_buffer(self.indices.raw.slice(..), wgpu::IndexFormat::Uint32);
        let mut bound = crate::renderer::Bound::default();
        // The pass arrives with the world's viewport.
        let mut layer = false;
        for (i, group) in self.cull.groups.iter().enumerate() {
            if group.flags & 1 == 0 {
                continue;
            }
            let batch = &self.batches[group.batch];
            let Some(hull) = &self.meshes[batch.mesh.0].outline else {
                continue;
            };
            if batch.viewmodel != layer {
                self.scene_viewport(pass, size, batch.viewmodel);
                layer = batch.viewmodel;
            }
            bound.pipeline(
                pass,
                pipelines.pipeline(
                    &self.device,
                    &self.pipelines,
                    group.mirrored,
                    self.batch_fading(group.batch),
                    direct,
                ),
            );
            bound.group(pass, 1, &self.shadow_sample);
            bound.group(pass, 2, &hull.bind);
            bound.group(pass, 3, self.models.bind.as_ref().unwrap());
            let mesh = &self.meshes[batch.mesh.0];
            pass.set_vertex_buffer(
                0,
                self.vertices
                    .raw
                    .slice(mesh.base_vertex as u64 * size_of::<crate::Vertex>() as u64..),
            );
            pass.set_vertex_buffer(1, hull.vertices.slice(..));
            self.draw_outline_group(pass, &mut bound, i);
            stats.draws += 1;
            let instances = u64::from(group.range.end - group.range.start);
            stats.instances += instances;
            stats.triangles += instances * u64::from(mesh.indices.end - mesh.indices.start) / 3;
        }
        pass.set_vertex_buffer(0, self.vertices.raw.slice(..));
        self.scene_viewport(pass, size, false);
        stats
    }
}
