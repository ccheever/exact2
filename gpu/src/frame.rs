//! One submit per tick (LLP 1009 D7): every canvas the host renders records
//! into one encoder the module owns; [`Module::flush`] submits it once and
//! then presents each canvas's drawable.
//!
//! On Metal a submit commits wgpu's pending writes and the frame's commands,
//! and each present commits one more command buffer: three per canvas when
//! each submitted alone, two plus one per canvas here. Any other call that
//! touches a canvas already in the open frame flushes it first, so what a
//! host sees about that canvas is what it saw with a submit per render.

use crate::{shaders, Frame, Module, SurfaceError};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

/// The tick's recorded commands and the canvases that recorded them.
pub(crate) struct Open {
    encoder: wgpu::CommandEncoder,
    canvases: Vec<Rendered>,
}

/// A canvas rendered into the open frame.
struct Rendered {
    id: u32,
    complete: bool,
    /// Its drawable, presented after the submit; `None` when the surface
    /// failed after recording — then only kept alive until the submit, since
    /// the recorded commands still name its texture, and discarded.
    texture: Option<wgpu::SurfaceTexture>,
    /// Kept alive, unpresented, until after the submit.
    failed: Option<wgpu::SurfaceTexture>,
    /// Ask for its next drawable off the presenter's thread once presented.
    request: bool,
    /// The buffer it rendered into, for a canvas without a window
    /// (`crate::buffers`), and whether the frame is published once the
    /// submit's work is done; the buffer is busy until then either way.
    buffer: Option<(usize, bool)>,
}

impl Module {
    /// Whether `id` rendered into the frame not yet submitted.
    fn in_open_frame(&self, id: u32) -> bool {
        self.open
            .as_ref()
            .is_some_and(|o| o.canvases.iter().any(|c| c.id == id))
    }

    /// Whether `id`'s last render drew into the frame not yet submitted. A render that went without
    /// a drawable, or whose drawable request timed out or found its window occluded, drew nothing.
    pub fn drew(&self, id: u32) -> bool {
        self.in_open_frame(id)
    }

    /// Flush the open frame when `id` is in it: a call about that canvas
    /// sees it submitted and its present issued (on Android, perhaps still
    /// queued on the canvas's acquiring thread), as without batching.
    pub(crate) fn settle(&mut self, id: u32) {
        if self.in_open_frame(id) {
            self.flush();
        }
    }

    /// The canvas's raster relative to its device pixels: the surface's
    /// `render_scale` when finite in (0, 1], else 1. A host sizing the
    /// drawable itself (a web canvas's backing store) multiplies by it.
    pub fn render_scale(&self, id: u32) -> f32 {
        let scale = self
            .instances
            .get(&id)
            .map_or(1.0, |inst| inst.surface.render_scale());
        if scale.is_finite() && scale > 0.0 && scale <= 1.0 {
            scale
        } else {
            1.0
        }
    }

