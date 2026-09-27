//! Canvas 2D on Linux (LLP 1056 D7): the recorded lists replayed into a
//! tiny-skia pixmap, the canvas's kept bitmap, under both painters. The
//! painters composite each canvas's latest snapshot in its content box; the
//! GPU painter uploads it as an image, keyed by the snapshot's identity, so a
//! new revision is a new upload and a retained region holds the old one.
//!
//! The list's geometry is resolved (`exact_canvas::list`): paths arrive in
//! canvas coordinates, so a fill or clip draws them under the base scale; a
//! stroke takes them back to user space and strokes under base ∘ author, so
//! a line's width follows the author's matrix as the canvas's does.

use exact_canvas::list::{self, Op};
use exact_kernel::ViewId;
use exact_runner::CanvasList;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use tiny_skia::{
    BlendMode, Color, FillRule, GradientStop, LineCap, LineJoin, LinearGradient, Mask, Paint, Path,
    PathBuilder, Pixmap, Point, RadialGradient, Shader, SpreadMode, Stroke, StrokeDash, Transform,
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
    Gradient(u32),
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
    blend: BlendMode,
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
            blend: BlendMode::SourceOver,
            clip: None,
        }
    }
}

enum Kind {
    Linear([f32; 4]),
    Radial([f32; 6]),
}

struct Gradient {
    kind: Kind,
    stops: Vec<(f32, [f64; 4])>,
}

/// One canvas's replayer: its bitmap and the state the lists build on.
pub(crate) struct Replayer {
    pixmap: Pixmap,
    base: Transform,
    lifetime: u64,
    generation: u32,
    state: State,
    stack: Vec<State>,
    path: PathBuilder,
    gradients: HashMap<u32, Gradient>,
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

impl Replayer {
    fn new(w: u32, h: u32, scale: f64, lifetime: u64, generation: u32) -> Option<Replayer> {
        Some(Replayer {
            pixmap: Pixmap::new(w, h)?,
            base: Transform::from_scale(scale as f32, scale as f32),
            lifetime,
            generation,
            state: State::default(),
            stack: Vec::new(),
            path: PathBuilder::new(),
            gradients: HashMap::new(),
        })
    }

    fn device(&self) -> Transform {
        self.base.pre_concat(self.state.author)
    }

    /// The paint for a style, with its shader in `space` (user space mapped
    /// by the path's transform), or `None` when it paints nothing.
    fn paint(&self, style: Style, shader_ts: Transform) -> Option<Paint<'static>> {
        let alpha = self.state.alpha;
        let shader = match style {
            Style::Color(c) => Shader::SolidColor(color(c, alpha)),
            Style::Gradient(id) => {
                let g = self.gradients.get(&id)?;
                if g.stops.is_empty() {
                    return None;
                }
                let stops: Vec<GradientStop> = g
                    .stops
                    .iter()
                    .map(|(o, c)| GradientStop::new(*o, color(*c, alpha)))
                    .collect();
                match g.kind {
                    Kind::Linear([x0, y0, x1, y1]) => {
                        if x0 == x1 && y0 == y1 {
                            return None;
                        }
                        LinearGradient::new(
                            Point::from_xy(x0, y0),
                            Point::from_xy(x1, y1),
                            stops,
                            SpreadMode::Pad,
                            shader_ts,
                        )?
                    }
                    Kind::Radial([x0, y0, r0, x1, y1, r1]) => {
                        if x0 == x1 && y0 == y1 && r0 == r1 {
                            return None;
                        }
                        RadialGradient::new(
                            Point::from_xy(x0, y0),
                            r0,
                            Point::from_xy(x1, y1),
                            r1,
                            stops,
                            SpreadMode::Pad,
                            shader_ts,
                        )?
                    }
                }
            }
        };
        Some(Paint {
            shader,
            blend_mode: self.state.blend,
            anti_alias: true,
            ..Default::default()
        })
    }

