//! The crypto list's sparkline surface (LLP 1009), one instance per row canvas.
//!
//! `spark(series, up, freeze)`: the 48-point series drawn as a polyline into the
//! canvas's inner 96 × 32 pt box (the canvas is that box outset by 9 pt), a 1.5 pt
//! stroke with round joins and caps, green or red by `up`. The line draws in over
//! 600 ms (CSS `ease-out`, by length) from the instance's first frame — a row's
//! canvas is created each time the row comes on screen — then a dot and a
//! breathing ring (radius 3 → 9, opacity 0.5 → 0, 1,200 ms, `ease-out`, forever)
//! sit on the last point, timed by the host's frame clock (`Frame::now_ms`). A new
//! series redraws at once without replaying the draw-in; the pulse keeps its
//! phase. `freeze` settles everything: full line, ring at radius 6, opacity 0.25.

#![deny(missing_docs)]

use std::cell::RefCell;

use exact_gpu::json::number;
use exact_gpu::wgpu;
use exact_gpu::{Frame, Registry, Surface, SurfaceError, Value};

/// Shader interfaces reflected from WGSL during the build.
pub mod shaders {
    include!(concat!(env!("OUT_DIR"), "/shaders.rs"));
}

use shaders::spark::{entry, module, Uniforms, GROUP_0, U};

const POINTS: usize = 48;
const INSET: f32 = 9.0;
const CHART_W: f32 = 96.0;
const CHART_H: f32 = 32.0;
const DRAW_MS: f64 = 600.0;
const PULSE_MS: f64 = 1200.0;

/// CSS `ease-out`: cubic-bezier(0, 0, 0.58, 1) at progress `x` in 0..=1.
pub fn ease_out(x: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }
    // B(t) with P1 = (0, 0), P2 = (0.58, 1): x(t) = 3(1-t)t²·0.58 + t³, y(t) = 3(1-t)t² + t³.
    let bx = |t: f64| 3.0 * (1.0 - t) * t * t * 0.58 + t * t * t;
    let by = |t: f64| 3.0 * (1.0 - t) * t * t + t * t * t;
    let (mut lo, mut hi) = (0.0, 1.0);
    for _ in 0..30 {
        let mid = 0.5 * (lo + hi);
        if bx(mid) < x {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    by(0.5 * (lo + hi))
}

/// `#16a34a` / `#dc2626` as 0–1 sRGB.
fn line_color(up: bool) -> [f32; 3] {
    let (r, g, b) = if up { (0x16, 0xa3, 0x4a) } else { (0xdc, 0x26, 0x26) };
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0]
}

fn to_linear(c: f32) -> f32 {
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

/// The pipeline every instance shares (per format and shader generation).
struct Shared {
    key: (wgpu::TextureFormat, u32),
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
}

thread_local! {
    static SHARED: RefCell<Option<Shared>> = const { RefCell::new(None) };
}

struct Gpu {
    key: (wgpu::TextureFormat, u32),
    /// The per-frame uniform, written in place (LLP 1009 D7), and a bind group per slot.
    uniforms: exact_gpu::FrameUniform,
    groups: Vec<wgpu::BindGroup>,
    pipeline: wgpu::RenderPipeline,
}

/// One row's sparkline.
#[derive(Default)]
pub struct Spark {
    series: Vec<f64>,
    up: bool,
    freeze: bool,
    /// The first frame's clock: the draw-in starts there.
    start_ms: Option<f64>,
    gpu: Option<Gpu>,
}

impl Spark {
    /// The points in canvas points (x, y, cumulative length) and the total length.
    fn points(&self) -> ([[f32; 4]; POINTS], f32) {
        let mut pts = [[0.0f32; 4]; POINTS];
        let (lo, hi) = self.series.iter().fold((f64::MAX, f64::MIN), |(lo, hi), v| (lo.min(*v), hi.max(*v)));
        let n = self.series.len().min(POINTS);
        let mut cum = 0.0f32;
        for i in 0..POINTS {
            let v = self.series.get(i.min(n.saturating_sub(1))).copied().unwrap_or(0.0);
            let y = if hi > lo { CHART_H - CHART_H * ((v - lo) / (hi - lo)) as f32 } else { CHART_H / 2.0 };
            let x = CHART_W * i as f32 / (POINTS - 1) as f32;
            let (x, y) = (INSET + x, INSET + y);
            if i > 0 {
                let [px, py, ..] = pts[i - 1];
                cum += ((x - px).powi(2) + (y - py).powi(2)).sqrt();
            }
            pts[i] = [x, y, cum, 0.0];
        }
        (pts, cum)
    }

    /// Draw-in fraction, ring radius, ring alpha, dot radius at `now`.
    fn anim(&self, now: f64) -> (f32, f32, f32, f32) {
        if self.freeze {
            return (1.0, 6.0, 0.25, 3.0);
        }
        let t = now - self.start_ms.unwrap_or(now);
        if t < DRAW_MS {
            return (ease_out(t / DRAW_MS) as f32, 0.0, 0.0, 0.0);
        }
        let e = ease_out(((t - DRAW_MS) % PULSE_MS) / PULSE_MS) as f32;
        (1.0, 3.0 + 6.0 * e, 0.5 * (1.0 - e), 3.0)
    }
}

impl Surface for Spark {
    fn bind(&mut self, inputs: &[Value], _: Option<f64>) -> Result<(), SurfaceError> {
        let [series, up, freeze] = inputs else {
            return Err(SurfaceError(format!("spark: expected 3 inputs, got {}", inputs.len())));
        };
        let Value::List(items) = series else {
            return Err(SurfaceError("spark.series: expected a list of numbers".into()));
        };
        let series = items.iter().map(|v| number(v, "series")).collect::<Result<Vec<_>, _>>()?;
        let (Value::Bool(up), Value::Bool(freeze)) = (up, freeze) else {
            return Err(SurfaceError("spark.up, spark.freeze: expected booleans".into()));
        };
        self.series = series;
        self.up = *up;
        self.freeze = *freeze;
        Ok(())
    }

    fn render(
        &mut self,
        frame: &Frame,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        format: wgpu::TextureFormat,
    ) -> bool {
        let key = (format, frame.shader_generation);
        if self.gpu.as_ref().map(|g| g.key) != Some(key) {
            self.gpu = Some(build(device, key));
        }
        self.start_ms.get_or_insert(frame.now_ms);
        let (pts, total) = self.points();
        let (draw, ring_r, ring_a, dot_r) = self.anim(frame.now_ms);
        let mut color = line_color(self.up);
        if format.is_srgb() {
            color = color.map(to_linear);
        }
        let uniforms = Uniforms {
            pts,
            color: [color[0], color[1], color[2], 1.0],
            size: [frame.width as f32, frame.height as f32, frame.scale as f32, 0.75],
            anim: [draw * total, ring_r, ring_a, dot_r],
        };
        let gpu = self.gpu.as_mut().unwrap();
        let slot = gpu.uniforms.write(queue, &uniforms.bytes());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("spark"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::WHITE), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&gpu.pipeline);
            pass.set_bind_group(0, &gpu.groups[slot], &[]);
            pass.draw(0..3, 0..1);
        }
        // The pulse repeats forever; a frozen chart is still.
        !self.freeze
    }
}

