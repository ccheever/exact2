//! The process's GPU context and one canvas's GPU side.
//!
//! One device, queue and vello renderer per process, made at the first
//! canvas and never torn down (`trim` empties its caches), behind one lock.
//! A canvas keeps its replay state and the textures wrapping the host's
//! IOSurfaces (at most three, retained until it is freed). A replay encodes
//! its scene outside the lock, then renders under it: the shadows first
//! (offscreen, blurred), then the scene into `target`, each pixel starting
//! from `previous` unless the lists cover the canvas, and waits for the GPU.

use crate::kernels;
use crate::replay::{Frame, Host, Replayer, ShadowJob};
use crate::surface::{self, Surface};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::Instant;
use vello::peniko::Color;
use vello::{AaConfig, AaSupport, RenderParams, Renderer, RendererOptions, Scene};

/// The shared context.
pub struct Gpu {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub renderer: Renderer,
    blur: [(wgpu::ComputePipeline, wgpu::BindGroupLayout); 2],
    /// A 1×1 transparent texture: the backdrop of a canvas with no pixels.
    empty: wgpu::TextureView,
}

/// How long the context took to make, for the cold-start report.
#[derive(Clone, Copy, Debug, Default)]
pub struct Startup {
    /// Instance, adapter, device and queue, ms.
    pub context_ms: f64,
    /// Every compute pipeline from the embedded Metal libraries, ms.
    pub pipelines_ms: f64,
}

static GPU: OnceLock<Result<(Mutex<Gpu>, Startup), String>> = OnceLock::new();

/// Poll a future that completes without waking (wgpu's native futures).
pub fn block_on<F: std::future::Future>(f: F) -> F::Output {
    use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
    fn noop(_: *const ()) {}
    fn clone(p: *const ()) -> RawWaker {
        RawWaker::new(p, &VTABLE)
    }
    static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, noop, noop, noop);
    // SAFETY: a waker whose functions do nothing, over a pointer never read.
    let waker = unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &VTABLE)) };
    let mut cx = Context::from_waker(&waker);
    let mut f = std::pin::pin!(f);
    loop {
        if let Poll::Ready(v) = f.as_mut().poll(&mut cx) {
            return v;
        }
        std::thread::yield_now();
    }
}

fn make() -> Result<(Mutex<Gpu>, Startup), String> {
    let t0 = Instant::now();
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::METAL,
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: false,
    }))
    .map_err(|e| format!("no adapter: {e}"))?;
    let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("exact canvas"),
        required_features: wgpu::Features::PASSTHROUGH_SHADERS | wgpu::Features::BGRA8UNORM_STORAGE,
        // What this GPU has: an iPhone's is under wgpu's defaults.
        required_limits: adapter.limits(),
        ..Default::default()
    }))
    .map_err(|e| format!("no device: {e}"))?;
    device.on_uncaptured_error(std::sync::Arc::new(|e| eprintln!("exact canvas gpu: {e}")));
    let t1 = Instant::now();
    let renderer = Renderer::new(
        &device,
        RendererOptions {
            use_cpu: false,
            antialiasing_support: AaSupport::area_only(),
            num_init_threads: std::num::NonZeroUsize::new(1),
            pipeline_cache: None,
            shaders: kernels::vello_shaders(),
        },
    )
    .map_err(|e| format!("vello: {e}"))?;
    // The iOS Simulator reports only the Apple2 family, so wgpu refuses
    // indirect dispatch there; the renderer then dispatches directly.
    let mut renderer = renderer;
    renderer.set_indirect(
        adapter
            .get_downlevel_capabilities()
            .flags
            .contains(wgpu::DownlevelFlags::INDIRECT_EXECUTION),
    );
    let blur = [
        blur_pipeline(&device, "shadow_blur_x", wgpu::TextureFormat::R32Float)?,
        blur_pipeline(&device, "shadow_blur_y", wgpu::TextureFormat::Rgba8Unorm)?,
    ];
    let empty = device
        .create_texture(&texture_desc(
            1,
            1,
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureUsages::TEXTURE_BINDING,
        ))
        .create_view(&Default::default());
    let startup = Startup {
        context_ms: (t1 - t0).as_secs_f64() * 1e3,
        pipelines_ms: t1.elapsed().as_secs_f64() * 1e3,
    };
    Ok((
        Mutex::new(Gpu {
            device,
            queue,
            renderer,
            blur,
            empty,
        }),
        startup,
    ))
}

