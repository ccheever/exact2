//! Exact geometry painted by Android's `Canvas` (LLP 1076 §3.3): the kernel
//! lays out, cosmic-text shapes, and the paint walk runs as everywhere else,
//! but the backend records what it would draw — rounded rects, paths,
//! clips, layers, pictures and positioned glyph runs — into one flat op
//! stream that the app's `View` replays in `onDraw`. HWUI (Skia on the
//! RenderThread, with its glyph atlas) does the drawing; text is drawn with
//! `Canvas.drawGlyphs` from the same font files cosmic-text shaped with, so
//! measurement and pixels agree.
//!
//! The presenter lives on the Android main thread ([`CanvasHost`]); every
//! call comes from there.
//!
//! The stream is `u32` words (floats as bits), little-endian:
//! - `1 MATRIX a b c d e f` — device pixels; the rest of the stream until
//!   the next MATRIX is drawn under it (saved/restored by the reader around
//!   each clip scope as Canvas does).
//! - `2 RRECT color x y w h r0x r0y r1x r1y r2x r2y r3x r3y`
//! - `3 PATH color rule n (tag coords…)×n` — tags 0 move 1 line 2 cubic 3 close
//! - `4 CLIP_RRECT x y w h radii×8` (save + clip)
//! - `5 CLIP_PATH rule n (tag coords…)×n` (save + clip)
//! - `6 RESTORE`
//! - `7 LAYER alpha` (save layer)
//! - `8 IMAGE id x y w h`
//! - `9 GLYPHS font size color skew n (glyph x y)×n`
//! - `10 FONT key index weight len utf8-path(padded to 4)` — once per face
//! - `11 IMAGE_DEF id w h` — fetch its pixels with [`CanvasHost::image`]
//! - `12 IMAGE_FREE id`
//! - `13 STROKE color width cap join n (tag coords…)×n` — caps/joins as SVG (0 butt/miter, 1 round, 2 square/bevel)
//!
//! Geometry is in device pixels; colours are ARGB.
//!
//! @ref LLP 1076 §3.3 (Exact plus Android Canvas)

use crate::image::Bitmap;
use crate::paint::border::{BorderFill, PathOp};
use crate::paint::{Backend, GradientPaint, Rect4, Shape};
use crate::presenter::Presenter;
use crate::text::{Paragraph, RunPaint, TextEngine};
use exact_runner::DataSource;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Arc, Weak};
use tiny_skia::{Pixmap, Transform};

const MATRIX: u32 = 1;
const RRECT: u32 = 2;
const PATH: u32 = 3;
const CLIP_RRECT: u32 = 4;
const CLIP_PATH: u32 = 5;
const RESTORE: u32 = 6;
const LAYER: u32 = 7;
const IMAGE: u32 = 8;
const GLYPHS: u32 = 9;
const FONT: u32 = 10;
const IMAGE_DEF: u32 = 11;
const IMAGE_FREE: u32 = 12;
const STROKE: u32 = 13;

thread_local! {
    /// The last finished recording and the pictures it introduced.
    static FINISHED: RefCell<Option<Vec<u32>>> = const { RefCell::new(None) };
    /// Pictures the reader has not fetched yet, by id.
    static PENDING: RefCell<std::collections::BTreeMap<u32, Arc<Bitmap>>> =
        const { RefCell::new(std::collections::BTreeMap::new()) };
}

/// The recording backend.
pub struct Recorder {
    scale: f32,
    ops: Vec<u32>,
    /// The matrix last written into `ops` (points → device pixels).
    matrix: Option<[f32; 6]>,
    /// Faces announced to the reader, by (file, index, weight).
    fonts: HashMap<(Arc<str>, u32, u16), u32>,
    /// Pictures announced to the reader, by allocation, with a weak handle
    /// so a dropped picture is freed on the reader too.
    images: HashMap<usize, (u32, Weak<Bitmap>)>,
    next_image: u32,
}

impl Default for Recorder {
    fn default() -> Self {
        Self::new()
    }
}

impl Recorder {
    /// An empty recorder.
    pub fn new() -> Recorder {
        Recorder {
            scale: 1.0,
            ops: Vec::new(),
            matrix: None,
            fonts: HashMap::new(),
            images: HashMap::new(),
            next_image: 1,
        }
    }

    fn f(&mut self, v: f32) {
        self.ops.push(v.to_bits());
    }

