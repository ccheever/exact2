//! A canvas that renders into GPU buffers the platform's own renderer draws
//! (Android's `AHardwareBuffer`), instead of presenting to a window.
//!
//! A canvas with a window presents each frame to a swapchain; the reader's
//! `TextureView` then takes that frame from a `SurfaceTexture` and draws it
//! into its own pass: a queue present, a buffer dequeue and a texture update
//! per canvas frame, on three threads. Here the canvas owns a few buffers,
//! renders each frame into the next, and once that frame's commands have
//! completed the reader draws the buffer as a hardware bitmap in the pass it
//! already runs, as it draws a decoded picture.
//!
//! The ring has six buffers: up to [`IN_FLIGHT`] on the GPU, a finished one
//! waiting to be shown, the one the reader shows and the one it showed a frame
//! ago. Frames are published in the order they finish, one a call (`take`):
//! when frames finish two in one display frame and none in the next, which a
//! canvas rendered at the display's rate does, the newest alone would drop one
//! and show another twice. No buffer in any of those states is rendered into
//! again (`target`); past one waiting, the oldest is dropped. A canvas renders
//! no new frame while [`IN_FLIGHT`] are on the GPU (`Module::render_to_buffer`;
//! under the agent's clock the frame waits for the GPU instead): more only
//! queue work the reader will drop. Fewer leave the GPU idle between
//! frames, and Android's governor then lowers its clock (on a Pixel 5, 60 frames
//! a second of 64,000 cubes at three, 52 at two and 20 at one).
#![allow(unsafe_code)]
use crate::ring_order::{Order, SLOTS};
use crate::Gpu;
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Frames a canvas keeps on the GPU at once (see the module's text).
pub(crate) const IN_FLIGHT: usize = 3;
/// `AHARDWAREBUFFER_FORMAT_R8G8B8A8_UNORM`.
const R8G8B8A8_UNORM: u32 = 1;
/// `AHARDWAREBUFFER_USAGE_GPU_SAMPLED_IMAGE | GPU_FRAMEBUFFER`.
const USAGE: u64 = (1 << 8) | (1 << 9);
/// What the buffers hold; premultiplied, as the reader draws bitmaps.
pub(crate) const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

#[repr(C)]
struct Desc {
    width: u32,
    height: u32,
    layers: u32,
    format: u32,
    usage: u64,
    stride: u32,
    rfu0: u32,
    rfu1: u64,
}

#[link(name = "android")]
extern "C" {
    fn AHardwareBuffer_allocate(desc: *const Desc, out: *mut *mut c_void) -> i32;
    fn AHardwareBuffer_release(buffer: *mut c_void);
}

struct Slot {
    buffer: *mut c_void,
    view: wgpu::TextureView,
    _texture: wgpu::Texture,
}

impl Drop for Slot {
    fn drop(&mut self) {
        // SAFETY: the reference `Ring::new` took; Vulkan and a reader that
        // wrapped the buffer hold their own.
        unsafe { AHardwareBuffer_release(self.buffer) };
    }
}

/// A canvas's buffers.
pub(crate) struct Ring {
    pub(crate) size: (u32, u32),
    pub(crate) format: wgpu::TextureFormat,
    slots: Vec<Slot>,
    order: Order,
    /// Distinguishes this ring's buffers from an earlier ring's.
    generation: u32,
}
// SAFETY: an AHardwareBuffer is reference counted and usable from any thread.
unsafe impl Send for Ring {}

static GENERATION: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);

