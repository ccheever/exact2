//! Android presentation: vello renders the frame into the window's Vulkan
//! swapchain instead of a texture read back to the CPU. The host's render
//! thread names the window (`set_window`) before the painter is created; the
//! device is then requested compatible with that surface.
//!
//! Rendering is pipelined, as Android's own UI thread / RenderThread split
//! and Flutter's UI / raster threads are: the host thread walks the tree into
//! a vello scene, hands it to a present thread, and walks the next frame
//! while the present thread encodes (vello's CPU resolve), submits and
//! presents this one. The handoff is one deep: a frame waits for the
//! previous one's render before it is handed over, so at most one frame is
//! in flight and image (un)registration stays ordered against renders. The
//! scene comes back for reuse.
//!
//! A swapchain image that takes storage writes in `Rgba8Unorm` is vello's
//! target directly; otherwise vello's own texture is blitted into it on the
//! host thread (no pipelining). The presenter still gets a frame, a 1×1
//! placeholder: nothing reads these pixels back (no VNC, no screenshot, no
//! CPU damage reuse).
//!
//! @ref LLP 1076 §3.4 (reuse Exact's Rust painter; present without a
//! per-frame CPU readback)

use super::*;
use raw_window_handle::{
    AndroidDisplayHandle, AndroidNdkWindowHandle, RawDisplayHandle, RawWindowHandle,
};
use std::cell::Cell;
use std::ffi::c_void;
use std::ptr::NonNull;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Mutex;

thread_local! {
    static WINDOW: Cell<Option<(NonNull<c_void>, u32, u32)>> = const { Cell::new(None) };
}

/// The `ANativeWindow` the next painter created on this thread presents to,
/// with its size in pixels. The caller holds a reference to the window for
/// as long as the painter lives.
pub fn set_window(window: *mut c_void, width: u32, height: u32) {
    WINDOW.set(NonNull::new(window).map(|w| (w, width, height)));
}

/// The window named on this thread, for a custom painter that presents
/// itself (`presenter::set_custom_painter`).
pub fn current_window() -> Option<(*mut c_void, u32, u32)> {
    WINDOW
        .get()
        .map(|(w, width, height)| (w.as_ptr(), width, height))
}

struct Job {
    scene: vello::Scene,
    /// Images no longer drawn, unregistered once this job has rendered.
    retired: Vec<vello::peniko::ImageData>,
    width: u32,
    height: u32,
}

struct Worker {
    jobs: Sender<Job>,
    done: Receiver<vello::Scene>,
    in_flight: bool,
}

pub(super) struct Presenting {
    /// The surface, when frames render on this thread (the blit path).
    surface: Option<(wgpu::Surface<'static>, wgpu::SurfaceConfiguration)>,
    blitter: Option<wgpu::util::TextureBlitter>,
    worker: Option<Worker>,
}

/// The surface for the window named on this thread, if any.
pub(super) fn surface(
    instance: &wgpu::Instance,
) -> Result<Option<(wgpu::Surface<'static>, u32, u32)>, String> {
    let Some((window, width, height)) = WINDOW.get() else {
        return Ok(None);
    };
    // SAFETY: the window is a live ANativeWindow whose reference the host
    // holds until after the painter (and so this surface) is dropped.
    let surface = unsafe {
        instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
            raw_display_handle: Some(RawDisplayHandle::Android(AndroidDisplayHandle::new())),
            raw_window_handle: RawWindowHandle::AndroidNdk(AndroidNdkWindowHandle::new(window)),
        })
    }
    .map_err(|e| format!("surface: {e}"))?;
    Ok(Some((surface, width, height)))
}

fn params(width: u32, height: u32) -> RenderParams {
    RenderParams {
        base_color: Color::WHITE,
        width,
        height,
        antialiasing_method: AaConfig::Area,
    }
}

/// Acquire the next image, reconfiguring a stale surface; `None` skips the frame.
fn acquire(
    surface: &wgpu::Surface<'static>,
    config: &mut wgpu::SurfaceConfiguration,
    device: &wgpu::Device,
    (width, height): (u32, u32),
) -> Option<wgpu::SurfaceTexture> {
    if config.width != width || config.height != height {
        config.width = width;
        config.height = height;
        surface.configure(device, config);
    }
    match crate::android::trace(c"exact acquire", || surface.get_current_texture()) {
        wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => {
            Some(t)
        }
        wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
            surface.configure(device, config);
            None
        }
        _ => None,
    }
}

