//! Core WebGPU selection, stable compaction and indirect draws. No readback in encoding.
use crate::{
    Renderer,
    gpu::{Result, buffer},
    scene::Camera,
    select::CandidateIndex,
};
use bytemuck::{Pod, Zeroable};
use clod_format::Reader;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Selector {
    Cpu,
    #[default]
    Gpu,
    Brute,
}
impl Selector {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "cpu" => Ok(Self::Cpu),
            "gpu" => Ok(Self::Gpu),
            "brute" => Ok(Self::Brute),
            _ => Err("select must be cpu|gpu|brute".into()),
        }
    }
}
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Config {
    planes: [[f32; 4]; 6],
    eye: [f32; 4],
    direction: [f32; 4],
    sphere: [f32; 4],
    projection: [f32; 4],
    sizes: [u32; 4],
    options: [u32; 4],
    threshold: [f32; 4],
}
pub(crate) struct ComputePass {
    uniform: wgpu::Buffer,
    pub visible: wgpu::Buffer,
    pub draws: wgpu::Buffer,
    counts: wgpu::Buffer,
    bind: wgpu::BindGroup,
}
pub(crate) struct Compute {
    pub passes: [ComputePass; 2],
    pipelines: [wgpu::ComputePipeline; 4],
    pub quota: u32,
    cluster_count: u32,
    instance_count: u32,
    page_count: u32,
    sphere: [f32; 4],
    pub bytes: u64,
}
impl Compute {
    pub fn new(renderer: &Renderer, reader: &Reader<'_>, capacity: Option<u32>) -> Result<Self> {
        let device = &renderer.device;
        let instances = renderer.instance_count;
        if instances as u64 * reader.clusters.len() as u64 > u32::MAX as u64 {
            return Err("scene exceeds the u32 candidate counter limit".into());
        }
        let quota = capacity
            .unwrap_or(4_194_304 / instances)
            .min(reader.clusters.len() as u32)
            .max(1);
        let slots = instances as u64 * quota as u64;
        if slots * 8 > 128 * 1024 * 1024 {
            return Err("visible capacity exceeds 128 MiB core binding".into());
        }
        let index = CandidateIndex::new(reader);
        let envelope = buffer(
            device,
            "error envelopes",
            bytemuck::cast_slice(&index.envelopes),
            wgpu::BufferUsages::STORAGE,
        );
        let make = |label, size, usage| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage,
                mapped_at_creation: false,
            })
        };
        let scratch = make(
            "instance cut scratch",
            slots * 4,
            wgpu::BufferUsages::STORAGE,
        );
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("validated selection WGSL"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/select.wgsl").into()),
        });
        let entries: Vec<_> = (0..8)
            .map(|binding| wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: if binding == 0 {
                        wgpu::BufferBindingType::Uniform
                    } else {
                        wgpu::BufferBindingType::Storage {
                            read_only: binding < 4,
                        }
                    },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            })
            .collect();
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("seven storage buffers"),
            entries: &entries,
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipelines = ["choose", "page_prefix", "finish", "scatter"].map(|entry| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(entry),
                layout: Some(&pipeline_layout),
                module: &shader,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                cache: None,
            })
        });
        let mut bytes = envelope.size() + scratch.size();
        let passes = std::array::from_fn(|_| {
            let uniform = make(
                "selection config",
                std::mem::size_of::<Config>() as u64,
                wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            );
            let visible = make(
                "fixed visible pairs",
                slots * 8,
                wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            );
            let draws = make(
                "page indirect args and counters",
                (reader.pages.len() as u64 * 8 + 4) * 4,
                wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::INDIRECT
                    | wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::COPY_SRC,
            );
            let counts = make(
                "instance page counts",
                (instances as u64 * reader.pages.len() as u64 * 3 + instances as u64 * 4) * 4,
                wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            );
            bytes += uniform.size() + visible.size() + draws.size() + counts.size();
            let buffers = [
                &uniform,
                &renderer.clusters,
                &renderer.instances,
                &envelope,
                &scratch,
                &counts,
                &draws,
                &visible,
            ];
            let entries: Vec<_> = buffers
                .iter()
                .enumerate()
                .map(|(binding, b)| wgpu::BindGroupEntry {
                    binding: binding as u32,
                    resource: b.as_entire_binding(),
                })
                .collect();
            let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("selection"),
                layout: &layout,
                entries: &entries,
            });
            ComputePass {
                uniform,
                visible,
                draws,
                counts,
                bind,
            }
        });
        Ok(Self {
            passes,
            pipelines,
            quota,
            cluster_count: reader.clusters.len() as u32,
            instance_count: instances,
            page_count: reader.pages.len() as u32,
            sphere: index.sphere,
            bytes,
        })
    }
    #[allow(clippy::too_many_arguments)]
    pub fn encode(
        &self,
        renderer: &Renderer,
        encoder: &mut wgpu::CommandEncoder,
        camera: &Camera,
        height: u32,
        threshold: f32,
        cull: bool,
        brute: bool,
        pass_id: usize,
    ) {
        let pass = &self.passes[pass_id];
        let config = Config {
            planes: camera.planes().map(|p| p.to_array()),
            eye: camera.eye.extend(1.0).to_array(),
            direction: camera
                .matrix
                .transpose()
                .z_axis
                .truncate()
                .normalize()
                .extend(0.0)
                .to_array(),
            sphere: self.sphere,
            projection: [
                height as f32,
                camera.cot,
                camera.near,
                camera.orthographic_span.unwrap_or(0.0),
            ],
            sizes: [
                self.cluster_count,
                self.instance_count,
                self.page_count,
                self.quota,
            ],
            options: [cull as u32, brute as u32, renderer.max_triangles, 0],
            threshold: [threshold, 0.0, 0.0, 0.0],
        };
        renderer
            .queue
            .write_buffer(&pass.uniform, 0, bytemuck::bytes_of(&config));
        encoder.clear_buffer(&pass.counts, 0, None);
        // Separate passes are also storage ordering barriers; only endpoint passes write timestamps.
        for (id, groups) in [self.instance_count, self.page_count, 1, self.instance_count]
            .into_iter()
            .enumerate()
        {
            let timestamps =
                renderer
                    .query
                    .as_ref()
                    .filter(|_| id == 0 || id == 3)
                    .map(|query_set| wgpu::ComputePassTimestampWrites {
                        query_set,
                        beginning_of_pass_write_index: (id == 0).then_some(pass_id as u32 * 2),
                        end_of_pass_write_index: (id == 3).then_some(1 + pass_id as u32 * 2),
                    });
            let mut compute = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("select/compact"),
                timestamp_writes: timestamps,
            });
            compute.set_pipeline(&self.pipelines[id]);
            compute.set_bind_group(0, &pass.bind, &[]);
            compute.dispatch_workgroups(groups, 1, 1);
        }
    }
}
impl Renderer {
    /// Allocate at scene load. Normal frame encoding never reallocates or maps selection buffers.
    pub fn enable_gpu_selection(
        &mut self,
        reader: &Reader<'_>,
        capacity: Option<u32>,
    ) -> Result<()> {
        if self.mode != crate::Mode::Cluster {
            return Ok(());
        }
        let compute = Compute::new(self, reader, capacity)?;
        for (pass, lists) in compute
            .passes
            .iter()
            .zip([&mut self.lists, &mut self.shadow_lists])
        {
            for (page, list) in self.pages.iter().zip(lists) {
                *list = Self::bind_list(
                    &self.device,
                    &self.page_layout,
                    page,
                    &self.clusters,
                    &self.instances,
                    pass.visible.clone(),
                    &pass.draws,
                );
            }
        }
        self.compute = Some(compute);
        Ok(())
    }
    pub fn gpu_capacity_per_instance(&self) -> Option<u32> {
        self.compute.as_ref().map(|c| c.quota)
    }
    /// Test/CLI-only copy, invoked after rendering. No frame depends on its contents.
    pub fn selection_readback(&self, lists: bool) -> Result<[wgpu::Buffer; 2]> {
        let compute = self.compute.as_ref().ok_or("GPU selection not enabled")?;
        let mut encoder = self.device.create_command_encoder(&Default::default());
        let result = compute.passes.each_ref().map(|p| {
            let extra = if lists { p.visible.size() } else { 0 };
            let read = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("selection oracle readback"),
                size: p.draws.size() + extra,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            encoder.copy_buffer_to_buffer(&p.draws, 0, &read, 0, p.draws.size());
            if lists {
                encoder.copy_buffer_to_buffer(&p.visible, 0, &read, p.draws.size(), extra);
            }
            read
        });
        self.queue.submit([encoder.finish()]);
        Ok(result)
    }
}

impl Renderer {
    /// Test-only shadow attachment copy, separate from frame encoding.
    pub fn shadow_readback(&self) -> wgpu::Buffer {
        let read = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("shadow oracle"),
            size: 2048 * 2048 * 4,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.shadow_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::DepthOnly,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &read,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(2048 * 4),
                    rows_per_image: None,
                },
            },
            wgpu::Extent3d {
                width: 2048,
                height: 2048,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([encoder.finish()]);
        read
    }
}