    fn color(c: [u8; 4]) -> u32 {
        u32::from_be_bytes([c[3], c[0], c[1], c[2]])
    }

    /// Write the transform for the next op if it differs from the last.
    fn transform(&mut self, ts: Transform) {
        let s = self.scale;
        let m = [
            ts.sx * s,
            ts.ky * s,
            ts.kx * s,
            ts.sy * s,
            ts.tx * s,
            ts.ty * s,
        ];
        if self.matrix != Some(m) {
            self.matrix = Some(m);
            self.ops.push(MATRIX);
            for v in m {
                self.f(v);
            }
        }
    }

    fn rect_radii(&mut self, s: &Shape) {
        for v in [s.rect.0, s.rect.1, s.rect.2, s.rect.3] {
            self.f(v);
        }
        for (rx, ry) in s.radii {
            self.f(rx);
            self.f(ry);
        }
    }

    fn path(&mut self, ops: &[PathOp]) {
        self.ops.push(ops.len() as u32);
        for op in ops {
            match *op {
                PathOp::Move(x, y) => {
                    self.ops.push(0);
                    self.f(x);
                    self.f(y);
                }
                PathOp::Line(x, y) => {
                    self.ops.push(1);
                    self.f(x);
                    self.f(y);
                }
                PathOp::Cubic(a, b, c, d, e, g) => {
                    self.ops.push(2);
                    for v in [a, b, c, d, e, g] {
                        self.f(v);
                    }
                }
                PathOp::Close => self.ops.push(3),
            }
        }
    }

    fn font(&mut self, file: &(Arc<str>, u32), weight: u16) -> u32 {
        let key = (file.0.clone(), file.1, weight);
        if let Some(k) = self.fonts.get(&key) {
            return *k;
        }
        let k = self.fonts.len() as u32 + 1;
        self.fonts.insert(key, k);
        let bytes = file.0.as_bytes();
        self.ops
            .extend([FONT, k, file.1, u32::from(weight), bytes.len() as u32]);
        for chunk in bytes.chunks(4) {
            let mut w = [0u8; 4];
            w[..chunk.len()].copy_from_slice(chunk);
            self.ops.push(u32::from_le_bytes(w));
        }
        k
    }

    fn image_id(&mut self, image: &Arc<Bitmap>) -> u32 {
        let key = Arc::as_ptr(image) as usize;
        if let Some((id, weak)) = self.images.get(&key) {
            if weak.upgrade().is_some_and(|live| Arc::ptr_eq(&live, image)) {
                return *id;
            }
        }
        let id = self.next_image;
        self.next_image += 1;
        self.images.insert(key, (id, Arc::downgrade(image)));
        self.ops
            .extend([IMAGE_DEF, id, image.width(), image.height()]);
        PENDING.with(|p| p.borrow_mut().insert(id, image.clone()));
        id
    }
}