impl Ring {
    /// Buffers of `width`×`height` imported as render targets, or `None`
    /// when the device cannot (the canvas then needs a window).
    pub(crate) fn new(
        gpu: &Gpu,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) -> Option<Ring> {
        let (width, height) = (width.max(1), height.max(1));
        let view_formats = if format == FORMAT {
            vec![]
        } else {
            vec![format]
        };
        let mut slots = Vec::with_capacity(SLOTS);
        for _ in 0..SLOTS {
            let desc = Desc {
                width,
                height,
                layers: 1,
                format: R8G8B8A8_UNORM,
                usage: USAGE,
                stride: 0,
                rfu0: 0,
                rfu1: 0,
            };
            let mut buffer = std::ptr::null_mut();
            // SAFETY: `desc` and `buffer` outlive the call.
            if unsafe { AHardwareBuffer_allocate(&desc, &mut buffer) } != 0 || buffer.is_null() {
                return None;
            }
            let size = wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            };
            let usage =
                wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING;
            // SAFETY: the buffer is live and of this size and format, with
            // the GPU usages a colour target and a sampled image need; the
            // hal texture is handed to wgpu with the descriptor it was made by.
            let texture = unsafe {
                let hal = gpu.device.as_hal::<wgpu::hal::api::Vulkan>().and_then(|d| {
                    d.texture_from_hardware_buffer(
                        buffer,
                        &wgpu::hal::TextureDescriptor {
                            label: Some("exact canvas buffer"),
                            size,
                            mip_level_count: 1,
                            sample_count: 1,
                            dimension: wgpu::TextureDimension::D2,
                            format: FORMAT,
                            usage: wgpu::TextureUses::COLOR_TARGET | wgpu::TextureUses::RESOURCE,
                            memory_flags: wgpu::hal::MemoryFlags::empty(),
                            view_formats: view_formats.clone(),
                        },
                    )
                    .ok()
                });
                let Some(hal) = hal else {
                    AHardwareBuffer_release(buffer);
                    return None;
                };
                gpu.device
                    .create_texture_from_hal::<wgpu::hal::api::Vulkan>(
                        hal,
                        &wgpu::TextureDescriptor {
                            label: Some("exact canvas buffer"),
                            size,
                            mip_level_count: 1,
                            sample_count: 1,
                            dimension: wgpu::TextureDimension::D2,
                            format: FORMAT,
                            usage,
                            view_formats: &view_formats,
                        },
                        wgpu::TextureUses::UNINITIALIZED,
                    )
            };
            slots.push(Slot {
                buffer,
                view: texture.create_view(&wgpu::TextureViewDescriptor {
                    format: Some(format),
                    ..Default::default()
                }),
                _texture: texture,
            });
        }
        Some(Ring {
            format,
            size: (width, height),
            slots,
            order: Order::default(),
            generation: GENERATION.fetch_add(1, Ordering::Relaxed),
        })
    }

    /// The slot this frame renders into and its view ([`Order::target`]).
    pub(crate) fn target(&mut self) -> Option<(usize, wgpu::TextureView)> {
        let slot = self.order.target()?;
        Some((slot, self.slots[slot].view.clone()))
    }

    /// `slot`'s frame was submitted: busy until the GPU has run it, then
    /// published if `publish`.
    pub(crate) fn submitted(&mut self, slot: usize, publish: bool, queue: &wgpu::Queue) {
        let done = Arc::new(AtomicBool::new(false));
        let flag = done.clone();
        queue.on_submitted_work_done(move || flag.store(true, Ordering::Release));
        self.order.submitted(slot, done, publish);
    }

    /// Frames submitted that the GPU has not finished.
    pub(crate) fn in_flight(&self) -> usize {
        self.order.in_flight()
    }

    /// Take the frames the GPU has finished ([`Order::settle`]).
    pub(crate) fn settle(&mut self) {
        self.order.settle();
    }

    /// Publish the oldest finished frame ([`Order::take`]) and return the
    /// published frame (as [`Ring::latest`]).
    pub(crate) fn take(&mut self) -> Option<([u32; 3], *mut c_void)> {
        self.order.take();
        self.latest()
    }

    /// The newest complete frame: `[generation, slot, serial]` and its
    /// buffer (an `AHardwareBuffer*`, alive while the ring is).
    pub(crate) fn latest(&self) -> Option<([u32; 3], *mut c_void)> {
        let (slot, serial) = self.order.latest()?;
        Some((
            [self.generation, slot as u32, serial],
            self.slots[slot].buffer,
        ))
    }
}

#[cfg(test)]
#[path = "buffers_tests.rs"]
mod tests;
