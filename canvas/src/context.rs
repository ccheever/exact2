//! The recorder (LLP 1056 D3): the context an author calls. It owns the
//! authoritative drawing state, applies the spec's argument rules at the
//! call — ignore, no-op or throw — so getters answer exactly, and appends
//! each effective call to the list the host replays.
//!
//! Method names are web-sys's for the same IDL members, so porting web-sys
//! drawing code is mostly renaming the type. Where web-sys takes a
//! `JsValue`, this takes a typed value (`set_line_dash(&[f64])`,
//! `round_rect_with_radii(&[Radius])`); a method that can throw returns
//! `Result<_, DomException>`, as web-sys's returns `Result<_, JsValue>`.
//!
//! The rules, member by member, are these methods. The TypeScript recorder
//! implements the same rules, and the shared cases in `tests/cases.txt`
//! hold both recorders to them.

use crate::color::{self, Parsed, Rgba};
use crate::font::{Font, TextEngine};
use crate::geom::{self, Matrix, Radius, Seg};
use crate::list::{Op, Writer, SEAL_BYTES};
use crate::COMPOSITE;
use std::sync::{Arc, Mutex, MutexGuard};

#[path = "context/image.rs"]
mod image;
#[path = "context/text.rs"]
mod text;
pub use image::{images_in, CanvasPattern, ImageData, ImageSlot, ImageTable, Images};

/// A thrown `DOMException` (or `TypeError`, by name).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomException {
    /// `IndexSizeError`, `SyntaxError`, `TypeError`, `RangeError`.
    pub name: &'static str,
    /// What was wrong.
    pub message: String,
}

impl std::fmt::Display for DomException {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.name, self.message)
    }
}

impl std::error::Error for DomException {}

pub(crate) fn throw<T>(name: &'static str, message: impl Into<String>) -> Result<T, DomException> {
    Err(DomException {
        name,
        message: message.into(),
    })
}

/// `CanvasWindingRule`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CanvasWindingRule {
    /// `"nonzero"`.
    #[default]
    Nonzero,
    /// `"evenodd"`.
    Evenodd,
}

impl CanvasWindingRule {
    pub(crate) fn code(self) -> f64 {
        match self {
            CanvasWindingRule::Nonzero => 0.0,
            CanvasWindingRule::Evenodd => 1.0,
        }
    }
}

/// A fill or stroke style, as its getter returns it.
#[derive(Debug, Clone, PartialEq)]
pub enum Style {
    /// A colour's serialisation (`#rrggbb` or `rgba(…)`).
    Color(String),
    /// A gradient.
    Gradient(CanvasGradient),
    /// A pattern.
    Pattern(CanvasPattern),
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Paint {
    /// A colour, and a wide colour's own serialisation.
    Color(Rgba, Option<Arc<str>>),
    Gradient(u32),
    Pattern(u32),
}

/// What the runner tells a context about where it draws (LLP 1056 D4, D8,
/// D9): the host's text engine, the image handles it can draw, and the
/// canvas node's `color` and `direction`.
#[derive(Clone, Default)]
pub struct Env {
    /// The canvas's lifetime id, which an image a draw asked for names.
    pub canvas: u64,
    /// The host's text engine, callable on this thread.
    pub text: Option<Arc<dyn TextEngine>>,
    /// The runner's image table, made by the first image call.
    pub images: ImageSlot,
    /// The canvas node's CSS `color`: what `currentColor` resolves to.
    pub current_color: Option<Rgba>,
    /// The canvas node's CSS `direction` is `rtl`: what `direction =
    /// "inherit"` resolves to.
    pub rtl: bool,
}

/// `DOMMatrix`'s 2D members, detached: what `get_transform` returns.
pub type DomMatrix = Matrix;

/// The text attributes (LLP 1056 D8): what shapes a run, and where it goes.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TextState {
    pub(crate) font: Font,
    /// `letterSpacing` and `wordSpacing`: the authored text and px.
    pub(crate) letter_spacing: (String, f64),
    pub(crate) word_spacing: (String, f64),
    pub(crate) kerning: u8,
    pub(crate) rendering: u8,
    pub(crate) align: u8,
    pub(crate) baseline: u8,
    /// 0 ltr, 1 rtl, 2 inherit.
    pub(crate) direction: u8,
}

