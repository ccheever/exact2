//! Canvas 2D on Linux (LLP 1056 D7): the recorded lists replayed into a
//! tiny-skia pixmap, the canvas's kept bitmap, under both painters. The
//! painters composite each canvas's latest snapshot in its content box; the
//! GPU painter uploads it as an image, keyed by the snapshot's identity, so a
//! new revision is a new upload and a retained region holds the old one.
//!
//! The list's geometry is resolved (`exact_canvas::list`): paths arrive in
//! canvas coordinates, so a fill or clip draws them under the base scale; a
//! stroke takes them back to user space and strokes under base ∘ author, so
//! a line's width follows the author's matrix as the canvas's does. Every
//! paint goes through one pipeline (`draw.rs`): shadows, the clip-extent
//! operators, patterns, images and conic gradients; text is glyph outlines
//! (`text.rs`).

mod draw;
pub(crate) mod text;

use draw::Geom;
use exact_canvas::list::{self, text_at, Op};
use exact_kernel::ViewId;
use exact_runner::CanvasList;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use text::{CanvasText, RunStyle};
use tiny_skia::{
    BlendMode, Color, FillRule, LineCap, LineJoin, Mask, Paint, Path, PathBuilder, Pixmap, Shader,
    Stroke, StrokeDash, Transform,
};

/// One canvas's snapshot for the painters.
#[derive(Clone)]
pub(crate) struct CanvasPaint {
    /// The bitmap as last replayed; a new `Arc` per revision.
    pub pixels: Arc<Pixmap>,
}

#[derive(Clone, Copy, PartialEq)]
enum Style {
    Color([f64; 4]),
    /// A gradient or pattern, by id.
    Object(u32),
}

#[derive(Clone)]
struct State {
    author: Transform,
    fill: Style,
    stroke: Style,
    width: f32,
    cap: LineCap,
    join: LineJoin,
    miter: f32,
    dash: Vec<f32>,
    dash_offset: f32,
    alpha: f32,
    composite: usize,
    shadow_color: [f64; 4],
    shadow_blur: f32,
    shadow_offset: (f32, f32),
    smoothing: (bool, u8),
    font: Option<Arc<RunStyle>>,
    clip: Option<Mask>,
}

impl Default for State {
    fn default() -> Self {
        State {
            author: Transform::identity(),
            fill: Style::Color([0.0, 0.0, 0.0, 1.0]),
            stroke: Style::Color([0.0, 0.0, 0.0, 1.0]),
            width: 1.0,
            cap: LineCap::Butt,
            join: LineJoin::Miter,
            miter: 10.0,
            dash: Vec::new(),
            dash_offset: 0.0,
            alpha: 1.0,
            composite: 0,
            shadow_color: [0.0; 4],
            shadow_blur: 0.0,
            shadow_offset: (0.0, 0.0),
            smoothing: (true, 0),
            font: None,
            clip: None,
        }
    }
}

enum Object {
    Linear([f32; 4], Vec<(f32, [f64; 4])>),
    Radial([f32; 6], Vec<(f32, [f64; 4])>),
    Conic([f32; 3], Vec<(f32, [f64; 4])>),
    Pattern {
        image: u32,
        repetition: u8,
        transform: Transform,
    },
    Image(String),
}

/// The decoded image handles, shared by every replayer (LLP 1056 D9).
pub(crate) type ImageCache = HashMap<String, Arc<Pixmap>>;

/// One canvas's replayer: its bitmap and the state the lists build on.
pub(crate) struct Replayer {
    pixmap: Pixmap,
    base: Transform,
    scale: f32,
    lifetime: u64,
    generation: u32,
    state: State,
    stack: Vec<State>,
    path: PathBuilder,
    /// A `Path2D`'s segments for the next path paint.
    scratch: PathBuilder,
    objects: HashMap<u32, Object>,
}