    fn fill_canvas_path(&mut self, path: &Path, rule: FillRule) {
        let Some(paint) = self.paint(self.state.fill, self.state.author) else {
            return;
        };
        self.pixmap
            .fill_path(path, &paint, rule, self.base, self.state.clip.as_ref());
    }

    fn stroke_user_path(&mut self, user: &Path) {
        let Some(paint) = self.paint(self.state.stroke, Transform::identity()) else {
            return;
        };
        let mut stroke = Stroke {
            width: self.state.width,
            miter_limit: self.state.miter,
            line_cap: self.state.cap,
            line_join: self.state.join,
            dash: None,
        };
        if !self.state.dash.is_empty() {
            if self.state.dash.iter().all(|v| *v == 0.0) {
                return;
            }
            stroke.dash = StrokeDash::new(self.state.dash.clone(), self.state.dash_offset);
        }
        let ts = self.device();
        self.pixmap
            .stroke_path(user, &paint, &stroke, ts, self.state.clip.as_ref());
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

    fn apply(&mut self, bytes: &[u8]) -> Result<(), String> {
        let recs = list::records(bytes).map_err(|e| e.to_string())?;
        for r in recs {
            let f = |i: usize| r.at(i) as f32;
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
                }
                Op::SetTransform => {
                    self.state.author = Transform::from_row(f(0), f(1), f(2), f(3), f(4), f(5))
                }
                Op::FillColor => {
                    self.state.fill = Style::Color([r.at(0), r.at(1), r.at(2), r.at(3)])
                }
                Op::FillGradient => self.state.fill = Style::Gradient(r.at(0) as u32),
                Op::StrokeColor => {
                    self.state.stroke = Style::Color([r.at(0), r.at(1), r.at(2), r.at(3)])
                }
                Op::StrokeGradient => self.state.stroke = Style::Gradient(r.at(0) as u32),
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
                Op::Composite => self.state.blend = blend(r.at(0) as usize),
                Op::LinearGradient => {
                    self.gradients.insert(
                        r.at(0) as u32,
                        Gradient {
                            kind: Kind::Linear([f(1), f(2), f(3), f(4)]),
                            stops: Vec::new(),
                        },
                    );
                }
                Op::RadialGradient => {
                    self.gradients.insert(
                        r.at(0) as u32,
                        Gradient {
                            kind: Kind::Radial([f(1), f(2), f(3), f(4), f(5), f(6)]),
                            stops: Vec::new(),
                        },
                    );
                }
                Op::ColorStop => {
                    if let Some(g) = self.gradients.get_mut(&(r.at(0) as u32)) {
                        let at = f(1);
                        // Stops at one offset keep their order (spec).
                        let i = g.stops.partition_point(|(o, _)| *o <= at);
                        g.stops
                            .insert(i, (at, [r.at(2), r.at(3), r.at(4), r.at(5)]));
                    }
                }
                Op::BeginPath => self.path = PathBuilder::new(),
                Op::MoveTo => self.path.move_to(f(0), f(1)),
                Op::LineTo => self.path.line_to(f(0), f(1)),
                Op::QuadTo => self.path.quad_to(f(0), f(1), f(2), f(3)),
                Op::CubicTo => self.path.cubic_to(f(0), f(1), f(2), f(3), f(4), f(5)),
                Op::ClosePath => self.path.close(),
                Op::Fill => {
                    if let Some(path) = self.path.clone().finish() {
                        let rule = if r.at(0) == 1.0 {
                            FillRule::EvenOdd
                        } else {
                            FillRule::Winding
                        };
                        self.fill_canvas_path(&path, rule);
                    }
                }
                Op::Stroke => {
                    let inv = self.state.author.invert();
                    if let (Some(path), Some(inv)) = (self.path.clone().finish(), inv) {
                        if let Some(user) = path.transform(inv) {
                            self.stroke_user_path(&user);
                        }
                    }
                }
                Op::Clip => {
                    let rule = if r.at(0) == 1.0 {
                        FillRule::EvenOdd
                    } else {
                        FillRule::Winding
                    };
                    let path = self.path.clone().finish();
                    let clip = self.state.clip.get_or_insert_with(|| {
                        let mut m =
                            Mask::new(self.pixmap.width(), self.pixmap.height()).expect("sized");
                        m.data_mut().fill(255);
                        m
                    });
                    match path {
                        Some(path) => clip.intersect_path(&path, rule, true, self.base),
                        None => clip.data_mut().fill(0),
                    }
                }
                Op::FillRect => {
                    if let Some(p) = Self::rect_path(r.at(0), r.at(1), r.at(2), r.at(3)) {
                        let Some(paint) = self.paint(self.state.fill, Transform::identity()) else {
                            continue;
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
                        self.stroke_user_path(&p);
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
            }
        }
        Ok(())
    }
}

/// Every 2D canvas's replayer and snapshot.
#[derive(Default)]
pub(crate) struct Canvases {
    replayers: BTreeMap<ViewId, Replayer>,
    snapshots: BTreeMap<ViewId, CanvasPaint>,
}

impl Canvases {
    /// Apply the runner's stamped lists in order (LLP 1056 D4). A fresh one
    /// allocates the generation's bitmap, cleared; a zero size is none.
    pub(crate) fn apply(&mut self, lists: Vec<CanvasList>) -> Vec<String> {
        let mut errors = Vec::new();
        let mut touched = Vec::new();
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
                if let Err(e) = r.apply(l) {
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

    /// Drop the canvases whose nodes are gone.
    pub(crate) fn retain(&mut self, live: &[ViewId]) {
        self.replayers.retain(|v, _| live.contains(v));
        self.snapshots.retain(|v, _| live.contains(v));
    }

    /// The painters' view of every canvas.
    pub(crate) fn snapshots(&self) -> BTreeMap<ViewId, CanvasPaint> {
        self.snapshots.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use exact_canvas::Context2d;

    fn replay(w: u32, h: u32, scale: f64, draw: impl Fn(&Context2d)) -> Pixmap {
        let ctx = Context2d::new();
        draw(&ctx);
        let mut r = Replayer::new(w, h, scale, 1, 0).unwrap();
        for l in ctx.take_lists() {
            r.apply(&l).unwrap();
        }
        r.pixmap
    }

    fn at(p: &Pixmap, x: u32, y: u32) -> [u8; 4] {
        let c = p.pixel(x, y).unwrap();
        [c.red(), c.green(), c.blue(), c.alpha()]
    }

    #[test]
    fn a_clockwise_arc_bulges_down_in_canvas_space() {
        // arc(50, 50, 40, 0, π): clockwise from +x through +y (down).
        let p = replay(100, 100, 1.0, |c| {
            c.set_fill_style_str("red");
            c.begin_path();
            c.arc(50.0, 50.0, 40.0, 0.0, std::f64::consts::PI).unwrap();
            c.fill();
        });
        assert_eq!(at(&p, 50, 80), [255, 0, 0, 255], "the lower half is filled");
        assert_eq!(at(&p, 50, 20)[3], 0, "the upper half is not");
    }

    #[test]
    fn device_scale_and_author_matrix_compose() {
        let p = replay(40, 40, 2.0, |c| {
            c.translate(10.0, 0.0).unwrap();
            c.fill_rect(0.0, 0.0, 5.0, 5.0);
        });
        assert_eq!(at(&p, 21, 1)[3], 255);
        assert_eq!(at(&p, 19, 1)[3], 0);
        assert_eq!(at(&p, 29, 9)[3], 255);
        assert_eq!(at(&p, 31, 9)[3], 0);
    }

    #[test]
    fn clear_rect_obeys_the_clip_and_the_path_survives_fill() {
        let p = replay(20, 20, 1.0, |c| {
            c.fill_rect(0.0, 0.0, 20.0, 20.0);
            c.begin_path();
            c.rect(0.0, 0.0, 10.0, 20.0);
            c.clip();
            c.clear_rect(0.0, 0.0, 20.0, 20.0);
        });
        assert_eq!(at(&p, 5, 5)[3], 0);
        assert_eq!(at(&p, 15, 5)[3], 255);
    }
}