fn blur_pipeline(
    device: &wgpu::Device,
    name: &str,
    out: wgpu::TextureFormat,
) -> Result<(wgpu::ComputePipeline, wgpu::BindGroupLayout), String> {
    let k = kernels::kernel(name)
        .ok_or_else(|| format!("no kernel {name}"))?
        .precompiled();
    // SAFETY: a kernel compiled at build time from shaders/*.wgsl with the
    // slots wgpu-hal's Metal layout assigns to this layout (build.rs).
    let module = unsafe {
        device.create_shader_module_passthrough(wgpu::ShaderModuleDescriptorPassthrough {
            label: Some(name),
            num_workgroups: (k.workgroup[0], k.workgroup[1], k.workgroup[2]),
            metallib: Some(std::borrow::Cow::Borrowed(k.metallib)),
            ..Default::default()
        })
    };
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some(name),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::StorageTexture {
                    access: wgpu::StorageTextureAccess::WriteOnly,
                    format: out,
                    view_dimension: wgpu::TextureViewDimension::D2,
                },
                count: None,
            },
        ],
    });
    let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(name),
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some(name),
        layout: Some(&pl),
        module: &module,
        entry_point: Some(k.entry),
        compilation_options: Default::default(),
        cache: None,
    });
    Ok((pipeline, layout))
}

pub fn texture_desc(
    w: u32,
    h: u32,
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
) -> wgpu::TextureDescriptor<'static> {
    wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: w.max(1),
            height: h.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage,
        view_formats: &[],
    }
}

/// The shared context, made on first use; the error when there is no GPU.
pub fn gpu() -> Result<MutexGuard<'static, Gpu>, String> {
    let (m, _) = GPU.get_or_init(make).as_ref().map_err(Clone::clone)?;
    Ok(m.lock().unwrap_or_else(|p| p.into_inner()))
}

/// How long the context took, once made.
pub fn startup() -> Option<Startup> {
    GPU.get().and_then(|r| r.as_ref().ok()).map(|(_, s)| *s)
}

/// The context if it has been made (never makes it).
pub fn made() -> Option<MutexGuard<'static, Gpu>> {
    let (m, _) = GPU.get()?.as_ref().ok()?;
    Some(m.lock().unwrap_or_else(|p| p.into_inner()))
}

/// Why a replay failed.
pub enum Failed {
    /// A list could not be read; the rest were drawn.
    List,
    /// The GPU failed; the target is untouched.
    Gpu(String),
}

/// One canvas.
pub struct Canvas {
    pub replayer: Replayer,
    surfaces: Vec<Surface>,
    /// A copy of `previous` when the host passes it as the target too.
    scratch: Option<(wgpu::Texture, wgpu::TextureView)>,
    /// The last replay's milliseconds: CPU encode, GPU, wall.
    pub stats: [f64; 3],
}

impl Canvas {
    pub fn new(w: u32, h: u32, scale: f64) -> Canvas {
        Canvas {
            replayer: Replayer::new(w, h, scale),
            surfaces: Vec::new(),
            scratch: None,
            stats: [0.0; 3],
        }
    }

    /// Replay `lists` over `previous` (null: none) into `target`.
    ///
    /// # Safety
    /// `target` and `previous` are IOSurfaceRefs (or null for `previous`).
    pub unsafe fn replay(
        &mut self,
        target: surface::IOSurfaceRef,
        previous: surface::IOSurfaceRef,
        lists: &[&[u8]],
        host: &mut dyn Host,
    ) -> Result<(), Failed> {
        let t0 = Instant::now();
        let mut frame = Frame {
            scene: Scene::new(),
            keep: !previous.is_null(),
            shadows: Vec::new(),
        };
        self.replayer.begin(&mut frame.scene);
        let mut bad = false;
        for l in lists {
            bad |= self.replayer.apply(l, &mut frame, host).is_err();
        }
        self.replayer.end(&mut frame.scene);
        let encoded = t0.elapsed().as_secs_f64() * 1e3;
        let mut g = gpu().map_err(Failed::Gpu)?;
        let g = &mut *g;
        // SAFETY: the caller's contract.
        let target_ix = unsafe { self.surface(g, target) }.map_err(Failed::Gpu)?;
        let backdrop_ix = if frame.keep {
            // SAFETY: the caller's contract.
            Some(unsafe { self.surface(g, previous) }.map_err(Failed::Gpu)?)
        } else {
            None
        };
        if backdrop_ix == Some(target_ix) {
            self.copy_to_scratch(g, target_ix);
        }
        let scope = g.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let mut outs = Vec::new();
        let mut gpu_ms = 0.0;
        for job in &frame.shadows {
            match shadow(g, job) {
                Ok((out, ms)) => {
                    gpu_ms += ms;
                    let base = wgpu::TexelCopyTextureInfoBase {
                        texture: out.clone(),
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    };
                    g.renderer.override_image(&job.image, Some(base));
                    outs.push(out);
                }
                Err(e) => return Err(Failed::Gpu(e)),
            }
        }
        let (w, h) = (self.replayer.width.max(1), self.replayer.height.max(1));
        let backdrop = match backdrop_ix {
            Some(i) if i == target_ix => &self.scratch.as_ref().expect("scratch").1,
            Some(i) => &self.surfaces[i].view,
            None => &g.empty,
        };
        let params = RenderParams {
            base_color: Color::TRANSPARENT,
            width: w,
            height: h,
            antialiasing_method: AaConfig::Area,
        };
        let result = g.renderer.render_exact(
            &g.device,
            &g.queue,
            &frame.scene,
            &self.surfaces[target_ix].view,
            backdrop,
            true,
            &params,
        );
        for job in &frame.shadows {
            g.renderer.override_image(&job.image, None);
        }
        for out in outs {
            out.destroy();
        }
        let error = block_on(scope.pop());
        let stats = result.map_err(|e| Failed::Gpu(format!("vello: {e}")))?;
        if let Some(e) = error {
            return Err(Failed::Gpu(format!("wgpu: {e}")));
        }
        gpu_ms += stats.gpu_ms;
        self.stats = [
            encoded + stats.encode_ms,
            gpu_ms,
            t0.elapsed().as_secs_f64() * 1e3,
        ];
        if bad {
            Err(Failed::List)
        } else {
            Ok(())
        }
    }

