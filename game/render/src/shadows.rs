use crate::{FrameInput, Shadows};
use exact_gpu::wgpu;
use glam::camera::rh::proj::directx;
use glam::{Mat4, Vec3};

pub(crate) const RESOLUTION: u32 = 2048;
pub(crate) const MAX_CASCADES: usize = 3;

pub(crate) struct Cascades {
    pub matrices: [Mat4; 3],
    pub splits: [f32; 3],
    pub texels: [f32; 3],
    pub count: u32,
}

impl Cascades {
    pub fn new(frame: &FrameInput<'_>, settings: Shadows) -> Self {
        let inverse = frame.proj.inverse();
        let near = -inverse.project_point3(Vec3::ZERO).z;
        let far = (-inverse.project_point3(Vec3::Z).z).min(settings.distance);
        assert!(
            near > 0.0 && far > near,
            "shadow distance must exceed camera near"
        );
        let count = settings.cascades.clamp(1, 3);
        let mut result = Self {
            matrices: [Mat4::IDENTITY; 3],
            splits: [far; 3],
            texels: [0.0; 3],
            count,
        };
        let light = frame.sun.unwrap().direction.normalize();
        // Duff et al., JCGT 2017, listing 3. Permute Y to the sign axis so
        // the hemisphere seam is at the horizon, away from a vertical sun.
        // https://graphics.pixar.com/library/OrthonormalB/paper.pdf
        let n = Vec3::new(-light.z, -light.x, -light.y);
        let sign = 1.0_f32.copysign(n.z);
        let a = -1.0 / (sign + n.z);
        let b = n.x * n.y * a;
        let x = Vec3::new(sign * b, -sign * n.x, 1.0 + sign * n.x * n.x * a);
        let y = Vec3::new(sign + n.y * n.y * a, -n.y, b);
        let light_view = Mat4::from_cols(
            x.extend(0.0),
            y.extend(0.0),
            (-light).extend(0.0),
            glam::Vec4::W,
        )
        .transpose();
        let camera_world = frame.view.inverse();
        let mut start = near;
        let mut prev_start = near;
        for i in 0..count as usize {
            let t = (i + 1) as f32 / count as f32;
            let end = 0.7 * near * (far / near).powf(t) + 0.3 * (near + (far - near) * t);
            result.splits[i] = end;
            // Overlap the preceding slice's final 10% for the shader's cross-fade.
            let overlap_start = if i == 0 {
                near
            } else {
                start - (start - prev_start) * 0.1
            };
            let corners: [Vec3; 8] = std::array::from_fn(|j| {
                let x = if j & 1 == 0 { -1.0 } else { 1.0 };
                let y = if j & 2 == 0 { -1.0 } else { 1.0 };
                let a = inverse.project_point3(Vec3::new(x, y, 0.0));
                let b = inverse.project_point3(Vec3::new(x, y, 0.5));
                let depth = if j & 4 == 0 { overlap_start } else { end };
                a + (b - a) * ((-depth - a.z) / (b.z - a.z))
            });
            // Sphere in VIEW space: rotation cannot change its radius. Pad for PCF
            // and snapping, then quantize up to avoid floating-point breathing.
            let center = corners.iter().copied().sum::<Vec3>() / 8.0;
            let radius = corners
                .iter()
                .map(|p| p.distance(center))
                .fold(0.0, f32::max);
            let radius = (radius * 1.005 * 16.0).ceil() / 16.0;
            let texel = 2.0 * radius / RESOLUTION as f32;
            result.texels[i] = texel;
            let mut c = light_view.transform_point3(camera_world.transform_point3(center));
            c.x = (c.x / texel).round() * texel;
            c.y = (c.y / texel).round() * texel;
            // Include off-frustum casters towards the sun, without walking objects.
            let projection = directx::orthographic(
                c.x - radius,
                c.x + radius,
                c.y - radius,
                c.y + radius,
                -c.z - radius - far,
                -c.z + radius,
            );
            result.matrices[i] = projection * light_view;
            prev_start = start;
            start = end;
        }
        result
    }
}

pub(crate) struct ShadowMaps {
    pub count: u32,
    pub layers: Vec<wgpu::TextureView>,
    pub cameras: Vec<wgpu::BindGroup>,
    /// Every cascade, sampled in group 1 (`Renderer::shadow_sample`).
    pub view: wgpu::TextureView,
    uniform: wgpu::Buffer,
}

impl ShadowMaps {
    pub fn new(device: &wgpu::Device, count: u32, camera_layout: &wgpu::BindGroupLayout) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("game sun cascades"),
            size: wgpu::Extent3d {
                width: RESOLUTION,
                height: RESOLUTION,
                depth_or_array_layers: count,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("game cascade cameras"),
            size: 3 * 256,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let layers = (0..count)
            .map(|i| {
                texture.create_view(&wgpu::TextureViewDescriptor {
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    base_array_layer: i,
                    array_layer_count: Some(1),
                    ..Default::default()
                })
            })
            .collect();
        let cameras = (0..count)
            .map(|i| {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("game cascade camera"),
                    layout: camera_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &uniform,
                            offset: u64::from(i) * 256,
                            size: std::num::NonZeroU64::new(64),
                        }),
                    }],
                })
            })
            .collect();
        Self {
            count,
            layers,
            cameras,
            view,
            uniform,
        }
    }

    pub fn write(&self, queue: &wgpu::Queue, cascades: &Cascades) {
        let mut data = [0.0f32; 3 * 64];
        for i in 0..self.count as usize {
            data[i * 64..i * 64 + 16].copy_from_slice(&cascades.matrices[i].to_cols_array());
        }
        queue.write_buffer(&self.uniform, 0, crate::buffers::bytes(&data));
    }
}