fn build(device: &wgpu::Device, key: (wgpu::TextureFormat, u32)) -> Gpu {
    let (pipeline, layout) = SHARED.with(|s| {
        let mut s = s.borrow_mut();
        if s.as_ref().map(|s| s.key) != Some(key) {
            let shader = device.create_shader_module(module());
            let layout = device.create_bind_group_layout(&GROUP_0);
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("spark"),
                bind_group_layouts: &[Some(&layout)],
                immediate_size: 0,
            });
            let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("spark"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some(entry::VS),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(entry::FS),
                    targets: &[Some(key.0.into())],
                    compilation_options: Default::default(),
                }),
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            });
            *s = Some(Shared { key, pipeline, layout });
        }
        let s = s.as_ref().unwrap();
        (s.pipeline.clone(), s.layout.clone())
    });
    let uniforms = exact_gpu::FrameUniform::new(device, Uniforms::SIZE, "spark uniforms");
    let groups = (0..uniforms.slots())
        .map(|slot| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("spark"),
                layout: &layout,
                entries: &[wgpu::BindGroupEntry { binding: U.binding, resource: uniforms.buffer(slot).as_entire_binding() }],
            })
        })
        .collect();
    Gpu { key, uniforms, groups, pipeline }
}

fn spark() -> Box<dyn Surface> {
    Box::new(Spark::default())
}

/// The module's one surface and its reflected shader.
pub static REGISTRY: Registry = Registry { surfaces: &[("spark", 3, spark)], shaders: shaders::SHADERS };

exact_gpu::module!(REGISTRY);

#[cfg(test)]
mod tests {
    use super::*;
    use exact_gpu::fixture;

    #[test]
    fn ease_out_matches_css() {
        assert!((ease_out(0.0)).abs() < 1e-6);
        assert!((ease_out(1.0) - 1.0).abs() < 1e-6);
        // cubic-bezier(0,0,0.58,1) at 0.5 ≈ 0.6866 (Chrome's value).
        assert!((ease_out(0.5) - 0.6866).abs() < 2e-3, "{}", ease_out(0.5));
    }

    #[test]
    fn draws_in_then_pulses() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("shaders");
        exact_gpu::shaders::load_dir(&dir, &REGISTRY).unwrap();
        let gpu = match fixture::device() {
            Ok(gpu) => gpu,
            Err(error) => {
                eprintln!("{error}; spark readback skipped");
                return;
            }
        };
        let series: Vec<Value> = (0..48).map(|i| Value::Number((i as f64 * 0.4).sin() + i as f64 * 0.05)).collect();
        let mut s = Spark::default();
        s.bind(&[Value::list(series.clone()), Value::Bool(true), Value::Bool(false)], None).unwrap();
        let mut frame = Frame {
            width: 114.0,
            height: 50.0,
            scale: 2.0,
            now_ms: 1000.0,
            children_generation: 0,
            seekable: true,
            period_ms: 8.33,
            shader_generation: exact_gpu::shaders::shader_generation(),
        };
        let (first, more) = fixture::render(&gpu, &mut s, &frame).unwrap();
        assert!(more);
        let ink = |p: &fixture::Pixels| (0..100).flat_map(|y| (0..228).map(move |x| (x, y))).filter(|&(x, y)| p.at(x, y)[0] < 200).count();
        first.save("spark-first");
        assert_eq!(ink(&first), 0, "nothing drawn at t = 0");
        frame.now_ms += 300.0;
        let (half, _) = fixture::render(&gpu, &mut s, &frame).unwrap();
        frame.now_ms += 1000.0;
        let (full, _) = fixture::render(&gpu, &mut s, &frame).unwrap();
        assert!(ink(&half) > 0 && ink(&half) < ink(&full), "{} {}", ink(&half), ink(&full));
        full.save("spark-full");
        s.bind(&[Value::list(series), Value::Bool(false), Value::Bool(true)], None).unwrap();
        let (frozen, more) = fixture::render(&gpu, &mut s, &frame).unwrap();
        assert!(!more);
        frozen.save("spark-frozen");
    }
}