/// vello's own target when the swapchain is not written directly: the
/// present thread then copies it into the image with a render pass.
fn intermediate(device: &wgpu::Device, (width, height): (u32, u32)) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("exact frame"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
        .create_view(&Default::default())
}

/// The present thread: render each scene into the next swapchain image
/// (directly, or into vello's own texture and then blitted with `blit`).
#[allow(clippy::too_many_arguments)]
fn work(
    surface: wgpu::Surface<'static>,
    mut config: wgpu::SurfaceConfiguration,
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: Arc<Mutex<Renderer>>,
    blit: Option<wgpu::util::TextureBlitter>,
    jobs: Receiver<Job>,
    done: Sender<vello::Scene>,
) {
    let mut own: Option<((u32, u32), wgpu::TextureView)> = None;
    for job in jobs {
        let size = (job.width, job.height);
        if let Some(frame) = acquire(&surface, &mut config, &device, size) {
            let swap = frame.texture.create_view(&Default::default());
            let view = match &blit {
                None => swap.clone(),
                Some(_) => {
                    if own.as_ref().is_none_or(|(s, _)| *s != size) {
                        own = Some((size, intermediate(&device, size)));
                    }
                    own.as_ref().expect("made").1.clone()
                }
            };
            let rendered = crate::android::trace(c"exact vello", || {
                renderer.lock().unwrap().render_to_texture(
                    &device,
                    &queue,
                    &job.scene,
                    &view,
                    &params(job.width, job.height),
                )
            });
            if let (Some(blit), Ok(())) = (&blit, &rendered) {
                crate::android::trace(c"exact blit", || {
                    let mut encoder = device.create_command_encoder(&Default::default());
                    blit.copy(&device, &mut encoder, &view, &swap);
                    queue.submit([encoder.finish()]);
                });
            }
            match rendered {
                Ok(()) => crate::android::trace(c"exact present", || frame.present()),
                Err(e) => eprintln!("exact: vello render: {e}"),
            }
        }
        {
            let mut renderer = renderer.lock().unwrap();
            for image in job.retired {
                renderer.unregister_texture(image);
            }
        }
        if done.send(job.scene).is_err() {
            break;
        }
    }
}

impl Presenting {
    pub(super) fn new(
        surface: wgpu::Surface<'static>,
        (width, height): (u32, u32),
        adapter: &wgpu::Adapter,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        renderer: &Arc<Mutex<Renderer>>,
    ) -> Result<Presenting, String> {
        let caps = surface.get_capabilities(adapter);
        let rgba = wgpu::TextureFormat::Rgba8Unorm;
        let format = if caps.formats.contains(&rgba) {
            rgba
        } else {
            *caps
                .formats
                .iter()
                .find(|f| !f.is_srgb())
                .or(caps.formats.first())
                .ok_or("surface: no formats")?
        };
        let storage = format == rgba && caps.usages.contains(wgpu::TextureUsages::STORAGE_BINDING);
        // `EXACT_GPU_EXPERIMENT=blit`: keep storage usage off the swapchain
        // (on Adreno it may cost the image its compressed layout).
        let direct = storage && experiment() != Some("blit");
        let usage = if direct {
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::STORAGE_BINDING
        } else {
            wgpu::TextureUsages::RENDER_ATTACHMENT
        };
        let alpha_mode = if caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::Opaque) {
            wgpu::CompositeAlphaMode::Opaque
        } else {
            caps.alpha_modes[0]
        };
        let config = wgpu::SurfaceConfiguration {
            usage,
            format,
            width: width.max(1),
            height: height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode,
            view_formats: vec![],
        };
        surface.configure(device, &config);
        eprintln!(
            "exact: presenting {width}x{height} {format:?}, {}",
            if direct {
                "vello writes the swapchain on a present thread"
            } else {
                "vello's texture blitted on the host thread"
            }
        );
        // Without storage writes on the swapchain (some GPUs offer none in
        // RGBA8), the present thread blits vello's own texture instead; the
        // host-thread blit path remains only for an explicit experiment.
        if experiment() == Some("hostblit") {
            return Ok(Presenting {
                surface: Some((surface, config)),
                blitter: Some(wgpu::util::TextureBlitter::new(device, format)),
                worker: None,
            });
        }
        let (jobs, rx) = channel();
        let (tx, done) = channel();
        let blit = (!direct).then(|| wgpu::util::TextureBlitter::new(device, format));
        let (device, queue, renderer) = (device.clone(), queue.clone(), renderer.clone());
        std::thread::Builder::new()
            .name("exact-present".into())
            .spawn(move || work(surface, config, device, queue, renderer, blit, rx, tx))
            .map_err(|e| format!("present thread: {e}"))?;
        Ok(Presenting {
            surface: None,
            blitter: None,
            worker: Some(Worker {
                jobs,
                done,
                in_flight: false,
            }),
        })
    }
}