impl Default for TextState {
    fn default() -> Self {
        TextState {
            font: Font::default(),
            letter_spacing: ("0px".into(), 0.0),
            word_spacing: ("0px".into(), 0.0),
            kerning: 0,
            rendering: 0,
            align: 0,
            baseline: 3,
            direction: 2,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct State {
    pub(crate) transform: Matrix,
    pub(crate) fill: Paint,
    pub(crate) stroke: Paint,
    line_width: f64,
    cap: u8,
    join: u8,
    miter: f64,
    dash: Vec<f64>,
    dash_offset: f64,
    alpha: f64,
    composite: u8,
    shadow_color: (Rgba, Option<Arc<str>>),
    shadow_blur: f64,
    shadow_offset: (f64, f64),
    pub(crate) smoothing: (bool, u8),
    pub(crate) text: TextState,
    /// The `Font` record the replayer's state holds, which save and restore
    /// carry as the replayer's stack does.
    pub(crate) font_sent: Option<Arc<[f64]>>,
}

impl Default for State {
    fn default() -> Self {
        State {
            transform: Matrix::IDENTITY,
            fill: Paint::Color(Rgba::BLACK, None),
            stroke: Paint::Color(Rgba::BLACK, None),
            line_width: 1.0,
            cap: 0,
            join: 0,
            miter: 10.0,
            dash: Vec::new(),
            dash_offset: 0.0,
            alpha: 1.0,
            composite: 0,
            shadow_color: (
                Rgba {
                    r: 0,
                    g: 0,
                    b: 0,
                    a: 0.0,
                },
                None,
            ),
            shadow_blur: 0.0,
            shadow_offset: (0.0, 0.0),
            smoothing: (true, 0),
            text: TextState::default(),
            font_sent: None,
        }
    }
}

#[derive(Default)]
pub(crate) struct Inner {
    pub(crate) state: State,
    stack: Vec<State>,
    writer: Writer,
    sealed: Vec<Vec<u8>>,
    /// The current subpath's start and last point, in canvas coordinates.
    subpath: Option<((f64, f64), (f64, f64))>,
    /// Gradients, patterns and images share one id space.
    pub(crate) next_id: u32,
    /// Image handles given an id in this generation.
    pub(crate) image_ids: Vec<(String, u32)>,
    pub(crate) env: Env,
    notes: Vec<String>,
}

impl std::fmt::Debug for Inner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Inner")
    }
}

impl Inner {
    pub(crate) fn op(&mut self, op: Op, operands: &[f64]) {
        if self.writer.len() + 8 + operands.len() * 8 > SEAL_BYTES && !self.writer.is_empty() {
            let full = std::mem::take(&mut self.writer);
            self.sealed.push(full.finish());
        }
        self.writer.op(op, operands);
    }

    pub(crate) fn paint_op(&mut self, fill: bool, paint: &Paint) {
        match (fill, paint) {
            (true, Paint::Color(c, _)) => self.op(Op::FillColor, &c.operands()),
            (true, Paint::Gradient(id)) => self.op(Op::FillGradient, &[*id as f64]),
            (true, Paint::Pattern(id)) => self.op(Op::FillPattern, &[*id as f64]),
            (false, Paint::Color(c, _)) => self.op(Op::StrokeColor, &c.operands()),
            (false, Paint::Gradient(id)) => self.op(Op::StrokeGradient, &[*id as f64]),
            (false, Paint::Pattern(id)) => self.op(Op::StrokePattern, &[*id as f64]),
        }
    }

    /// A new id for a gradient, pattern or image.
    pub(crate) fn next_object(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn set_transform(&mut self, m: Matrix) {
        if self.state.transform != m {
            self.state.transform = m;
            self.op(Op::SetTransform, &m.operands());
        }
    }

    /// A point in user space, in canvas coordinates; `None` when the matrix
    /// cannot be inverted (Chrome then ignores path calls).
    fn to_canvas(&self, x: f64, y: f64) -> Option<(f64, f64)> {
        self.state
            .transform
            .invertible()
            .then(|| self.state.transform.apply(x, y))
    }

    fn move_to(&mut self, x: f64, y: f64) {
        if let Some(p) = self.to_canvas(x, y) {
            self.op(Op::MoveTo, &[p.0, p.1]);
            self.subpath = Some((p, p));
        }
    }

    fn line_to(&mut self, x: f64, y: f64) {
        let Some(p) = self.to_canvas(x, y) else {
            return;
        };
        match self.subpath.as_mut() {
            Some(sp) => {
                sp.1 = p;
                self.op(Op::LineTo, &[p.0, p.1]);
            }
            None => {
                self.op(Op::MoveTo, &[p.0, p.1]);
                self.subpath = Some((p, p));
            }
        }
    }

    /// "Ensure there is a subpath" for a point, then segments after it.
    fn ensure(&mut self, x: f64, y: f64) {
        if self.subpath.is_none() {
            self.move_to(x, y);
        }
    }

    fn segments(&mut self, segs: &[Seg]) {
        let m = self.state.transform;
        for s in segs {
            match *s {
                Seg::Line(x, y) => self.line_to(x, y),
                Seg::Cubic(a, b, c, d, x, y) => {
                    let (p1, p2, p) = (m.apply(a, b), m.apply(c, d), m.apply(x, y));
                    self.op(Op::CubicTo, &[p1.0, p1.1, p2.0, p2.1, p.0, p.1]);
                    if let Some(sp) = self.subpath.as_mut() {
                        sp.1 = p;
                    }
                }
            }
        }
    }

    /// Whether paints do anything: Chrome skips them under a singular matrix.
    pub(crate) fn paints(&self) -> bool {
        self.state.transform.invertible()
    }

    /// A colour assignment's value, with `currentColor` resolved.
    pub(crate) fn color(&self, v: &str) -> Option<(Rgba, Option<Arc<str>>)> {
        match color::parse(v)? {
            Parsed::Color(c) => Some((c, None)),
            Parsed::Wide(c, text) => Some((c, Some(text.into()))),
            Parsed::Current => Some((self.env.current_color.unwrap_or(Rgba::BLACK), None)),
        }
    }

    pub(crate) fn note(&mut self, line: String) {
        if self.notes.len() < 32 {
            self.notes.push(line);
        }
    }
}

/// A `CanvasGradient`: an id in its context's list. Stops added after it is
/// assigned affect later paints, not earlier ones, as on the web.
#[derive(Clone)]
pub struct CanvasGradient {
    id: u32,
    inner: Arc<Mutex<Inner>>,
}

impl std::fmt::Debug for CanvasGradient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CanvasGradient({})", self.id)
    }
}