    /// Record one frame for a canvas at the given size into the open frame;
    /// returns whether the surface wants another. None with no error means no
    /// device/target. Nothing happens before the first bind. Nothing reaches
    /// the screen until [`Module::flush`].
    pub fn render(&mut self, id: u32, frame: &Frame) -> Option<bool> {
        self.check_device();
        // A canvas renders once per frame: a second render submits the first.
        self.settle(id);
        let scale = self.render_scale(id);
        let Some(inst) = self.instances.get_mut(&id) else {
            return self.fail("no such canvas");
        };
        let frame = &Frame {
            scale: frame.scale * scale,
            ..*frame
        };
        let (w, h) = frame.pixels();
        let gpu = self.gpu.as_ref()?;
        if !inst.bound {
            return Some(false);
        }
        #[cfg(target_os = "android")]
        if inst.ring.is_some() {
            return self.render_to_buffer(id, frame);
        }
        let target = inst.presentation.as_ref()?;
        let config = inst.config.as_mut()?;
        // Configuring again needs nothing of the surface acquired.
        #[cfg(not(target_arch = "wasm32"))]
        let mut idle = || inst.acquire.reconfigure();
        #[cfg(target_arch = "wasm32")]
        let idle = || true;
        let alpha = crate::target::alpha_mode(inst.surface.opaque(), &inst.alpha_modes);
        if config.alpha_mode != alpha {
            if !idle() {
                return Some(true);
            }
            inst.held = None;
            config.alpha_mode = alpha;
            crate::swapchains(|| target.configure(&gpu.device, config));
        }
        let format = crate::view_format(config);
        inst.surface.prepare_assets(&gpu.device, &gpu.queue, format);
        // Off the presenter's thread (`acquire`) unless the clock is the
        // agent's, whose frames are rendered when asked for.
        #[cfg(not(target_arch = "wasm32"))]
        let off_thread = crate::acquire::Acquire::ENABLED && !self.seekable;
        #[cfg(target_arch = "wasm32")]
        let off_thread = false;
        if config.width != w || config.height != h {
            if !idle() {
                return Some(true);
            }
            inst.held = None;
            config.width = w;
            config.height = h;
            crate::swapchains(|| target.configure(&gpu.device, config));
        }
        use wgpu::CurrentSurfaceTexture as Current;
        // The drawable an incomplete frame was submitted into.
        let retry = inst.held.is_some();
        let current = if let Some(texture) = inst.held.take() {
            Current::Success(texture)
        } else {
            #[cfg(not(target_arch = "wasm32"))]
            let current = if off_thread {
                inst.acquire.request(target);
                match inst.acquire.take_or_starve() {
                    Some(current) => current,
                    // Not released by the compositor yet: this canvas keeps its
                    // last frame, and its inputs stay dirty.
                    None => return Some(true),
                }
            } else {
                match inst.acquire.take(true) {
                    Some(current) => current,
                    None => target.get_current_texture(),
                }
            };
            #[cfg(target_arch = "wasm32")]
            let current = target.get_current_texture();
            current
        };
        let texture = match current {
            Current::Success(t) => t,
            Current::Suboptimal(t) => {
                drop(t);
                #[cfg(not(target_arch = "wasm32"))]
                {
                    inst.acquire.reconfigure();
                }
                crate::swapchains(|| target.configure(&gpu.device, config));
                return Some(true);
            }
            // Nothing to draw into this frame; the inputs stay dirty.
            Current::Timeout | Current::Occluded => return Some(true),
            Current::Lost => {
                self.lose_device();
                return None;
            }
            other => {
                self.error = format!("surface: {other:?}");
                return None;
            }
        };
        let view = texture.texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(format),
            ..Default::default()
        });
        // Until a frame is complete its drawable is not written again:
        // wgpu-hal's Vulkan swapchain signals one more semaphore for each
        // submission that writes an image before its present. Retries render
        // offscreen and a complete one is copied in, so a drawable takes two
        // submissions at most, whatever a pending surface does.
        #[cfg(not(target_arch = "wasm32"))]
        let scratch = if retry {
            Some(scratch(
                &mut inst.scratch,
                &gpu.device,
                format,
                (config.width, config.height),
            ))
        } else {
            inst.scratch = None;
            None
        };
        // The browser shows every frame: none is held.
        #[cfg(target_arch = "wasm32")]
        let scratch: Option<wgpu::TextureView> = {
            let _ = retry;
            None
        };
        let frame = Frame {
            seekable: self.seekable,
            period_ms: self.period_ms,
            children_generation: inst.children_generation,
            shader_generation: shaders::shader_generation(),
            headroom: if inst.hdr { inst.headroom } else { 1.0 },
            ..*frame
        };
        let open = self.open.get_or_insert_with(|| Open {
            encoder: gpu
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("exact canvases"),
                }),
            canvases: Vec::new(),
        });
        let wants = inst.surface.render(
            &frame,
            &gpu.device,
            &gpu.queue,
            &mut open.encoder,
            scratch.as_ref().unwrap_or(&view),
            format,
        );
        inst.drain();
        let failure = inst.surface.take_error();
        let ready = cfg!(target_arch = "wasm32") || inst.surface.frame_ready();
        #[cfg(not(target_arch = "wasm32"))]
        if let (Some(from), Some((_, blitter)), true, None) =
            (&scratch, &inst.scratch, ready, &failure)
        {
            blitter.copy(&gpu.device, &mut open.encoder, from, &view);
            inst.scratch = None;
        }
        let (texture, failed) = if failure.is_some() {
            (None, Some(texture))
        } else if ready {
            (Some(texture), None)
        } else {
            inst.held = Some(texture);
            (None, None)
        };
        open.canvases.push(Rendered {
            id,
            complete: ready,
            texture,
            failed,
            request: off_thread && wants && ready,
            buffer: None,
        });
        if let Some(SurfaceError(e)) = failure {
            self.error = e;
            return None;
        }
        inst.dirty = !ready;
        Some(wants || !ready)
    }

    /// [`Module::render`] for a canvas with buffers: the frame goes into
    /// the ring's next one.
    #[cfg(target_os = "android")]
    fn render_to_buffer(&mut self, id: u32, frame: &Frame) -> Option<bool> {
        let (w, h) = frame.pixels();
        let gpu = self.gpu.as_ref()?;
        let inst = self.instances.get_mut(&id)?;
        if inst
            .ring
            .as_ref()
            .is_some_and(|r| r.size != (w.max(1), h.max(1)))
        {
            inst.ring = crate::buffers::Ring::new(gpu, w, h, inst.offscreen_format());
        }
        // The GPU's word on the frames it has finished, before counting them.
        let _ = gpu.device.poll(wgpu::PollType::Poll);
        let ring = inst.ring.as_mut()?;
        ring.settle();
        // As many frames on the GPU as it may hold. The agent's clock renders
        // each frame when asked for, so its frame waits for the GPU.
        if self.seekable && ring.in_flight() >= crate::buffers::IN_FLIGHT {
            let _ = gpu.device.poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: None,
            });
            ring.settle();
        }
        // A live frame waits a display frame, and the surface takes this
        // one's share of its work now; so does the agent's when the wait
        // freed no slot. No frame renders into a slot in use.
        let target = (ring.in_flight() < crate::buffers::IN_FLIGHT)
            .then(|| ring.target())
            .flatten();
        let format = ring.format;
        let Some((slot, view)) = target else {
            inst.surface.advance(frame, &gpu.device, &gpu.queue);
            return Some(true);
        };
        inst.surface.prepare_assets(&gpu.device, &gpu.queue, format);
        let frame = Frame {
            seekable: self.seekable,
            period_ms: self.period_ms,
            children_generation: inst.children_generation,
            shader_generation: shaders::shader_generation(),
            headroom: 1.0,
            ..*frame
        };
        let open = self.open.get_or_insert_with(|| Open {
            encoder: gpu
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("exact canvases"),
                }),
            canvases: Vec::new(),
        });
        let wants = inst.surface.render(
            &frame,
            &gpu.device,
            &gpu.queue,
            &mut open.encoder,
            &view,
            format,
        );
        inst.drain();
        let failure = inst.surface.take_error();
        let ready = inst.surface.frame_ready();
        open.canvases.push(Rendered {
            id,
            complete: ready,
            texture: None,
            failed: None,
            request: false,
            buffer: Some((slot, failure.is_none() && ready)),
        });
        if let Some(SurfaceError(e)) = failure {
            self.error = e;
            return None;
        }
        inst.dirty = !ready;
        Some(wants || !ready)
    }

    /// Submit the open frame once, then present each canvas's drawable in
    /// the order rendered; on Android, a canvas that asks for its next one
    /// is presented on its acquiring thread instead, after this returns and
    /// in no order with the others. `false`, with the reason in
    /// [`Module::take_error`], when a surface failed after its commands were
    /// submitted; nothing open is `true`.
    pub fn flush(&mut self) -> bool {
        self.check_device();
        self.reap();
        let Some(open) = self.open.take() else {
            return true;
        };
        let Some(gpu) = self.gpu.as_ref() else {
            return true;
        };
        // On Metal the frame's presentations ride its one submit, on the
        // last command buffer that encoded anything (the patched wgpu-hal,
        // vendor/wgpu-hal/EXACT-PATCHES.md): one committed command buffer a
        // canvas frame, as an `MTKView` draw is. Not when a surface failed
        // after recording: its drawable is in the submit, and must not be
        // shown.
        // Canvases whose first frame this is (`Module::seen`).
        let first: Vec<Arc<AtomicBool>> = open
            .canvases
            .iter()
            .filter(|c| c.complete && (c.texture.is_some() || c.buffer.is_some_and(|(_, p)| p)))
            .filter_map(|c| self.instances.get(&c.id))
            .filter(|i| !i.seen.load(Ordering::Acquire))
            .map(|i| i.seen.clone())
            .collect();
        #[cfg(any(target_os = "macos", target_os = "ios", target_os = "tvos"))]
        let mut first = first;
        #[cfg(any(target_os = "macos", target_os = "ios", target_os = "tvos"))]
        let rode = {
            let ride = open
                .canvases
                .iter()
                .all(|c| c.failed.is_none() && c.complete);
            let drawn = open.canvases.iter().filter(|c| c.texture.is_some()).count();
            let before = ride
                .then(|| ride_next_submit(&gpu.queue, std::mem::take(&mut first)))
                .flatten();
            gpu.queue.submit([open.encoder.finish()]);
            // Every drawable of the frame, or none: a surface that drew
            // nothing into its target is in no command buffer, and then each
            // is presented the ordinary way (one already scheduled is not
            // scheduled twice).
            before.is_some_and(|n| presented_with_submit(&gpu.queue) == Some(n + drawn))
        };
        #[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "tvos")))]
        let rode = {
            gpu.queue.submit([open.encoder.finish()]);
            false
        };
        let mut ok = true;
        for c in open.canvases {
            // Android presents a canvas that asks for its next texture on its
            // acquiring thread, just before it acquires that one. A canvas's
            // first frame is presented here, before the module counts it seen.
            #[cfg(target_os = "android")]
            let mut later = None;
            #[cfg(target_os = "android")]
            let seen = self
                .instances
                .get(&c.id)
                .is_some_and(|i| i.seen.load(Ordering::Acquire));
            if let Some(texture) = c.texture {
                if rode {
                    // Scheduled by the submit. Letting the texture go tells
                    // wgpu the canvas may acquire its next one; `present`
                    // would make wgpu-core submit once more, empty, which
                    // commits a command buffer only to signal it.
                    drop(texture);
                } else {
                    #[cfg(target_os = "android")]
                    if c.request && seen {
                        later = Some(texture);
                    } else {
                        gpu.queue.present(texture);
                    }
                    #[cfg(not(target_os = "android"))]
                    gpu.queue.present(texture);
                }
            }
            drop(c.failed);
            let Some(inst) = self.instances.get_mut(&c.id) else {
                #[cfg(target_os = "android")]
                if let Some(texture) = later {
                    gpu.queue.present(texture);
                }
                continue;
            };
            #[cfg(target_os = "android")]
            if let (Some((slot, publish)), Some(ring)) = (c.buffer, inst.ring.as_mut()) {
                ring.submitted(slot, publish, &gpu.queue);
            }
            #[cfg(not(target_os = "android"))]
            let _ = c.buffer;
            inst.surface.submitted();
            inst.drain();
            if let Some(SurfaceError(e)) = inst.surface.take_error() {
                self.error = e;
                ok = false;
            }
            // The next drawable, waited for while this frame is composited;
            // the presenter hears when it lands.
            #[cfg(not(target_arch = "wasm32"))]
            if c.request {
                #[cfg(target_os = "android")]
                match (later, &inst.presentation) {
                    (Some(texture), Some(target)) => inst
                        .acquire
                        .present_then_request(target, &gpu.queue, texture),
                    (later, target) => {
                        if let Some(texture) = later {
                            gpu.queue.present(texture);
                        }
                        if let Some(target) = target {
                            inst.acquire.request_awaited(target);
                        }
                    }
                }
                #[cfg(not(target_os = "android"))]
                if let Some(target) = &inst.presentation {
                    inst.acquire.request_awaited(target);
                }
            }
            #[cfg(target_arch = "wasm32")]
            let _ = c.request;
        }
        // Presented the ordinary way, or not on Metal: shown as far as the
        // module can tell.
        if !first.is_empty() {
            shown(first);
        }
        ok
    }

    /// Whether a canvas's first frame has been handed to the compositor: a
    /// presenter that reuses a target (a pooled `CAMetalLayer` still holds
    /// the last picture presented to it) keeps it hidden until then.
    pub fn seen(&self, id: u32) -> bool {
        self.instances
            .get(&id)
            .is_some_and(|i| i.seen.load(Ordering::Acquire))
    }
}

