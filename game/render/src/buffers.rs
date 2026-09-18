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
        assert!(size_of::<Vertex>() == 40);
        assert!(align_of::<Vertex>() == 4);
    }
    unsafe { std::slice::from_raw_parts(values.as_ptr().cast(), size_of_val(values)) }
}

pub(crate) struct Buffer {
    pub raw: wgpu::Buffer,
    pub capacity: u64,
    pub live: u64,
    usage: wgpu::BufferUsages,
    label: &'static str,
}

impl Buffer {
    pub fn new(
        device: &wgpu::Device,
        size: u64,
        usage: wgpu::BufferUsages,
        label: &'static str,
    ) -> Self {
        if usage.contains(wgpu::BufferUsages::VERTEX) {
            crate::audit::record(crate::audit::VERTEX, label, 1);
        }
        if usage.contains(wgpu::BufferUsages::INDEX) {
            crate::audit::record(crate::audit::INDEX, label, 1);
        }
        let usage = usage | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST;
        Self {
            raw: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage,
                mapped_at_creation: false,
            }),
            capacity: size,
            live: 0,
            usage,
            label,
        }
    }

    pub fn grow(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, needed: u64) -> bool {
        if needed <= self.capacity {
            return false;
        }
        let limit = if self.usage.contains(wgpu::BufferUsages::STORAGE) {
            device
                .limits()
                .max_storage_buffer_binding_size
                .min(device.limits().max_buffer_size)
        } else {
            device.limits().max_buffer_size
        };
        assert!(needed <= limit, "buffer exceeds device limits");
        let capacity = needed.next_power_of_two().min(limit);
        if self
            .usage
            .intersects(wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::UNIFORM)
        {
            crate::audit::record(crate::audit::GROW, self.label, 1);
            if matches!(
                self.label,
                "game slots" | "game model instances" | "game transforms a"
            ) {
                crate::audit::record(crate::audit::SLOTS, self.label, 1);
            }
        }
        let mut next = Self::new(device, capacity, self.usage, self.label);
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
}

pub(crate) struct Targets {
    pub size: (u32, u32),
    pub color: wgpu::TextureView,
    pub depth: wgpu::TextureView,
    pub resolved: wgpu::TextureView,
    pub tone_bind: wgpu::BindGroup,
}

impl Targets {
    pub fn new(
        device: &wgpu::Device,
        size: (u32, u32),
        layout: &wgpu::BindGroupLayout,
        uniform: &wgpu::Buffer,
    ) -> Self {
        let texture = |format, sample_count, usage| {
            crate::audit::record(crate::audit::TEXTURE, "game offscreen", 1);
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
        let color = texture(
            wgpu::TextureFormat::Rgba16Float,
            4,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TRANSIENT_ATTACHMENT,
        );
        let depth = texture(
            wgpu::TextureFormat::Depth32Float,
            4,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TRANSIENT_ATTACHMENT,
        );
        let resolved = texture(
            wgpu::TextureFormat::Rgba16Float,
            1,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        );
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
            color,
            depth,
            resolved,
            tone_bind,
        }
    }
}
