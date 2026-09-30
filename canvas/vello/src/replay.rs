//! A canvas's lists (LLP 1056 D3) replayed into a vello scene, with the
//! reference replayer's semantics (`host/apple/Sources/ExactKit/
//! Canvas2D*.swift`): paths arrive in canvas coordinates, so a fill or clip
//! draws them under the base scale; a stroke takes them back to user space
//! and strokes under base ∘ author, so its width follows the author's
//! matrix. Paints go through `paint.rs`: global alpha, the operators, the
//! shadows. The state, the save stack, the clips, the current path and the
//! gradients, patterns and image handles persist from replay to replay, as
//! a web canvas keeps them.

use crate::paint::{Geom, Style};
use exact_canvas::list::{self, text_at, Op, Record};
use std::collections::HashMap;
use vello::kurbo::{Affine, BezPath, Cap, Join, Rect, Shape};
use vello::peniko::{Fill, ImageData};
use vello::Scene;

/// What a replay reads beyond its lists: the host's text and images.
pub trait Host {
    /// A text line's outlines (LLP 1056 ABI `text_path`): line space, y up,
    /// at the run's left end on its alphabetic baseline.
    fn text(&mut self, font: &[f64], text: &[u32], rtl: bool) -> Option<BezPath>;
    /// An image handle's pixels, premultiplied, or none when not decoded.
    fn image(&mut self, src: &str) -> Option<ImageData>;
}

/// The state a paint applies (`Canvas2DState`).
#[derive(Clone, Debug)]
pub struct State {
    pub author: Affine,
    pub fill: Style,
    pub stroke: Style,
    pub width: f64,
    pub cap: Cap,
    pub join: Join,
    pub miter: f64,
    pub dash: Vec<f64>,
    pub dash_offset: f64,
    pub alpha: f64,
    pub composite: usize,
    pub shadow_color: [f64; 4],
    pub shadow_blur: f64,
    pub shadow_offset: (f64, f64),
    pub smoothing: bool,
    pub quality: u8,
    pub font: Option<std::rc::Rc<[f64]>>,
    /// Clip layers pushed since the matching `save`.
    pub clips: usize,
}

impl Default for State {
    fn default() -> Self {
        State {
            author: Affine::IDENTITY,
            fill: Style::Color([0.0, 0.0, 0.0, 1.0]),
            stroke: Style::Color([0.0, 0.0, 0.0, 1.0]),
            width: 1.0,
            cap: Cap::Butt,
            join: Join::Miter,
            miter: 10.0,
            dash: Vec::new(),
            dash_offset: 0.0,
            alpha: 1.0,
            composite: 0,
            shadow_color: [0.0; 4],
            shadow_blur: 0.0,
            shadow_offset: (0.0, 0.0),
            smoothing: true,
            quality: 0,
            font: None,
            clips: 0,
        }
    }
}

/// A gradient: its geometry and stops (offset, colour), kept sorted.
#[derive(Clone, Debug)]
pub enum Gradient {
    Linear([f64; 4], Vec<(f64, [f64; 4])>),
    Radial([f64; 6], Vec<(f64, [f64; 4])>),
    Conic([f64; 3], Vec<(f64, [f64; 4])>),
}

/// A pattern: an image handle, its repetition and its transform.
#[derive(Clone, Debug)]
pub struct Pattern {
    pub image: u32,
    pub repetition: u8,
    pub transform: Affine,
}

/// A shadow a paint needs drawn offscreen and blurred before the scene
/// renders (`paint.rs` makes them).
pub struct ShadowJob {
    /// The shape's paint at its alpha, device pixels, offset to the origin.
    pub scene: Scene,
    pub width: u32,
    pub height: u32,
    /// Standard deviation, device pixels.
    pub sigma: f64,
    /// The shadow colour, non-premultiplied 0–1.
    pub color: [f32; 4],
    /// The placeholder the main scene draws; the blurred texture stands in.
    pub image: ImageData,
}

/// One replay's output.
pub struct Frame {
    pub scene: Scene,
    /// The canvas's pixels before this replay still show: the scene draws
    /// over them.
    pub keep: bool,
    pub shadows: Vec<ShadowJob>,
}

/// One canvas's replay state.
pub struct Replayer {
    /// Canvas units to backing pixels.
    pub scale: f64,
    /// Backing size, pixels.
    pub width: u32,
    pub height: u32,
    pub state: State,
    stack: Vec<State>,
    path: BezPath,
    scratch: BezPath,
    pub gradients: HashMap<u32, Gradient>,
    pub patterns: HashMap<u32, Pattern>,
    pub images: HashMap<u32, String>,
    /// Every clip in force (canvas coordinates, rule), outermost first:
    /// pushed again when a scene starts under them.
    open: Vec<(BezPath, Fill)>,
    /// What could not be drawn, by op name.
    pub unsupported: Vec<&'static str>,
}