/// The view of `slot`'s texture at `format` and `size`, made (with what
/// copies it into a drawable) when it does not exist or differs.
#[cfg(not(target_arch = "wasm32"))]
fn scratch(
    slot: &mut Option<(wgpu::Texture, wgpu::util::TextureBlitter)>,
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    (width, height): (u32, u32),
) -> wgpu::TextureView {
    if slot
        .as_ref()
        .is_none_or(|(t, _)| t.format() != format || (t.width(), t.height()) != (width, height))
    {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("exact canvas incomplete frame"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        *slot = Some((texture, wgpu::util::TextureBlitter::new(device, format)));
    }
    slot.as_ref().unwrap().0.create_view(&Default::default())
}

/// The presenter's callback for a canvas's first frame (`gpu_on_presented`),
/// as an address; 0 when none is registered. On Metal it runs on the thread
/// that scheduled the frame's command buffer.
static SHOWN: AtomicUsize = AtomicUsize::new(0);

/// Register (or, with `None`, remove) the presenter's callback.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn on_presented(callback: Option<extern "C" fn()>) {
    SHOWN.store(callback.map_or(0, |f| f as usize), Ordering::SeqCst);
}

/// These canvases' first frames are with the compositor: say so, and tell
/// the presenter.
fn shown(first: Vec<Arc<AtomicBool>>) {
    for seen in first {
        seen.store(true, Ordering::Release);
    }
    let address = SHOWN.load(Ordering::SeqCst);
    if address != 0 {
        // SAFETY: only `on_presented` stores here, and only an `extern "C" fn()`.
        let callback: extern "C" fn() = unsafe { std::mem::transmute(address) };
        callback();
    }
}

