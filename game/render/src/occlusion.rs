//! The camera's occlusion culling. What the camera drew last frame (the cull's
//! camera lists, still in their buffers) is drawn again, depth only and at this
//! frame's poses, into a depth texture at a quarter of the frame's size; a
//! pyramid of the farthest depth under each texel is built from it; and this
//! frame's cull drops from the camera's lists what lies behind the pyramid
//! everywhere it can cover (cull.wgsl `occluded`). The occluders are this
//! frame's geometry, so something newly in view is never hidden by where things
//! were; and only closed convex meshes occlude, each drawn shrunk and pushed
//! back by a texel (occluder.wgsl), so a crack between two narrower than a
//! texel hides nothing it shows.
//!
//! On a phone's tiling GPU a hidden cube costs its vertices and triangles even
//! where depth rejects its pixels: at 256,000 cubes on a Pixel 5, two thirds of
//! the forward pass went to cubes nothing showed.
use crate::Vertex;
use exact_gpu::wgpu;
use glam::Vec3;

/// Slots in the cull's lists at or above which a frame occludes by default:
/// below it, the depth pass and pyramid cost more than the draws they save.
pub(crate) const ITEMS: usize = 32_768;

/// Bytes between groups' occluder uniforms: the dynamic offset alignment.
const STRIDE: u64 = 256;

pub(crate) struct Occlusion {
    reduce: wgpu::ComputePipeline,
    occluder: wgpu::RenderPipeline,
    occluder_layout: wgpu::BindGroupLayout,
    /// Each occluding group's uniform, `STRIDE` apart: the camera, the depth
    /// texture's texel and its mesh's radii (occluder.wgsl `Occluder`).
    uniforms: wgpu::Buffer,
    pub uniforms_bind: wgpu::BindGroup,
    /// This frame's occluding groups and their uniforms' offsets.
    pub groups: Vec<(usize, u32)>,
    reduce_layout: wgpu::BindGroupLayout,
    targets: Option<Targets>,
}

struct Targets {
    size: (u32, u32),
    pub depth: wgpu::TextureView,
    pub pyramid: wgpu::TextureView,
    /// One bind group and size per level: its source and its storage target.
    levels: Vec<(wgpu::BindGroup, (u32, u32))>,
}

impl Occlusion {
    pub fn new(device: &wgpu::Device, scene_layout: &wgpu::BindGroupLayout) -> Self {
        let reduce_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("game occlusion reduce"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::R32Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("game occlusion"),
            source: wgpu::ShaderSource::Wgsl(crate::pipeline::OCCLUSION.into()),
        });
        let reduce = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("game occlusion reduce"),
            layout: Some(
                &device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("game occlusion reduce"),
                    bind_group_layouts: &[Some(&reduce_layout)],
                    immediate_size: 0,
                }),
            ),
            module: &module,
            entry_point: Some("reduce"),
            compilation_options: Default::default(),
            cache: None,
        });
        let occluder_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("game occluder"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: wgpu::BufferSize::new(112),
                },
                count: None,
            }],
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("game occluder"),
            source: wgpu::ShaderSource::Wgsl(crate::pipeline::occluder_source().into()),
        });
        let occluder = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("game occluder"),
            layout: Some(
                &device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("game occluder"),
                    bind_group_layouts: &[Some(scene_layout), Some(&occluder_layout)],
                    immediate_size: 0,
                }),
            ),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_occluder"),
                buffers: &crate::pipeline::vertex_layout(),
                compilation_options: Default::default(),
            },
            fragment: None,
            // Both windings: a mirrored group's front faces wind the other way.
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        let (uniforms, uniforms_bind) = uniforms(device, &occluder_layout, 1);
        Self {
            reduce,
            reduce_layout,
            occluder,
            occluder_layout,
            uniforms,
            uniforms_bind,
            groups: Vec::new(),
            targets: None,
        }
    }

    /// The pyramid this frame's cull reads, once prepared.
    pub fn pyramid(&self) -> Option<&wgpu::TextureView> {
        self.targets.as_ref().map(|t| &t.pyramid)
    }

    /// Ready the targets for a frame of `size` pixels, and each occluding group's
    /// uniform: `groups` are its index, its mesh's inscribed radius, its largest
    /// |position| per axis and whether it is a box.
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        size: (u32, u32),
        view_proj: glam::Mat4,
        eye: Vec3,
        groups: impl Iterator<Item = (usize, f32, Vec3, bool)>,
    ) {
        let depth = (size.0.div_ceil(4).max(1), size.1.div_ceil(4).max(1));
        if self.targets.as_ref().is_none_or(|t| t.size != depth) {
            self.targets = Some(Targets::new(device, &self.reduce_layout, depth));
        }
        let rows = view_proj.transpose();
        let texel = [
            1.0 / (rows.x_axis.truncate().length() * depth.0 as f32),
            1.0 / (rows.y_axis.truncate().length() * depth.1 as f32),
        ];
        let mut words = Vec::new();
        self.groups.clear();
        for (index, inner, reach, boxed) in groups {
            let at = words.len() as u32 * 4;
            words.extend(view_proj.to_cols_array());
            let boxed = if boxed { 1.0 } else { 0.0 };
            words.extend([eye.x, eye.y, eye.z, 0.0, texel[0], texel[1], inner, boxed]);
            words.extend([reach.x, reach.y, reach.z, 0.0]);
            words.resize(words.len().next_multiple_of(STRIDE as usize / 4), 0.0);
            self.groups.push((index, at));
        }
        let bytes = (words.len() as u64 * 4).max(STRIDE);
        if self.uniforms.size() < bytes {
            (self.uniforms, self.uniforms_bind) = uniforms(
                device,
                &self.occluder_layout,
                bytes.div_ceil(STRIDE).next_power_of_two(),
            );
        }
        queue.write_buffer(&self.uniforms, 0, crate::buffers::bytes(&words));
    }

    pub fn occluder(&self) -> &wgpu::RenderPipeline {
        &self.occluder
    }

    /// The depth pass's attachment: cleared to the far plane.
    pub fn depth(&self) -> &wgpu::TextureView {
        &self.targets.as_ref().expect("prepared").depth
    }

    /// Build the pyramid from the depth pass.
    pub fn reduce(&self, encoder: &mut wgpu::CommandEncoder) {
        let targets = self.targets.as_ref().expect("prepared");
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("game occlusion pyramid"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.reduce);
        for (bind, size) in &targets.levels {
            pass.set_bind_group(0, bind, &[]);
            pass.dispatch_workgroups(size.0.div_ceil(8), size.1.div_ceil(8), 1);
        }
    }
}

