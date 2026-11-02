use crate::{FrameInput, ShadowFilter, Shadows};
use exact_gpu::wgpu;
use glam::camera::rh::proj::directx;
use glam::{Mat4, Vec3};

pub(crate) const MAX_CASCADES: usize = 3;
/// A plane every point is inside of (`cull::planes`' form).
pub(crate) const PASS_ALL: [f32; 4] = [0., 0., 0., 1.];

pub(crate) struct Cascades {
    pub cache_enabled: bool,
    pub resolution: u32,
    pub matrices: [Mat4; 3],
    pub splits: [f32; 3],
    pub texels: [f32; 3],
    pub count: u32,
    /// Per cascade, planes containing the light direction that bound what the
    /// camera reads from that cascade (`outline`); all `PASS_ALL` where the
    /// casters a frame draws must not depend on the camera: a fixed box, or a
    /// static cache drawn once for several views.
    pub outlines: [[[f32; 4]; 6]; MAX_CASCADES],
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
        let resolution = settings.resolution.clamp(128, 4096);
        let mut result = Self {
            cache_enabled: settings.cache_static,
            resolution,
            matrices: [Mat4::IDENTITY; 3],
            splits: [far; 3],
            texels: [0.0; 3],
            count,
            outlines: [[PASS_ALL; 6]; MAX_CASCADES],
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
        if let Some(fixed) = settings.fixed_box {
            let w = fixed.half_width;
            let d = fixed.half_depth;
            assert!(
                fixed.center.is_finite() && w.is_finite() && w > 0. && d.is_finite() && d > 0.,
                "sun box must be finite with positive extents"
            );
            let c = light_view.transform_point3(fixed.center);
            result.count = 1;
            result.texels[0] = 2. * w / resolution as f32;
            result.matrices[0] =
                directx::orthographic(c.x - w, c.x + w, c.y - w, c.y + w, -c.z - d, -c.z + d)
                    * light_view;
            return result;
        }
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
            let texel = 2.0 * radius / resolution as f32;
            result.texels[i] = texel;
            if !settings.cache_static {
                // How far from where a receiver projects its filter reads
                // (shadow_sample.wgsl), with room `outline` may give up to
                // rounding: the receiver-plane filter's taps reach sqrt(2) *
                // softness and each tap's 2 x 2 texels sqrt(2) more, after at
                // most half a texel along the normal; PCF-soft's taps reach
                // 1.5 * sqrt(2) + 1 texels, and it and bilinear first move the
                // receiver by their normal bias, in metres.
                let bias = match settings.filter {
                    ShadowFilter::ReceiverPlane => 0.0,
                    ShadowFilter::PcfSoft { normal_bias, .. }
                    | ShadowFilter::Bilinear { normal_bias, .. } => normal_bias.abs(),
                };
                let margin = texel * (2.0 * settings.softness.max(0.0) + 5.0) + 1.5 * bias;
                let world = corners.map(|p| camera_world.transform_point3(p));
                result.outlines[i] = outline(&world, x, y, margin);
            }
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
    pub cached: Option<crate::renderer::shadow_cache::StaticShadowCache>,
    pub cache_enabled: bool,
    /// How cached casters enter each frame's map (`shadow_cache::seeds`).
    pub seed: bool,
    pub resolution: u32,
    pub count: u32,
    pub layers: Vec<wgpu::TextureView>,
    pub cameras: Vec<wgpu::BindGroup>,
    /// Every cascade, sampled in group 1 (`Renderer::shadow_sample`).
    pub view: wgpu::TextureView,
    uniform: wgpu::Buffer,
}

impl ShadowMaps {
    pub fn new(
        device: &wgpu::Device,
        count: u32,
        resolution: u32,
        cache_enabled: bool,
        camera_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let seed = crate::renderer::shadow_cache::seeds(device);
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("game sun cascades"),
            size: wgpu::Extent3d {
                width: resolution,
                height: resolution,
                depth_or_array_layers: count,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | if cache_enabled && !seed {
                    wgpu::TextureUsages::COPY_DST
                } else {
                    wgpu::TextureUsages::empty()
                }
                // Tests read the depth, and copy where this device would seed.
                | if cfg!(test) {
                    wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::COPY_DST
                } else {
                    wgpu::TextureUsages::empty()
                },
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
            cached: None,
            cache_enabled,
            seed,
            resolution,
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

/// The camera's slice of a cascade, `corners` in the world, seen along the
/// light: the planes, each containing the light direction (`x` and `y` span the
/// plane across it), whose intersection is that silhouette's convex hull grown
/// by `margin`. A caster wholly outside one writes no texel a receiver in the
/// slice reads, whatever its depth along the light. The cull tests them with the
/// cascade's own planes. Where rounding could tilt a plane into the slice (an
/// edge too short to give its direction, a hull of over six sides, any corner
/// more than a quarter of the margin outside) it bounds nothing.
fn outline(corners: &[Vec3; 8], x: Vec3, y: Vec3, margin: f32) -> [[f32; 4]; 6] {
    if !(corners.iter().all(|p| p.is_finite()) && margin.is_finite()) {
        return [PASS_ALL; 6];
    }
    let points = corners.map(|p| (x.dot(p), y.dot(p)));
    let (lo, hi) = points.iter().fold(
        ((f32::MAX, f32::MAX), (f32::MIN, f32::MIN)),
        |(lo, hi), p| {
            (
                (lo.0.min(p.0), lo.1.min(p.1)),
                (hi.0.max(p.0), hi.1.max(p.1)),
            )
        },
    );
    let extent = (hi.0 - lo.0).max(hi.1 - lo.1);
    // Corners that meet seen along the light (a slice edge parallel to it)
    // become one point, whose rounding would give a short edge no direction.
    let close = 1e-4 * extent;
    let mut distinct = [(0.0, 0.0); 8];
    let mut m = 0;
    for p in points {
        if distinct[..m]
            .iter()
            .all(|q| (p.0 - q.0).abs() > close || (p.1 - q.1).abs() > close)
        {
            distinct[m] = p;
            m += 1;
        }
    }
    let distinct = &mut distinct[..m];
    distinct.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
    let turn = |o: (f32, f32), a: (f32, f32), b: (f32, f32)| {
        (a.0 - o.0) * (b.1 - o.1) - (a.1 - o.1) * (b.0 - o.0)
    };
    // Andrew's monotone chain: lower then upper hull, counter-clockwise.
    let mut chain = [(0.0, 0.0); 16];
    let mut n = 0;
    for upper in [false, true] {
        let start = n;
        for k in 0..m {
            let p = distinct[if upper { m - 1 - k } else { k }];
            while n >= start + 2 && turn(chain[n - 2], chain[n - 1], p) <= 0.0 {
                n -= 1;
            }
            chain[n] = p;
            n += 1;
        }
        n -= 1;
    }
    let hull = &chain[..n];
    let mut planes = [PASS_ALL; 6];
    if !(3..=6).contains(&hull.len()) {
        return planes;
    }
    for (k, plane) in planes.iter_mut().enumerate().take(hull.len()) {
        let (a, b) = (hull[k], hull[(k + 1) % hull.len()]);
        let length = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
        if length.is_nan() || length < 1e-4 * extent {
            return [PASS_ALL; 6];
        }
        // Outward, to the right of a counter-clockwise edge.
        let (nx, ny) = ((b.1 - a.1) / length, (a.0 - b.0) / length);
        let edge = nx * a.0 + ny * a.1;
        if points
            .iter()
            .any(|p| nx * p.0 + ny * p.1 - edge > 0.25 * margin)
        {
            return [PASS_ALL; 6];
        }
        let inward = -(x * nx + y * ny);
        *plane = [inward.x, inward.y, inward.z, edge + margin];
    }
    planes
}

#[cfg(test)]
mod outline_tests {
    use super::*;

    #[test]
    fn the_outline_holds_the_slice_seen_along_the_light_and_its_margin() {
        // A light down -z: its plane is x-y, and depth along it does not matter.
        let (x, y) = (Vec3::X, Vec3::Y);
        let corners: [Vec3; 8] = std::array::from_fn(|j| {
            let near = j & 4 == 0;
            let w = if near { 1.0 } else { 3.0 };
            Vec3::new(
                if j & 1 == 0 { -w } else { w },
                if j & 2 == 0 { -1.0 } else { 1.0 },
                if near { 0.0 } else { -5.0 },
            )
        });
        let planes = outline(&corners, x, y, 0.5);
        let inside = |p: Vec3| {
            planes
                .iter()
                .all(|q| q[0] * p.x + q[1] * p.y + q[2] * p.z + q[3] >= 0.0)
        };
        for c in corners {
            assert!(inside(c) && inside(c + Vec3::Z * 100.0) && inside(c - Vec3::Z * 100.0));
        }
        assert!(
            inside(Vec3::new(3.4, 1.4, 50.0)),
            "within the margin, past a corner"
        );
        assert!(!inside(Vec3::new(3.6, 0.0, 0.0)) && !inside(Vec3::new(0.0, -1.6, -3.0)));
        // Each plane contains the light direction.
        assert!(planes.iter().all(|q| q[2] == 0.0));
    }

    #[test]
    fn under_any_light_the_slice_stays_inside_by_most_of_the_margin() {
        // A camera's slice from 0.1 m to 6.6 m, far from the origin, under
        // lights on a sphere, along each slice edge among them.
        let proj = directx::perspective(1.0, 0.5, 0.1, 100.0).inverse();
        let place = glam::Mat4::from_rotation_translation(
            glam::Quat::from_rotation_y(0.7),
            Vec3::new(3000.0, 20.0, -1500.0),
        );
        let corners: [Vec3; 8] = std::array::from_fn(|j| {
            let ndc = Vec3::new(
                if j & 1 == 0 { -1.0 } else { 1.0 },
                if j & 2 == 0 { -1.0 } else { 1.0 },
                0.0,
            );
            let (a, b) = (
                proj.project_point3(ndc),
                proj.project_point3(ndc + Vec3::Z * 0.5),
            );
            let depth = if j & 4 == 0 { 0.1 } else { 6.6 };
            place.transform_point3(a + (b - a) * ((-depth - a.z) / (b.z - a.z)))
        });
        let mut lights: Vec<Vec3> = (0..400)
            .map(|k| {
                let (t, p) = (k as f32 * 0.137, k as f32 * 0.0785);
                Vec3::new(t.cos() * p.sin(), p.cos(), t.sin() * p.sin())
            })
            .collect();
        let edges = (0..4).map(|j| (corners[j + 4] - corners[j]).normalize());
        lights.extend(edges);
        let margin = 0.05;
        let mut bounded = 0;
        for (k, light) in lights.iter().enumerate() {
            let x = light.any_orthonormal_vector();
            let y = light.cross(x);
            let planes = outline(&corners, x, y, margin);
            if planes == [PASS_ALL; 6] {
                // Along a slice edge the light sees it end on: a point.
                assert!(k < 400, "edge-aligned light {light:?} bounds nothing");
                continue;
            }
            bounded += 1;
            for q in planes.iter().filter(|q| **q != PASS_ALL) {
                for c in corners {
                    let d = q[0] * c.x + q[1] * c.y + q[2] * c.z + q[3];
                    assert!(d >= 0.7 * margin, "{light:?}: corner {c:?} at {d}");
                }
            }
        }
        assert!(bounded >= 380, "{bounded} of 404 lights bound the slice");
    }

    #[test]
    fn a_filter_s_normal_bias_widens_every_outline_by_more_than_itself() {
        let frame = FrameInput {
            sun: Some(crate::Sun {
                direction: Vec3::new(1., -2., 0.5),
                color: Vec3::ONE,
                illuminance: 1.,
                shadows: None,
            }),
            view: glam::camera::rh::view::look_at_mat4(Vec3::new(4., 5., 6.), Vec3::ZERO, Vec3::Y),
            proj: directx::perspective(1., 1.7, 0.2, 300.),
            ..Default::default()
        };
        let plain = Cascades::new(&frame, Shadows::default());
        for filter in [
            ShadowFilter::PcfSoft {
                depth_bias: 0.,
                normal_bias: -1.1,
            },
            ShadowFilter::Bilinear {
                depth_bias: 0.,
                normal_bias: 1.1,
            },
        ] {
            let biased = Cascades::new(
                &frame,
                Shadows {
                    filter,
                    ..Shadows::default()
                },
            );
            for (a, b) in plain.outlines.iter().zip(&biased.outlines) {
                assert!(a.iter().filter(|q| **q != PASS_ALL).count() >= 3, "{a:?}");
                for (p, q) in a.iter().zip(b).filter(|(p, _)| **p != PASS_ALL) {
                    assert_eq!(p[..3], q[..3]);
                    assert!(q[3] - p[3] >= 1.5 * 1.1 * (1.0 - 1e-5), "{p:?} {q:?}");
                }
            }
        }
    }

    #[test]
    fn a_degenerate_slice_bounds_nothing() {
        let planes = outline(&[Vec3::ZERO; 8], Vec3::X, Vec3::Y, 1.0);
        assert_eq!(planes, [PASS_ALL; 6]);
    }
}
