//! The portable device and S0 control. @ref LLP 1149 D5.1, D6.3, D11.

use super::{Device, Error, Format, Info, Result, Submission, TextureDesc};
use crate::instance::{flags, Batch, Bindings, Brush, Frame, Instance, Kind, Scissor, TextureId};
use ::wgpu::util::DeviceExt;
use std::collections::{BTreeMap, HashMap};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex,
};
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
#[cfg(target_arch = "wasm32")]
use web_time::Instant;

mod cache;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;

const SHADER: &str = include_str!("../../shaders/ink.wgsl");
const ATTRIBUTES: [::wgpu::VertexAttribute; 12] = ::wgpu::vertex_attr_array![
    0 => Float32x4, 1 => Float32x4, 2 => Uint32x4, 3 => Float32x4,
    4 => Float32x4, 5 => Float32x4, 6 => Float32x4, 7 => Uint32x4,
    8 => Float32x4, 9 => Float32x4, 10 => Float32x4, 11 => Float32x4
];

struct Texture {
    raw: ::wgpu::Texture,
    view: ::wgpu::TextureView,
    desc: TextureDesc,
    last: Submission,
}

#[derive(Clone)]
pub struct Buffer {
    raw: ::wgpu::Buffer,
    last: Arc<AtomicU64>,
}

struct BufferSlot {
    buffer: Buffer,
    viewport: ::wgpu::Buffer,
    capacity: u64,
}

pub struct Wgpu {
    device: ::wgpu::Device,
    queue: ::wgpu::Queue,
    pipeline: ::wgpu::RenderPipeline,
    replace_pipeline: ::wgpu::RenderPipeline,
    layout: ::wgpu::BindGroupLayout,
    sampler: ::wgpu::Sampler,
    repeat_sampler: ::wgpu::Sampler,
    buffers: [BufferSlot; 3],
    next_buffer: usize,
    white_alpha: Texture,
    white_color: Texture,
    textures: HashMap<TextureId, Texture>,
    next_texture: u64,
    submitted: Submission,
    done: Arc<AtomicU64>,
    pending: BTreeMap<u64, ::wgpu::SubmissionIndex>,
    retired_textures: Vec<(Submission, Texture)>,
    retired_buffers: Vec<(Submission, ::wgpu::Buffer)>,
    info: Info,
    errors: Arc<Mutex<Option<String>>>,
}

impl Wgpu {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new() -> Result<Self> {
        block_on(Self::open())
    }

