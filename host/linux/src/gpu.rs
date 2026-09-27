//! The GPU backend: vello over wgpu — the main painter. The walk's shapes,
//! images, and glyph runs are encoded into a vello scene, rendered into an
//! `Rgba8Unorm` texture, and read back into the frame the presenter and the
//! display already consume. The device is created at boot; vello's
//! pipelines are compiled then, behind a wgpu pipeline cache persisted to
//! disk where the driver supports one (Vulkan), so only the first launch on
//! a machine pays for the shaders.
//!
//! @ref LLP 1015 §2 (r2: the GPU painter); LLP 1009 D1 (wgpu is the one
//! GPU API on every host) and D5 (shaders — here on the boot path, the
//! trade Charlie took 2026-08-29)

#![allow(unsafe_code)]

use crate::image::Bitmap;
use crate::paint::border::{BorderFill, PathOp};
use crate::paint::{Backend, Rect4, Shape, POINTER};
use crate::text::{Paragraph, RunPaint, TextEngine};
mod images;
use crate::paint::GradientPaint;
use exact_kernel::gradient::Geometry;
use images::ImageCache;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tiny_skia::{IntSize, Pixmap, Transform};
use vello::kurbo::{Affine, BezPath, Rect, Stroke};
use vello::peniko::{Color, Fill, Gradient, ImageBrush, Mix};
use vello::{AaConfig, AaSupport, RenderParams, Renderer, RendererOptions};

struct Target {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    buffer: wgpu::Buffer,
    width: u32,
    height: u32,
    padded: u32,
}

/// The vello backend on one device.
pub struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: Renderer,
    image_refused: bool,
    scene: vello::Scene,
    scale: f32,
    width: f32,
    height: f32,
    target: Option<Target>,
    /// Image brushes by the picture's address, each entry holding the
    /// picture so the address cannot be reused while the brush is cached.
    images: ImageCache,
    /// The adapter's name.
    pub adapter: String,
    /// The wgpu backend's name (`Vulkan`, `Metal`, …).
    pub api: String,
    /// Instance, adapter, and device, milliseconds.
    pub device_ms: f64,
    /// vello's renderer — the shaders — milliseconds.
    pub shaders_ms: f64,
    /// Whether a pipeline cache file for this adapter was found and handed
    /// to the driver (whether it was used shows in `shaders_ms`).
    pub cached: bool,
    /// The last frame's encode, render, and readback, milliseconds.
    pub last_ms: (f64, f64),
}

/// Run a future to completion on this thread (wgpu's requests complete
/// synchronously on native backends).
fn block_on<F: std::future::Future>(f: F) -> F::Output {
    use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
    fn noop(_: *const ()) {}
    fn clone(p: *const ()) -> RawWaker {
        RawWaker::new(p, &VTABLE)
    }
    static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, noop, noop, noop);
    // SAFETY: the vtable's functions do nothing and the data pointer is null and never read.
    let waker = unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &VTABLE)) };
    let mut cx = Context::from_waker(&waker);
    let mut f = std::pin::pin!(f);
    loop {
        if let Poll::Ready(v) = f.as_mut().poll(&mut cx) {
            return v;
        }
        std::thread::yield_now();
    }
}

/// Where the pipeline cache for an adapter lives: `EXACT_CACHE`, else
/// `$XDG_CACHE_HOME/exact`, else `~/.cache/exact`.
fn cache_path(key: &str) -> Option<PathBuf> {
    let dir = std::env::var_os("EXACT_CACHE")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_CACHE_HOME").map(|d| PathBuf::from(d).join("exact")))
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache/exact")))?;
    let safe: String = key
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    Some(dir.join(format!("pipelines-{safe}.bin")))
}