impl Targets {
    fn new(device: &wgpu::Device, layout: &wgpu::BindGroupLayout, size: (u32, u32)) -> Self {
        let depth = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("game occlusion depth"),
                size: wgpu::Extent3d {
                    width: size.0,
                    height: size.1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Depth32Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&Default::default());
        // Level 0 is half the depth's size; each level halves the last, rounding
        // down as mip levels do, to one texel.
        let half = |(w, h): (u32, u32)| ((w / 2).max(1), (h / 2).max(1));
        let mut sizes = vec![half(size)];
        while let Some(&(w, h)) = sizes.last().filter(|&&(w, h)| w > 1 || h > 1) {
            sizes.push(half((w, h)));
        }
        let texture = pyramid_texture(device, sizes[0], sizes.len() as u32);
        let level = |k: u32| {
            texture.create_view(&wgpu::TextureViewDescriptor {
                base_mip_level: k,
                mip_level_count: Some(1),
                ..Default::default()
            })
        };
        let levels = (0..sizes.len() as u32)
            .map(|k| {
                let source = if k == 0 { depth.clone() } else { level(k - 1) };
                let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("game occlusion level"),
                    layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(&source),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(&level(k)),
                        },
                    ],
                });
                (bind, sizes[k as usize])
            })
            .collect();
        Self {
            size,
            depth,
            pyramid: texture.create_view(&Default::default()),
            levels,
        }
    }
}

fn uniforms(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    groups: u64,
) -> (wgpu::Buffer, wgpu::BindGroup) {
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("game occluders"),
        size: groups * STRIDE,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("game occluders"),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer: &buffer,
                offset: 0,
                size: wgpu::BufferSize::new(112),
            }),
        }],
    });
    (buffer, bind)
}

/// Whether a mesh's every face is square to an axis: a box, which may shrink by
/// the same distance along each axis (occluder.wgsl).
pub(crate) fn boxed(vertices: &[Vertex], indices: &[u32]) -> bool {
    let at = |i: u32| Vec3::from_array(vertices[i as usize].position);
    indices.chunks_exact(3).all(|t| {
        let n = (at(t[1]) - at(t[0]))
            .cross(at(t[2]) - at(t[0]))
            .normalize_or_zero()
            .abs();
        n.max_element() > 1.0 - 1e-5
    })
}

/// The radius of the largest ball about the origin inside a closed convex mesh:
/// the least distance from it to a face's plane. Zero where the mesh is not
/// closed (an edge, by position, not shared by exactly two triangles), not
/// convex about the origin (a vertex outside a face's plane), or too large to
/// check here: such a mesh occludes nothing.
pub(crate) fn inscribed(vertices: &[Vertex], indices: &[u32]) -> f32 {
    let triangles = indices.len() / 3;
    if triangles < 4 || triangles * vertices.len() > 1 << 22 {
        return 0.0;
    }
    let at = |i: u32| Vec3::from_array(vertices[i as usize].position);
    let key = |p: Vec3| p.to_array().map(f32::to_bits);
    let mut edges = std::collections::HashMap::new();
    let mut radius = f32::INFINITY;
    for t in indices.chunks_exact(3) {
        let [a, b, c] = [at(t[0]), at(t[1]), at(t[2])];
        let n = (b - a).cross(c - a);
        if n.length_squared() <= f32::EPSILON * f32::EPSILON {
            return 0.0;
        }
        let n = n.normalize();
        let d = n.dot(a);
        let slack = 1e-5 * (1.0 + d.abs());
        if vertices
            .iter()
            .any(|v| n.dot(Vec3::from_array(v.position)) > d + slack)
        {
            return 0.0;
        }
        radius = radius.min(d);
        for (p, q) in [(a, b), (b, c), (c, a)] {
            let (p, q) = (key(p), key(q));
            *edges
                .entry(if p < q { (p, q) } else { (q, p) })
                .or_insert(0u32) += 1;
        }
    }
    if edges.values().any(|&n| n != 2) {
        return 0.0;
    }
    radius.max(0.0)
}

fn pyramid_texture(device: &wgpu::Device, size: (u32, u32), levels: u32) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("game occlusion pyramid"),
        size: wgpu::Extent3d {
            width: size.0,
            height: size.1,
            depth_or_array_layers: 1,
        },
        mip_level_count: levels,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::R32Float,
        usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    })
}