fn blend(k: usize) -> BlendMode {
    use BlendMode::*;
    [
        SourceOver,
        SourceIn,
        SourceOut,
        SourceAtop,
        DestinationOver,
        DestinationIn,
        DestinationOut,
        DestinationAtop,
        Plus,
        Source,
        Xor,
        Multiply,
        Screen,
        Overlay,
        Darken,
        Lighten,
        ColorDodge,
        ColorBurn,
        HardLight,
        SoftLight,
        Difference,
        Exclusion,
        Hue,
        Saturation,
        Color,
        Luminosity,
    ]
    .get(k)
    .copied()
    .unwrap_or(SourceOver)
}

fn color(c: [f64; 4], alpha: f32) -> Color {
    Color::from_rgba(
        c[0] as f32 / 255.0,
        c[1] as f32 / 255.0,
        c[2] as f32 / 255.0,
        (c[3] as f32 * alpha).clamp(0.0, 1.0),
    )
    .unwrap_or(Color::TRANSPARENT)
}

/// What a replay needs beyond the list: the images and the text engine.
pub(crate) struct Env<'a> {
    pub images: &'a ImageCache,
    pub text: Option<&'a CanvasText>,
}

impl Replayer {
    fn new(w: u32, h: u32, scale: f64, lifetime: u64, generation: u32) -> Option<Replayer> {
        Some(Replayer {
            pixmap: Pixmap::new(w, h)?,
            base: Transform::from_scale(scale as f32, scale as f32),
            scale: scale as f32,
            lifetime,
            generation,
            state: State::default(),
            stack: Vec::new(),
            path: PathBuilder::new(),
            scratch: PathBuilder::new(),
            objects: HashMap::new(),
        })
    }

    fn device(&self) -> Transform {
        self.base.pre_concat(self.state.author)
    }

    fn stroke_style(&self) -> Option<Stroke> {
        let mut stroke = Stroke {
            width: self.state.width,
            miter_limit: self.state.miter,
            line_cap: self.state.cap,
            line_join: self.state.join,
            dash: None,
        };
        if !self.state.dash.is_empty() {
            if self.state.dash.iter().all(|v| *v == 0.0) {
                return None;
            }
            stroke.dash = StrokeDash::new(self.state.dash.clone(), self.state.dash_offset);
        }
        Some(stroke)
    }

    /// Fill a path in canvas coordinates with the fill style.
    fn fill_canvas_path(&mut self, path: &Path, rule: FillRule, env: &Env<'_>) {
        let ts = self.base;
        let source = self.source(self.state.fill, env);
        self.draw(Geom::Fill(path, rule, ts), source);
    }

    /// Stroke a path in canvas coordinates: back to user space, stroked
    /// under base ∘ author.
    fn stroke_canvas_path(&mut self, path: &Path, env: &Env<'_>) {
        let Some(inv) = self.state.author.invert() else {
            return;
        };
        if let Some(user) = path.clone().transform(inv) {
            self.stroke_user_path(&user, env);
        }
    }

    fn stroke_user_path(&mut self, user: &Path, env: &Env<'_>) {
        let Some(stroke) = self.stroke_style() else {
            return;
        };
        let ts = self.device();
        let source = self.source(self.state.stroke, env);
        self.draw(Geom::Stroke(user, stroke, ts), source);
    }

    fn clip_path(&mut self, path: Option<Path>, rule: FillRule) {
        let clip = self.state.clip.get_or_insert_with(|| {
            let mut m = Mask::new(self.pixmap.width(), self.pixmap.height()).expect("sized");
            m.data_mut().fill(255);
            m
        });
        match path {
            Some(path) => clip.intersect_path(&path, rule, true, self.base),
            None => clip.data_mut().fill(0),
        }
    }

    fn rect_path(x: f64, y: f64, w: f64, h: f64) -> Option<Path> {
        let mut pb = PathBuilder::new();
        let (x, y, w, h) = (x as f32, y as f32, w as f32, h as f32);
        pb.move_to(x, y);
        pb.line_to(x + w, y);
        pb.line_to(x + w, y + h);
        pb.line_to(x, y + h);
        pb.close();
        pb.finish()
    }