impl Gpu {
    /// Create the device and vello's renderer; `Err` names why there is no
    /// GPU (no adapter, no device, a renderer that failed).
    pub fn new() -> Result<Gpu, String> {
        let t0 = Instant::now();
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        }))
        .map_err(|e| format!("no adapter: {e}"))?;
        let info = adapter.get_info();
        let caching = adapter.features().contains(wgpu::Features::PIPELINE_CACHE);
        let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("exact"),
            required_features: if caching {
                wgpu::Features::PIPELINE_CACHE
            } else {
                wgpu::Features::empty()
            },
            ..Default::default()
        }))
        .map_err(|e| format!("no device: {e}"))?;
        let device_ms = t0.elapsed().as_secs_f64() * 1000.0;
        let t1 = Instant::now();
        let path = if caching {
            wgpu::util::pipeline_cache_key(&info).and_then(|k| cache_path(&k))
        } else {
            None
        };
        let data = path.as_ref().and_then(|p| std::fs::read(p).ok());
        let cached = data.is_some();
        let cache = if caching {
            // SAFETY: the bytes are what this driver's cache wrote earlier on
            // this machine (or nothing); wgpu validates the header and
            // `fallback` makes a mismatch an empty cache, never a crash.
            Some(unsafe {
                device.create_pipeline_cache(&wgpu::PipelineCacheDescriptor {
                    label: Some("exact"),
                    data: data.as_deref(),
                    fallback: true,
                })
            })
        } else {
            None
        };
        let renderer = Renderer::new(
            &device,
            RendererOptions {
                use_cpu: false,
                antialiasing_support: AaSupport::area_only(),
                num_init_threads: if cfg!(target_os = "macos") {
                    NonZeroUsize::new(1)
                } else {
                    None
                },
                pipeline_cache: cache.clone(),
            },
        )
        .map_err(|e| format!("vello: {e}"))?;
        let shaders_ms = t1.elapsed().as_secs_f64() * 1000.0;
        if let (Some(cache), Some(path)) = (cache.as_ref(), path) {
            if let Some(bytes) = cache.get_data() {
                if data.as_deref() != Some(bytes.as_slice()) {
                    // Whole or absent, never truncated: written beside and
                    // renamed into place, so a crash mid-write or two
                    // launches at once leave the old file or none.
                    if let Some(dir) = path.parent() {
                        let _ = std::fs::create_dir_all(dir);
                    }
                    let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
                    if std::fs::write(&tmp, bytes).is_ok() && std::fs::rename(&tmp, &path).is_err()
                    {
                        let _ = std::fs::remove_file(&tmp);
                    }
                }
            }
        }
        Ok(Gpu {
            device,
            queue,
            renderer,
            image_refused: false,
            scene: vello::Scene::new(),
            scale: 1.0,
            width: 1.0,
            height: 1.0,
            target: None,
            images: ImageCache::default(),
            adapter: info.name.clone(),
            api: format!("{:?}", info.backend),
            device_ms,
            shaders_ms,
            cached,
            last_ms: (0.0, 0.0),
        })
    }

    fn affine(&self, ts: Transform) -> Affine {
        Affine::scale(self.scale as f64)
            * Affine::new([
                ts.sx as f64,
                ts.ky as f64,
                ts.kx as f64,
                ts.sy as f64,
                ts.tx as f64,
                ts.ty as f64,
            ])
    }

    fn target(&mut self, width: u32, height: u32) -> &Target {
        let stale = self
            .target
            .as_ref()
            .is_none_or(|t| t.width != width || t.height != height);
        if stale {
            let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("exact frame"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::STORAGE_BINDING
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            let view = texture.create_view(&Default::default());
            let padded = (width * 4).div_ceil(256) * 256;
            let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("exact readback"),
                size: (padded * height) as u64,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            self.target = Some(Target {
                texture,
                view,
                buffer,
                width,
                height,
                padded,
            });
        }
        self.target.as_ref().expect("just made")
    }

    fn brush(&mut self, image: &Arc<Bitmap>) -> Option<ImageBrush> {
        let renderer = &mut self.renderer;
        let device = &self.device;
        let queue = &self.queue;
        self.images.brush(image, |data, pixels| {
            let size = wgpu::Extent3d {
                width: pixels.width(),
                height: pixels.height(),
                depth_or_array_layers: 1,
            };
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("exact raster"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            // Borrow charged pixels directly. No Exact-owned staging Vec or
            // Blob copy; GPU texture/atlas and wgpu internal staging are separate.
            queue.write_texture(
                texture.as_image_copy(),
                pixels.as_ref(),
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(pixels.width() * 4),
                    rows_per_image: Some(pixels.height()),
                },
                size,
            );
            renderer.override_image(
                data,
                Some(wgpu::TexelCopyTextureInfoBase {
                    texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                }),
            );
        })
    }
}

/// A shape's path with its CSS-reduced radii. kurbo's `RoundedRect` would
/// clamp each radius to half the shorter side, which CSS does not (a 40pt
/// corner on a 70pt-high box whose neighbours fit keeps 40), so the path is
/// the CPU painter's cubic arcs.
fn shape(s: &Shape) -> BezPath {
    let mut ops = Vec::new();
    crate::paint::border::rounded_rect(&mut ops, s.rect, s.radii.map(|r| (r, r)));
    bez(&ops)
}