    /// Browser callers await this without blocking the event loop.
    pub async fn open() -> Result<Self> {
        let started = Instant::now();
        let instance = ::wgpu::Instance::new(::wgpu::InstanceDescriptor {
            backends: ::wgpu::Backends::PRIMARY,
            ..::wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let adapter = instance
            .request_adapter(&::wgpu::RequestAdapterOptions {
                power_preference: ::wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
                ..Default::default()
            })
            .await
            .map_err(|e| Error::NoAdapter(e.to_string()))?;
        let adapter_info = adapter.get_info();
        let caching = adapter
            .features()
            .contains(::wgpu::Features::PIPELINE_CACHE);
        let (device, queue) = adapter
            .request_device(&::wgpu::DeviceDescriptor {
                label: Some("exact-ink"),
                required_features: if caching {
                    ::wgpu::Features::PIPELINE_CACHE
                } else {
                    ::wgpu::Features::empty()
                },
                ..Default::default()
            })
            .await
            .map_err(|e| Error::Device(e.to_string()))?;
        let failures = Arc::new(Mutex::new(None));
        let errors = failures.clone();
        device.on_uncaptured_error(Arc::new(move |error| {
            errors
                .lock()
                .unwrap()
                .get_or_insert_with(|| format!("{error}"));
        }));
        let device_ms = started.elapsed().as_secs_f64() * 1000.;
        let started = Instant::now();
        let errors = device.push_error_scope(::wgpu::ErrorFilter::Validation);
        let path = caching
            .then(|| cache::path(&adapter_info, SHADER))
            .flatten();
        let data = path.as_ref().and_then(|p| std::fs::read(p).ok());
        let cache = caching.then(|| {
            // SAFETY: only bytes from this adapter's wgpu cache are supplied.
            // The key includes the driver and shader; fallback rejects stale headers.
            unsafe {
                device.create_pipeline_cache(&::wgpu::PipelineCacheDescriptor {
                    label: Some("exact-ink"),
                    data: data.as_deref(),
                    fallback: true,
                })
            }
        });
        let shader = device.create_shader_module(::wgpu::ShaderModuleDescriptor {
            label: Some("ink.wgsl"),
            source: ::wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let mut entries = vec![::wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: ::wgpu::ShaderStages::VERTEX,
            ty: ::wgpu::BindingType::Buffer {
                ty: ::wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: ::wgpu::BufferSize::new(16),
            },
            count: None,
        }];
        for binding in [1, 2, 3, 4, 5, 7, 8] {
            entries.push(::wgpu::BindGroupLayoutEntry {
                binding,
                visibility: ::wgpu::ShaderStages::FRAGMENT,
                ty: ::wgpu::BindingType::Texture {
                    sample_type: ::wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: ::wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            });
        }
        entries.push(::wgpu::BindGroupLayoutEntry {
            binding: 6,
            visibility: ::wgpu::ShaderStages::FRAGMENT,
            ty: ::wgpu::BindingType::Sampler(::wgpu::SamplerBindingType::Filtering),
            count: None,
        });
        let layout = device.create_bind_group_layout(&::wgpu::BindGroupLayoutDescriptor {
            label: Some("ink"),
            entries: &entries,
        });
        let pipeline_layout = device.create_pipeline_layout(&::wgpu::PipelineLayoutDescriptor {
            label: Some("ink"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let make_pipeline = |blend| {
            device.create_render_pipeline(&::wgpu::RenderPipelineDescriptor {
                label: Some("ink"),
                layout: Some(&pipeline_layout),
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
                        format: ::wgpu::TextureFormat::Rgba8Unorm,
                        blend,
                        write_mask: ::wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: cache.as_ref(),
            })
        };
        let pipeline = make_pipeline(Some(::wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING));
        let replace_pipeline = make_pipeline(None);
        if let Some(e) = errors.pop().await {
            return Err(Error::Device(e.to_string()));
        }
        let shaders_ms = started.elapsed().as_secs_f64() * 1000.;
        if let (Some(cache), Some(path)) = (cache.as_ref(), path) {
            if let Some(bytes) = cache.get_data() {
                cache::save(&path, &bytes, data.as_deref());
            }
        }
        let sampler = device.create_sampler(&::wgpu::SamplerDescriptor {
            label: Some("ink"),
            mag_filter: ::wgpu::FilterMode::Linear,
            min_filter: ::wgpu::FilterMode::Linear,
            mipmap_filter: ::wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        let repeat_sampler = device.create_sampler(&::wgpu::SamplerDescriptor {
            label: Some("ink pattern"),
            address_mode_u: ::wgpu::AddressMode::Repeat,
            address_mode_v: ::wgpu::AddressMode::Repeat,
            mag_filter: ::wgpu::FilterMode::Linear,
            min_filter: ::wgpu::FilterMode::Linear,
            mipmap_filter: ::wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        let mut initial = Vec::new();
        for _ in 0..3 {
            let viewport = checked(&device, &failures, || {
                device.create_buffer(&::wgpu::BufferDescriptor {
                    label: Some("ink viewport"),
                    size: 16,
                    usage: ::wgpu::BufferUsages::UNIFORM | ::wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                })
            })?;
            initial.push(BufferSlot {
                viewport,
                buffer: instance_buffer(&device, &failures, 192)?,
                capacity: 192,
            });
        }
        let buffers = initial.try_into().ok().unwrap();
        let white_alpha = allocate_texture(
            &device,
            &failures,
            TextureDesc {
                width: 1,
                height: 1,
                format: Format::Alpha,
                mipmapped: false,
            },
        )?;
        let white_color = allocate_texture(&device, &failures, TextureDesc::rgba(1, 1))?;
        upload(&queue, &white_alpha, [0, 0], [1, 1], &[255]);
        upload(&queue, &white_color, [0, 0], [1, 1], &[255; 4]);
        Ok(Self {
            device,
            queue,
            pipeline,
            replace_pipeline,
            layout,
            sampler,
            repeat_sampler,
            buffers,
            next_buffer: 0,
            white_alpha,
            white_color,
            textures: HashMap::new(),
            next_texture: 1,
            submitted: Submission(0),
            done: Arc::new(AtomicU64::new(0)),
            pending: BTreeMap::new(),
            retired_textures: Vec::new(),
            retired_buffers: Vec::new(),
            errors: failures,
            info: Info {
                adapter: adapter_info.name,
                api: format!("wgpu {:?}", adapter_info.backend),
                device_ms,
                shaders_ms,
                cached: data.is_some(),
            },
        })
    }

    fn texture(&self, id: TextureId) -> Result<&Texture> {
        self.textures
            .get(&id)
            .ok_or_else(|| Error::Invalid(format!("unknown texture {}", id.0)))
    }

    fn slot(&self, id: TextureId, format: Format) -> Result<&::wgpu::TextureView> {
        let texture = if id.0 == 0 {
            if format == Format::Alpha {
                &self.white_alpha
            } else {
                &self.white_color
            }
        } else {
            self.texture(id)?
        };
        if texture.desc.format != format {
            return Err(Error::Invalid("texture slot format".into()));
        }
        Ok(&texture.view)
    }

    fn bindings(
        &self,
        b: Bindings,
        uniform: &::wgpu::Buffer,
        image: Option<&::wgpu::TextureView>,
    ) -> Result<::wgpu::BindGroup> {
        let views = [
            self.slot(b.glyph, Format::Alpha)?,
            self.slot(b.color, Format::Rgba)?,
            self.slot(b.path, Format::Alpha)?,
            self.slot(b.ramp, Format::Rgba)?,
            match image {
                Some(v) => v,
                None => self.slot(b.image, Format::Rgba)?,
            },
        ];
        let mut entries = vec![::wgpu::BindGroupEntry {
            binding: 0,
            resource: uniform.as_entire_binding(),
        }];
        for (i, view) in views.into_iter().enumerate() {
            entries.push(::wgpu::BindGroupEntry {
                binding: i as u32 + 1,
                resource: ::wgpu::BindingResource::TextureView(view),
            });
        }
        for (binding, id, format) in [(7, b.mask, Format::Alpha), (8, b.destination, Format::Rgba)]
        {
            entries.push(::wgpu::BindGroupEntry {
                binding,
                resource: ::wgpu::BindingResource::TextureView(if binding == 7 && id.0 != 0 {
                    &self.texture(id)?.view
                } else {
                    self.slot(id, format)?
                }),
            });
        }
        entries.push(::wgpu::BindGroupEntry {
            binding: 6,
            resource: ::wgpu::BindingResource::Sampler(if b.repeat {
                &self.repeat_sampler
            } else {
                &self.sampler
            }),
        });
        Ok(self.device.create_bind_group(&::wgpu::BindGroupDescriptor {
            label: Some("ink"),
            layout: &self.layout,
            entries: &entries,
        }))
    }

    fn submit(&mut self, commands: ::wgpu::CommandBuffer) -> Submission {
        self.submitted.0 += 1;
        let id = self.submitted;
        let index = self.queue.submit([commands]);
        self.pending.insert(id.0, index);
        let done = self.done.clone();
        self.queue.on_submitted_work_done(move || {
            done.fetch_max(id.0, Ordering::Release);
        });
        id
    }

    fn encode_pass(
        &self,
        encoder: &mut ::wgpu::CommandEncoder,
        target: &::wgpu::TextureView,
        buffer: &::wgpu::Buffer,
        batches: &[Batch],
        groups: &[::wgpu::BindGroup],
        clear: Option<[f64; 4]>,
    ) {
        let load = match clear {
            Some([r, g, b, a]) => ::wgpu::LoadOp::Clear(::wgpu::Color { r, g, b, a }),
            None => ::wgpu::LoadOp::Load,
        };
        let attachments = [Some(::wgpu::RenderPassColorAttachment {
            view: target,
            depth_slice: None,
            resolve_target: None,
            ops: ::wgpu::Operations {
                load,
                store: ::wgpu::StoreOp::Store,
            },
        })];
        let mut pass = encoder.begin_render_pass(&::wgpu::RenderPassDescriptor {
            label: Some("ink"),
            color_attachments: &attachments,
            ..Default::default()
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_vertex_buffer(0, buffer.slice(..));
        for (batch, bindings) in batches.iter().zip(groups) {
            let s = batch.scissor;
            if s.width == 0 || s.height == 0 || batch.instances.is_empty() {
                continue;
            }
            pass.set_pipeline(if batch.replace {
                &self.replace_pipeline
            } else {
                &self.pipeline
            });
            pass.set_scissor_rect(s.x, s.y, s.width, s.height);
            pass.set_bind_group(0, bindings, &[]);
            pass.draw(0..4, batch.instances.clone());
        }
    }

    /// Tight rows, with no GPU copy alignment padding. Alpha textures return
    /// one byte per pixel; RGBA textures return four premultiplied bytes.
    pub async fn readback_async(&mut self, id: TextureId, level: u32) -> Result<Vec<u8>> {
        let texture = self.texture(id)?;
        if level >= texture.desc.levels() {
            return Err(Error::Invalid("mip level".into()));
        }
        let width = (texture.desc.width >> level).max(1);
        let height = (texture.desc.height >> level).max(1);
        let row = width * texture.desc.format.channels();
        let padded = row.div_ceil(::wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            * ::wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let buffer = checked(&self.device, &self.errors, || {
            self.device.create_buffer(&::wgpu::BufferDescriptor {
                label: Some("ink readback"),
                size: u64::from(padded) * u64::from(height),
                usage: ::wgpu::BufferUsages::COPY_DST | ::wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            })
        })?;
        let mut encoder = self.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            ::wgpu::TexelCopyTextureInfo {
                texture: &texture.raw,
                mip_level: level,
                origin: ::wgpu::Origin3d::ZERO,
                aspect: ::wgpu::TextureAspect::All,
            },
            ::wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: ::wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: None,
                },
            },
            extent([width, height]),
        );
        let submitted = self.submit(encoder.finish());
        self.textures.get_mut(&id).unwrap().last = submitted;
        #[derive(Default)]
        struct Mapped {
            result: Option<std::result::Result<(), ::wgpu::BufferAsyncError>>,
            waker: Option<std::task::Waker>,
        }
        let state = Arc::new(Mutex::new(Mapped::default()));
        let callback = state.clone();
        buffer
            .slice(..)
            .map_async(::wgpu::MapMode::Read, move |result| {
                let waker = {
                    let mut s = callback.lock().unwrap();
                    s.result = Some(result);
                    s.waker.take()
                };
                if let Some(waker) = waker {
                    waker.wake();
                }
            });
        #[cfg(not(target_arch = "wasm32"))]
        self.wait(submitted)?;
        #[cfg(target_arch = "wasm32")]
        let _ = submitted;
        std::future::poll_fn(|cx| {
            let mut s = state.lock().unwrap();
            match s.result.take() {
                Some(result) => std::task::Poll::Ready(result),
                None => {
                    s.waker = Some(cx.waker().clone());
                    std::task::Poll::Pending
                }
            }
        })
        .await
        .map_err(|e| Error::Device(e.to_string()))?;
        let mapped = buffer
            .slice(..)
            .get_mapped_range()
            .map_err(|e| Error::Device(e.to_string()))?;
        let mut pixels = vec![0; row as usize * height as usize];
        for (dst, src) in pixels
            .chunks_exact_mut(row as usize)
            .zip(mapped.chunks_exact(padded as usize))
        {
            dst.copy_from_slice(&src[..row as usize]);
        }
        drop(mapped);
        buffer.unmap();
        if let Some(error) = &*self.errors.lock().unwrap() {
            return Err(Error::Device(error.clone()));
        }
        Ok(pixels)
    }
}

impl Device for Wgpu {
    type Buffer = Buffer;
    fn info(&self) -> &Info {
        &self.info
    }

    fn create_buffer(&self, bytes: u64) -> Result<Self::Buffer> {
        if bytes == 0 || bytes > self.device.limits().max_buffer_size {
            return Err(Error::Invalid("instance buffer size".into()));
        }
        instance_buffer(&self.device, &self.errors, bytes)
    }

    fn upload_buffer(
        &mut self,
        buffer: &Self::Buffer,
        offset: u64,
        data: &[u8],
    ) -> Result<Submission> {
        if !offset.is_multiple_of(4)
            || !data.len().is_multiple_of(4)
            || offset
                .checked_add(data.len() as u64)
                .is_none_or(|end| end > buffer.raw.size())
        {
            return Err(Error::Invalid(
                "instance buffer upload bounds or alignment".into(),
            ));
        }
        self.wait(Submission(buffer.last.load(Ordering::Acquire)))?;
        self.queue.write_buffer(&buffer.raw, offset, data);
        let submission = self.submit(
            self.device
                .create_command_encoder(&Default::default())
                .finish(),
        );
        buffer.last.store(submission.0, Ordering::Release);
        Ok(submission)
    }

    fn create_texture(&mut self, desc: TextureDesc) -> Result<TextureId> {
        let limit = self.device.limits().max_texture_dimension_2d;
        if desc.width == 0 || desc.height == 0 || desc.width > limit || desc.height > limit {
            return Err(Error::Invalid("texture dimensions".into()));
        }
        if desc.mipmapped && desc.format == Format::Alpha {
            return Err(Error::Unsupported("mip chains are RGBA image textures"));
        }
        let id = TextureId(self.next_texture);
        self.next_texture += 1;
        self.textures
            .insert(id, allocate_texture(&self.device, &self.errors, desc)?);
        Ok(id)
    }

    fn upload_texture(
        &mut self,
        id: TextureId,
        origin: [u32; 2],
        size: [u32; 2],
        data: &[u8],
    ) -> Result<Submission> {
        self.wait(self.last_use(id)?)?;
        let texture = self.texture(id)?;
        check_region(texture.desc, origin, size)?;
        let bytes =
            u64::from(size[0]) * u64::from(size[1]) * u64::from(texture.desc.format.channels());
        if data.len() as u64 != bytes {
            return Err(Error::Invalid("texture upload length".into()));
        }
        upload(&self.queue, texture, origin, size, data);
        let submission = self.submit(
            self.device
                .create_command_encoder(&Default::default())
                .finish(),
        );
        self.textures.get_mut(&id).unwrap().last = submission;
        Ok(submission)
    }

    fn last_use(&self, id: TextureId) -> Result<Submission> {
        Ok(self.texture(id)?.last)
    }

    fn generate_mips(&mut self, id: TextureId) -> Result<Submission> {
        self.completed()?;
        let texture = self.texture(id)?;
        if texture.desc.format != Format::Rgba {
            return Err(Error::Unsupported("alpha mip chain"));
        }
        let mut encoder = self.device.create_command_encoder(&Default::default());
        for level in 1..texture.desc.levels() {
            let width = (texture.desc.width >> level).max(1);
            let height = (texture.desc.height >> level).max(1);
            let source = texture.raw.create_view(&::wgpu::TextureViewDescriptor {
                base_mip_level: level - 1,
                mip_level_count: Some(1),
                ..Default::default()
            });
            let target = texture.raw.create_view(&::wgpu::TextureViewDescriptor {
                base_mip_level: level,
                mip_level_count: Some(1),
                ..Default::default()
            });
            let viewport = checked(&self.device, &self.errors, || {
                self.device
                    .create_buffer_init(&::wgpu::util::BufferInitDescriptor {
                        label: Some("ink mip viewport"),
                        contents: bytemuck::cast_slice(&[width as f32, height as f32, 0., 0.]),
                        usage: ::wgpu::BufferUsages::UNIFORM,
                    })
            })?;
            let instance = Instance::new(
                Kind::Downsample,
                Brush::Solid,
                [0., 0., width as f32, height as f32],
            );
            let buffer = checked(&self.device, &self.errors, || {
                self.device
                    .create_buffer_init(&::wgpu::util::BufferInitDescriptor {
                        label: Some("ink mip quad"),
                        contents: bytemuck::bytes_of(&instance),
                        usage: ::wgpu::BufferUsages::VERTEX,
                    })
            })?;
            let group = self.bindings(Bindings::default(), &viewport, Some(&source))?;
            let batch = Batch {
                replace: false,
                instances: 0..1,
                scissor: Scissor {
                    x: 0,
                    y: 0,
                    width,
                    height,
                },
                bindings: Bindings::default(),
            };
            self.encode_pass(
                &mut encoder,
                &target,
                &buffer,
                &[batch],
                &[group],
                Some([0.; 4]),
            );
        }
        let submission = self.submit(encoder.finish());
        self.textures.get_mut(&id).unwrap().last = submission;
        Ok(submission)
    }

    fn resize_target(&mut self, id: TextureId, width: u32, height: u32) -> Result<TextureId> {
        let old = self.texture(id)?.desc;
        if old.format != Format::Rgba || old.mipmapped {
            return Err(Error::Invalid("resize requires a target".into()));
        }
        if old.width == width && old.height == height {
            return Ok(id);
        }
        let next = self.create_target(width, height)?;
        self.retire_texture(id)?;
        Ok(next)
    }

    fn copy_region(
        &mut self,
        source: TextureId,
        origin: [u32; 2],
        target: TextureId,
        dst: [u32; 2],
        size: [u32; 2],
    ) -> Result<Submission> {
        let from = self.texture(source)?;
        let to = self.texture(target)?;
        check_region(from.desc, origin, size)?;
        check_region(to.desc, dst, size)?;
        if source == target || from.desc.format != to.desc.format {
            return Err(Error::Invalid(
                "copy requires distinct textures of one format".into(),
            ));
        }
        let mut encoder = self.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_texture(
            ::wgpu::TexelCopyTextureInfo {
                origin: point(origin),
                ..from.raw.as_image_copy()
            },
            ::wgpu::TexelCopyTextureInfo {
                origin: point(dst),
                ..to.raw.as_image_copy()
            },
            extent(size),
        );
        let submission = self.submit(encoder.finish());
        for id in [source, target] {
            self.textures.get_mut(&id).unwrap().last = submission;
        }
        Ok(submission)
    }

    fn render(
        &mut self,
        frame: &Frame,
        target: TextureId,
        clear: Option<[f64; 4]>,
    ) -> Result<Submission> {
        self.completed()?;
        let desc = self.texture(target)?.desc;
        if desc.format != Format::Rgba || desc.mipmapped {
            return Err(Error::Invalid(
                "render target must be single-level RGBA".into(),
            ));
        }
        if frame
            .instances
            .iter()
            .any(|i| i.kind_brush & 65535 == Kind::Shadow as u32 && i.flags & flags::CLIP1 == 0)
        {
            return Err(Error::Invalid(
                "a shadow requires its border box in clip1".into(),
            ));
        }
        for batch in &frame.batches {
            if batch.instances.start > batch.instances.end
                || batch.instances.end as usize > frame.instances.len()
            {
                return Err(Error::Invalid("batch instance range".into()));
            }
            if batch.scissor.width != 0 && batch.scissor.height != 0 {
                check_region(
                    desc,
                    [batch.scissor.x, batch.scissor.y],
                    [batch.scissor.width, batch.scissor.height],
                )?;
            }
            let b = batch.bindings;
            if [
                b.glyph,
                b.color,
                b.path,
                b.ramp,
                b.image,
                b.mask,
                b.destination,
            ]
            .contains(&target)
            {
                return Err(Error::Invalid(
                    "a render target cannot sample itself".into(),
                ));
            }
        }
        let slot = self.next_buffer;
        self.next_buffer = (slot + 1) % self.buffers.len();
        self.wait(Submission(
            self.buffers[slot].buffer.last.load(Ordering::Acquire),
        ))?;
        let bytes = bytemuck::cast_slice(&frame.instances);
        if bytes.len() as u64 > self.buffers[slot].capacity {
            let capacity = (bytes.len() as u64).next_power_of_two();
            let buffer = self.create_buffer(capacity)?;
            let old = std::mem::replace(&mut self.buffers[slot].buffer, buffer);
            self.buffers[slot].capacity = capacity;
            self.retired_buffers.push((self.submitted, old.raw));
        }
        if !bytes.is_empty() {
            self.queue
                .write_buffer(&self.buffers[slot].buffer.raw, 0, bytes);
        }
        self.queue.write_buffer(
            &self.buffers[slot].viewport,
            0,
            bytemuck::cast_slice(&[desc.width as f32, desc.height as f32, 0., 0.]),
        );
        let groups = frame
            .batches
            .iter()
            .map(|b| self.bindings(b.bindings, &self.buffers[slot].viewport, None))
            .collect::<Result<Vec<_>>>()?;
        let mut encoder = self.device.create_command_encoder(&Default::default());
        self.encode_pass(
            &mut encoder,
            &self.texture(target)?.view,
            &self.buffers[slot].buffer.raw,
            &frame.batches,
            &groups,
            clear,
        );
        let submission = self.submit(encoder.finish());
        self.buffers[slot]
            .buffer
            .last
            .store(submission.0, Ordering::Release);
        self.textures.get_mut(&target).unwrap().last = submission;
        for batch in &frame.batches {
            let b = batch.bindings;
            for id in [
                b.glyph,
                b.color,
                b.path,
                b.ramp,
                b.image,
                b.mask,
                b.destination,
            ] {
                if id.0 != 0 {
                    self.textures.get_mut(&id).unwrap().last = submission;
                }
            }
        }
        Ok(submission)
    }

    fn readback(&mut self, texture: TextureId) -> Result<Vec<u8>> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            block_on(self.readback_async(texture, 0))
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = texture;
            Err(Error::Unsupported(
                "synchronous WebGPU readback; await readback_async",
            ))
        }
    }

    fn wait(&mut self, submission: Submission) -> Result<()> {
        if submission > self.submitted {
            return Err(Error::Invalid("unknown submission".into()));
        }
        if submission.0 <= self.done.load(Ordering::Acquire) {
            return Ok(());
        }
        #[cfg(target_arch = "wasm32")]
        {
            Err(Error::Unsupported("blocking the WebGPU event loop"))
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.device
                .poll(::wgpu::PollType::Wait {
                    submission_index: self.pending.get(&submission.0).cloned(),
                    timeout: Some(std::time::Duration::from_secs(30)),
                })
                .map_err(|e| Error::Device(e.to_string()))?;
            self.completed()?;
            Ok(())
        }
    }

    fn completed(&mut self) -> Result<Submission> {
        if let Some(e) = self.errors.lock().unwrap().as_ref() {
            return Err(Error::Device(e.clone()));
        }
        self.device
            .poll(::wgpu::PollType::Poll)
            .map_err(|e| Error::Device(e.to_string()))?;
        let done = Submission(self.done.load(Ordering::Acquire));
        self.pending.retain(|id, _| *id > done.0);
        self.retired_textures.retain(|(last, _)| *last > done);
        self.retired_buffers.retain(|(last, _)| *last > done);
        Ok(done)
    }

    fn retire_texture(&mut self, texture: TextureId) -> Result<()> {
        let texture = self
            .textures
            .remove(&texture)
            .ok_or_else(|| Error::Invalid("retired texture".into()))?;
        self.retired_textures.push((texture.last, texture));
        self.completed()?;
        Ok(())
    }

    fn trim(&mut self) -> Result<()> {
        self.completed()?;
        for slot in &mut self.buffers {
            if slot.capacity > 192 {
                let old = std::mem::replace(
                    &mut slot.buffer,
                    instance_buffer(&self.device, &self.errors, 192)?,
                );
                slot.buffer
                    .last
                    .store(old.last.load(Ordering::Acquire), Ordering::Release);
                slot.capacity = 192;
                self.retired_buffers
                    .push((Submission(old.last.load(Ordering::Acquire)), old.raw));
            }
        }
        Ok(())
    }
}

fn instance_buffer(
    device: &::wgpu::Device,
    failures: &Arc<Mutex<Option<String>>>,
    size: u64,
) -> Result<Buffer> {
    let raw = checked(device, failures, || {
        device.create_buffer(&::wgpu::BufferDescriptor {
            label: Some("ink instances"),
            size,
            usage: ::wgpu::BufferUsages::VERTEX | ::wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    })?;
    Ok(Buffer {
        raw,
        last: Arc::new(AtomicU64::new(0)),
    })
}

fn allocate_texture(
    device: &::wgpu::Device,
    failures: &Arc<Mutex<Option<String>>>,
    desc: TextureDesc,
) -> Result<Texture> {
    let raw = checked(device, failures, || {
        device.create_texture(&::wgpu::TextureDescriptor {
            label: Some("ink"),
            size: extent([desc.width, desc.height]),
            mip_level_count: desc.levels(),
            sample_count: 1,
            dimension: ::wgpu::TextureDimension::D2,
            format: if desc.format == Format::Alpha {
                ::wgpu::TextureFormat::R8Unorm
            } else {
                ::wgpu::TextureFormat::Rgba8Unorm
            },
            usage: ::wgpu::TextureUsages::TEXTURE_BINDING
                | ::wgpu::TextureUsages::COPY_DST
                | ::wgpu::TextureUsages::COPY_SRC
                | ::wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
    })?;
    let view = raw.create_view(&Default::default());
    Ok(Texture {
        raw,
        view,
        desc,
        last: Submission(0),
    })
}

fn check_region(desc: TextureDesc, origin: [u32; 2], size: [u32; 2]) -> Result<()> {
    if size.contains(&0)
        || origin[0]
            .checked_add(size[0])
            .is_none_or(|v| v > desc.width)
        || origin[1]
            .checked_add(size[1])
            .is_none_or(|v| v > desc.height)
    {
        Err(Error::Invalid("texture region bounds".into()))
    } else {
        Ok(())
    }
}

fn extent([width, height]: [u32; 2]) -> ::wgpu::Extent3d {
    ::wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    }
}
fn point([x, y]: [u32; 2]) -> ::wgpu::Origin3d {
    ::wgpu::Origin3d { x, y, z: 0 }
}
fn upload(queue: &::wgpu::Queue, texture: &Texture, origin: [u32; 2], size: [u32; 2], data: &[u8]) {
    queue.write_texture(
        ::wgpu::TexelCopyTextureInfo {
            origin: point(origin),
            ..texture.raw.as_image_copy()
        },
        data,
        ::wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(size[0] * texture.desc.format.channels()),
            rows_per_image: None,
        },
        extent(size),
    );
}

#[cfg(not(target_arch = "wasm32"))]
fn block_on<F: std::future::Future>(future: F) -> F::Output {
    struct Wake(std::thread::Thread);
    impl std::task::Wake for Wake {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }
    let waker = std::task::Waker::from(Arc::new(Wake(std::thread::current())));
    let mut context = std::task::Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        match future.as_mut().poll(&mut context) {
            std::task::Poll::Ready(value) => return value,
            std::task::Poll::Pending => std::thread::park(),
        }
    }
}

fn checked<T>(
    device: &::wgpu::Device,
    failures: &Arc<Mutex<Option<String>>>,
    make: impl FnOnce() -> T,
) -> Result<T> {
    if let Some(error) = &*failures.lock().unwrap() {
        return Err(Error::Device(error.clone()));
    }
    let scopes = [
        ::wgpu::ErrorFilter::OutOfMemory,
        ::wgpu::ErrorFilter::Internal,
        ::wgpu::ErrorFilter::Validation,
    ]
    .map(|filter| device.push_error_scope(filter));
    let value = make();
    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut failure = None;
        for scope in scopes.into_iter().rev() {
            if let Some(error) = block_on(scope.pop()) {
                failure = Some(Error::Device(error.to_string()));
            }
        }
        if let Some(error) = failure {
            return Err(error);
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        // WebGPU reports scopes asynchronously; completion/readback observes the latch.
        for scope in scopes.into_iter().rev() {
            let future = scope.pop();
            let failures = failures.clone();
            wasm_bindgen_futures::spawn_local(async move {
                if let Some(error) = future.await {
                    failures
                        .lock()
                        .unwrap()
                        .get_or_insert_with(|| error.to_string());
                }
            });
        }
    }
    Ok(value)
}