impl Backend for Recorder {
    fn name(&self) -> &'static str {
        "canvas"
    }

    fn begin(&mut self, _width: f32, _height: f32, scale: f32) {
        self.scale = scale;
        self.ops.clear();
        self.matrix = None;
        // Pictures the presenter dropped since the last frame.
        let dead: Vec<(usize, u32)> = self
            .images
            .iter()
            .filter(|(_, (_, weak))| weak.strong_count() == 0)
            .map(|(k, (id, _))| (*k, *id))
            .collect();
        for (k, id) in dead {
            self.images.remove(&k);
            self.ops.extend([IMAGE_FREE, id]);
            PENDING.with(|p| p.borrow_mut().remove(&id));
        }
    }

    fn fill(&mut self, s: &Shape, color: [u8; 4], ts: Transform) {
        if s.rect.2 <= 0.0 || s.rect.3 <= 0.0 || color[3] == 0 {
            return;
        }
        self.transform(ts);
        self.ops.extend([RRECT, Self::color(color)]);
        self.rect_radii(s);
    }

    fn fill_gradient(&mut self, s: &Shape, g: &GradientPaint, ts: Transform) {
        // Not in this benchmark's rows: the first stop stands in (a gap).
        if let Some(&(_, c)) = g.stops.first() {
            self.fill(s, crate::paint::rgba(c), ts);
        }
    }

    fn fill_border(&mut self, part: &BorderFill, ts: Transform) {
        self.transform(ts);
        if let Some(clip) = &part.clip {
            self.ops.extend([CLIP_PATH, 0]);
            self.path(clip);
        }
        self.ops.extend([PATH, Self::color(part.color), 1]);
        self.path(&part.region);
        if part.clip.is_some() {
            self.ops.push(RESTORE);
        }
    }

    fn image(
        &mut self,
        image: &Arc<Bitmap>,
        dst: Rect4,
        clips: &[Shape],
        ts: Transform,
        _tint: Option<[u8; 4]>,
    ) {
        if dst.2 <= 0.0 || dst.3 <= 0.0 {
            return;
        }
        let id = self.image_id(image);
        self.transform(ts);
        for c in clips {
            self.ops.push(CLIP_RRECT);
            self.rect_radii(c);
        }
        self.ops.extend([IMAGE, id]);
        for v in [dst.0, dst.1, dst.2, dst.3] {
            self.f(v);
        }
        for _ in clips {
            self.ops.push(RESTORE);
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
        self.transform(ts.pre_translate(origin.0, origin.1));
        for run in text.glyph_runs(paragraph, palette) {
            if run.paint.color[3] == 0 {
                continue;
            }
            let Some(file) = run.file.as_ref() else {
                continue;
            };
            let font = self.font(file, run.weight);
            self.ops.extend([GLYPHS, font]);
            self.f(run.size);
            self.ops.push(Self::color(run.paint.color));
            self.f(if run.synthetic_italic { -0.25 } else { 0.0 });
            self.ops.push(run.glyphs.len() as u32);
            for (id, x, y) in &run.glyphs {
                self.ops.push(*id);
                self.f(*x);
                self.f(*y);
            }
        }
    }

    fn svg_path(&mut self, s: &crate::paint::SvgPaint<'_>, ts: Transform) {
        // Solid inks only (symbols, plain SVG shapes); a gradient or pattern
        // draws nothing here (a gap of this backend).
        let ops: Vec<PathOp> = s
            .path
            .0
            .iter()
            .map(|seg| match *seg {
                exact_kernel::svg::Seg::Move(x, y) => PathOp::Move(x, y),
                exact_kernel::svg::Seg::Line(x, y) => PathOp::Line(x, y),
                exact_kernel::svg::Seg::Cubic(a, b, c, d, x, y) => PathOp::Cubic(a, b, c, d, x, y),
                exact_kernel::svg::Seg::Close => PathOp::Close,
            })
            .collect();
        self.transform(ts);
        if let Some(crate::paint::Ink::Solid(c)) = &s.fill {
            self.ops
                .extend([PATH, Self::color(*c), u32::from(s.even_odd)]);
            self.path(&ops);
        }
        if let Some(crate::paint::Ink::Solid(c)) = &s.stroke {
            self.ops.extend([STROKE, Self::color(*c)]);
            self.f(s.width);
            self.ops.extend([u32::from(s.cap), u32::from(s.join)]);
            self.path(&ops);
        }
    }

    fn push_clip(&mut self, s: &Shape, ts: Transform) {
        self.transform(ts);
        self.ops.push(CLIP_RRECT);
        self.rect_radii(s);
    }

    fn pop_clip(&mut self) {
        self.ops.push(RESTORE);
    }

    fn push_opacity(&mut self, alpha: f32) {
        self.ops.push(LAYER);
        self.f(alpha);
    }

    fn pop_opacity(&mut self) {
        self.ops.push(RESTORE);
    }

    fn pointer(&mut self, _x: f32, _y: f32) {}

    fn finish(&mut self) -> Result<Pixmap, String> {
        let ops = std::mem::take(&mut self.ops);
        FINISHED.with(|f| *f.borrow_mut() = Some(ops));
        Pixmap::new(1, 1).ok_or_else(|| "placeholder".to_string())
    }
}

/// The presenter on the Android main thread, painting through [`Recorder`].
pub struct CanvasHost<D: DataSource> {
    p: Presenter<D>,
    started: std::time::Instant,
    scale: f32,
    viewport: (f32, f32),
}