impl PartialEq for CanvasGradient {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && Arc::ptr_eq(&self.inner, &other.inner)
    }
}

impl CanvasGradient {
    /// `addColorStop(offset, color)`: `IndexSizeError` outside [0, 1],
    /// `SyntaxError` for a colour that does not parse. `currentColor` is
    /// opaque black here, as the spec parses a stop with no element.
    pub fn add_color_stop(&self, offset: f32, color: &str) -> Result<(), DomException> {
        self.add_color_stop_f64(offset as f64, color)
    }

    /// [`add_color_stop`](Self::add_color_stop) with a double offset.
    pub fn add_color_stop_f64(&self, offset: f64, color: &str) -> Result<(), DomException> {
        if !offset.is_finite() {
            return throw("TypeError", "The provided double value is non-finite.");
        }
        if !(0.0..=1.0).contains(&offset) {
            return throw(
                "IndexSizeError",
                format!("The provided value ({offset}) is outside the range (0.0, 1.0)."),
            );
        }
        let c = match color::parse(color) {
            Some(Parsed::Color(c) | Parsed::Wide(c, _)) => c,
            Some(Parsed::Current) => Rgba::BLACK,
            None => {
                return throw(
                    "SyntaxError",
                    format!("The value provided ('{color}') could not be parsed as a color."),
                )
            }
        };
        let mut inner = lock(&self.inner);
        let [r, g, b, a] = c.operands();
        inner.op(Op::ColorStop, &[self.id as f64, offset, r, g, b, a]);
        Ok(())
    }
}

pub(crate) fn lock(inner: &Arc<Mutex<Inner>>) -> MutexGuard<'_, Inner> {
    inner.lock().unwrap_or_else(|e| e.into_inner())
}

/// The recording `CanvasRenderingContext2D` for one canvas generation.
#[derive(Clone, Default)]
pub struct Context2d {
    pub(crate) inner: Arc<Mutex<Inner>>,
}

impl std::fmt::Debug for Context2d {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Context2d")
    }
}

macro_rules! finite {
    ($($v:expr),*) => { if !($($v.is_finite())&&*) { return; } };
}
macro_rules! finite_ok {
    ($($v:expr),*) => { if !($($v.is_finite())&&*) { return Ok(()); } };
}

impl Context2d {
    /// A fresh context at the spec's defaults.
    pub fn new() -> Context2d {
        Context2d::default()
    }