    fn rule(v: f64) -> FillRule {
        if v == 1.0 {
            FillRule::EvenOdd
        } else {
            FillRule::Winding
        }
    }

    fn stops(&mut self, id: u32) -> Option<&mut Vec<(f32, [f64; 4])>> {
        match self.objects.get_mut(&id)? {
            Object::Linear(_, s) | Object::Radial(_, s) | Object::Conic(_, s) => Some(s),
            _ => None,
        }
    }

    fn text(&mut self, fill: bool, r: &list::Record<'_>, env: &Env<'_>) {
        let (Some(engine), Some(font)) = (env.text, self.state.font.clone()) else {
            return;
        };
        let text = text_at(r, 4);
        let shaped = engine.shape(&font, &text, r.at(3) == 1.0);
        let Some(path) = shaped.path else {
            return;
        };
        let (x, y, sx) = (r.at(0) as f32, r.at(1) as f32, r.at(2) as f32);
        let ts = self.device().pre_translate(x, y).pre_scale(sx, 1.0);
        if fill {
            let source = self.source(self.state.fill, env);
            self.draw(Geom::Fill(&path, FillRule::Winding, ts), source);
        } else if let Some(stroke) = self.stroke_style() {
            let source = self.source(self.state.stroke, env);
            self.draw(Geom::Stroke(&path, stroke, ts), source);
        }
    }