impl<D: DataSource + Default> CanvasHost<D> {
    /// Boot `D`'s app over a view of `size` pixels at `scale` pixels per
    /// point. The environment is read as on Linux (`EXACT_ASSETS`, …);
    /// `EXACT_PAINTER` is set to `canvas` here.
    pub fn boot(
        plan: &'static [u8],
        compat: &'static str,
        size: (u32, u32),
        scale: f32,
    ) -> Result<CanvasHost<D>, String> {
        let started = std::time::Instant::now();
        std::env::set_var("EXACT_PAINTER", "canvas");
        std::env::set_var("EXACT_SCALE", scale.to_string());
        let viewport = (size.0 as f32 / scale, size.1 as f32 / scale);
        let mut config = crate::app::Config::from_env(plan, compat);
        config.scale = scale;
        let (p, error) = crate::app::boot_presenter::<D>(&mut config, viewport)?;
        if let Some(e) = error {
            eprintln!("exact: {e}");
        }
        eprintln!(
            "exact: canvas {}x{} px, scale {scale}, boot {:.1} ms",
            size.0,
            size.1,
            started.elapsed().as_secs_f64() * 1000.0
        );
        Ok(CanvasHost {
            p,
            started,
            scale,
            viewport,
        })
    }

    fn now(&self) -> f64 {
        self.started.elapsed().as_secs_f64() * 1000.0
    }

    /// One turn: the presenter's work, then a recording when anything
    /// changed. `Some` is the new op stream (valid until the next call).
    pub fn frame(&mut self) -> Option<Vec<u32>> {
        let p = &mut self.p;
        let now = self.started.elapsed().as_secs_f64() * 1000.0;
        if let Some(e) = p.pump(now) {
            eprintln!("exact: {e}");
        }
        p.poll_update();
        p.run_commands(D::default);
        if p.host().wants_frames() {
            if let Some(e) = p.animation_frame(now) {
                eprintln!("exact: {e}");
            }
        } else if p.host().timer_due_ms().is_some_and(|due| due <= now) {
            if let Some(e) = p.advance(now) {
                eprintln!("exact: {e}");
            }
        }
        if p.needs_animation_frame() {
            p.tick(now);
        }
        p.poll_images();
        if !p.dirty() {
            return None;
        }
        let frame = p.display_frame()?;
        p.display_complete(&frame);
        if p.module_pending() {
            p.first_pixel();
        }
        FINISHED.with(|f| f.borrow_mut().take())
    }

    /// Drain the wakes (executor replies, decoded images) without painting;
    /// whether a frame is now wanted.
    pub fn poll(&mut self) -> bool {
        let now = self.now();
        if let Some(e) = self.p.pump(now) {
            eprintln!("exact: {e}");
        }
        self.p.run_commands(D::default);
        self.p.poll_images();
        self.animating()
    }

    /// Whether another frame is wanted at the next vsync.
    pub fn animating(&self) -> bool {
        self.p.dirty() || self.p.needs_animation_frame() || self.p.host().wants_frames()
    }

    /// Milliseconds until the next timer, if any.
    pub fn next_due(&self) -> Option<f64> {
        self.p
            .host()
            .timer_due_ms()
            .map(|due| (due - self.now()).max(0.0))
    }

    /// The descriptors that wake the presenter (executor replies, decoded images).
    pub fn fds(&self) -> [i32; 2] {
        [self.p.executor_fd(), self.p.image_fd()]
    }

    /// Scroll what is under the viewport's center by `dy` pixels.
    pub fn scroll(&mut self, dy: f32) {
        let (x, y) = (self.viewport.0 / 2.0, self.viewport.1 / 2.0);
        self.p.wheel_at(x, y, 0.0, dy / self.scale);
    }

    /// A touch (0 down, 1 up, 2 move, 3 cancel) at pixels.
    pub fn touch(&mut self, action: i32, x: f32, y: f32) {
        let now = self.now();
        let (x, y) = (x / self.scale, y / self.scale);
        let r = match action {
            0 => self.p.pointer_down(x, y, now).map(|_| ()),
            1 => self.p.pointer_up(x, y, now).map(|_| ()),
            2 => self.p.pointer_move(x, y, now).map(|_| ()),
            _ => self.p.pointer_cancel(now),
        };
        if let Err(e) = r {
            eprintln!("exact: {e}");
        }
    }

    /// A picture announced by `IMAGE_DEF`, once (premultiplied RGBA rows).
    pub fn image(id: u32) -> Option<Arc<Bitmap>> {
        PENDING.with(|p| p.borrow_mut().remove(&id))
    }
}
