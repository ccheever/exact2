//! A look frame's quads (LLP 1127.000 D1, D4): what `Quads::frame` did with each
//! sprite and emitter, each quad draw's batch, and the particle counting pass.
//!
//! The counting pass draws every particle draw again, after the scene, with a
//! fragment shader that writes no colour, reads the frame's depth (sample 0)
//! and adds one per fragment in front of it to its draw's counter in a storage
//! buffer (`shaders/count.wgsl`). It is the same on every host that has
//! fragment storage writes (the web has no occlusion queries, see
//! `renderer/probe.rs`; GL has no such writes, and its counts are unmeasured),
//! and it runs only in a look frame: a live frame encodes no such pass and
//! makes no such pipeline. A count is of the quad's fragments, the square
//! around a dot included, once per pixel whatever the sample count.
use super::*;

/// What a look frame's quads did with one sprite or emitter.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum QuadFact {
    /// Outside the camera's view, culled on the CPU.
    Culled,
    /// Its texture is not resident: the sprite was not drawn.
    Pending(String),
    /// Drawn: the quads it drew this frame (a particle count, or 1).
    Drawn(u32),
}

pub(super) struct Coverage {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
}

/// Whether `d` can count: fragment shaders that write storage, which GL lacks.
pub(crate) fn counts(d: &wgpu::Device) -> bool {
    d.adapter_info().backend != wgpu::Backend::Gl
}

/// One counter word per draw, each draw's index at a dynamic uniform offset.
const STRIDE: u64 = 256;

impl Quads {
    /// Draw `index`'s batch: what it shares with every quad drawn with it.
    pub(crate) fn batch_label(&self, index: usize) -> String {
        let Some(draw) = self.draws.get(index) else {
            return "?".into();
        };
        match draw.kind {
            Kind::Particle(additive, look, soft) => {
                let what = match self.looks.get(look as usize).filter(|_| look != NO_LOOK) {
                    Some(l) => format!("particles {}", l.value.texture),
                    None => "particle dots".into(),
                };
                let blend = if additive { "additive" } else { "alpha" };
                format!("{what} · {blend}{}", if soft { " · soft" } else { "" })
            }
            Kind::Sprite(i) => format!(
                "sprite {}",
                self.sprites
                    .get(i)
                    .map_or("?", |s| s.value.texture.as_str())
            ),
            Kind::Model(batch, _) => format!("blended model · batch {batch}"),
            #[cfg(not(target_arch = "wasm32"))]
            Kind::Child(i) => format!("placed child {i}"),
        }
    }

    /// Count each particle draw's fragments in front of `depth` (the frame's,
    /// read after the scene's passes, and bound for sampling): a buffer of one
    /// count per draw and the indices of the draws counted, or `None` with no
    /// particle draw or where the device cannot count.
    pub(crate) fn count_particles(
        &mut self,
        d: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        depth: &wgpu::TextureView,
        size: (u32, u32),
        near: f32,
    ) -> Option<(wgpu::Buffer, Vec<usize>)> {
        let draws: Vec<usize> = (self.draws.iter().enumerate())
            .filter(|(_, draw)| matches!(draw.kind, Kind::Particle(..)))
            .map(|(i, _)| i)
            .collect();
        let arena = (self.particle_arena.as_ref()).filter(|_| !draws.is_empty() && counts(d))?;
        if self.coverage.is_none() {
            self.coverage = Some(coverage(d, &self.layout, self.samples));
        }
        let coverage = self.coverage.as_ref().unwrap();
        let counts = d.create_buffer(&wgpu::BufferDescriptor {
            label: Some("game look particle counts"),
            size: draws.len() as u64 * 4,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let which = d.create_buffer(&wgpu::BufferDescriptor {
            label: Some("game look particle draw"),
            size: draws.len() as u64 * STRIDE,
            usage: wgpu::BufferUsages::UNIFORM,
            mapped_at_creation: true,
        });
        {
            let mut bytes = vec![0u8; draws.len() * STRIDE as usize];
            for k in 0..draws.len() {
                let at = k * STRIDE as usize;
                bytes[at..at + 4].copy_from_slice(&(k as u32).to_le_bytes());
            }
            which
                .slice(..)
                .get_mapped_range_mut()
                .ok()?
                .copy_from_slice(&bytes);
        }
        which.unmap();
        let bind = d.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("game look particle counts"),
            layout: &coverage.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: counts.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &which,
                        offset: 0,
                        size: wgpu::BufferSize::new(16),
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(depth),
                },
            ],
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("game look particle coverage"),
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth,
                depth_ops: None,
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_viewport(0., 0., size.0 as f32, size.1 as f32, near, 1.);
        pass.set_scissor_rect(0, 0, size.0, size.1);
        pass.set_pipeline(&coverage.pipeline);
        pass.set_bind_group(0, &self.bind, &[]);
        pass.set_vertex_buffer(0, arena.buffer.raw.slice(..));
        for (k, &i) in draws.iter().enumerate() {
            pass.set_bind_group(1, &bind, &[(k as u64 * STRIDE) as u32]);
            pass.draw(0..6, self.draws[i].range.clone());
        }
        drop(pass);
        Some((counts, draws))
    }
}

fn coverage(d: &wgpu::Device, frame: &wgpu::BindGroupLayout, samples: u32) -> Coverage {
    let layout = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("game look particle counts"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: wgpu::BufferSize::new(16),
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
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
    let shader = source(
        d,
        &crate::pipeline::depth_source(include_str!("shaders/count.wgsl"), samples),
    );
    let pipeline_layout = d.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("game look particle coverage"),
        bind_group_layouts: &[Some(frame), Some(&layout)],
        immediate_size: 0,
    });
    let attrs =
        wgpu::vertex_attr_array![0=>Float32x4,1=>Float32x4,2=>Float32x4,3=>Float32x4,4=>Float32x4];
    let pipeline = d.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("game look particle coverage"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("quad_vs"),
            buffers: &[Some(wgpu::VertexBufferLayout {
                array_stride: 80,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &attrs,
            })],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("quad_count_fs"),
            targets: &[],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            cull_mode: Some(wgpu::Face::Back),
            ..Default::default()
        },
        // The depth is attached read-only, as the pass needs an attachment;
        // the shader makes the test.
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: Some(false),
            depth_compare: Some(wgpu::CompareFunction::Always),
            stencil: Default::default(),
            bias: Default::default(),
        }),
        multisample: wgpu::MultisampleState {
            count: samples,
            ..Default::default()
        },
        multiview_mask: None,
        cache: None,
    });
    Coverage { pipeline, layout }
}