    fn apply(&mut self, bytes: &[u8], env: &Env<'_>) -> Result<(), String> {
        let recs = list::records(bytes).map_err(|e| e.to_string())?;
        for r in recs {
            let f = |i: usize| r.at(i) as f32;
            let c4 = |i: usize| [r.at(i), r.at(i + 1), r.at(i + 2), r.at(i + 3)];
            match r.op {
                Op::Save => self.stack.push(self.state.clone()),
                Op::Restore => {
                    if let Some(s) = self.stack.pop() {
                        self.state = s;
                    }
                }
                Op::Reset => {
                    self.pixmap.fill(Color::TRANSPARENT);
                    self.state = State::default();
                    self.stack.clear();
                    self.path = PathBuilder::new();
                    self.scratch = PathBuilder::new();
                }
                Op::SetTransform => {
                    self.state.author = Transform::from_row(f(0), f(1), f(2), f(3), f(4), f(5))
                }
                Op::FillColor => self.state.fill = Style::Color(c4(0)),
                Op::FillGradient | Op::FillPattern => {
                    self.state.fill = Style::Object(r.at(0) as u32)
                }
                Op::StrokeColor => self.state.stroke = Style::Color(c4(0)),
                Op::StrokeGradient | Op::StrokePattern => {
                    self.state.stroke = Style::Object(r.at(0) as u32)
                }
                Op::LineWidth => self.state.width = f(0),
                Op::LineCap => {
                    self.state.cap =
                        [LineCap::Butt, LineCap::Round, LineCap::Square][r.at(0) as usize]
                }
                Op::LineJoin => {
                    self.state.join =
                        [LineJoin::Miter, LineJoin::Round, LineJoin::Bevel][r.at(0) as usize]
                }
                Op::MiterLimit => self.state.miter = f(0),
                Op::LineDash => self.state.dash = r.operands().map(|v| v as f32).collect(),
                Op::LineDashOffset => self.state.dash_offset = f(0),
                Op::GlobalAlpha => self.state.alpha = f(0),
                Op::Composite => self.state.composite = r.at(0) as usize,
                Op::ShadowColor => self.state.shadow_color = c4(0),
                Op::ShadowBlur => self.state.shadow_blur = f(0),
                Op::ShadowOffset => self.state.shadow_offset = (f(0), f(1)),
                Op::ImageSmoothing => self.state.smoothing = (r.at(0) == 1.0, r.at(1) as u8),
                Op::LinearGradient => {
                    let g = Object::Linear([f(1), f(2), f(3), f(4)], Vec::new());
                    self.objects.insert(r.at(0) as u32, g);
                }
                Op::RadialGradient => {
                    let g = Object::Radial([f(1), f(2), f(3), f(4), f(5), f(6)], Vec::new());
                    self.objects.insert(r.at(0) as u32, g);
                }
                Op::ConicGradient => {
                    let g = Object::Conic([f(1), f(2), f(3)], Vec::new());
                    self.objects.insert(r.at(0) as u32, g);
                }
                Op::ColorStop => {
                    let at = f(1);
                    let c = c4(2);
                    if let Some(stops) = self.stops(r.at(0) as u32) {
                        // Stops at one offset keep their order (spec).
                        let i = stops.partition_point(|(o, _)| *o <= at);
                        stops.insert(i, (at, c));
                    }
                }
                Op::Pattern => {
                    let p = Object::Pattern {
                        image: r.at(1) as u32,
                        repetition: r.at(2) as u8,
                        transform: Transform::identity(),
                    };
                    self.objects.insert(r.at(0) as u32, p);
                }
                Op::PatternTransform => {
                    if let Some(Object::Pattern { transform, .. }) =
                        self.objects.get_mut(&(r.at(0) as u32))
                    {
                        *transform = Transform::from_row(f(1), f(2), f(3), f(4), f(5), f(6));
                    }
                }
                Op::Image => {
                    self.objects
                        .insert(r.at(0) as u32, Object::Image(text_at(&r, 1)));
                }
                Op::BeginPath => self.path = PathBuilder::new(),
                Op::MoveTo => self.path.move_to(f(0), f(1)),
                Op::LineTo => self.path.line_to(f(0), f(1)),
                Op::QuadTo => self.path.quad_to(f(0), f(1), f(2), f(3)),
                Op::CubicTo => self.path.cubic_to(f(0), f(1), f(2), f(3), f(4), f(5)),
                Op::ClosePath => self.path.close(),
                Op::PathMoveTo => self.scratch.move_to(f(0), f(1)),
                Op::PathLineTo => self.scratch.line_to(f(0), f(1)),
                Op::PathQuadTo => self.scratch.quad_to(f(0), f(1), f(2), f(3)),
                Op::PathCubicTo => self.scratch.cubic_to(f(0), f(1), f(2), f(3), f(4), f(5)),
                Op::PathClose => self.scratch.close(),
                Op::Fill => {
                    if let Some(path) = self.path.clone().finish() {
                        self.fill_canvas_path(&path, Self::rule(r.at(0)), env);
                    }
                }
                Op::FillPath => {
                    if let Some(path) = std::mem::take(&mut self.scratch).finish() {
                        self.fill_canvas_path(&path, Self::rule(r.at(0)), env);
                    }
                }
                Op::Stroke => {
                    if let Some(path) = self.path.clone().finish() {
                        self.stroke_canvas_path(&path, env);
                    }
                }
                Op::StrokePath => {
                    if let Some(path) = std::mem::take(&mut self.scratch).finish() {
                        self.stroke_canvas_path(&path, env);
                    }
                }
                Op::Clip => {
                    let path = self.path.clone().finish();
                    self.clip_path(path, Self::rule(r.at(0)));
                }
                Op::ClipPath => {
                    let path = std::mem::take(&mut self.scratch).finish();
                    self.clip_path(path, Self::rule(r.at(0)));
                }
                Op::FillRect => {
                    if let Some(p) = Self::rect_path(r.at(0), r.at(1), r.at(2), r.at(3)) {
                        let ts = self.device();
                        let source = self.source(self.state.fill, env);
                        self.draw(Geom::Fill(&p, FillRule::Winding, ts), source);
                    }
                }
                Op::StrokeRect => {
                    let (x, y, w, h) = (r.at(0), r.at(1), r.at(2), r.at(3));
                    if w == 0.0 && h == 0.0 {
                        continue;
                    }
                    let p = if w == 0.0 || h == 0.0 {
                        let mut pb = PathBuilder::new();
                        pb.move_to(x as f32, y as f32);
                        pb.line_to((x + w) as f32, (y + h) as f32);
                        pb.finish()
                    } else {
                        Self::rect_path(x, y, w, h)
                    };
                    if let Some(p) = p {
                        self.stroke_user_path(&p, env);
                    }
                }
                Op::ClearRect => {
                    if let Some(p) = Self::rect_path(r.at(0), r.at(1), r.at(2), r.at(3)) {
                        // Destination-out with opaque black clears what the
                        // rectangle covers; tiny-skia's `Clear` ignores the mask.
                        let paint = Paint {
                            shader: Shader::SolidColor(Color::BLACK),
                            blend_mode: BlendMode::DestinationOut,
                            anti_alias: true,
                            ..Default::default()
                        };
                        let ts = self.device();
                        self.pixmap.fill_path(
                            &p,
                            &paint,
                            FillRule::Winding,
                            ts,
                            self.state.clip.as_ref(),
                        );
                    }
                }
                Op::Font => {
                    self.state.font = Some(Arc::new(RunStyle {
                        size: f(0),
                        weight: r.at(1) as u16,
                        style: r.at(2) as u8,
                        stretch: f(3),
                        kerning: r.at(5) as u8,
                        letter_spacing: f(7),
                        word_spacing: f(8),
                        families: text_at(&r, 9).split(',').map(str::to_string).collect(),
                    }))
                }
                Op::FillText => self.text(true, &r, env),
                Op::StrokeText => self.text(false, &r, env),
                Op::DrawImage => self.draw_image(&r, env),
                Op::PutImageData => self.put_image_data(&r),
            }
        }
        Ok(())
    }
}