/// Ask the Metal queue to present, with its next submit, what that submit
/// draws (the patched wgpu-hal), and to say when that command buffer has
/// been scheduled if canvases in it draw their `first` frame; how many
/// drawables it has presented that way so far. `None` off Metal, with
/// `first` shown at once.
#[cfg(any(target_os = "macos", target_os = "ios", target_os = "tvos"))]
fn ride_next_submit(queue: &wgpu::Queue, first: Vec<Arc<AtomicBool>>) -> Option<usize> {
    // SAFETY: a flag, a callback and a counter of the queue wgpu owns;
    // nothing is encoded.
    let Some(q) = (unsafe { queue.as_hal::<wgpu::hal::api::Metal>() }) else {
        if !first.is_empty() {
            shown(first);
        }
        return None;
    };
    q.present_with_next_submit(true);
    if !first.is_empty() {
        q.on_next_submit_scheduled(Box::new(move || shown(first)));
    }
    Some(q.counts().presented_with_submit)
}

/// How many drawables this queue has presented with the submit that drew
/// them; `None` off Metal.
#[cfg(any(target_os = "macos", target_os = "ios", target_os = "tvos"))]
fn presented_with_submit(queue: &wgpu::Queue) -> Option<usize> {
    // SAFETY: a counter of the queue wgpu owns is read; nothing is encoded.
    unsafe { queue.as_hal::<wgpu::hal::api::Metal>() }.map(|q| q.counts().presented_with_submit)
}