fn rule(v: f64) -> Fill {
    if v == 1.0 {
        Fill::EvenOdd
    } else {
        Fill::NonZero
    }
}

impl Replayer {
    pub fn new(width: u32, height: u32, scale: f64) -> Replayer {
        Replayer {
            scale,
            width,
            height,
            state: State::default(),
            stack: Vec::new(),
            path: BezPath::new(),
            scratch: BezPath::new(),
            gradients: HashMap::new(),
            patterns: HashMap::new(),
            images: HashMap::new(),
            open: Vec::new(),
            unsupported: Vec::new(),
        }
    }

    /// Canvas coordinates to device pixels.
    pub fn base(&self) -> Affine {
        Affine::scale(self.scale)
    }

    /// User space to device pixels.
    pub fn device(&self) -> Affine {
        self.base() * self.state.author
    }

    /// The canvas in device pixels.
    pub fn whole(&self) -> Rect {
        Rect::new(0.0, 0.0, f64::from(self.width), f64::from(self.height))
    }

    pub fn note(&mut self, what: &'static str) {
        if !self.unsupported.contains(&what) {
            self.unsupported.push(what);
        }
    }

    /// A clip is in force.
    pub fn clipped(&self) -> bool {
        !self.open.is_empty()
    }

    /// Start a scene over the kept pixels: the clips in force pushed again.
    pub fn begin(&self, scene: &mut Scene) {
        for (p, fill) in &self.open {
            scene.push_clip_layer(*fill, self.base(), p);
        }
    }

    /// Close the clip layers still open at the end of a scene.
    pub fn end(&self, scene: &mut Scene) {
        for _ in &self.open {
            scene.pop_layer();
        }
    }

    /// Replay one list into `frame`; an unreadable list applies nothing.
    pub fn apply(
        &mut self,
        bytes: &[u8],
        frame: &mut Frame,
        host: &mut dyn Host,
    ) -> Result<(), String> {
        let recs = list::records(bytes).map_err(|e| e.to_string())?;
        for r in recs {
            self.record(&r, frame, host);
        }
        Ok(())
    }

    /// Drop what the scene drew: something now covers every pixel.
    fn restart(&self, frame: &mut Frame) {
        frame.scene.reset();
        frame.shadows.clear();
        frame.keep = false;
        self.begin(&mut frame.scene);
    }

    fn pop_clips(&mut self, scene: &mut Scene, n: usize) {
        for _ in 0..n.min(self.open.len()) {
            scene.pop_layer();
            self.open.pop();
        }
    }

    /// Whether a `fillRect` (or `clearRect`) of `r` now covers every pixel
    /// (`Canvas2DRecord.swift` `covers`).
    fn covers(&self, r: Rect, clear: bool) -> bool {
        let s = &self.state;
        let m = s.author.as_coeffs();
        if m[1] != 0.0 || m[2] != 0.0 || self.clipped() {
            return false;
        }
        if !clear {
            let Style::Color(c) = s.fill else {
                return false;
            };
            if c[3] != 1.0 || s.alpha != 1.0 || s.composite != 0 {
                return false;
            }
        }
        let b = self.device().transform_rect_bbox(r.abs());
        b.x0 <= 0.0
            && b.y0 <= 0.0
            && b.x1 >= f64::from(self.width)
            && b.y1 >= f64::from(self.height)
    }