/// A border part's path for vello.
fn bez(ops: &[PathOp]) -> BezPath {
    let mut b = BezPath::new();
    let p = |x: f32, y: f32| (x as f64, y as f64);
    for op in ops {
        match *op {
            PathOp::Move(x, y) => b.move_to(p(x, y)),
            PathOp::Line(x, y) => b.line_to(p(x, y)),
            PathOp::Cubic(a, c, d, e, f, g) => b.curve_to(p(a, c), p(d, e), p(f, g)),
            PathOp::Close => b.close_path(),
        }
    }
    b
}

/// A `path` node's normalized commands as a kurbo path (LLP 1065 D7).
fn vector_path(commands: &[exact_kernel::vector::Command]) -> BezPath {
    use exact_kernel::vector::Command;
    let p = |[x, y]: [f32; 2]| (f64::from(x), f64::from(y));
    let mut b = BezPath::new();
    for command in commands {
        match *command {
            Command::Move(a) => b.move_to(p(a)),
            Command::Line(a) => b.line_to(p(a)),
            Command::Cubic(c1, c2, a) => b.curve_to(p(c1), p(c2), p(a)),
            Command::Close => b.close_path(),
        }
    }
    b
}

fn color(c: [u8; 4]) -> Color {
    Color::from_rgba8(c[0], c[1], c[2], c[3])
}

