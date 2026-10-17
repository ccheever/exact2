use crate::Vertex;
use exact_gpu::wgpu;

// Closed to this module's three padding-free types. No arbitrary T can reach bytes.
pub(crate) trait Packed: private::Sealed {}
mod private {
    pub trait Sealed {}
    impl Sealed for f32 {}
    impl Sealed for u32 {}
    impl Sealed for super::Vertex {}
}
impl Packed for f32 {}
impl Packed for u32 {}
impl Packed for Vertex {}

/// The sole unsafe operation: these sealed types contain only initialized scalar
/// fields, have no padding, and the returned borrow cannot outlive the input.
#[allow(unsafe_code)]
pub(crate) fn bytes<T: Packed>(values: &[T]) -> &[u8] {
    const {
        assert!(size_of::<f32>() == 4);
        assert!(size_of::<u32>() == 4);
        assert!(size_of::<Vertex>() == 48);
        assert!(align_of::<Vertex>() == 4);
    }
    unsafe { std::slice::from_raw_parts(values.as_ptr().cast(), size_of_val(values)) }
}

/// Writes bytes at an offset: how a [`Fill`] puts its data.
pub(crate) type Put<'a> = dyn FnMut(usize, &[u8]) + 'a;
/// Puts a write's bytes wherever its writer keeps them; a writer calls it once.
pub(crate) type Fill<'a> = dyn FnMut(&mut Put<'_>) + 'a;
/// `len` floats that `fill` puts as bytes: the copying path beside a write into
/// staging memory.
pub(crate) fn filled(len: usize, fill: &mut Fill<'_>) -> Vec<f32> {
    let mut out = vec![0u8; len * 4];
    fill(&mut |at, bytes| out[at..at + bytes.len()].copy_from_slice(bytes));
    out.chunks_exact(4)
        .map(|b| f32::from_ne_bytes([b[0], b[1], b[2], b[3]]))
        .collect()
}

pub(crate) struct Buffer {
    pub raw: wgpu::Buffer,
    pub live: u64,
    label: &'static str,
}

impl Buffer {
    pub fn new(
        device: &wgpu::Device,
        size: u64,
        usage: wgpu::BufferUsages,
        label: &'static str,
    ) -> Self {
        let usage = usage | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST;
        Self {
            raw: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage,
                mapped_at_creation: false,
            }),
            live: 0,
            label,
        }
    }

    pub fn grow(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, needed: u64) -> bool {
        if needed <= self.raw.size() {
            return false;
        }
        let limit = if self.raw.usage().contains(wgpu::BufferUsages::STORAGE) {
            device
                .limits()
                .max_storage_buffer_binding_size
                .min(device.limits().max_buffer_size)
        } else {
            device.limits().max_buffer_size
        };
        assert!(needed <= limit, "buffer exceeds device limits");
        let capacity = needed.next_power_of_two().min(limit);
        let mut next = Self::new(device, capacity, self.raw.usage(), self.label);
        if self.live != 0 {
            let mut encoder = device.create_command_encoder(&Default::default());
            encoder.copy_buffer_to_buffer(&self.raw, 0, &next.raw, 0, self.live);
            // Submit now: later queue writes must overlay, not precede, this copy.
            queue.submit([encoder.finish()]);
        }
        next.live = self.live;
        *self = next;
        true
    }

    pub fn write(&mut self, queue: &wgpu::Queue, offset: u64, data: &[u8]) {
        if data.is_empty() {
            return;
        }
        queue.write_buffer(&self.raw, offset, data);
        self.live = self.live.max(offset + data.len() as u64);
    }
    /// Write `len` bytes at `offset` that `fill` puts straight into the queue's
    /// staging memory: no copy of the data beside the one the queue makes.
    pub fn write_with(&mut self, queue: &wgpu::Queue, offset: u64, len: u64, fill: &mut Fill<'_>) {
        let Some(size) = wgpu::BufferSize::new(len) else {
            return;
        };
        match queue.write_buffer_with(&self.raw, offset, size) {
            Some(mut view) => {
                fill(&mut |at, bytes| view.slice(at..at + bytes.len()).copy_from_slice(bytes))
            }
            None => return,
        }
        self.live = self.live.max(offset + len);
    }
}

