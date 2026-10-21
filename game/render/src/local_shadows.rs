//! Shadow maps for local lights: one depth layer per shadowed spot light and six
//! (a cube's faces) per shadowed point light, nearest lights first, at most
//! `MAX_VIEWS` layers a frame. Each layer culls its casters on the GPU, four
//! views per cull pass as the camera and cascades are, and draws them depth-only.
use crate::LightInput;
use exact_gpu::wgpu;
use glam::camera::rh::proj::directx;
use glam::{Mat4, Vec3};

pub(crate) const RESOLUTION: u32 = 1024;
/// Shadow layers a frame: a shadowed point light takes six, a spot one.
pub(crate) const MAX_VIEWS: usize = 8;
// Spots wider than this shadow only their central cone.
const MAX_FOV: f32 = 170. * std::f32::consts::PI / 180.;

/// This frame's local shadow views and the layer each light starts at.
#[derive(Default)]
pub(crate) struct Plan {
    pub views: Vec<Mat4>,
    /// Per frame light: first layer, and a shadow texel's width per metre of
    /// distance (the normal offset's scale).
    pub layers: Vec<Option<(u32, f32)>>,
}

impl Plan {
    pub fn of(&mut self, lights: &[LightInput]) {
        self.views.clear();
        self.layers.clear();
        for light in lights.iter().take(crate::MAX_LIGHTS) {
            let need = if light.cone.is_some() { 1 } else { 6 };
            if !light.shadows
                || light.intensity.is_nan()
                || light.intensity <= 0.
                || light.range.is_nan()
                || light.range <= 0.
                || !light.position.is_finite()
                || self.views.len() + need > MAX_VIEWS
            {
                self.layers.push(None);
                continue;
            }
            let near = (light.range * 1e-3).clamp(0.01, 0.05);
            let first = self.views.len() as u32;
            let texel = match light.cone {
                Some([_, outer]) => {
                    let fov = (2. * outer.clamp(-1., 1.).acos()).clamp(0.01, MAX_FOV);
                    let direction = light.direction.normalize_or(-Vec3::Z);
                    let up = if direction.y.abs() > 0.99 {
                        Vec3::Z
                    } else {
                        Vec3::Y
                    };
                    self.views.push(
                        directx::perspective(fov, 1., near, light.range)
                            * glam::camera::rh::view::look_to_mat4(light.position, direction, up),
                    );
                    2. * (fov * 0.5).tan() / RESOLUTION as f32
                }
                None => {
                    let projection =
                        directx::perspective(std::f32::consts::FRAC_PI_2, 1., near, light.range);
                    // Face order +X −X +Y −Y +Z −Z: lights.wgsl picks by major axis.
                    for (axis, up) in [
                        (Vec3::X, -Vec3::Y),
                        (-Vec3::X, -Vec3::Y),
                        (Vec3::Y, Vec3::Z),
                        (-Vec3::Y, -Vec3::Z),
                        (Vec3::Z, -Vec3::Y),
                        (-Vec3::Z, -Vec3::Y),
                    ] {
                        self.views.push(
                            projection
                                * glam::camera::rh::view::look_to_mat4(light.position, axis, up),
                        );
                    }
                    2. / RESOLUTION as f32
                }
            };
            self.layers.push(Some((first, texel)));
        }
    }
}

/// The depth layers, a camera binding per layer, and the culls that list each
/// layer's casters (`cull.rs`, four views each).
pub(crate) struct LocalMaps {
    pub count: u32,
    pub layers: Vec<wgpu::TextureView>,
    pub sample: wgpu::TextureView,
    pub cameras: Vec<wgpu::BindGroup>,
    uniform: wgpu::Buffer,
}

impl LocalMaps {
    pub fn new(device: &wgpu::Device, count: u32, camera_layout: &wgpu::BindGroupLayout) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("game local shadows"),
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
        let sample = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
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
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("game local shadow cameras"),
            size: u64::from(count) * 256,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let cameras = (0..count)
            .map(|i| {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("game local shadow camera"),
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
            sample,
            cameras,
            uniform,
        }
    }

    pub fn write(&self, queue: &wgpu::Queue, views: &[Mat4]) {
        let mut data = vec![0f32; self.count as usize * 64];
        for (i, view) in views.iter().enumerate() {
            data[i * 64..i * 64 + 16].copy_from_slice(&view.to_cols_array());
        }
        queue.write_buffer(&self.uniform, 0, crate::buffers::bytes(&data));
    }
}

/// A one-texel depth array standing in for absent shadow maps in group 1.
pub(crate) fn placeholder(device: &wgpu::Device) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("game no shadow map"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
        .create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        })
}