/// Every 2D canvas's replayer and snapshot, the decoded images, and the
/// text engine.
#[derive(Default)]
pub(crate) struct Canvases {
    replayers: BTreeMap<ViewId, Replayer>,
    snapshots: BTreeMap<ViewId, CanvasPaint>,
    pub(crate) images: ImageCache,
    pub(crate) text: Option<Arc<CanvasText>>,
}

impl Canvases {
    /// Apply the runner's stamped lists in order (LLP 1056 D4). A fresh one
    /// allocates the generation's bitmap, cleared; a zero size is none.
    pub(crate) fn apply(&mut self, lists: Vec<CanvasList>) -> Vec<String> {
        let mut errors = Vec::new();
        let mut touched = Vec::new();
        let env = Env {
            images: &self.images,
            text: self.text.as_deref(),
        };
        for c in lists {
            if c.fresh {
                self.snapshots.remove(&c.view);
                match Replayer::new(
                    c.pixel_width,
                    c.pixel_height,
                    c.scale,
                    c.lifetime,
                    c.generation,
                ) {
                    Some(r) => {
                        self.replayers.insert(c.view, r);
                        touched.push(c.view);
                    }
                    None => {
                        self.replayers.remove(&c.view);
                    }
                }
            }
            let Some(r) = self.replayers.get_mut(&c.view) else {
                continue;
            };
            if r.lifetime != c.lifetime || r.generation != c.generation {
                continue;
            }
            for l in &c.lists {
                if let Err(e) = r.apply(l, &env) {
                    errors.push(format!("canvas {}: {e}", c.view));
                }
            }
            touched.push(c.view);
        }
        touched.dedup();
        for view in touched {
            if let Some(r) = self.replayers.get(&view) {
                self.snapshots.insert(
                    view,
                    CanvasPaint {
                        pixels: Arc::new(r.pixmap.clone()),
                    },
                );
            }
        }
        errors
    }

    /// Drop the canvases whose nodes are gone, and the images when no
    /// canvas is left.
    pub(crate) fn retain(&mut self, live: &[ViewId]) {
        self.replayers.retain(|v, _| live.contains(v));
        self.snapshots.retain(|v, _| live.contains(v));
        if live.is_empty() {
            self.images.clear();
        }
    }

    /// The painters' view of every canvas.
    pub(crate) fn snapshots(&self) -> BTreeMap<ViewId, CanvasPaint> {
        self.snapshots.clone()
    }
}

#[cfg(test)]
mod tests;