    #[allow(clippy::too_many_lines)]
    fn record(&mut self, r: &Record<'_>, frame: &mut Frame, host: &mut dyn Host) {
        let c4 = |i: usize| [r.at(i), r.at(i + 1), r.at(i + 2), r.at(i + 3)];
        let pt = |i: usize| (r.at(i), r.at(i + 1));
        let rect = || Rect::new(r.at(0), r.at(1), r.at(0) + r.at(2), r.at(1) + r.at(3));
        match r.op {
            Op::Save => {
                self.stack.push(self.state.clone());
                self.state.clips = 0;
            }
            Op::Restore => {
                if let Some(s) = self.stack.pop() {
                    let n = self.state.clips;
                    self.pop_clips(&mut frame.scene, n);
                    self.state = s;
                }
            }
            Op::Reset => {
                self.pop_clips(&mut frame.scene, self.open.len());
                self.state = State::default();
                self.stack.clear();
                self.path = BezPath::new();
                self.scratch = BezPath::new();
                self.restart(frame);
            }
            Op::SetTransform => {
                self.state.author =
                    Affine::new([r.at(0), r.at(1), r.at(2), r.at(3), r.at(4), r.at(5)])
            }
            Op::FillColor => self.state.fill = Style::Color(c4(0)),
            Op::FillGradient => self.state.fill = Style::Gradient(r.at(0) as u32),
            Op::FillPattern => self.state.fill = Style::Pattern(r.at(0) as u32),
            Op::StrokeColor => self.state.stroke = Style::Color(c4(0)),
            Op::StrokeGradient => self.state.stroke = Style::Gradient(r.at(0) as u32),
            Op::StrokePattern => self.state.stroke = Style::Pattern(r.at(0) as u32),
            Op::LineWidth => self.state.width = r.at(0),
            Op::LineCap => {
                self.state.cap = [Cap::Butt, Cap::Round, Cap::Square][(r.at(0) as usize).min(2)]
            }
            Op::LineJoin => {
                self.state.join = [Join::Miter, Join::Round, Join::Bevel][(r.at(0) as usize).min(2)]
            }
            Op::MiterLimit => self.state.miter = r.at(0),
            Op::LineDash => self.state.dash = r.operands().collect(),
            Op::LineDashOffset => self.state.dash_offset = r.at(0),
            Op::GlobalAlpha => self.state.alpha = r.at(0),
            Op::Composite => self.state.composite = (r.at(0) as usize).min(25),
            Op::ShadowColor => self.state.shadow_color = c4(0),
            Op::ShadowBlur => self.state.shadow_blur = r.at(0),
            Op::ShadowOffset => self.state.shadow_offset = (r.at(0), r.at(1)),
            Op::ImageSmoothing => {
                self.state.smoothing = r.at(0) != 0.0;
                self.state.quality = (r.at(1) as u8).min(2);
            }
            Op::LinearGradient => {
                let g = Gradient::Linear([r.at(1), r.at(2), r.at(3), r.at(4)], Vec::new());
                self.gradients.insert(r.at(0) as u32, g);
            }
            Op::RadialGradient => {
                let g = Gradient::Radial(
                    [r.at(1), r.at(2), r.at(3), r.at(4), r.at(5), r.at(6)],
                    Vec::new(),
                );
                self.gradients.insert(r.at(0) as u32, g);
            }
            Op::ConicGradient => {
                self.gradients.insert(
                    r.at(0) as u32,
                    Gradient::Conic([r.at(1), r.at(2), r.at(3)], Vec::new()),
                );
            }
            Op::ColorStop => {
                let (at, c) = (r.at(1), c4(2));
                if let Some(
                    Gradient::Linear(_, s) | Gradient::Radial(_, s) | Gradient::Conic(_, s),
                ) = self.gradients.get_mut(&(r.at(0) as u32))
                {
                    let i = s.partition_point(|(o, _)| *o <= at);
                    s.insert(i, (at, c));
                }
            }
            Op::Pattern => {
                let p = Pattern {
                    image: r.at(1) as u32,
                    repetition: r.at(2) as u8,
                    transform: Affine::IDENTITY,
                };
                self.patterns.insert(r.at(0) as u32, p);
            }
            Op::PatternTransform => {
                if let Some(p) = self.patterns.get_mut(&(r.at(0) as u32)) {
                    p.transform =
                        Affine::new([r.at(1), r.at(2), r.at(3), r.at(4), r.at(5), r.at(6)]);
                }
            }
            Op::Image => {
                self.images.insert(r.at(0) as u32, text_at(r, 1));
            }
            Op::BeginPath => self.path = BezPath::new(),
            Op::MoveTo => self.path.move_to(pt(0)),
            Op::LineTo => line_to(&mut self.path, pt(0)),
            Op::QuadTo => {
                ensure_start(&mut self.path, pt(0));
                self.path.quad_to(pt(0), pt(2))
            }
            Op::CubicTo => {
                ensure_start(&mut self.path, pt(0));
                self.path.curve_to(pt(0), pt(2), pt(4))
            }
            Op::ClosePath => close(&mut self.path),
            Op::PathMoveTo => self.scratch.move_to(pt(0)),
            Op::PathLineTo => line_to(&mut self.scratch, pt(0)),
            Op::PathQuadTo => {
                ensure_start(&mut self.scratch, pt(0));
                self.scratch.quad_to(pt(0), pt(2))
            }
            Op::PathCubicTo => {
                ensure_start(&mut self.scratch, pt(0));
                self.scratch.curve_to(pt(0), pt(2), pt(4))
            }
            Op::PathClose => close(&mut self.scratch),
            Op::Fill | Op::FillPath => {
                let path = if r.op == Op::Fill {
                    self.path.clone()
                } else {
                    std::mem::take(&mut self.scratch)
                };
                if path.elements().is_empty() {
                    return;
                }
                let base = self.base();
                let style = self.state.fill.clone();
                self.paint(frame, host, Geom::Fill(&path, rule(r.at(0)), base), &style);
            }
            Op::Stroke | Op::StrokePath => {
                let path = if r.op == Op::Stroke {
                    self.path.clone()
                } else {
                    std::mem::take(&mut self.scratch)
                };
                if path.elements().is_empty() {
                    return;
                }
                let inv = self.state.author.inverse();
                if !inv.is_finite() {
                    return;
                }
                self.stroke_user(frame, host, &(inv * path));
            }
            Op::Clip | Op::ClipPath => {
                let path = if r.op == Op::Clip {
                    self.path.clone()
                } else {
                    std::mem::take(&mut self.scratch)
                };
                frame
                    .scene
                    .push_clip_layer(rule(r.at(0)), self.base(), &path);
                self.open.push((path, rule(r.at(0))));
                self.state.clips += 1;
            }
            Op::FillRect => {
                let rc = rect();
                if self.covers(rc, false) {
                    self.restart(frame);
                }
                let t = self.device();
                let style = self.state.fill.clone();
                let p = rc.abs().to_path(0.1);
                self.paint(frame, host, Geom::Fill(&p, Fill::NonZero, t), &style);
            }
            Op::StrokeRect => {
                let (x, y, w, h) = (r.at(0), r.at(1), r.at(2), r.at(3));
                if w == 0.0 && h == 0.0 {
                    return;
                }
                let p = if w == 0.0 || h == 0.0 {
                    let mut b = BezPath::new();
                    b.move_to((x, y));
                    b.line_to((x + w, y + h));
                    b
                } else {
                    let mut b = BezPath::new();
                    b.move_to((x, y));
                    b.line_to((x + w, y));
                    b.line_to((x + w, y + h));
                    b.line_to((x, y + h));
                    b.close_path();
                    b
                };
                self.stroke_user(frame, host, &p);
            }
            Op::ClearRect => {
                let rc = rect();
                if self.covers(rc, true) {
                    self.restart(frame);
                    return;
                }
                self.clear(frame, rc);
            }
            Op::Font => self.state.font = Some(r.operands().collect::<Vec<_>>().into()),
            Op::FillText | Op::StrokeText => {
                if r.len() < 4 {
                    return;
                }
                let Some(font) = self.state.font.clone() else {
                    return;
                };
                let text: Vec<u32> = (4..r.len()).map(|i| r.at(i) as u32).collect();
                let Some(outline) = host.text(&font, &text, r.at(3) != 0.0) else {
                    return;
                };
                let t = self.device()
                    * Affine::translate((r.at(0), r.at(1)))
                    * Affine::scale_non_uniform(r.at(2), 1.0)
                    * Affine::FLIP_Y;
                if r.op == Op::FillText {
                    let style = self.state.fill.clone();
                    self.paint(frame, host, Geom::Fill(&outline, Fill::NonZero, t), &style);
                } else if let Some(stroke) = self.stroke_style() {
                    let style = self.state.stroke.clone();
                    self.paint(frame, host, Geom::Stroke(&outline, stroke, t), &style);
                }
            }
            Op::DrawImage => self.draw_image(frame, host, r),
            Op::PutImageData => self.put_image_data(frame, r),
        }
    }