    pub(crate) fn g(&self) -> MutexGuard<'_, Inner> {
        lock(&self.inner)
    }

    /// Where this context draws: set by the runner before each draw.
    pub fn set_env(&self, env: Env) {
        self.g().env = env;
    }

    /// The environment's text engine, for a seam that measures on the
    /// executor's behalf (a TypeScript recorder's `measureText`).
    pub fn env(&self) -> Env {
        self.g().env.clone()
    }

    /// The lists recorded since the last take, in order: a draw longer than
    /// [`SEAL_BYTES`] is several.
    pub fn take_lists(&self) -> Vec<Vec<u8>> {
        let mut g = self.g();
        let mut out = std::mem::take(&mut g.sealed);
        // The next draw is likely this one's size: its writer starts there.
        let next = Writer::with_capacity(g.writer.len());
        let w = std::mem::replace(&mut g.writer, next);
        if !w.is_empty() {
            out.push(w.finish());
        }
        out
    }

    /// Development notes: stage-unsupported values that were ignored.
    pub fn take_notes(&self) -> Vec<String> {
        std::mem::take(&mut self.g().notes)
    }

    // --- State ---------------------------------------------------------

    /// `save()`.
    pub fn save(&self) {
        let mut g = self.g();
        let s = g.state.clone();
        g.stack.push(s);
        g.op(Op::Save, &[]);
    }

    /// `restore()`: an unmatched one does nothing.
    pub fn restore(&self) {
        let mut g = self.g();
        if let Some(s) = g.stack.pop() {
            g.state = s;
            g.op(Op::Restore, &[]);
        }
    }

    /// `reset()`: the bitmap cleared, the state, stack and path at their
    /// defaults.
    pub fn reset(&self) {
        let mut g = self.g();
        g.state = State::default();
        g.stack.clear();
        g.subpath = None;
        g.op(Op::Reset, &[]);
    }

    // --- Transforms ----------------------------------------------------

    /// `translate(x, y)`.
    pub fn translate(&self, x: f64, y: f64) -> Result<(), DomException> {
        finite_ok!(x, y);
        let mut g = self.g();
        let m = g
            .state
            .transform
            .then(Matrix::new(1.0, 0.0, 0.0, 1.0, x, y));
        g.set_transform(m);
        Ok(())
    }

    /// `rotate(angle)`.
    pub fn rotate(&self, angle: f64) -> Result<(), DomException> {
        finite_ok!(angle);
        let (s, c) = angle.sin_cos();
        let mut g = self.g();
        let m = g.state.transform.then(Matrix::new(c, s, -s, c, 0.0, 0.0));
        g.set_transform(m);
        Ok(())
    }

    /// `scale(x, y)`.
    pub fn scale(&self, x: f64, y: f64) -> Result<(), DomException> {
        finite_ok!(x, y);
        let mut g = self.g();
        let m = g
            .state
            .transform
            .then(Matrix::new(x, 0.0, 0.0, y, 0.0, 0.0));
        g.set_transform(m);
        Ok(())
    }

    /// `transform(a, b, c, d, e, f)`.
    pub fn transform(
        &self,
        a: f64,
        b: f64,
        c: f64,
        d: f64,
        e: f64,
        f: f64,
    ) -> Result<(), DomException> {
        finite_ok!(a, b, c, d, e, f);
        let mut g = self.g();
        let m = g.state.transform.then(Matrix::new(a, b, c, d, e, f));
        g.set_transform(m);
        Ok(())
    }

    /// `setTransform(a, b, c, d, e, f)`.
    pub fn set_transform(
        &self,
        a: f64,
        b: f64,
        c: f64,
        d: f64,
        e: f64,
        f: f64,
    ) -> Result<(), DomException> {
        finite_ok!(a, b, c, d, e, f);
        self.g().set_transform(Matrix::new(a, b, c, d, e, f));
        Ok(())
    }

    /// `setTransform(DOMMatrix2DInit)`: the matrix, already validated
    /// finite by the caller's init dictionary (a non-finite entry throws
    /// `TypeError`, as the dictionary conversion does).
    pub fn set_transform_with_dom_matrix_2d_init(&self, m: &DomMatrix) -> Result<(), DomException> {
        if !m.operands().iter().all(|v| v.is_finite()) {
            return throw("TypeError", "The matrix has non-finite entries.");
        }
        self.g().set_transform(*m);
        Ok(())
    }

    /// `resetTransform()`.
    pub fn reset_transform(&self) -> Result<(), DomException> {
        self.g().set_transform(Matrix::IDENTITY);
        Ok(())
    }

    /// `getTransform()`: a detached copy of the author matrix.
    pub fn get_transform(&self) -> Result<DomMatrix, DomException> {
        Ok(self.g().state.transform)
    }

    // --- Paths ---------------------------------------------------------

    /// `beginPath()`.
    pub fn begin_path(&self) {
        let mut g = self.g();
        g.subpath = None;
        g.op(Op::BeginPath, &[]);
    }

    /// `moveTo(x, y)`.
    pub fn move_to(&self, x: f64, y: f64) {
        finite!(x, y);
        self.g().move_to(x, y);
    }

    /// `lineTo(x, y)`.
    pub fn line_to(&self, x: f64, y: f64) {
        finite!(x, y);
        self.g().line_to(x, y);
    }

    /// `quadraticCurveTo(cpx, cpy, x, y)`.
    pub fn quadratic_curve_to(&self, cpx: f64, cpy: f64, x: f64, y: f64) {
        finite!(cpx, cpy, x, y);
        let mut g = self.g();
        if !g.state.transform.invertible() {
            return;
        }
        g.ensure(cpx, cpy);
        let m = g.state.transform;
        let (c, p) = (m.apply(cpx, cpy), m.apply(x, y));
        g.op(Op::QuadTo, &[c.0, c.1, p.0, p.1]);
        if let Some(sp) = g.subpath.as_mut() {
            sp.1 = p;
        }
    }

    /// `bezierCurveTo(cp1x, cp1y, cp2x, cp2y, x, y)`.
    pub fn bezier_curve_to(&self, cp1x: f64, cp1y: f64, cp2x: f64, cp2y: f64, x: f64, y: f64) {
        finite!(cp1x, cp1y, cp2x, cp2y, x, y);
        let mut g = self.g();
        if !g.state.transform.invertible() {
            return;
        }
        g.ensure(cp1x, cp1y);
        g.segments(&[Seg::Cubic(cp1x, cp1y, cp2x, cp2y, x, y)]);
    }

    /// `closePath()`.
    pub fn close_path(&self) {
        let mut g = self.g();
        if let Some((start, _)) = g.subpath {
            g.subpath = Some((start, start));
            g.op(Op::ClosePath, &[]);
        }
    }

    /// `rect(x, y, w, h)`: a closed subpath, then a new one at (x, y).
    pub fn rect(&self, x: f64, y: f64, w: f64, h: f64) {
        finite!(x, y, w, h);
        let mut g = self.g();
        if !g.state.transform.invertible() {
            return;
        }
        g.move_to(x, y);
        g.line_to(x + w, y);
        g.line_to(x + w, y + h);
        g.line_to(x, y + h);
        drop(g);
        self.close_path();
        self.g().move_to(x, y);
    }

    /// `arc(x, y, radius, startAngle, endAngle)`.
    pub fn arc(
        &self,
        x: f64,
        y: f64,
        radius: f64,
        start: f64,
        end: f64,
    ) -> Result<(), DomException> {
        self.arc_with_anticlockwise(x, y, radius, start, end, false)
    }

    /// `arc(x, y, radius, startAngle, endAngle, anticlockwise)`.
    pub fn arc_with_anticlockwise(
        &self,
        x: f64,
        y: f64,
        radius: f64,
        start: f64,
        end: f64,
        anticlockwise: bool,
    ) -> Result<(), DomException> {
        self.ellipse_with_anticlockwise(x, y, radius, radius, 0.0, start, end, anticlockwise)
            .map_err(|mut e| {
                e.message = format!("The radius provided ({radius}) is negative.");
                e
            })
    }

    /// `ellipse(x, y, radiusX, radiusY, rotation, startAngle, endAngle)`.
    #[allow(clippy::too_many_arguments)]
    pub fn ellipse(
        &self,
        x: f64,
        y: f64,
        rx: f64,
        ry: f64,
        rotation: f64,
        start: f64,
        end: f64,
    ) -> Result<(), DomException> {
        self.ellipse_with_anticlockwise(x, y, rx, ry, rotation, start, end, false)
    }

    /// `ellipse(…, anticlockwise)`.
    #[allow(clippy::too_many_arguments)]
    pub fn ellipse_with_anticlockwise(
        &self,
        x: f64,
        y: f64,
        rx: f64,
        ry: f64,
        rotation: f64,
        start: f64,
        end: f64,
        anticlockwise: bool,
    ) -> Result<(), DomException> {
        finite_ok!(x, y, rx, ry, rotation, start, end);
        if rx < 0.0 || ry < 0.0 {
            let r = if rx < 0.0 { rx } else { ry };
            return throw(
                "IndexSizeError",
                format!("The radius provided ({r}) is negative."),
            );
        }
        let mut g = self.g();
        if !g.state.transform.invertible() {
            return Ok(());
        }
        let (first, segs) = geom::ellipse(x, y, rx, ry, rotation, start, end, anticlockwise);
        g.line_to(first.0, first.1);
        g.segments(&segs);
        Ok(())
    }

    /// `arcTo(x1, y1, x2, y2, radius)`.
    pub fn arc_to(
        &self,
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        radius: f64,
    ) -> Result<(), DomException> {
        finite_ok!(x1, y1, x2, y2, radius);
        let mut g = self.g();
        if !g.state.transform.invertible() {
            return Ok(());
        }
        g.ensure(x1, y1);
        if radius < 0.0 {
            return throw(
                "IndexSizeError",
                format!("The radius provided ({radius}) is negative."),
            );
        }
        let last = g.subpath.map(|s| s.1).unwrap_or((0.0, 0.0));
        let inv = g.state.transform.invert().unwrap_or_default();
        let p0 = inv.apply(last.0, last.1);
        match geom::arc_to(p0, (x1, y1), (x2, y2), radius) {
            None => g.line_to(x1, y1),
            Some((t1, segs)) => {
                g.line_to(t1.0, t1.1);
                g.segments(&segs);
            }
        }
        Ok(())
    }

    /// `roundRect(x, y, w, h)`: radii 0.
    pub fn round_rect(&self, x: f64, y: f64, w: f64, h: f64) -> Result<(), DomException> {
        self.round_rect_with_radii(x, y, w, h, &[Radius { x: 0.0, y: 0.0 }])
    }

    /// `roundRect(x, y, w, h, radius)`.
    pub fn round_rect_with_f64(
        &self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        r: f64,
    ) -> Result<(), DomException> {
        self.round_rect_with_radii(x, y, w, h, &[Radius { x: r, y: r }])
    }

    /// `roundRect(x, y, w, h, radii)` with a list of numbers or
    /// `DOMPointInit`s, each given as its `x` and `y` (a number is both).
    pub fn round_rect_with_radii(
        &self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        radii: &[Radius],
    ) -> Result<(), DomException> {
        finite_ok!(x, y, w, h);
        if radii.is_empty() || radii.len() > 4 {
            return throw(
                "RangeError",
                format!(
                    "{} radii provided. Between one and four radii are necessary.",
                    radii.len()
                ),
            );
        }
        for r in radii {
            finite_ok!(r.x, r.y);
            if r.x < 0.0 || r.y < 0.0 {
                let v = if r.x < 0.0 { r.x } else { r.y };
                return throw("RangeError", format!("Radius value {v} is negative."));
            }
        }
        let mut g = self.g();
        if !g.state.transform.invertible() {
            return Ok(());
        }
        let (start, segs) = geom::round_rect(x, y, w, h, radii);
        g.move_to(start.0, start.1);
        g.segments(&segs);
        drop(g);
        self.close_path();
        self.g().move_to(x, y);
        Ok(())
    }

    // --- Painting ------------------------------------------------------

    /// `fill()`.
    pub fn fill(&self) {
        self.fill_with_canvas_winding_rule(CanvasWindingRule::Nonzero)
    }

    /// `fill(rule)`.
    pub fn fill_with_canvas_winding_rule(&self, rule: CanvasWindingRule) {
        let mut g = self.g();
        if g.paints() {
            g.op(Op::Fill, &[rule.code()]);
        }
    }

    /// `stroke()`.
    pub fn stroke(&self) {
        let mut g = self.g();
        if g.paints() {
            g.op(Op::Stroke, &[]);
        }
    }

    /// `clip()`.
    pub fn clip(&self) {
        self.clip_with_canvas_winding_rule(CanvasWindingRule::Nonzero)
    }

    /// `clip(rule)`.
    pub fn clip_with_canvas_winding_rule(&self, rule: CanvasWindingRule) {
        let mut g = self.g();
        g.op(Op::Clip, &[rule.code()]);
    }

    fn rect_op(&self, op: Op, x: f64, y: f64, w: f64, h: f64) {
        finite!(x, y, w, h);
        let mut g = self.g();
        if g.paints() {
            g.op(op, &[x, y, w, h]);
        }
    }

    /// `fillRect(x, y, w, h)`.
    pub fn fill_rect(&self, x: f64, y: f64, w: f64, h: f64) {
        self.rect_op(Op::FillRect, x, y, w, h)
    }

    /// `strokeRect(x, y, w, h)`.
    pub fn stroke_rect(&self, x: f64, y: f64, w: f64, h: f64) {
        self.rect_op(Op::StrokeRect, x, y, w, h)
    }

    /// `clearRect(x, y, w, h)`: obeys the transform and clip; ignores
    /// style, alpha and compositing; keeps the current path.
    pub fn clear_rect(&self, x: f64, y: f64, w: f64, h: f64) {
        self.rect_op(Op::ClearRect, x, y, w, h)
    }

    // --- Line styles ---------------------------------------------------

    /// `lineWidth = v`: ignored unless finite and positive.
    pub fn set_line_width(&self, v: f64) {
        let mut g = self.g();
        if v.is_finite() && v > 0.0 && g.state.line_width != v {
            g.state.line_width = v;
            g.op(Op::LineWidth, &[v]);
        }
    }

    /// `lineWidth`.
    pub fn line_width(&self) -> f64 {
        self.g().state.line_width
    }

    /// `lineCap = v`: `butt`, `round` or `square`; anything else ignored.
    pub fn set_line_cap(&self, v: &str) {
        let Some(k) = ["butt", "round", "square"].iter().position(|n| *n == v) else {
            return;
        };
        let mut g = self.g();
        if g.state.cap != k as u8 {
            g.state.cap = k as u8;
            g.op(Op::LineCap, &[k as f64]);
        }
    }

    /// `lineCap`.
    pub fn line_cap(&self) -> String {
        ["butt", "round", "square"][self.g().state.cap as usize].into()
    }

    /// `lineJoin = v`: `miter`, `round` or `bevel`; anything else ignored.
    pub fn set_line_join(&self, v: &str) {
        let Some(k) = ["miter", "round", "bevel"].iter().position(|n| *n == v) else {
            return;
        };
        let mut g = self.g();
        if g.state.join != k as u8 {
            g.state.join = k as u8;
            g.op(Op::LineJoin, &[k as f64]);
        }
    }

    /// `lineJoin`.
    pub fn line_join(&self) -> String {
        ["miter", "round", "bevel"][self.g().state.join as usize].into()
    }

    /// `miterLimit = v`: ignored unless finite and positive.
    pub fn set_miter_limit(&self, v: f64) {
        let mut g = self.g();
        if v.is_finite() && v > 0.0 && g.state.miter != v {
            g.state.miter = v;
            g.op(Op::MiterLimit, &[v]);
        }
    }

    /// `miterLimit`.
    pub fn miter_limit(&self) -> f64 {
        self.g().state.miter
    }

    /// `setLineDash(segments)`: ignored if any is negative or non-finite; an
    /// odd list is doubled.
    pub fn set_line_dash(&self, segments: &[f64]) -> Result<(), DomException> {
        if segments.iter().any(|v| !v.is_finite() || *v < 0.0) {
            return Ok(());
        }
        let mut list = segments.to_vec();
        if list.len() % 2 == 1 {
            list.extend_from_slice(segments);
        }
        let mut g = self.g();
        g.op(Op::LineDash, &list);
        g.state.dash = list;
        Ok(())
    }

    /// `getLineDash()`.
    pub fn get_line_dash(&self) -> Vec<f64> {
        self.g().state.dash.clone()
    }

    /// `lineDashOffset = v`: ignored unless finite.
    pub fn set_line_dash_offset(&self, v: f64) {
        let mut g = self.g();
        if v.is_finite() && g.state.dash_offset != v {
            g.state.dash_offset = v;
            g.op(Op::LineDashOffset, &[v]);
        }
    }

    /// `lineDashOffset`.
    pub fn line_dash_offset(&self) -> f64 {
        self.g().state.dash_offset
    }

    // --- Fill and stroke styles ----------------------------------------

    fn set_style_str(&self, fill: bool, v: &str) {
        let mut g = self.g();
        let Some((c, text)) = g.color(v) else {
            return;
        };
        let p = Paint::Color(c, text);
        let slot = if fill {
            &mut g.state.fill
        } else {
            &mut g.state.stroke
        };
        if *slot != p {
            let changed =
                !matches!((&*slot, &p), (Paint::Color(a, _), Paint::Color(b, _)) if a == b);
            *slot = p.clone();
            if changed {
                g.paint_op(fill, &p);
            }
        }
    }

    fn set_style_gradient(&self, fill: bool, gradient: &CanvasGradient) {
        let mut g = self.g();
        if !Arc::ptr_eq(&gradient.inner, &self.inner) {
            // Another canvas's gradient: this context cannot paint it.
            g.note("a gradient from another canvas was ignored".into());
            return;
        }
        let p = Paint::Gradient(gradient.id);
        g.paint_op(fill, &p);
        if fill {
            g.state.fill = p;
        } else {
            g.state.stroke = p;
        }
    }

    fn style(&self, fill: bool) -> Style {
        let g = self.g();
        match if fill { &g.state.fill } else { &g.state.stroke } {
            Paint::Color(_, Some(text)) => Style::Color(text.to_string()),
            Paint::Color(c, None) => Style::Color(c.serialize()),
            Paint::Gradient(id) => Style::Gradient(CanvasGradient {
                id: *id,
                inner: self.inner.clone(),
            }),
            Paint::Pattern(id) => Style::Pattern(CanvasPattern {
                id: *id,
                inner: self.inner.clone(),
            }),
        }
    }

    /// `fillStyle = "…"`: an unparseable colour is ignored.
    pub fn set_fill_style_str(&self, v: &str) {
        self.set_style_str(true, v)
    }

    /// `fillStyle = gradient`.
    pub fn set_fill_style_canvas_gradient(&self, v: &CanvasGradient) {
        self.set_style_gradient(true, v)
    }

    /// `fillStyle`.
    pub fn fill_style(&self) -> Style {
        self.style(true)
    }

    /// `strokeStyle = "…"`: an unparseable colour is ignored.
    pub fn set_stroke_style_str(&self, v: &str) {
        self.set_style_str(false, v)
    }

    /// `strokeStyle = gradient`.
    pub fn set_stroke_style_canvas_gradient(&self, v: &CanvasGradient) {
        self.set_style_gradient(false, v)
    }

    /// `strokeStyle`.
    pub fn stroke_style(&self) -> Style {
        self.style(false)
    }

    fn gradient(&self, op: Op, operands: &[f64]) -> CanvasGradient {
        let mut g = self.g();
        let id = g.next_object();
        let mut all = vec![id as f64];
        all.extend_from_slice(operands);
        g.op(op, &all);
        CanvasGradient {
            id,
            inner: self.inner.clone(),
        }
    }

    /// `createLinearGradient(x0, y0, x1, y1)`: `TypeError` for a non-finite
    /// argument (they are `double`).
    pub fn create_linear_gradient(
        &self,
        x0: f64,
        y0: f64,
        x1: f64,
        y1: f64,
    ) -> Result<CanvasGradient, DomException> {
        if ![x0, y0, x1, y1].iter().all(|v| v.is_finite()) {
            return throw("TypeError", "The provided double value is non-finite.");
        }
        Ok(self.gradient(Op::LinearGradient, &[x0, y0, x1, y1]))
    }

    /// `createRadialGradient(x0, y0, r0, x1, y1, r1)`: `TypeError` for a
    /// non-finite argument, `IndexSizeError` for a negative radius.
    pub fn create_radial_gradient(
        &self,
        x0: f64,
        y0: f64,
        r0: f64,
        x1: f64,
        y1: f64,
        r1: f64,
    ) -> Result<CanvasGradient, DomException> {
        if ![x0, y0, r0, x1, y1, r1].iter().all(|v| v.is_finite()) {
            return throw("TypeError", "The provided double value is non-finite.");
        }
        if r0 < 0.0 || r1 < 0.0 {
            let which = if r0 < 0.0 { "r0" } else { "r1" };
            return throw(
                "IndexSizeError",
                format!("The {which} provided is less than 0."),
            );
        }
        Ok(self.gradient(Op::RadialGradient, &[x0, y0, r0, x1, y1, r1]))
    }

    /// `createConicGradient(startAngle, x, y)`: `TypeError` for a
    /// non-finite argument.
    pub fn create_conic_gradient(
        &self,
        start_angle: f64,
        x: f64,
        y: f64,
    ) -> Result<CanvasGradient, DomException> {
        if ![start_angle, x, y].iter().all(|v| v.is_finite()) {
            return throw("TypeError", "The provided double value is non-finite.");
        }
        Ok(self.gradient(Op::ConicGradient, &[start_angle, x, y]))
    }

    // --- Compositing ---------------------------------------------------

    /// `globalAlpha = v`: ignored unless finite and within [0, 1].
    pub fn set_global_alpha(&self, v: f64) {
        let mut g = self.g();
        if v.is_finite() && (0.0..=1.0).contains(&v) && g.state.alpha != v {
            g.state.alpha = v;
            g.op(Op::GlobalAlpha, &[v]);
        }
    }

    /// `globalAlpha`.
    pub fn global_alpha(&self) -> f64 {
        self.g().state.alpha
    }

    /// `globalCompositeOperation = v`: an unknown value is ignored.
    pub fn set_global_composite_operation(&self, v: &str) -> Result<(), DomException> {
        let Some(k) = COMPOSITE.iter().position(|n| *n == v) else {
            return Ok(());
        };
        let mut g = self.g();
        if g.state.composite != k as u8 {
            g.state.composite = k as u8;
            g.op(Op::Composite, &[k as f64]);
        }
        Ok(())
    }

    /// `globalCompositeOperation`.
    pub fn global_composite_operation(&self) -> String {
        COMPOSITE[self.g().state.composite as usize].into()
    }

    // --- Shadows -------------------------------------------------------

    /// `shadowColor = v`: an unparseable colour is ignored.
    pub fn set_shadow_color(&self, v: &str) {
        let mut g = self.g();
        let Some((c, text)) = g.color(v) else {
            return;
        };
        if g.state.shadow_color.0 != c {
            g.op(Op::ShadowColor, &c.operands());
        }
        g.state.shadow_color = (c, text);
    }

    /// `shadowColor`.
    pub fn shadow_color(&self) -> String {
        let g = self.g();
        match &g.state.shadow_color {
            (_, Some(text)) => text.to_string(),
            (c, None) => c.serialize(),
        }
    }

    /// `shadowBlur = v`: ignored unless finite and non-negative.
    pub fn set_shadow_blur(&self, v: f64) {
        let mut g = self.g();
        if v.is_finite() && v >= 0.0 && g.state.shadow_blur != v {
            g.state.shadow_blur = v;
            g.op(Op::ShadowBlur, &[v]);
        }
    }

    /// `shadowBlur`.
    pub fn shadow_blur(&self) -> f64 {
        self.g().state.shadow_blur
    }

    fn set_shadow_offset(&self, x: Option<f64>, y: Option<f64>) {
        let mut g = self.g();
        let (ox, oy) = g.state.shadow_offset;
        let next = (x.unwrap_or(ox), y.unwrap_or(oy));
        if next.0.is_finite() && next.1.is_finite() && next != (ox, oy) {
            g.state.shadow_offset = next;
            g.op(Op::ShadowOffset, &[next.0, next.1]);
        }
    }

    /// `shadowOffsetX = v`: ignored unless finite.
    pub fn set_shadow_offset_x(&self, v: f64) {
        self.set_shadow_offset(Some(v), None)
    }

    /// `shadowOffsetX`.
    pub fn shadow_offset_x(&self) -> f64 {
        self.g().state.shadow_offset.0
    }

    /// `shadowOffsetY = v`: ignored unless finite.
    pub fn set_shadow_offset_y(&self, v: f64) {
        self.set_shadow_offset(None, Some(v))
    }

    /// `shadowOffsetY`.
    pub fn shadow_offset_y(&self) -> f64 {
        self.g().state.shadow_offset.1
    }

    // --- Image smoothing -------------------------------------------------

    fn set_smoothing(&self, next: (bool, u8)) {
        let mut g = self.g();
        if g.state.smoothing != next {
            g.state.smoothing = next;
            g.op(Op::ImageSmoothing, &[next.0 as u8 as f64, next.1 as f64]);
        }
    }

    /// `imageSmoothingEnabled = v`.
    pub fn set_image_smoothing_enabled(&self, v: bool) {
        let q = self.g().state.smoothing.1;
        self.set_smoothing((v, q))
    }

    /// `imageSmoothingEnabled`.
    pub fn image_smoothing_enabled(&self) -> bool {
        self.g().state.smoothing.0
    }

    /// `imageSmoothingQuality = v`: `low`, `medium` or `high`; anything else
    /// ignored.
    pub fn set_image_smoothing_quality(&self, v: &str) {
        let Some(k) = ["low", "medium", "high"].iter().position(|n| *n == v) else {
            return;
        };
        let e = self.g().state.smoothing.0;
        self.set_smoothing((e, k as u8))
    }

    /// `imageSmoothingQuality`.
    pub fn image_smoothing_quality(&self) -> String {
        ["low", "medium", "high"][self.g().state.smoothing.1 as usize].into()
    }
}
