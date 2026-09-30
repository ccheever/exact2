//! A small uniform a surface writes every frame, without a staging copy.
//!
//! `queue.write_buffer` stages each write in a buffer of its own (on Metal a
//! new `MTLBuffer` per call) and makes the tick commit wgpu's pending-writes
//! command buffer: about 57 µs of the iPhone's main thread per frame for an
//! animated shader row's 80 bytes (LLP 1009 §3). On Apple, where the GPU
//! reads the CPU's memory, the uniform is instead a ring of shared-storage
//! buffers written in place: the frame writes the slot after the last one,
//! and binds that slot's bind group. Four slots: a canvas holds at most three
//! drawables, so the slot a frame writes was last read by a frame whose
//! drawable has since been returned, its commands complete. Elsewhere — the
//! browser, Vulkan — it is one buffer written through the queue.

/// A per-frame uniform: `slots()` buffers, one bound each frame.
pub struct FrameUniform {
    slots: Vec<Slot>,
    next: usize,
}

struct Slot {
    buffer: wgpu::Buffer,
    /// The slot's memory, written in place (Apple's shared storage); `None`
    /// writes through the queue.
    contents: Option<std::ptr::NonNull<u8>>,
    size: usize,
    /// Keeps the Metal buffer, and so `contents`, alive with the slot.
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    _raw: Option<objc2::rc::Retained<objc2::runtime::ProtocolObject<dyn objc2_metal::MTLBuffer>>>,
}

impl FrameUniform {
    /// Slots shared in place on Apple.
    const RING: usize = 4;

    /// A uniform of `size` bytes, labelled `label`.
    pub fn new(device: &wgpu::Device, size: usize, label: &str) -> FrameUniform {
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        if let Some(slots) = shared(device, size, label) {
            return FrameUniform { slots, next: 0 };
        }
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: size as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        FrameUniform {
            slots: vec![Slot {
                buffer,
                contents: None,
                size,
                #[cfg(any(target_os = "macos", target_os = "ios"))]
                _raw: None,
            }],
            next: 0,
        }
    }

    /// How many buffers there are: a surface makes a bind group for each.
    pub fn slots(&self) -> usize {
        self.slots.len()
    }

    /// The `slot`th buffer.
    pub fn buffer(&self, slot: usize) -> &wgpu::Buffer {
        &self.slots[slot].buffer
    }

    /// Write this frame's bytes; the slot to bind for it.
    pub fn write(&mut self, queue: &wgpu::Queue, bytes: &[u8]) -> usize {
        let slot = self.next;
        self.next = (self.next + 1) % self.slots.len();
        let s = &self.slots[slot];
        assert!(
            bytes.len() <= s.size,
            "{} bytes into a {}-byte uniform",
            bytes.len(),
            s.size
        );
        match s.contents {
            // SAFETY: `contents` is the slot's own shared allocation of
            // `size` bytes, alive as long as `_raw`; the GPU last read this
            // slot RING frames ago (see the module note).
            Some(p) => unsafe {
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), p.as_ptr(), bytes.len())
            },
            None => queue.write_buffer(&s.buffer, 0, bytes),
        }
        slot
    }
}

/// The ring on Metal: shared-storage buffers wgpu binds as uniforms.
#[cfg(any(target_os = "macos", target_os = "ios"))]
fn shared(device: &wgpu::Device, size: usize, label: &str) -> Option<Vec<Slot>> {
    use objc2_metal::{MTLBuffer, MTLDevice, MTLResourceOptions};
    // SAFETY: only reads the raw device to allocate from it.
    let hal = unsafe { device.as_hal::<wgpu::hal::api::Metal>() }?;
    let mut slots = Vec::with_capacity(FrameUniform::RING);
    for _ in 0..FrameUniform::RING {
        let raw = hal.raw_device().newBufferWithLength_options(
            size.max(16),
            MTLResourceOptions::StorageModeShared | MTLResourceOptions::CPUCacheModeWriteCombined,
        )?;
        let contents = std::ptr::NonNull::new(raw.contents().as_ptr() as *mut u8)?;
        // SAFETY: a buffer of this device, of this size, handed to wgpu as
        // a uniform it never maps; wgpu treats it as initialized.
        let buffer = unsafe {
            let hal_buffer = wgpu::hal::metal::Device::buffer_from_raw(raw.clone(), size as u64);
            device.create_buffer_from_hal::<wgpu::hal::api::Metal>(
                hal_buffer,
                &wgpu::BufferDescriptor {
                    label: Some(label),
                    size: size as u64,
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                },
            )
        };
        slots.push(Slot {
            buffer,
            contents: Some(contents),
            size,
            _raw: Some(raw),
        });
    }
    Some(slots)
}
