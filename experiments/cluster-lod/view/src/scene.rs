//! Deterministic scene and camera, in source coordinates mapped to a Z-up world.
use bytemuck::{Pod, Zeroable};
use clod_format::{ORIGINAL, Reader};
use glam::{Mat4, Quat, Vec3, Vec4};

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable, serde::Serialize, serde::Deserialize)]
pub struct Instance {
    pub matrix: [f32; 16],
}
impl Instance {
    pub fn new(translation: Vec3, rotation: Quat, scale: f32) -> Self {
        Self {
            matrix: Mat4::from_scale_rotation_translation(
                Vec3::splat(scale),
                rotation,
                translation,
            )
            .to_cols_array(),
        }
    }
    pub fn transform(&self) -> Mat4 {
        Mat4::from_cols_array(&self.matrix)
    }
    pub fn scale(&self) -> f32 {
        clod_format::projection::length(self.transform().x_axis.truncate().to_array())
    }
}
#[derive(Clone, Copy, Debug)]
pub struct AssetBounds {
    pub min: Vec3,
    pub max: Vec3,
}
impl AssetBounds {
    pub fn read(reader: &Reader<'_>) -> Self {
        let mut result = Self {
            min: Vec3::splat(f32::INFINITY),
            max: Vec3::splat(f32::NEG_INFINITY),
        };
        for (id, _) in reader
            .clusters
            .iter()
            .enumerate()
            .filter(|(_, c)| c.refined == ORIGINAL)
        {
            for v in reader.geometry(id).expect("validated geometry").0 {
                let p = Vec3::from_array(v.position);
                result.min = result.min.min(p);
                result.max = result.max.max(p);
            }
        }
        result
    }
    pub fn center(self) -> Vec3 {
        (self.min + self.max) * 0.5
    }
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Scene {
    pub instances: Vec<Instance>,
    pub center: Vec3,
    pub radius: f32,
    pub hero: Vec3,
    pub hero_direction: Vec3,
    pub median_edge: f32,
    pub avenue: bool,
    pub shadow_focus: Option<(Vec3, f32)>,
    pub relief: Vec3,
    pub sun: Vec3,
}
impl Scene {
    pub fn validate(&self) -> Result<(), String> {
        if self.instances.is_empty() || self.instances.len() > 10000 {
            return Err("scene needs 1..10000 instances".into());
        }
        for instance in &self.instances {
            let m = instance.transform();
            let scale = instance.scale();
            if !m.is_finite() || !scale.is_finite() || scale <= 0.0 {
                return Err("instance requires a finite positive uniform scale".into());
            }
            let axes = [
                m.x_axis.truncate() / scale,
                m.y_axis.truncate() / scale,
                m.z_axis.truncate() / scale,
            ];
            if axes.iter().any(|a| (a.length_squared() - 1.0).abs() > 2e-5)
                || axes[0].dot(axes[1]).abs() > 2e-5
                || axes[0].dot(axes[2]).abs() > 2e-5
                || axes[1].dot(axes[2]).abs() > 2e-5
                || axes[0].cross(axes[1]).dot(axes[2]) < 0.0
                || [m.x_axis.w, m.y_axis.w, m.z_axis.w, m.w_axis.w] != [0.0, 0.0, 0.0, 1.0]
            {
                return Err("instance must be an affine rotation with positive uniform scale (no shear or non-uniform scale)".into());
            }
        }
        Ok(())
    }
    pub fn layout(reader: &Reader<'_>, layout: &str) -> Result<Self, String> {
        let source_bounds = AssetBounds::read(reader);
        // Source metadata for the known Smithsonian OBJ: Y-up. Others default to Z-up.
        let washington = reader.header.source_sha256
            == [
                206, 85, 142, 72, 20, 96, 211, 22, 120, 177, 227, 29, 74, 24, 96, 44, 190, 222,
                122, 137, 250, 131, 173, 221, 241, 9, 195, 189, 91, 229, 60, 185,
            ];
        let basis = if washington {
            Mat4::from_rotation_x(std::f32::consts::FRAC_PI_2)
        } else {
            Mat4::IDENTITY
        };
        let mut bounds = AssetBounds {
            min: Vec3::splat(f32::INFINITY),
            max: Vec3::splat(f32::NEG_INFINITY),
        };
        for corner in corners(source_bounds) {
            let p = basis.transform_point3(corner);
            bounds.min = bounds.min.min(p);
            bounds.max = bounds.max.max(p);
        }
        let extent = bounds.max - bounds.min;
        if !extent.is_finite() || extent.max_element() <= 0.0 {
            return Err("asset extent must be finite and positive".into());
        }
        let scale = 2.0 / extent.max_element();
        let origin = Vec3::new(bounds.center().x, bounds.center().y, bounds.min.z);
        let base = Mat4::from_scale(Vec3::splat(scale)) * Mat4::from_translation(-origin) * basis;
        let (kind, count, mut seed) = if layout == "single" {
            ("single", 1, 1u32)
        } else {
            let (kind, count) = layout
                .split_once(':')
                .ok_or("layout must be single|ring:N|grid:N|field:N,seed")?;
            let (count, seed) = count.split_once(',').unwrap_or((count, "1"));
            (
                kind,
                count.parse::<u32>().map_err(|_| "bad instance count")?,
                seed.parse::<u32>().map_err(|_| "bad seed")?,
            )
        };
        if !(1..=10000).contains(&count) {
            return Err("instance count must be 1..10000".into());
        }
        let side = (count as f32).sqrt().ceil();
        let mut instances = Vec::new();
        let mut lo = Vec3::splat(f32::INFINITY);
        let mut hi = Vec3::splat(f32::NEG_INFINITY);
        for i in 0..count {
            let mut random = || {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                (seed >> 8) as f32 / 16777216.0
            };
            let (p, angle, size) = match kind {
                "single" => (Vec3::ZERO, 0.0, 1.0),
                "avenue" => {
                    if i == 0 {
                        (Vec3::ZERO, 0.0, 1.0)
                    } else {
                        let row = ((i - 1) / 2) as f32;
                        let side = if i % 2 == 1 { -1.0 } else { 1.0 };
                        (
                            Vec3::new(side * 3.2, -4.0 - row * 3.5, 0.0),
                            -side * 0.12,
                            1.0,
                        )
                    }
                }
                "ring" => {
                    let a = i as f32 / count as f32 * std::f32::consts::TAU;
                    let r = (count as f32 * 0.5).max(3.0);
                    (Vec3::new(a.cos() * r, a.sin() * r, 0.0), a, 1.0)
                }
                "grid" => (
                    Vec3::new(
                        (i as f32 % side - (side - 1.0) * 0.5) * 3.0,
                        ((i as f32 / side).floor() - (side - 1.0) * 0.5) * 3.0,
                        0.0,
                    ),
                    (i % 4) as f32 * 0.17,
                    0.85 + (i % 5) as f32 * 0.075,
                ),
                "field" => (
                    Vec3::new(
                        (random() - 0.5) * side * 3.5,
                        (random() - 0.5) * side * 3.5,
                        0.0,
                    ),
                    random() * std::f32::consts::TAU,
                    0.65 + random() * 0.7,
                ),
                _ => return Err("unknown layout".into()),
            };
            let m = Mat4::from_scale_rotation_translation(
                Vec3::splat(size),
                Quat::from_rotation_z(angle),
                p,
            ) * base;
            for corner in corners(source_bounds) {
                let p = m.transform_point3(corner);
                lo = lo.min(p);
                hi = hi.max(p);
            }
            instances.push(Instance {
                matrix: m.to_cols_array(),
            });
        }
        let single_corners = corners(source_bounds).map(|p| base.transform_point3(p));
        let single_min = single_corners
            .iter()
            .fold(Vec3::splat(f32::INFINITY), |a, &b| a.min(b));
        let single_max = single_corners
            .iter()
            .fold(Vec3::splat(f32::NEG_INFINITY), |a, &b| a.max(b));
        let (hero, direction) =
            crate::hero::target(reader, base, single_min, single_max, washington);
        let first = instances[0].transform() * base.inverse();
        let scene = Self {
            hero: first.transform_point3(hero),
            hero_direction: first.transform_vector3(direction).normalize(),
            median_edge: crate::hero::median_edge(reader, scale),
            avenue: kind == "avenue",
            shadow_focus: None,
            sun: Vec3::new(-0.75, -0.65, 0.85).normalize(),
            relief: first.transform_point3(Vec3::new(-0.27, -0.37, 0.43)),
            instances,
            center: (lo + hi) * 0.5,
            radius: (hi - lo).length() * 0.5,
        };
        scene.validate()?;
        Ok(scene)
    }
    pub fn camera(&self, t: f32, aspect: f32, fov: f32) -> Camera {
        let t = t.clamp(0.0, 1.0);
        // Two pixels per median source edge at 1440p; a larger artistic floor avoids scan holes.
        let distance = self.minimum_distance(fov).max(0.38);
        let close = self.hero
            + if self.avenue {
                Vec3::new(0.85, -0.35, 0.28).normalize() * (distance + 0.40)
            } else {
                self.hero_direction * distance
            };
        let far = self.center
            + Vec3::new(0.85, -1.6, 0.85).normalize() * self.radius / (fov * 0.5).sin() * 1.1;
        let (eye, target) = if self.avenue {
            let eye = crate::hero::spline(
                t,
                &[
                    (0.0, Vec3::new(0.2, -30.0, 5.2)),
                    (0.2, Vec3::new(0.0, -13.0, 2.1)),
                    (0.4, Vec3::new(1.0, -3.4, 2.4)),
                    (0.56, self.hero + Vec3::new(0.8, -1.35, 0.55)),
                    (0.70, close),
                    (0.79, close + Vec3::new(0.10, 0.07, 0.025)),
                    (0.855, Vec3::new(0.40, -1.60, 1.25)),
                    (
                        0.94,
                        self.relief + Vec3::new(-0.08, -(distance + 0.15), 0.09),
                    ),
                    (
                        1.0,
                        self.relief + Vec3::new(-0.08, -(distance + 0.15), 0.09),
                    ),
                ],
            );
            let target = crate::hero::spline(
                t,
                &[
                    (0.0, Vec3::new(0.0, -12.0, 0.9)),
                    (0.2, Vec3::new(0.0, 0.0, 0.95)),
                    (0.4, Vec3::new(0.0, 0.0, 1.15)),
                    (0.56, self.hero),
                    (0.70, self.hero - Vec3::Z * 0.28),
                    (0.79, self.hero - Vec3::Z * 0.28),
                    (0.855, self.hero.lerp(self.relief, 0.46)),
                    (0.94, self.relief),
                    (1.0, self.relief),
                ],
            );
            (eye, target)
        } else {
            let s = t * t * t * (t * (t * 6.0 - 15.0) + 10.0);
            (far.lerp(close, s), self.center.lerp(self.hero, s))
        };
        Camera::perspective(eye, target, aspect, fov, 0.002, self.radius * 12.0 + 100.0)
    }
    pub fn minimum_distance(&self, fov: f32) -> f32 {
        self.median_edge * 1440.0 / (4.0 * (fov * 0.5).tan())
    }
    /// Source-triangle clearance at deterministic path samples, accelerated by cluster spheres.
    pub fn path_clearance(&self, reader: &Reader<'_>, steps: u32, fov: f32) -> (f32, f32) {
        let bounds = AssetBounds::read(reader);
        let center = bounds.center();
        let radius = (bounds.max - bounds.min).length() * 0.5;
        let mut minimum = f32::INFINITY;
        let mut worst_t = 0.0;
        for step in 0..steps {
            let t = step as f32 / (steps - 1) as f32;
            let eye = self.camera(t, 16.0 / 9.0, fov).eye;
            for instance in &self.instances {
                let scale = instance.scale();
                let source_eye = instance.transform().inverse().transform_point3(eye);
                if (source_eye.distance(center) - radius) * scale >= minimum {
                    continue;
                }
                let distance =
                    crate::hero::surface_distance(reader, source_eye, minimum / scale) * scale;
                if distance < minimum {
                    minimum = distance;
                    worst_t = t;
                }
            }
        }
        (minimum, worst_t)
    }
    pub fn fit_shadow(&mut self, t: f32) {
        if self.avenue {
            let ease = |x: f32| {
                let x = x.clamp(0.0, 1.0);
                x * x * x * (x * (x * 6.0 - 15.0) + 10.0)
            };
            let portrait = ease((t - 0.50) / 0.15) * (1.0 - ease((t - 0.80) / 0.12));
            self.sun = Vec3::new(-0.75, -0.65, 0.85)
                .lerp(Vec3::new(0.30, -0.85, 0.85), portrait)
                .normalize();
            let s = (t / 0.55).clamp(0.0, 1.0);
            let s = s * s * s * (s * (s * 6.0 - 15.0) + 10.0);
            self.shadow_focus = Some((
                self.center.lerp(Vec3::new(0.0, 0.0, 0.9), s),
                self.radius * 1.05 * (1.0 - s) + 1.7 * s,
            ));
        }
    }
    pub fn light_camera(&self) -> Camera {
        let sun = self.sun;
        let (center, r) = self
            .shadow_focus
            .unwrap_or((self.center, self.radius * 1.05));
        let depth = self.radius.max(r) * 6.0;
        let eye = center + sun * depth * 0.5;
        let matrix = glam::camera::rh::proj::directx::orthographic(-r, r, -r, r, 0.01, depth)
            * glam::camera::rh::view::look_at_mat4(eye, center, Vec3::Z);
        Camera {
            eye,
            matrix,
            near: 0.01,
            cot: 1.0,
            orthographic_span: Some(r * 2.0),
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub eye: Vec3,
    pub matrix: Mat4,
    pub near: f32,
    pub cot: f32,
    pub orthographic_span: Option<f32>,
}
impl Camera {
    pub fn perspective(
        eye: Vec3,
        target: Vec3,
        aspect: f32,
        fov: f32,
        near: f32,
        far: f32,
    ) -> Self {
        Self {
            eye,
            matrix: glam::camera::rh::proj::directx::perspective(fov, aspect, far, near)
                * glam::camera::rh::view::look_at_mat4(eye, target, Vec3::Z),
            near,
            cot: 1.0 / (fov * 0.5).tan(),
            orthographic_span: None,
        }
    }
    pub fn planes(self) -> [Vec4; 6] {
        let m = self.matrix.transpose();
        [
            m.w_axis + m.x_axis,
            m.w_axis - m.x_axis,
            m.w_axis + m.y_axis,
            m.w_axis - m.y_axis,
            m.z_axis,
            m.w_axis - m.z_axis,
        ]
        .map(|p| p / p.truncate().length())
    }
}

fn corners(bounds: AssetBounds) -> [Vec3; 8] {
    std::array::from_fn(|i| {
        Vec3::new(
            if i & 1 == 0 {
                bounds.min.x
            } else {
                bounds.max.x
            },
            if i & 2 == 0 {
                bounds.min.y
            } else {
                bounds.max.y
            },
            if i & 4 == 0 {
                bounds.min.z
            } else {
                bounds.max.z
            },
        )
    })
}