    /// The texture for `s`, wrapped the first time it is seen.
    ///
    /// # Safety
    /// `s` is a live IOSurfaceRef.
    unsafe fn surface(&mut self, g: &Gpu, s: surface::IOSurfaceRef) -> Result<usize, String> {
        if let Some(i) = self.surfaces.iter().position(|x| x.is(s)) {
            return Ok(i);
        }
        // SAFETY: the caller's contract.
        let wrapped = unsafe { Surface::wrap(&g.device, s) }?;
        // The host passes at most three a canvas; a fourth replaces the
        // oldest.
        if self.surfaces.len() >= 4 {
            self.surfaces.remove(0);
        }
        self.surfaces.push(wrapped);
        Ok(self.surfaces.len() - 1)
    }

    fn copy_to_scratch(&mut self, g: &Gpu, from: usize) {
        let s = &self.surfaces[from];
        let size = s.texture.size();
        if self.scratch.as_ref().is_none_or(|(t, _)| t.size() != size) {
            let t = g.device.create_texture(&texture_desc(
                size.width,
                size.height,
                wgpu::TextureFormat::Bgra8Unorm,
                wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            ));
            let v = t.create_view(&Default::default());
            self.scratch = Some((t, v));
        }
        let mut enc = g.device.create_command_encoder(&Default::default());
        enc.copy_texture_to_texture(
            s.texture.as_image_copy(),
            self.scratch.as_ref().expect("scratch").0.as_image_copy(),
            size,
        );
        g.queue.submit([enc.finish()]);
    }

    /// Bytes this canvas holds of its own (the scratch copy).
    pub fn bytes(&self) -> u64 {
        self.scratch
            .as_ref()
            .map_or(0, |(t, _)| u64::from(t.width()) * u64::from(t.height()) * 4)
    }
}

/// Draw a shadow job's paint offscreen, blur it and colour it; the texture
/// (premultiplied RGBA8) and the GPU milliseconds.
fn shadow(g: &mut Gpu, job: &ShadowJob) -> Result<(wgpu::Texture, f64), String> {
    use wgpu::TextureUsages as U;
    let (w, h) = (job.width, job.height);
    let coverage = g.device.create_texture(&texture_desc(
        w,
        h,
        wgpu::TextureFormat::Rgba8Unorm,
        U::STORAGE_BINDING | U::TEXTURE_BINDING,
    ));
    let tmp = g.device.create_texture(&texture_desc(
        w,
        h,
        wgpu::TextureFormat::R32Float,
        U::STORAGE_BINDING | U::TEXTURE_BINDING,
    ));
    let out = g.device.create_texture(&texture_desc(
        w,
        h,
        wgpu::TextureFormat::Rgba8Unorm,
        U::STORAGE_BINDING | U::TEXTURE_BINDING | U::COPY_SRC,
    ));
    let cv = coverage.create_view(&Default::default());
    let params = RenderParams {
        base_color: Color::TRANSPARENT,
        width: w,
        height: h,
        antialiasing_method: AaConfig::Area,
    };
    let stats = g
        .renderer
        .render_exact(
            &g.device, &g.queue, &job.scene, &cv, &g.empty, false, &params,
        )
        .map_err(|e| format!("vello shadow: {e}"))?;
    let mut uniform = [0u8; 32];
    for (i, v) in job.color.iter().enumerate() {
        uniform[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
    }
    uniform[16..20].copy_from_slice(&(job.sigma as f32).to_le_bytes());
    let radius = if job.sigma > 0.0 {
        (3.0 * job.sigma).ceil() as i32
    } else {
        0
    };
    uniform[20..24].copy_from_slice(&radius.to_le_bytes());
    let buf = g.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("shadow params"),
        size: 32,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    g.queue.write_buffer(&buf, 0, &uniform);
    let tv = tmp.create_view(&Default::default());
    let ov = out.create_view(&Default::default());
    let mut enc = g.device.create_command_encoder(&Default::default());
    for (i, (src, dst)) in [(&cv, &tv), (&tv, &ov)].into_iter().enumerate() {
        let (pipeline, layout) = &g.blur[i];
        let bind = g.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(src),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(dst),
                },
            ],
        });
        let mut pass = enc.begin_compute_pass(&Default::default());
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &bind, &[]);
        pass.dispatch_workgroups(w.div_ceil(16), h.div_ceil(16), 1);
    }
    g.queue.submit([enc.finish()]);
    Ok((out, stats.gpu_ms))
}
