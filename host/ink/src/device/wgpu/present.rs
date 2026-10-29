//! wgpu's surface over the carrier's same CAMetalLayer. @ref LLP 1149 D5.1, D7.
use super::*;
use crate::device::apple::{Layer, RenderLayer};
use crate::present::Thread;

pub(super) struct Window {
    pub thread: Thread,
    renderer: Arc<Mutex<Renderer>>,
    _layer: Layer,
}

struct Renderer {
    surface: ::wgpu::Surface<'static>,
    config: ::wgpu::SurfaceConfiguration,
    device: ::wgpu::Device,
    queue: ::wgpu::Queue,
    pipeline: ::wgpu::RenderPipeline,
    layout: ::wgpu::BindGroupLayout,
    sampler: ::wgpu::Sampler,
    white: ::wgpu::TextureView,
    ring: [(
        ::wgpu::Buffer,
        ::wgpu::Buffer,
        Option<::wgpu::SubmissionIndex>,
    ); 3],
    next: usize,
    layer: RenderLayer,
}

impl Wgpu {
    pub fn attach_layer(&mut self, layer: Layer) -> Result<()> {
        self.wait(self.submitted)?;
        let render_layer = layer.render_layer();
        // SAFETY: the window owner and its retained CAMetalLayer outlive the
        // surface and the joined render thread. This is the layer itself, not NSView.
        let surface = unsafe {
            self.instance
                .create_surface_unsafe(::wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(
                    objc2::rc::Retained::as_ptr(&render_layer.raw)
                        .cast_mut()
                        .cast(),
                ))
        }
        .map_err(|e| Error::Device(e.to_string()))?;
        let mut config = surface
            .get_default_config(&self.adapter, 1, 1)
            .ok_or_else(|| Error::Device("Metal surface configuration".into()))?;
        config.format = ::wgpu::TextureFormat::Bgra8Unorm;
        config.present_mode = ::wgpu::PresentMode::Fifo;
        config.desired_maximum_frame_latency = 2;
        config.alpha_mode = ::wgpu::CompositeAlphaMode::Opaque;
        config.view_formats.clear();
        let started = Instant::now();
        let shader = self
            .device
            .create_shader_module(::wgpu::ShaderModuleDescriptor {
                label: Some("ink presentation"),
                source: ::wgpu::ShaderSource::Wgsl(SHADER.into()),
            });
        let layout = self
            .device
            .create_pipeline_layout(&::wgpu::PipelineLayoutDescriptor {
                label: Some("ink presentation"),
                bind_group_layouts: &[Some(&self.layout)],
                immediate_size: 0,
            });
        let pipeline = self
            .device
            .create_render_pipeline(&::wgpu::RenderPipelineDescriptor {
                label: Some("ink presentation"),
                layout: Some(&layout),
                vertex: ::wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[Some(::wgpu::VertexBufferLayout {
                        array_stride: size_of::<Instance>() as u64,
                        step_mode: ::wgpu::VertexStepMode::Instance,
                        attributes: &ATTRIBUTES,
                    })],
                },
                primitive: ::wgpu::PrimitiveState {
                    topology: ::wgpu::PrimitiveTopology::TriangleStrip,
                    ..Default::default()
                },
                depth_stencil: None,
                multisample: Default::default(),
                fragment: Some(::wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    compilation_options: Default::default(),
                    targets: &[Some(::wgpu::ColorTargetState {
                        format: config.format,
                        blend: None,
                        write_mask: ::wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            });
        failure(&self.errors)?;
        self.info.shaders_ms += started.elapsed().as_secs_f64() * 1000.;
        let ring = std::array::from_fn(|_| {
            (
                self.device.create_buffer(&::wgpu::BufferDescriptor {
                    label: Some("drawable instance"),
                    size: 192,
                    usage: ::wgpu::BufferUsages::VERTEX | ::wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }),
                self.device.create_buffer(&::wgpu::BufferDescriptor {
                    label: Some("drawable viewport"),
                    size: 16,
                    usage: ::wgpu::BufferUsages::UNIFORM | ::wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }),
                None,
            )
        });
        self.window = Some(Window {
            thread: Thread::new()?,
            renderer: Arc::new(Mutex::new(Renderer {
                surface,
                config,
                device: self.device.clone(),
                queue: self.queue.clone(),
                pipeline,
                layout: self.layout.clone(),
                sampler: self.sampler.clone(),
                white: self.white_alpha.view.clone(),
                ring,
                next: 0,
                layer: render_layer,
            })),
            _layer: layer,
        });
        Ok(())
    }

    pub fn has_layer(&self) -> bool {
        self.window.is_some()
    }

    pub fn present(&mut self, texture: TextureId) -> Result<f64> {
        let started = Instant::now();
        failure(&self.errors)?;
        let source = self.texture(texture)?;
        let view = source.view.clone();
        let size = [source.desc.width, source.desc.height];
        let window = self
            .window
            .as_ref()
            .ok_or(Error::Unsupported("no Metal surface"))?;
        let renderer = window.renderer.clone();
        let done = self.done.clone();
        let errors = self.errors.clone();
        self.submitted.0 += 1;
        let submission = self.submitted;
        let receipt = window.thread.send(move || {
            let mut renderer = renderer.lock().unwrap();
            let result = renderer.present(view, size);
            if let Err(error) = &result {
                *errors.lock().unwrap() = Some(error.clone());
            }
            renderer.queue.on_submitted_work_done(move || {
                done.fetch_max(submission.0, Ordering::Release);
            });
            result
        })?;
        self.pending.insert(submission.0, Pending::Present(receipt));
        self.textures.get_mut(&texture).unwrap().last = submission;
        Ok(started.elapsed().as_secs_f64() * 1000.)
    }
}

impl Renderer {
    fn present(
        &mut self,
        source: ::wgpu::TextureView,
        [width, height]: [u32; 2],
    ) -> Result<::wgpu::SubmissionIndex> {
        if self.config.width != width
            || self.config.height != height
            || self.next == 0 && self.ring.iter().all(|s| s.2.is_none())
        {
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
            self.layer.raw.setAllowsNextDrawableTimeout(true);
        }
        // wgpu-hal honours presentsWithTransaction and waits until scheduled.
        self.layer
            .raw
            .setPresentsWithTransaction(self.layer.live_resize.load(Ordering::Acquire));
        let output = match self.surface.get_current_texture() {
            ::wgpu::CurrentSurfaceTexture::Success(t)
            | ::wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            ::wgpu::CurrentSurfaceTexture::Timeout | ::wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(self.queue.submit([]))
            }
            ::wgpu::CurrentSurfaceTexture::Outdated | ::wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.device, &self.config);
                return Ok(self.queue.submit([]));
            }
            other => return Err(Error::Device(format!("acquire Metal surface: {other:?}"))),
        };
        let (instances, viewport, last) = &mut self.ring[self.next];
        if let Some(index) = last.take() {
            self.device
                .poll(::wgpu::PollType::Wait {
                    submission_index: Some(index),
                    timeout: Some(std::time::Duration::from_secs(30)),
                })
                .map_err(|e| Error::Device(e.to_string()))?;
        }
        let instance = Instance::new(
            Kind::Layer,
            Brush::Image,
            [0., 0., width as f32, height as f32],
        );
        self.queue
            .write_buffer(instances, 0, bytemuck::bytes_of(&instance));
        self.queue.write_buffer(
            viewport,
            0,
            bytemuck::cast_slice(&[width as f32, height as f32, 0., 0.]),
        );
        let mut entries = vec![::wgpu::BindGroupEntry {
            binding: 0,
            resource: viewport.as_entire_binding(),
        }];
        for binding in [1, 2, 3, 4, 5, 7, 8] {
            entries.push(::wgpu::BindGroupEntry {
                binding,
                resource: ::wgpu::BindingResource::TextureView(if [1, 3, 7].contains(&binding) {
                    &self.white
                } else {
                    &source
                }),
            });
        }
        entries.push(::wgpu::BindGroupEntry {
            binding: 6,
            resource: ::wgpu::BindingResource::Sampler(&self.sampler),
        });
        let group = self.device.create_bind_group(&::wgpu::BindGroupDescriptor {
            label: Some("ink drawable"),
            layout: &self.layout,
            entries: &entries,
        });
        let view = output.texture.create_view(&Default::default());
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let attachments = [Some(::wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: ::wgpu::Operations {
                    load: ::wgpu::LoadOp::Clear(::wgpu::Color::TRANSPARENT),
                    store: ::wgpu::StoreOp::Store,
                },
            })];
            let mut pass = encoder.begin_render_pass(&::wgpu::RenderPassDescriptor {
                label: Some("ink drawable"),
                color_attachments: &attachments,
                ..Default::default()
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_vertex_buffer(0, instances.slice(..));
            pass.set_bind_group(0, &group, &[]);
            pass.draw(0..4, 0..1);
        }
        let index = self.queue.submit([encoder.finish()]);
        if self.layer.live_resize.load(Ordering::Acquire) {
            let queue = self.queue.clone();
            self.layer.transaction(move || queue.present(output));
        } else {
            self.queue.present(output);
        }
        *last = Some(index.clone());
        self.next = (self.next + 1) % self.ring.len();
        Ok(index)
    }
}