impl Gpu {
    /// Whether frames go to a window rather than back to the CPU.
    pub(super) fn presents(&self) -> bool {
        self.present.is_some()
    }

    /// Present the scene painted this frame: handed to the present thread
    /// (after the previous frame's render), or rendered and blitted here.
    pub(super) fn present_frame(&mut self) -> Result<Pixmap, String> {
        let width = ((self.width * self.scale).round() as u32).max(1);
        let height = ((self.height * self.scale).round() as u32).max(1);
        let placeholder = || Pixmap::new(1, 1).ok_or_else(|| "placeholder".to_string());
        let p = self.present.as_mut().expect("presents");
        if let Some(worker) = p.worker.as_mut() {
            let t0 = Instant::now();
            let recycled = if worker.in_flight {
                crate::android::trace(c"exact wait render", || worker.done.recv())
                    .map_err(|_| "present thread ended".to_string())?
            } else {
                vello::Scene::new()
            };
            let mut scene = std::mem::replace(&mut self.scene, recycled);
            if experiment() == Some("empty") {
                // Benchmark experiment: the pipeline's fixed cost alone.
                scene.reset();
            }
            worker
                .jobs
                .send(Job {
                    scene,
                    retired: std::mem::take(&mut self.retired),
                    width,
                    height,
                })
                .map_err(|_| "present thread ended".to_string())?;
            worker.in_flight = true;
            self.last_ms = (t0.elapsed().as_secs_f64() * 1000.0, 0.0);
            return placeholder();
        }
        let scene = std::mem::take(&mut self.scene);
        let result = self.render_blit(&scene, (width, height));
        {
            let mut renderer = self.renderer.lock().unwrap();
            for image in std::mem::take(&mut self.retired) {
                renderer.unregister_texture(image);
            }
        }
        self.scene = scene;
        result.and_then(|_| placeholder())
    }

    fn render_blit(&mut self, scene: &vello::Scene, size: (u32, u32)) -> Result<(), String> {
        let (device, queue) = (self.device.clone(), self.queue.clone());
        let view = self.target(size.0, size.1).view.clone();
        self.renderer
            .lock()
            .unwrap()
            .render_to_texture(&device, &queue, scene, &view, &params(size.0, size.1))
            .map_err(|e| format!("vello render: {e}"))?;
        let p = self.present.as_mut().expect("presents");
        let (surface, config) = p.surface.as_mut().expect("blit path");
        let Some(frame) = acquire(surface, config, &device, size) else {
            return Ok(());
        };
        let swap_view = frame.texture.create_view(&Default::default());
        let mut encoder = device.create_command_encoder(&Default::default());
        p.blitter
            .as_ref()
            .expect("blit path")
            .copy(&device, &mut encoder, &view, &swap_view);
        queue.submit([encoder.finish()]);
        frame.present();
        Ok(())
    }
}

/// `EXACT_GPU_EXPERIMENT` (`empty`, `notext`): isolating the painter's GPU
/// cost on a device; unset in ordinary runs.
pub(super) fn experiment() -> Option<&'static str> {
    static VALUE: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
    VALUE
        .get_or_init(|| {
            std::env::var("EXACT_GPU_EXPERIMENT")
                .ok()
                .filter(|v| !v.is_empty())
        })
        .as_deref()
}
