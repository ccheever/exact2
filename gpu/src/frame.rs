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

/// The tick's recorded commands and the canvases that recorded them.
pub(crate) struct Open {
    encoder: wgpu::CommandEncoder,
    canvases: Vec<Rendered>,
}

/// A canvas rendered into the open frame.
struct Rendered {
    id: u32,
    /// Its drawable, presented after the submit; `None` when the surface
    /// failed after recording — then only kept alive until the submit, since
    /// the recorded commands still name its texture, and discarded.
    texture: Option<wgpu::SurfaceTexture>,
    /// Kept alive, unpresented, until after the submit.
    failed: Option<wgpu::SurfaceTexture>,
    /// Ask for its next drawable off the presenter's thread once presented.
    request: bool,
}

impl Module {
    /// Whether `id` rendered into the frame not yet submitted.
    fn in_open_frame(&self, id: u32) -> bool {
        self.open
            .as_ref()
            .is_some_and(|o| o.canvases.iter().any(|c| c.id == id))
    }

    /// Flush the open frame when `id` is in it: a call about that canvas
    /// sees it submitted and presented, as it would have without batching.
    pub(crate) fn settle(&mut self, id: u32) {
        if self.in_open_frame(id) {
            self.flush();
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
        let (w, h) = frame.pixels();
        let Some(inst) = self.instances.get_mut(&id) else {
            return self.fail("no such canvas");
        };
        let gpu = self.gpu.as_ref()?;
        if !inst.bound {
            return Some(false);
        }
        let target = inst.presentation.as_ref()?;
        let config = inst.config.as_mut()?;
        inst.surface
            .prepare_assets(&gpu.device, &gpu.queue, config.format);
        // Off the presenter's thread (`acquire`) unless the clock is the
        // agent's, whose frames are rendered when asked for.
        #[cfg(not(target_arch = "wasm32"))]
        let off_thread = crate::acquire::Acquire::ENABLED && !self.seekable;
        #[cfg(target_arch = "wasm32")]
        let off_thread = false;
        if config.width != w || config.height != h {
            // The texture in flight was acquired at the old size.
            #[cfg(not(target_arch = "wasm32"))]
            drop(inst.acquire.take(true));
            config.width = w;
            config.height = h;
            target.configure(&gpu.device, config);
        }
        use wgpu::CurrentSurfaceTexture as Current;
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
        let texture = match current {
            Current::Success(t) => t,
            Current::Suboptimal(t) => {
                target.configure(&gpu.device, config);
                t
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
        let view = texture.texture.create_view(&Default::default());
        let frame = Frame {
            seekable: self.seekable,
            period_ms: self.period_ms,
            children_generation: inst.children_generation,
            shader_generation: shaders::shader_generation(),
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
            config.format,
        );
        inst.drain();
        let failure = inst.surface.take_error();
        let (texture, failed) = match failure {
            Some(_) => (None, Some(texture)),
            None => (Some(texture), None),
        };
        open.canvases.push(Rendered {
            id,
            texture,
            failed,
            request: off_thread && wants,
        });
        if let Some(SurfaceError(e)) = failure {
            self.error = e;
            return None;
        }
        inst.dirty = false;
        Some(wants)
    }

    /// Submit the open frame once, then present each canvas's drawable in
    /// the order rendered. `false`, with the reason in [`Module::take_error`],
    /// when a surface failed after its commands were submitted; nothing
    /// open is `true`.
    pub fn flush(&mut self) -> bool {
        self.check_device();
        let Some(open) = self.open.take() else {
            return true;
        };
        let Some(gpu) = self.gpu.as_ref() else {
            return true;
        };
        gpu.queue.submit([open.encoder.finish()]);
        let mut ok = true;
        for c in open.canvases {
            if let Some(texture) = c.texture {
                gpu.queue.present(texture);
            }
            drop(c.failed);
            let Some(inst) = self.instances.get_mut(&c.id) else {
                continue;
            };
            inst.surface.submitted();
            inst.drain();
            if let Some(SurfaceError(e)) = inst.surface.take_error() {
                self.error = e;
                ok = false;
            }
            // The next drawable, waited for while this frame is composited.
            #[cfg(not(target_arch = "wasm32"))]
            if c.request {
                if let Some(target) = &inst.presentation {
                    inst.acquire.request(target);
                }
            }
            #[cfg(target_arch = "wasm32")]
            let _ = c.request;
        }
        ok
    }
}
