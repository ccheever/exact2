//! Optional retained attachments; the default renderer never constructs these.
//! @ref llp/1046.006.000-render-hooks.rfc.md#d3-optional-services-the-scene-copy
use super::{Needs, PostInputs, SceneCopy};
use crate::{buffers::Targets, pipeline::Pipelines};
use exact_gpu::wgpu;

pub(crate) struct FrameBinding {
    pub layout: wgpu::BindGroupLayout,
    pub bind: wgpu::BindGroup,
}
impl FrameBinding {
    pub fn new(device: &wgpu::Device, uniform: &wgpu::Buffer) -> Self {
        let mut entries = super::frame_interface::frame::GROUP_0.entries.to_vec();
        // The shared library has no entry point; each game chooses which stages read it.
        for entry in &mut entries {
            entry.visibility = wgpu::ShaderStages::VERTEX_FRAGMENT | wgpu::ShaderStages::COMPUTE;
        }
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("game hook frame"),
            entries: &entries,
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("game hook frame"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });
        Self { layout, bind }
    }
}
struct PostTarget {
    view: wgpu::TextureView,
    tone: wgpu::BindGroup,
}
struct Depth {
    uniform: wgpu::Buffer,
    bind: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
}
pub(crate) struct HookTargets {
    pub size: (u32, u32),
    pub needs: Needs,
    scene: Option<(wgpu::TextureView, wgpu::TextureView)>,
    final_depth: Option<wgpu::TextureView>,
    post: Option<PostTarget>,
    depth: Option<Depth>,
}
impl HookTargets {
    pub fn new(
        device: &wgpu::Device,
        targets: &Targets,
        pipelines: &Pipelines,
        uniform: &wgpu::Buffer,
        needs: Needs,
    ) -> Self {
        let size = targets.size;
        let texture = |label, format| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d {
                        width: size.0,
                        height: size.1,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        };
        let scene = needs.contains(Needs::SCENE_COPY).then(|| {
            (
                texture("game opaque snapshot", wgpu::TextureFormat::Rgba16Float),
                texture("game opaque depth", wgpu::TextureFormat::R32Float),
            )
        });
        let final_depth = needs
            .contains(Needs::FINAL_DEPTH)
            .then(|| texture("game final depth", wgpu::TextureFormat::R32Float));
        let post = needs.contains(Needs::HDR_POST).then(|| {
            let view = texture("game post HDR", wgpu::TextureFormat::Rgba16Float);
            let tone = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("game post tone source"),
                layout: &pipelines.tone_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: uniform.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                ],
            });
            PostTarget { view, tone }
        });
        let depth = (scene.is_some() || final_depth.is_some()).then(|| {
            let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("game depth resolve"),
                source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/depth.wgsl").into()),
            });
            let layout = device.create_bind_group_layout(&depth_interface::depth::GROUP_0);
            let uniform = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("game depth inverse projection"),
                size: 80,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("game sampled MSAA depth"),
                layout: &layout,
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
            });
            let pipeline = crate::pipeline::make_pipeline(
                device,
                &shader,
                "game depth resolve",
                "vs",
                Some("fs"),
                &[Some(&layout)],
                &[],
                None,
                1,
                wgpu::TextureFormat::R32Float,
                None,
                &[],
                false,
            );
            Depth {
                uniform,
                bind,
                pipeline,
            }
        });
        Self {
            size,
            needs: needs.attachments(),
            scene,
            final_depth,
            post,
            depth,
        }
    }
    pub fn texture_count(&self) -> u64 {
        self.scene.is_some() as u64 * 2
            + self.final_depth.is_some() as u64
            + self.post.is_some() as u64
    }
    pub fn scene(&self) -> Option<SceneCopy<'_>> {
        self.scene
            .as_ref()
            .map(|(color, depth)| SceneCopy { color, depth })
    }
    pub fn hdr<'a>(&'a self, fallback: &'a wgpu::TextureView) -> &'a wgpu::TextureView {
        self.post.as_ref().map_or(fallback, |post| &post.view)
    }
    pub fn tone<'a>(&'a self, fallback: &'a wgpu::BindGroup) -> &'a wgpu::BindGroup {
        self.post.as_ref().map_or(fallback, |post| &post.tone)
    }
    pub fn post<'a>(&'a self, input: &'a wgpu::TextureView) -> Option<PostInputs<'a>> {
        self.post.as_ref().map(|post| PostInputs {
            input,
            output: &post.view,
            depth: self.final_depth.as_ref(),
            opaque_depth: self.scene.as_ref().map(|(_, d)| d),
        })
    }
    pub fn resolve_depth(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        queue: &wgpu::Queue,
        frame: &crate::FrameInput<'_>,
        size: (u32, u32),
        opaque: bool,
        split: f32,
    ) {
        let view = if opaque {
            self.scene.as_ref().map(|(_, d)| d)
        } else {
            self.final_depth.as_ref()
        };
        let (Some(view), Some(depth)) = (view, &self.depth) else {
            return;
        };
        let u = depth_interface::depth::DepthView {
            inverse_projection: frame.proj.inverse().to_cols_array_2d(),
            viewport: [size.0 as f32, size.1 as f32, split, 0.],
        };
        queue.write_buffer(&depth.uniform, 0, &u.bytes());
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(if opaque {
                "game opaque depth resolve"
            } else {
                "game final depth resolve"
            }),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: crate::timing::writes(frame.timestamps, if opaque { 22 } else { 23 }),
            occlusion_query_set: None,
            multiview_mask: None,
        });
        crate::renderer::viewport(&mut pass, size);
        pass.set_pipeline(&depth.pipeline);
        pass.set_bind_group(0, &depth.bind, &[]);
        pass.draw(0..3, 0..1);
    }
}
#[allow(dead_code)]
mod depth_interface {
    include!(concat!(env!("OUT_DIR"), "/depth_interface.rs"));
}