pub(crate) struct Targets {
    pub size: (u32, u32),
    pub color: wgpu::TextureView,
    pub depth: wgpu::TextureView,
    pub resolved: wgpu::TextureView,
    pub tone_bind: wgpu::BindGroup,
    pub retained: bool,
    pub samples: u32,
}

impl Targets {
    pub fn with_retention(
        device: &wgpu::Device,
        size: (u32, u32),
        layout: &wgpu::BindGroupLayout,
        uniform: &wgpu::Buffer,
        retained: bool,
        samples: u32,
    ) -> Self {
        let texture = |format, sample_count, usage| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some("game offscreen"),
                    size: wgpu::Extent3d {
                        width: size.0,
                        height: size.1,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        };
        let depth = texture(
            wgpu::TextureFormat::Depth32Float,
            samples,
            wgpu::TextureUsages::RENDER_ATTACHMENT
                | if retained {
                    wgpu::TextureUsages::TEXTURE_BINDING
                } else {
                    transient_usage()
                },
        );
        let resolved = texture(
            wgpu::TextureFormat::Rgba16Float,
            1,
            wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST,
        );
        let color = if samples == 1 {
            resolved.clone()
        } else {
            texture(
                wgpu::TextureFormat::Rgba16Float,
                samples,
                wgpu::TextureUsages::RENDER_ATTACHMENT
                    | if retained {
                        wgpu::TextureUsages::empty()
                    } else {
                        transient_usage()
                    },
            )
        };
        let tone_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("game tonemap"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&resolved),
                },
            ],
        });
        Self {
            size,
            retained,
            samples,
            color,
            depth,
            resolved,
            tone_bind,
        }
    }
}

/// A direct frame's attachments at its target's own size: at 4x, MSAA colour in
/// the target's format, which the forward pass resolves into the target; at 1x
/// none, as the pass draws into the target. Depth is transient unless the depth
/// prepass writes it for the forward pass to load (`retained`).
pub(crate) struct DirectTargets {
    pub format: wgpu::TextureFormat,
    pub size: wgpu::Extent3d,
    pub retained: bool,
    pub color: Option<wgpu::TextureView>,
    pub depth: wgpu::TextureView,
}

impl DirectTargets {
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        size: wgpu::Extent3d,
        samples: u32,
        retained: bool,
    ) -> Self {
        let texture = |format, transient| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some("game direct"),
                    size,
                    mip_level_count: 1,
                    sample_count: samples,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | if transient {
                            transient_usage()
                        } else {
                            wgpu::TextureUsages::empty()
                        },
                    view_formats: &[],
                })
                .create_view(&Default::default())
        };
        Self {
            format,
            size,
            retained,
            color: (samples > 1).then(|| texture(format, true)),
            depth: texture(wgpu::TextureFormat::Depth32Float, !retained),
        }
    }
    /// The textures it made, for `Stats::texture_creations`.
    pub fn textures(&self) -> u64 {
        1 + u64::from(self.color.is_some())
    }
}

/// `TRANSIENT_ATTACHMENT`, or no usage where the browser's WebGPU predates it: Android
/// WebView 136 (Dawn) refuses the bit without its `transient-attachments` feature, so
/// every frame's attachments were invalid and the canvas stayed black.
fn transient_usage() -> wgpu::TextureUsages {
    #[cfg(target_arch = "wasm32")]
    {
        use web_sys::js_sys::{global, Reflect};
        thread_local! {
            static KNOWN: bool = Reflect::get(&global(), &"GPUTextureUsage".into())
                .and_then(|usage| Reflect::has(&usage, &"TRANSIENT_ATTACHMENT".into()))
                .unwrap_or(false);
        }
        if !KNOWN.with(|known| *known) {
            return wgpu::TextureUsages::empty();
        }
    }
    wgpu::TextureUsages::TRANSIENT_ATTACHMENT
}