    /// Stroke `user` (user space) with the stroke style under base ∘ author.
    fn stroke_user(&mut self, frame: &mut Frame, host: &mut dyn Host, user: &BezPath) {
        let Some(stroke) = self.stroke_style() else {
            return;
        };
        let t = self.device();
        let style = self.state.stroke.clone();
        self.paint(frame, host, Geom::Stroke(user, stroke, t), &style);
    }

    /// `clearRect`: transparent black over the rectangle, under the author
    /// matrix and the clip; no alpha, operator or shadow.
    fn clear(&mut self, frame: &mut Frame, rc: Rect) {
        use vello::peniko::{BlendMode, Color, Compose, Mix};
        let t = self.device();
        let shape = rc.abs().to_path(0.1);
        let bbox = t.transform_rect_bbox(rc.abs()).intersect(self.whole());
        if bbox.is_zero_area() {
            return;
        }
        frame.scene.push_layer(
            Fill::NonZero,
            BlendMode::new(Mix::Normal, Compose::DestOut),
            1.0,
            Affine::IDENTITY,
            &bbox,
        );
        frame
            .scene
            .fill(Fill::NonZero, t, Color::BLACK, None, &shape);
        frame.scene.pop_layer();
    }

    /// `putImageData`: raw backing pixels over the dirty rectangle, with no
    /// transform, clip, alpha, operator or shadow (`Compose::Copy`).
    fn put_image_data(&mut self, frame: &mut Frame, r: &Record<'_>) {
        use vello::peniko::{
            BlendMode, Blob, Compose, ImageAlphaType, ImageBrush, ImageFormat, ImageQuality, Mix,
        };
        let (x, y, w, h) = (
            r.at(0),
            r.at(1),
            r.at(2).max(0.0) as u32,
            r.at(3).max(0.0) as u32,
        );
        if w == 0 || h == 0 || r.len() < 4 + (w * h) as usize {
            return;
        }
        let mut px = Vec::with_capacity((w * h * 4) as usize);
        for i in 0..(w * h) as usize {
            px.extend_from_slice(&(r.at(4 + i) as u32).to_be_bytes());
        }
        let image = ImageData {
            data: Blob::new(std::sync::Arc::new(px)),
            format: ImageFormat::Rgba8,
            alpha_type: ImageAlphaType::Alpha,
            width: w,
            height: h,
        };
        let scene = &mut frame.scene;
        for _ in &self.open {
            scene.pop_layer();
        }
        // Copy as a clear then a source-over draw: a `Compose::Copy` layer
        // cleared whole tiles around the rectangle (fx-pixels on dark).
        let at = Rect::new(x, y, x + f64::from(w), y + f64::from(h));
        scene.push_layer(
            Fill::NonZero,
            BlendMode::new(Mix::Normal, Compose::DestOut),
            1.0,
            Affine::IDENTITY,
            &at,
        );
        scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            vello::peniko::Color::BLACK,
            None,
            &at,
        );
        scene.pop_layer();
        let brush = ImageBrush::new(image).with_quality(ImageQuality::Low);
        scene.draw_image(&brush, Affine::translate((x, y)));
        self.begin(scene);
    }

    /// `drawImage`: `image sx sy sw sh dx dy dw dh`, the source clipped to
    /// the image, the destination in user space.
    fn draw_image(&mut self, frame: &mut Frame, host: &mut dyn Host, r: &Record<'_>) {
        let Some(src) = self.images.get(&(r.at(0) as u32)).cloned() else {
            return;
        };
        let Some(image) = host.image(&src) else {
            return;
        };
        let (sx, sy, sw, sh) = (r.at(1), r.at(2), r.at(3), r.at(4));
        let dest = Rect::new(r.at(5), r.at(6), r.at(5) + r.at(7), r.at(6) + r.at(8));
        if sw <= 0.0 || sh <= 0.0 || dest.width() == 0.0 || dest.height() == 0.0 {
            return;
        }
        let (kx, ky) = (dest.width() / sw, dest.height() / sh);
        // Image pixels to user space: the whole image, placed so the source
        // rectangle lands on the destination.
        let to_user = Affine::new([kx, 0.0, 0.0, ky, dest.x0 - sx * kx, dest.y0 - sy * ky]);
        let t = self.device();
        let shape = dest.abs().to_path(0.1);
        self.paint(
            frame,
            host,
            Geom::Image(&shape, t, image, to_user),
            &Style::Color([0.0; 4]),
        );
    }

    /// The stroke with the state's line style; none when the dash list is
    /// all zeros (nothing is drawn).
    pub fn stroke_style(&self) -> Option<vello::kurbo::Stroke> {
        let s = &self.state;
        let mut stroke = vello::kurbo::Stroke::new(s.width)
            .with_caps(s.cap)
            .with_join(s.join)
            .with_miter_limit(s.miter);
        if !s.dash.is_empty() {
            if s.dash.iter().all(|v| *v == 0.0) {
                return None;
            }
            stroke = stroke.with_dashes(s.dash_offset, s.dash.iter().copied());
        }
        Some(stroke)
    }
}

/// `lineTo` on an empty path is a `moveTo` (as `CGMutablePath` takes it).
fn line_to(p: &mut BezPath, to: (f64, f64)) {
    if p.elements().is_empty() {
        p.move_to(to);
    } else {
        p.line_to(to);
    }
}

/// A curve on an empty path starts at its first control point (the
/// recorder resolves the spec's rule; this keeps kurbo's path well formed).
fn ensure_start(p: &mut BezPath, at: (f64, f64)) {
    if p.elements().is_empty() {
        p.move_to(at);
    }
}

fn close(p: &mut BezPath) {
    if !p.elements().is_empty() {
        p.close_path();
    }
}