impl Backend for Gpu {
    fn name(&self) -> &'static str {
        "gpu"
    }

    fn begin(&mut self, width: f32, height: f32, scale: f32) {
        self.scale = scale;
        self.width = width;
        self.height = height;
        self.scene.reset();
        let renderer = &mut self.renderer;
        self.images
            .begin(|image| renderer.unregister_texture(image));
        self.image_refused = false;
    }

    fn fill(&mut self, s: &Shape, c: [u8; 4], ts: Transform) {
        if s.rect.2 <= 0.0 || s.rect.3 <= 0.0 {
            return;
        }
        let a = self.affine(ts);
        self.scene.fill(Fill::NonZero, a, color(c), None, &shape(s));
    }

    fn vector(&mut self, v: &crate::paint::VectorPaint, ts: Transform) {
        use exact_kernel::{FillRule, StrokeLinecap, StrokeLinejoin};
        use vello::kurbo::{Cap, Join, StrokeOpts};
        let a = self.affine(ts.pre_concat(v.fit));
        if let Some((c, d)) = &v.fill {
            let rule = match v.rule {
                FillRule::Nonzero => Fill::NonZero,
                FillRule::Evenodd => Fill::EvenOdd,
            };
            self.scene.fill(rule, a, color(*c), None, &vector_path(d));
        }
        let Some((c, d)) = v.stroke.as_ref().filter(|_| v.width > 0.0) else {
            return;
        };
        let a = self.affine(ts.pre_concat(v.stroke_fit));
        let stroke = Stroke::new(f64::from(v.width))
            .with_caps(match v.cap {
                StrokeLinecap::Butt => Cap::Butt,
                StrokeLinecap::Round => Cap::Round,
                StrokeLinecap::Square => Cap::Square,
            })
            .with_join(match v.join {
                StrokeLinejoin::Miter => Join::Miter,
                StrokeLinejoin::Round => Join::Round,
                StrokeLinejoin::Bevel => Join::Bevel,
            })
            .with_miter_limit(f64::from(v.miter));
        // A dashed stroke shows through its trimmed, undashed stroke: that
        // stroke's outline, to a tenth of a device pixel, is the clip.
        let reveal = v.reveal.as_deref().map(|trim| {
            let tolerance = 0.1
                / a.as_coeffs()[..4]
                    .iter()
                    .fold(1e-9f64, |m, c| m.max(c.abs()));
            vello::kurbo::stroke(
                vector_path(trim),
                &stroke,
                &StrokeOpts::default(),
                tolerance,
            )
        });
        if let Some(outline) = &reveal {
            self.scene.push_clip_layer(Fill::NonZero, a, outline);
        }
        self.scene
            .stroke(&stroke, a, color(*c), None, &vector_path(d));
        // Vello caps no zero-length subpath: SVG's dot (LLP 1065 D9) is its
        // cap, a disc or a square as wide as the stroke.
        let r = f64::from(v.width) / 2.0;
        for (i, pair) in d.windows(2).enumerate() {
            use exact_kernel::vector::Command;
            let [Command::Move(p), Command::Line(q)] = pair else {
                continue;
            };
            // The whole subpath, not a first segment of no length.
            if p != q || !matches!(d.get(i + 2), None | Some(Command::Move(_))) {
                continue;
            }
            let (x, y) = (f64::from(p[0]), f64::from(p[1]));
            match v.cap {
                StrokeLinecap::Round => {
                    let dot = vello::kurbo::Circle::new((x, y), r);
                    self.scene.fill(Fill::NonZero, a, color(*c), None, &dot);
                }
                StrokeLinecap::Square => {
                    let dot = Rect::new(x - r, y - r, x + r, y + r);
                    self.scene.fill(Fill::NonZero, a, color(*c), None, &dot);
                }
                StrokeLinecap::Butt => {}
            }
        }
        if reveal.is_some() {
            self.scene.pop_layer();
        }
    }

    fn fill_gradient(&mut self, s: &Shape, g: &GradientPaint, ts: Transform) {
        if s.rect.2 <= 0.0 || s.rect.3 <= 0.0 {
            return;
        }
        // Vello mixes premultiplied by default, as CSS does: the stops as given.
        let stops: Vec<(f32, Color)> = g
            .stops
            .iter()
            .map(|&(at, c)| (at, color(crate::paint::rgba(c))))
            .collect();
        let (brush, placed) = match g.geometry {
            Geometry::Linear { start, end } => (Gradient::new_linear(start, end), None),
            // The unit circle, scaled to the ellipse.
            Geometry::Radial { center, radii } => (
                Gradient::new_radial((0.0, 0.0), 1.0),
                Some(Affine::new([
                    radii.0 as f64,
                    0.0,
                    0.0,
                    radii.1 as f64,
                    center.0 as f64,
                    center.1 as f64,
                ])),
            ),
        };
        let a = self.affine(ts);
        self.scene.fill(
            Fill::NonZero,
            a,
            &brush.with_stops(stops.as_slice()),
            placed,
            &shape(s),
        );
    }

    fn fill_border(&mut self, part: &BorderFill, ts: Transform) {
        let a = self.affine(ts);
        if let Some(clip) = &part.clip {
            self.scene.push_clip_layer(Fill::NonZero, a, &bez(clip));
        }
        self.scene.fill(
            Fill::EvenOdd,
            a,
            color(part.color),
            None,
            &bez(&part.region),
        );
        if part.clip.is_some() {
            self.scene.pop_layer();
        }
    }

    fn image(&mut self, image: &Arc<Bitmap>, dst: Rect4, clips: &[Shape], ts: Transform) {
        let (nw, nh) = (image.width() as f64, image.height() as f64);
        if nw <= 0.0 || nh <= 0.0 || dst.2 <= 0.0 || dst.3 <= 0.0 {
            return;
        }
        let Some(brush) = self.brush(image) else {
            self.image_refused = true;
            return;
        };
        let a = self.affine(ts);
        for c in clips {
            self.scene.push_clip_layer(Fill::NonZero, a, &shape(c));
        }
        let place = a
            * Affine::translate((dst.0 as f64, dst.1 as f64))
            * Affine::scale_non_uniform(dst.2 as f64 / nw, dst.3 as f64 / nh);
        self.scene.draw_image(&brush, place);
        for _ in clips {
            self.scene.pop_layer();
        }
    }

    fn surface_image(&mut self, image: Arc<Pixmap>, dst: Rect4) {
        let clips: &[Shape] = &[];
        let ts = Transform::identity();
        struct Pixels(Arc<Pixmap>);
        impl AsRef<[u8]> for Pixels {
            fn as_ref(&self) -> &[u8] {
                self.0.data()
            }
        }
        let (nw, nh) = (image.width() as f64, image.height() as f64);
        if nw <= 0.0 || nh <= 0.0 || dst.2 <= 0.0 || dst.3 <= 0.0 {
            return;
        }
        let brush = vello::peniko::ImageBrush::new(vello::peniko::ImageData {
            data: vello::peniko::Blob::new(Arc::new(Pixels(image.clone()))),
            format: vello::peniko::ImageFormat::Rgba8,
            alpha_type: vello::peniko::ImageAlphaType::AlphaPremultiplied,
            width: image.width(),
            height: image.height(),
        });
        let a = self.affine(ts);
        for c in clips {
            self.scene.push_clip_layer(Fill::NonZero, a, &shape(c));
        }
        let place = a
            * Affine::translate((dst.0 as f64, dst.1 as f64))
            * Affine::scale_non_uniform(dst.2 as f64 / nw, dst.3 as f64 / nh);
        self.scene.draw_image(&brush, place);
        for _ in clips {
            self.scene.pop_layer();
        }
    }

    fn text(
        &mut self,
        text: &mut TextEngine,
        paragraph: &Paragraph,
        palette: &[RunPaint],
        origin: (f32, f32),
        ts: Transform,
    ) {
        let a = self.affine(ts) * Affine::translate((origin.0 as f64, origin.1 as f64));
        for run in text.glyph_runs(paragraph, palette) {
            if run.paint.color[3] == 0 {
                continue;
            }
            self.scene
                .draw_glyphs(&run.font)
                .font_size(run.size)
                .brush(color(run.paint.color))
                .transform(a)
                .glyph_transform(
                    run.synthetic_italic
                        .then(|| Affine::skew(14_f64.to_radians().tan(), 0.0)),
                )
                .hint(true)
                .draw(
                    Fill::NonZero,
                    run.glyphs.iter().map(|(id, x, y)| vello::Glyph {
                        id: *id,
                        x: *x,
                        y: *y,
                    }),
                );
        }
    }

    fn push_clip(&mut self, s: &Shape, ts: Transform) {
        let a = self.affine(ts);
        let clip = if s.rect.2 <= 0.0 || s.rect.3 <= 0.0 {
            vello::kurbo::Shape::to_path(&Rect::ZERO, 0.1)
        } else {
            shape(s)
        };
        self.scene.push_clip_layer(Fill::NonZero, a, &clip);
    }

    fn push_css_clip(&mut self, path: &exact_kernel::clip::ClipPath, ts: Transform) -> bool {
        let rule = match path.rule() {
            exact_kernel::FillRule::Nonzero => Fill::NonZero,
            exact_kernel::FillRule::Evenodd => Fill::EvenOdd,
        };
        self.scene
            .push_clip_layer(rule, self.affine(ts), &vector_path(path.commands()));
        true
    }

    fn pop_clip(&mut self) {
        self.scene.pop_layer();
    }

    fn push_opacity(&mut self, alpha: f32) {
        let whole = Rect::new(
            0.0,
            0.0,
            (self.width * self.scale) as f64,
            (self.height * self.scale) as f64,
        );
        self.scene
            .push_layer(Fill::NonZero, Mix::Normal, alpha, Affine::IDENTITY, &whole);
    }

    fn pop_opacity(&mut self) {
        self.scene.pop_layer();
    }

    fn pointer(&mut self, x: f32, y: f32) {
        let mut path = BezPath::new();
        for (i, (px, py)) in POINTER.iter().enumerate() {
            if i == 0 {
                path.move_to((*px as f64, *py as f64));
            } else {
                path.line_to((*px as f64, *py as f64));
            }
        }
        path.close_path();
        let a = self.affine(Transform::from_translate(x, y));
        self.scene.fill(Fill::NonZero, a, Color::WHITE, None, &path);
        self.scene
            .stroke(&Stroke::new(1.0), a, Color::BLACK, None, &path);
    }

    fn last_frame_ms(&self) -> Option<(f64, f64)> {
        Some(self.last_ms)
    }

    fn finish(&mut self) -> Result<Pixmap, String> {
        let width = ((self.width * self.scale).round() as u32).max(1);
        let height = ((self.height * self.scale).round() as u32).max(1);
        let t0 = Instant::now();
        if self.image_refused {
            return Err("GPU image descriptor capacity exceeded".into());
        }
        let scene = std::mem::take(&mut self.scene);
        let (device, queue) = (self.device.clone(), self.queue.clone());
        let result = {
            let target = self.target(width, height);
            let view = target.view.clone();
            self.renderer
                .render_to_texture(
                    &device,
                    &queue,
                    &scene,
                    &view,
                    &RenderParams {
                        base_color: Color::WHITE,
                        width,
                        height,
                        antialiasing_method: AaConfig::Area,
                    },
                )
                .map_err(|e| format!("vello render: {e}"))
        };
        self.scene = scene;
        result?;
        let target = self.target.as_ref().expect("rendered into it");
        let mut encoder = device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &target.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &target.buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(target.padded),
                    rows_per_image: None,
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        queue.submit([encoder.finish()]);
        let encode_ms = t0.elapsed().as_secs_f64() * 1000.0;
        let t1 = Instant::now();
        let slice = target.buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: None,
            })
            .map_err(|e| format!("poll: {e}"))?;
        rx.recv()
            .map_err(|_| "readback: no answer".to_string())?
            .map_err(|e| format!("readback: {e}"))?;
        let mut data = Vec::with_capacity((width * height * 4) as usize);
        {
            let mapped = slice.get_mapped_range();
            for row in 0..height as usize {
                let at = row * target.padded as usize;
                data.extend_from_slice(&mapped[at..at + (width * 4) as usize]);
            }
        }
        target.buffer.unmap();
        self.last_ms = (encode_ms, t1.elapsed().as_secs_f64() * 1000.0);
        IntSize::from_wh(width, height)
            .and_then(|s| Pixmap::from_vec(data, s))
            .ok_or_else(|| "readback: not a pixmap".to_string())
    }
}
