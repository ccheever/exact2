//! Canvas 2D's half of the protocol in the runner (LLP 1056 D4, D5).
//!
//! @ref LLP 1056 D4 (geometry, generations, stamps, errors, limits, first
//! frame) / D5 (time) / §5 (the agent)
//!
//! A `canvas surface=` whose name is in the data source's
//! [`DataSource::canvas_surfaces`] roster is a 2D canvas: its surface
//! arguments never reach a GPU module. The runner keeps one record per such
//! canvas: its lifetime and size generation, the causes of its next draw
//! (coalesced before the draw runs), the draw in flight, and — for a Rust
//! source — the recorder that persists across draws within a generation.
//! The host reports each canvas's content box and scale
//! ([`Runner::set_canvas_geometry`]), asks for due draws
//! ([`Runner::draw_canvases`]) and applies the stamped lists it takes
//! ([`Runner::take_canvas_lists`]) in order. A reply stamped with a retired
//! lifetime or generation is discarded whole.

use super::{DataSource, Runner};
use exact_canvas::{list, Causes, Context2d, Env, Frame, ImageSlot, Images, Rgba, TextEngine};
use exact_kernel::ViewId;
use exact_plan::Value;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// A canvas node's CSS `color` (for `currentColor`) and whether its
/// `direction` is `rtl` (for `direction = "inherit"`), LLP 1056 D3, D8.
pub type NodeStyle<'a> = &'a dyn Fn(ViewId) -> (Option<Rgba>, bool);

/// A canvas's content box in coordinate units and its device scale, as the
/// host that lays it out reports it; `bitmap` is an explicit bitmap size
/// (`bitmap-width`/`bitmap-height`, LLP 1056 D6 r3), stretched to the box.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Geometry {
    /// Content-box width, CSS px.
    pub width: f64,
    /// Content-box height, CSS px.
    pub height: f64,
    /// Device pixels per CSS px.
    pub scale: f64,
    /// An explicit bitmap size, when the author set one.
    pub bitmap: Option<(u32, u32)>,
}

/// The backing store a geometry implies: what a generation is keyed by.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Backing {
    pixel_width: u32,
    pixel_height: u32,
    /// Coordinate units per… the coordinate space's size.
    width: f64,
    height: f64,
    scale: f64,
    stretch: bool,
}

impl Backing {
    fn of(g: Geometry) -> Backing {
        match g.bitmap {
            Some((w, h)) => Backing {
                pixel_width: w,
                pixel_height: h,
                width: w as f64,
                height: h as f64,
                scale: 1.0,
                stretch: true,
            },
            None => Backing {
                pixel_width: (g.width * g.scale).round().max(0.0) as u32,
                pixel_height: (g.height * g.scale).round().max(0.0) as u32,
                width: g.width,
                height: g.height,
                scale: g.scale,
                stretch: false,
            },
        }
    }

    fn bytes(self) -> u64 {
        self.pixel_width as u64 * self.pixel_height as u64 * 4
    }

    fn empty(self) -> bool {
        self.pixel_width == 0 || self.pixel_height == 0
    }
}

/// The size limits (LLP 1056 D4, r3): what browsers enforce, and a total
/// budget of a quarter of physical memory, counting two bitmaps a canvas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Pixels per side (Chrome's `kMaxSkiaDim`).
    pub max_side: u32,
    /// Pixels per canvas (Chrome's `kMaxCanvasArea`; WebKit's on iOS).
    pub max_area: u64,
    /// Bytes charged across every canvas; `u64::MAX` where the browser owns
    /// the budget.
    pub budget: u64,
}

impl Limits {
    /// The web: the browser enforces its own.
    pub const WEB: Limits = Limits {
        max_side: 65_535,
        max_area: 268_435_456,
        budget: u64::MAX,
    };

    /// A native host with `physical` bytes of memory; `ios` takes WebKit's
    /// iOS area (8,192²).
    pub fn native(physical: u64, ios: bool) -> Limits {
        Limits {
            max_side: 65_535,
            max_area: if ios { 67_108_864 } else { 268_435_456 },
            budget: physical / 4,
        }
    }
}

impl Default for Limits {
    fn default() -> Self {
        Limits::WEB
    }
}

/// One draw, as the source is asked it.
#[derive(Debug)]
pub struct DrawRequest<'a> {
    /// The canvas's lifetime id: unique in the process.
    pub canvas: u64,
    /// Its size generation; a new one starts a fresh recorder.
    pub generation: u32,
    /// The request's sequence number within the canvas.
    pub seq: u64,
    /// The surface's name.
    pub surface: &'a str,
    /// Its arguments, positional or in the authored names' order.
    pub args: &'a [Value],
    /// The authored argument names, empty for positional calls.
    pub names: &'a [String],
    /// What the draw is told.
    pub frame: Frame,
    /// The canvas node's `color`, for `currentColor`.
    pub current_color: Option<Rgba>,
    /// The canvas node's `direction` is `rtl`.
    pub rtl: bool,
}

impl DrawRequest<'_> {
    /// The request as the TypeScript seam reads it (`canvas/recorder.js`):
    /// `{canvas, generation, seq, surface, args, frame}`, the arguments as
    /// a GPU surface's `bind` receives them.
    pub fn json(&self) -> String {
        let mut s = format!(
            "{{\"canvas\":{},\"generation\":{},\"seq\":{},\"surface\":",
            self.canvas, self.generation, self.seq
        );
        crate::agent::quote(self.surface, &mut s);
        s.push_str(",\"args\":");
        let named = !self.names.is_empty();
        s.push(if named { '{' } else { '[' });
        for (i, v) in self.args.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            if named {
                crate::agent::quote(&self.names[i], &mut s);
                s.push(':');
            }
            crate::instance::value_json(v, &mut s);
        }
        s.push(if named { '}' } else { ']' });
        let f = &self.frame;
        let _ = write!(
            s,
            ",\"frame\":{{\"time\":{},\"mounted\":{},\"cause\":\"{}\",\"causes\":[",
            crate::agent::num(f.time),
            crate::agent::num(f.mounted),
            f.cause.primary()
        );
        for (i, c) in f.cause.names().iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            let _ = write!(s, "\"{c}\"");
        }
        let _ = write!(
            s,
            "],\"width\":{},\"height\":{},\"pixelWidth\":{},\"pixelHeight\":{},\"scale\":{}}}}}",
            crate::agent::num(f.width),
            crate::agent::num(f.height),
            f.pixel_width,
            f.pixel_height,
            crate::agent::num(f.scale)
        );
        s.pop();
        if let Some(c) = self.current_color {
            let _ = write!(
                s,
                ",\"currentColor\":[{},{},{},{}]",
                c.r,
                c.g,
                c.b,
                crate::agent::num(c.a)
            );
        }
        let _ = write!(s, ",\"rtl\":{}}}", self.rtl);
        s
    }
}

impl DrawReply {
    /// A TypeScript seam's reply ([`exact_canvas::seam`]), or its fault as
    /// the draw's error.
    pub fn from_seam(json: &str) -> DrawReply {
        match exact_canvas::seam::reply(json) {
            Ok(r) => DrawReply {
                lists: r.lists,
                wants_frame: r.frame,
                error: r.error,
                notes: r.notes,
            },
            Err(e) => DrawReply {
                error: Some(e),
                ..Default::default()
            },
        }
    }
}

/// What a source did with a draw request.
#[derive(Debug)]
pub enum Drawn {
    /// It drew: here is the result.
    Now(DrawReply),
    /// It will reply through [`Runner::canvas_reply`].
    Later,
}

/// A draw's result.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct DrawReply {
    /// The recorded lists, in order.
    pub lists: Vec<Vec<u8>>,
    /// Whether it asked for another frame.
    pub wants_frame: bool,
    /// What it threw, if it threw; the lists hold what it did before.
    pub error: Option<String>,
    /// Development notes (ignored stage-2 values).
    pub notes: Vec<String>,
}

/// Stamped lists for the host to apply to one canvas's bitmap, in order.
#[derive(Debug, Clone, PartialEq)]
pub struct CanvasList {
    /// The canvas node.
    pub view: ViewId,
    /// Its lifetime.
    pub lifetime: u64,
    /// Its size generation.
    pub generation: u32,
    /// The draw's sequence number.
    pub seq: u64,
    /// A new generation starts here: the host allocates the bitmap at this
    /// size, cleared, with the replayer's state at its defaults, before
    /// applying `lists`. A zero size means no bitmap (the box's background).
    pub fresh: bool,
    /// The backing width, px.
    pub pixel_width: u32,
    /// The backing height, px.
    pub pixel_height: u32,
    /// Backing pixels per coordinate unit: the replayer's base scale.
    pub scale: f64,
    /// An explicit bitmap, stretched to the content box.
    pub stretch: bool,
    /// The lists.
    pub lists: Vec<Vec<u8>>,
    /// The draw asked for the next frame: the canvas animates (a host may
    /// present it differently, LLP 1056 §8.4).
    pub animating: bool,
}

static LIFETIMES: AtomicU64 = AtomicU64::new(1);

/// One 2D canvas.
struct Record {
    surface: String,
    names: Vec<String>,
    args: Vec<Value>,
    mounted: f64,
    lifetime: u64,
    generation: u32,
    next_seq: u64,
    applied_seq: u64,
    geometry: Option<Geometry>,
    backing: Option<Backing>,
    /// A new generation not yet announced to the host.
    fresh: bool,
    causes: Causes,
    in_flight: Option<(u32, u64)>,
    wants_frame: bool,
    error: Option<String>,
    refused: Option<String>,
    ctx: Context2d,
    gradients: Vec<u32>,
    last_list: Vec<u8>,
    last_ops: usize,
    last_bytes: usize,
    last_draw: Option<f64>,
    draws: u64,
    /// The clock value it was last drawn at for a frame (D5: at most once
    /// per clock value).
    framed_at: Option<f64>,
    /// Its last draw drew text: a font that loads redraws it (D8).
    text: bool,
}

/// Every 2D canvas the runner draws.
#[derive(Default)]
struct Canvases {
    records: BTreeMap<ViewId, Record>,
    roster: Option<Vec<(String, usize)>>,
    limits: Limits,
    out: Vec<CanvasList>,
    retired: Vec<(u64, u32)>,
    notes: Vec<String>,
    text: Option<Arc<dyn TextEngine>>,
    images: ImageSlot,
}

/// Lists waiting for the host beyond this hold new draws back (LLP 1056 D4).
const QUEUED_BYTES: usize = 8 << 20;

/// Canvas 2D's engine, linked by use (LLP 1047 D2, 1047.000 §9): only
/// [`engine`] makes one, and only [`crate::RunnerLinks::ALL`] and a web
/// artifact whose plan has a canvas with a surface name it, so a plan with
/// none carries none of it. The [`Runner`]'s canvas methods are its whole
/// interface; they pass in what the engine needs of the runner: the
/// source's roster and draws, the clock, which views are live, and the log.
pub trait CanvasEngine {
    /// Take one commit's 2D surface updates (mounts and arguments), and
    /// return the rest, the GPU module's, in order.
    fn publish(
        &mut self,
        updates: Vec<crate::SurfaceUpdate>,
        roster: &dyn Fn() -> Vec<(String, usize)>,
        now: f64,
    ) -> Vec<crate::SurfaceUpdate>;
    /// [`Runner::set_canvas_limits`].
    fn set_limits(&mut self, limits: Limits);
    /// The live canvases in tree order, dropping those `alive` denies.
    fn views(&mut self, alive: &dyn Fn(ViewId) -> bool) -> Vec<ViewId>;
    /// [`Runner::set_canvas_geometry`].
    fn set_geometry(&mut self, view: ViewId, geometry: Geometry);
    /// [`Runner::canvas_frame`], at `now`.
    fn frame(&mut self, now: f64);
    /// [`Runner::canvas_wants_frame`].
    fn wants_frame(&self) -> bool;
    /// [`Runner::canvas_owed`].
    fn owed(&self, ready: bool, on_screen: &dyn Fn(ViewId) -> bool) -> bool;
    /// Drop the canvases `alive` denies, and take the generations retired
    /// since the last take, for the source.
    fn take_retired(&mut self, alive: &dyn Fn(ViewId) -> bool) -> Vec<(u64, u32)>;
    /// [`Runner::draw_canvases`]: every due draw, through `draw`.
    #[allow(clippy::too_many_arguments)]
    fn draw(
        &mut self,
        ready: bool,
        now: f64,
        on_screen: &dyn Fn(ViewId) -> bool,
        style: NodeStyle<'_>,
        draw: &mut dyn FnMut(&DrawRequest<'_>, &Context2d) -> Drawn,
        log: &mut dyn FnMut(String),
    );
    /// [`Runner::canvas_reply`], at `now`.
    fn reply(
        &mut self,
        lifetime: u64,
        generation: u32,
        seq: u64,
        reply: DrawReply,
        now: f64,
        log: &mut dyn FnMut(String),
    ) -> Result<(), String>;
    /// [`Runner::take_canvas_lists`].
    fn take(&mut self) -> Vec<CanvasList>;
    /// [`Runner::is_canvas_2d`].
    fn is_2d(&self, view: ViewId) -> bool;
    /// `state.canvas`'s records.
    fn state(&self, s: &mut String);
    /// [`Runner::canvas_describe`].
    fn describe(&self, view: ViewId) -> Option<Vec<String>>;
    /// [`Runner::set_canvas_text`].
    fn set_text(&mut self, engine: Arc<dyn TextEngine>);
    /// [`Runner::take_canvas_image_requests`].
    fn take_image_requests(&mut self) -> Vec<String>;
    /// [`Runner::canvas_images_pending`].
    fn images_pending(&self) -> bool;
    /// [`Runner::canvas_image`].
    fn image(&mut self, src: &str, result: Result<(u32, u32), String>, also: &[u64]);
    /// [`Runner::canvas_fonts_loaded`].
    fn fonts_loaded(&mut self);
}

/// A Canvas 2D engine: what [`crate::RunnerLinks::canvas`] names.
pub fn engine() -> Box<dyn CanvasEngine> {
    Box::<Canvases>::default()
}

impl Canvases {
    /// The image table, if a draw has made one (it is linked by use).
    fn table(&self) -> Option<Arc<dyn Images>> {
        self.images
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    fn prune(&mut self, alive: &dyn Fn(ViewId) -> bool) {
        let gone: Vec<ViewId> = self
            .records
            .keys()
            .copied()
            .filter(|v| !alive(*v))
            .collect();
        for v in gone {
            let r = self.records.remove(&v).expect("listed");
            self.retired.push((r.lifetime, r.generation));
        }
    }
}

impl CanvasEngine for Canvases {
    fn publish(
        &mut self,
        updates: Vec<crate::SurfaceUpdate>,
        roster: &dyn Fn() -> Vec<(String, usize)>,
        now: f64,
    ) -> Vec<crate::SurfaceUpdate> {
        if self.roster.is_none() {
            self.roster = Some(roster());
        }
        let mut rest = Vec::new();
        for u in updates {
            let roster = self.roster.as_deref().unwrap_or(&[]);
            if !roster.iter().any(|(name, _)| *name == u.name) {
                rest.push(u);
                continue;
            }
            match self.records.get_mut(&u.view) {
                Some(r) if r.surface == u.name => {
                    r.args = u.values;
                    r.names = u.names;
                    r.causes = r.causes.with(Causes::ARGS);
                    // D4: an error stops frames until the next args.
                    r.error = None;
                }
                _ => {
                    if let Some(old) = self.records.remove(&u.view) {
                        self.retired.push((old.lifetime, old.generation));
                    }
                    self.records.insert(
                        u.view,
                        Record {
                            surface: u.name,
                            names: u.names,
                            args: u.values,
                            mounted: now,
                            lifetime: LIFETIMES.fetch_add(1, Ordering::Relaxed),
                            generation: 0,
                            next_seq: 1,
                            applied_seq: 0,
                            geometry: None,
                            backing: None,
                            fresh: false,
                            causes: Causes::MOUNT,
                            in_flight: None,
                            wants_frame: false,
                            error: None,
                            refused: None,
                            ctx: Context2d::new(),
                            gradients: Vec::new(),
                            last_list: Vec::new(),
                            last_ops: 0,
                            last_bytes: 0,
                            last_draw: None,
                            draws: 0,
                            framed_at: None,
                            text: false,
                        },
                    );
                }
            }
        }
        rest
    }

    fn set_limits(&mut self, limits: Limits) {
        self.limits = limits;
    }

    fn views(&mut self, alive: &dyn Fn(ViewId) -> bool) -> Vec<ViewId> {
        self.prune(alive);
        self.records.keys().copied().collect()
    }

    fn set_geometry(&mut self, view: ViewId, geometry: Geometry) {
        let limits = self.limits;
        let others: u64 = self
            .records
            .iter()
            .filter(|(v, r)| **v != view && r.refused.is_none())
            .filter_map(|(_, r)| r.backing.map(|b| 2 * b.bytes()))
            .sum();
        let Some(r) = self.records.get_mut(&view) else {
            return;
        };
        if r.geometry == Some(geometry) {
            return;
        }
        r.geometry = Some(geometry);
        let mut next = Backing::of(geometry);
        let refusal = if next.pixel_width > limits.max_side || next.pixel_height > limits.max_side {
            Some(format!(
                "{} × {} px is over the {} px side limit",
                next.pixel_width, next.pixel_height, limits.max_side
            ))
        } else if next.pixel_width as u64 * next.pixel_height as u64 > limits.max_area {
            Some(format!(
                "{} × {} px is over the {} px area limit",
                next.pixel_width, next.pixel_height, limits.max_area
            ))
        } else if others.saturating_add(2 * next.bytes()) > limits.budget {
            Some(format!(
                "{} × {} px would take canvas memory past the {} MiB budget",
                next.pixel_width,
                next.pixel_height,
                limits.budget >> 20
            ))
        } else {
            None
        };
        if refusal.is_some() {
            next.pixel_width = 0;
            next.pixel_height = 0;
        }
        let same = r.backing.is_some_and(|b| {
            b.pixel_width == next.pixel_width
                && b.pixel_height == next.pixel_height
                && b.stretch == next.stretch
                && (b.stretch || b.scale == next.scale)
        });
        r.refused = refusal;
        if same {
            // A CSS change that rounds to the same store is not a new
            // generation, but the coordinate space may still move.
            if let Some(b) = r.backing.as_mut() {
                b.width = next.width;
                b.height = next.height;
            }
            return;
        }
        let had = r.backing.is_some();
        r.backing = Some(next);
        if had {
            self.retired.push((r.lifetime, r.generation));
            r.generation += 1;
            r.causes = r.causes.with(Causes::SIZE);
        }
        r.fresh = true;
        r.ctx = Context2d::new();
        r.gradients.clear();
        r.in_flight = None;
        r.framed_at = None;
    }

    fn frame(&mut self, now: f64) {
        for r in self.records.values_mut() {
            if r.wants_frame && r.framed_at != Some(now) {
                r.causes = r.causes.with(Causes::FRAME);
            }
        }
    }

    fn wants_frame(&self) -> bool {
        self.records.values().any(|r| r.wants_frame)
    }

    fn owed(&self, ready: bool, on_screen: &dyn Fn(ViewId) -> bool) -> bool {
        // What `draw` would hand the host or the source: lists and retired
        // generations waiting, a fresh bitmap, or a due draw it would run.
        !self.out.is_empty()
            || !self.retired.is_empty()
            || self.records.iter().any(|(view, r)| {
                if r.fresh {
                    return true;
                }
                if !ready || r.in_flight.is_some() || r.backing.is_none_or(|b| b.empty()) {
                    return false;
                }
                let mut causes = r.causes;
                if causes.has(Causes::FRAME) && !on_screen(*view) {
                    causes = Causes(causes.0 & !Causes::FRAME.0);
                }
                !causes.is_empty()
            })
    }

    fn take_retired(&mut self, alive: &dyn Fn(ViewId) -> bool) -> Vec<(u64, u32)> {
        self.prune(alive);
        std::mem::take(&mut self.retired)
    }

    fn draw(
        &mut self,
        ready: bool,
        now: f64,
        on_screen: &dyn Fn(ViewId) -> bool,
        style: NodeStyle<'_>,
        draw: &mut dyn FnMut(&DrawRequest<'_>, &Context2d) -> Drawn,
        log: &mut dyn FnMut(String),
    ) {
        let views: Vec<ViewId> = self.records.keys().copied().collect();
        for view in views {
            let queued: usize = self
                .out
                .iter()
                .flat_map(|c| c.lists.iter().map(Vec::len))
                .sum();
            let r = self.records.get_mut(&view).expect("listed");
            if r.fresh {
                // The host learns the new generation even before a draw, so
                // an empty or refused store clears the old bitmap.
                r.fresh = false;
                let b = r.backing.expect("fresh has a backing");
                self.out.push(CanvasList {
                    view,
                    lifetime: r.lifetime,
                    generation: r.generation,
                    seq: 0,
                    fresh: true,
                    pixel_width: b.pixel_width,
                    pixel_height: b.pixel_height,
                    scale: b.scale,
                    stretch: b.stretch,
                    lists: Vec::new(),
                    animating: false,
                });
            }
            let Some(b) = r.backing else { continue };
            // A module not yet active draws nothing yet (D4: its canvases
            // are transparent until its executor is active); the causes wait.
            if !ready || b.empty() || r.in_flight.is_some() || queued > QUEUED_BYTES {
                continue;
            }
            let mut causes = r.causes;
            if causes.has(Causes::FRAME) && !on_screen(view) {
                // A frame is held until the canvas is seen; the other
                // causes draw now.
                causes = Causes(causes.0 & !Causes::FRAME.0);
            }
            if causes.is_empty() {
                continue;
            }
            r.causes = Causes(r.causes.0 & !causes.0);
            if causes.has(Causes::FRAME) {
                r.framed_at = Some(now);
            }
            let seq = r.next_seq;
            r.next_seq += 1;
            r.in_flight = Some((r.generation, seq));
            let frame = Frame {
                time: now,
                mounted: r.mounted,
                cause: causes,
                width: b.width,
                height: b.height,
                pixel_width: b.pixel_width,
                pixel_height: b.pixel_height,
                scale: b.scale,
            };
            let (lifetime, generation) = (r.lifetime, r.generation);
            let (surface, args, names, ctx) = (
                r.surface.clone(),
                r.args.clone(),
                r.names.clone(),
                r.ctx.clone(),
            );
            let (current_color, rtl) = style(view);
            ctx.set_env(Env {
                canvas: lifetime,
                text: self.text.clone(),
                images: self.images.clone(),
                current_color,
                rtl,
            });
            let request = DrawRequest {
                canvas: lifetime,
                generation,
                seq,
                surface: &surface,
                args: &args,
                names: &names,
                frame,
                current_color,
                rtl,
            };
            match draw(&request, &ctx) {
                Drawn::Now(reply) => {
                    let _ = self.reply(lifetime, generation, seq, reply, now, log);
                }
                Drawn::Later => {}
            }
        }
    }

    fn reply(
        &mut self,
        lifetime: u64,
        generation: u32,
        seq: u64,
        reply: DrawReply,
        now: f64,
        log: &mut dyn FnMut(String),
    ) -> Result<(), String> {
        let Some((&view, r)) = self
            .records
            .iter_mut()
            .find(|(_, r)| r.lifetime == lifetime)
        else {
            return Err(format!("canvas {lifetime} is gone"));
        };
        if r.generation != generation || r.in_flight != Some((generation, seq)) {
            return Err(format!(
                "canvas {lifetime}: reply for generation {generation} seq {seq} is stale"
            ));
        }
        r.in_flight = None;
        let mut gradients = r.gradients.clone();
        let mut ops = 0;
        for l in &reply.lists {
            match list::check(l, &mut gradients) {
                Ok(n) => ops += n,
                Err(e) => {
                    let line = format!(
                        "canvas {} ({}): {e}; a new generation starts",
                        view, r.surface
                    );
                    r.error = Some(e.to_string());
                    r.wants_frame = false;
                    self.retired.push((r.lifetime, r.generation));
                    r.generation += 1;
                    r.fresh = true;
                    r.causes = r.causes.with(Causes::SIZE);
                    r.ctx = Context2d::new();
                    r.gradients.clear();
                    log(line);
                    return Err("malformed list".into());
                }
            }
        }
        r.gradients = gradients;
        r.text = reply.lists.iter().any(|l| list::draws_text(l));
        r.applied_seq = seq;
        r.draws += 1;
        r.last_draw = Some(now);
        r.last_ops = ops;
        r.last_bytes = reply.lists.iter().map(Vec::len).sum();
        if let Some(last) = reply.lists.last() {
            r.last_list = last.clone();
        } else {
            r.last_list.clear();
        }
        r.wants_frame = reply.wants_frame && reply.error.is_none();
        let mut line = None;
        if let Some(e) = &reply.error {
            line = Some(format!("canvas {} ({}) draw threw: {e}", view, r.surface));
        }
        r.error = reply.error;
        let b = r.backing.expect("drawn canvases have a backing");
        let out = CanvasList {
            view,
            lifetime,
            generation,
            seq,
            fresh: false,
            pixel_width: b.pixel_width,
            pixel_height: b.pixel_height,
            scale: b.scale,
            stretch: b.stretch,
            lists: reply.lists,
            animating: r.wants_frame,
        };
        let surface = r.surface.clone();
        if !out.lists.is_empty() {
            self.out.push(out);
        }
        for n in reply.notes {
            self.notes.push(format!("canvas {view} ({surface}): {n}"));
        }
        if let Some(line) = line {
            log(line);
        }
        for n in std::mem::take(&mut self.notes) {
            log(n);
        }
        Ok(())
    }

    fn take(&mut self) -> Vec<CanvasList> {
        std::mem::take(&mut self.out)
    }

    fn is_2d(&self, view: ViewId) -> bool {
        self.records.contains_key(&view)
    }

    fn state(&self, s: &mut String) {
        s.push('[');
        for (i, (view, r)) in self.records.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            let _ = write!(s, "{{\"view\":{view},\"surface\":");
            crate::agent::quote(&r.surface, s);
            let requested = r.next_seq - 1;
            let _ = write!(
                s,
                ",\"context\":\"2d\",\"artifact\":\"data\",\"lifetime\":{},\"generation\":{},\"applied\":{},\"requested\":{},\"pending\":{},\"animating\":{},\"draws\":{},\"ops\":{},\"bytes\":{}",
                r.lifetime,
                r.generation,
                r.applied_seq,
                requested,
                r.draws == 0 || r.applied_seq < requested,
                r.wants_frame,
                r.draws,
                r.last_ops,
                r.last_bytes
            );
            match r.backing {
                Some(b) => {
                    let _ = write!(
                        s,
                        ",\"width\":{},\"height\":{},\"scale\":{},\"stretch\":{}",
                        b.pixel_width,
                        b.pixel_height,
                        crate::agent::num(b.scale),
                        b.stretch
                    );
                }
                None => {
                    s.push_str(",\"width\":null,\"height\":null,\"scale\":null,\"stretch\":false")
                }
            }
            s.push_str(",\"lastDraw\":");
            match r.last_draw {
                Some(t) => {
                    let _ = write!(s, "{}", crate::agent::num(t));
                }
                None => s.push_str("null"),
            }
            let broken = self.table().map_or_else(Vec::new, |t| t.broken(r.lifetime));
            s.push_str(",\"brokenImages\":[");
            for (i, b) in broken.iter().enumerate() {
                if i > 0 {
                    s.push(',');
                }
                crate::agent::quote(b, s);
            }
            s.push(']');
            for (key, v) in [("error", &r.error), ("refused", &r.refused)] {
                let _ = write!(s, ",\"{key}\":");
                match v {
                    Some(e) => crate::agent::quote(e, s),
                    None => s.push_str("null"),
                }
            }
            s.push('}');
        }
        s.push(']');
    }

    fn describe(&self, view: ViewId) -> Option<Vec<String>> {
        let r = self.records.get(&view)?;
        if r.last_list.is_empty() {
            return Some(Vec::new());
        }
        Some(list::describe(&r.last_list, 200, 16 * 1024))
    }

    fn set_text(&mut self, engine: Arc<dyn TextEngine>) {
        self.text = Some(engine);
    }

    fn take_image_requests(&mut self) -> Vec<String> {
        self.table().map_or_else(Vec::new, |t| t.take_requests())
    }

    fn images_pending(&self) -> bool {
        self.table().is_some_and(|t| t.pending())
    }

    fn image(&mut self, src: &str, result: Result<(u32, u32), String>, also: &[u64]) {
        let subscribers = match self.table() {
            Some(t) => t.answer(src, result, also),
            None => also.to_vec(),
        };
        for r in self.records.values_mut() {
            if subscribers.contains(&r.lifetime) {
                r.causes = r.causes.with(Causes::IMAGE);
            }
        }
    }

    fn fonts_loaded(&mut self) {
        for r in self.records.values_mut() {
            if r.text {
                r.causes = r.causes.with(Causes::FONT);
            }
        }
    }
}

/// The runner's Canvas 2D interface. Each call reaches the engine only when
/// it is linked; unlinked, there are no 2D canvases, and every canvas with
/// a surface is the GPU module's.
impl<D: DataSource> Runner<D> {
    /// Route one commit's surface updates: a 2D surface's to its record
    /// (mount or arguments), the rest to the GPU module's side-output.
    pub(super) fn publish_surfaces(&mut self, updates: Vec<crate::SurfaceUpdate>) {
        if updates.is_empty() {
            return;
        }
        let now = self.now_ms;
        let rest = match self.canvases.as_mut() {
            Some(c) => {
                let data = &self.data;
                c.publish(updates, &|| data.canvas_surfaces(), now)
            }
            None => updates,
        };
        self.surfaces.extend(rest);
    }

    /// The size limits this host enforces; the web's by default.
    pub fn set_canvas_limits(&mut self, limits: Limits) {
        if let Some(c) = self.canvases.as_mut() {
            c.set_limits(limits);
        }
    }

    /// The live 2D canvases, in tree order: the views whose geometry the
    /// host reports. Canvases whose node is gone are dropped here.
    pub fn canvas_views(&mut self) -> Vec<ViewId> {
        let kernel = &self.kernel;
        match self.canvases.as_mut() {
            Some(c) => c.views(&|v| kernel.node(v).is_some()),
            None => Vec::new(),
        }
    }

    /// The host's layout of one 2D canvas (LLP 1056 D4): natively the
    /// kernel's, on the web the browser's `ResizeObserver`. A change of
    /// backing size or scale starts a new generation; a refused size is
    /// reported through `state` and shows the box's background.
    pub fn set_canvas_geometry(&mut self, view: ViewId, geometry: Geometry) {
        if let Some(c) = self.canvases.as_mut() {
            c.set_geometry(view, geometry);
        }
    }

    /// A canvas's explicit bitmap size, from `bitmap-width` and
    /// `bitmap-height` (LLP 1056 D6, r3): an unset one is the web's
    /// default, 300 or 150; neither set is the automatic default.
    pub fn canvas_bitmap(&self, view: ViewId) -> Option<(u32, u32)> {
        let node = self.kernel.node(view)?;
        let int = |id| match node.props.get(id) {
            Some(exact_kernel::PropValue::Int(v)) => Some((*v).clamp(0, u32::MAX as i64) as u32),
            Some(exact_kernel::PropValue::Float(v)) if v.is_finite() => Some(v.max(0.0) as u32),
            _ => None,
        };
        let (w, h) = (
            int(exact_kernel::PropId::BitmapWidth),
            int(exact_kernel::PropId::BitmapHeight),
        );
        (w.is_some() || h.is_some()).then(|| (w.unwrap_or(300), h.unwrap_or(150)))
    }

    /// A native host's geometry for every 2D canvas, from the kernel's
    /// layout in this turn (LLP 1056 D4): the content box, the device
    /// `scale`, and an explicit bitmap size ([`Runner::canvas_bitmap`]).
    pub fn layout_canvases(&mut self, scale: f64) {
        for view in self.canvas_views() {
            let Some(node) = self.kernel.node(view) else {
                continue;
            };
            let (_, _, w, h) = exact_kernel::svg::scene::content_box(&node);
            let bitmap = self.canvas_bitmap(view);
            self.set_canvas_geometry(
                view,
                Geometry {
                    width: w as f64,
                    height: h as f64,
                    scale,
                    bitmap,
                },
            );
        }
    }

    /// The host presented a frame (LLP 1056 D5): every canvas that asked
    /// for one is due, once per clock value.
    pub fn canvas_frame(&mut self) {
        let now = self.now_ms;
        if let Some(c) = self.canvases.as_mut() {
            c.frame(now);
        }
    }

    /// Whether any canvas asked for another frame: the host keeps its frame
    /// source running while this holds.
    pub fn canvas_wants_frame(&self) -> bool {
        self.canvases.as_ref().is_some_and(|c| c.wants_frame())
    }

    /// Whether [`Runner::draw_canvases`] has work: a due draw (a frame
    /// request only while `on_screen`), a fresh bitmap to announce, or
    /// lists and retired generations waiting. A host that runs its draws
    /// in a turn of their own (LLP 1072 §8.5) asks after every other turn.
    pub fn canvas_owed(&self, on_screen: &dyn Fn(ViewId) -> bool) -> bool {
        let ready = self.data.ready();
        self.canvases
            .as_ref()
            .is_some_and(|c| c.owed(ready, on_screen))
    }

    /// Run every due draw, in tree order, after this turn's commits and
    /// geometry. `on_screen` is the host's judgement; a frame request waits
    /// for it, every other cause draws regardless (D4).
    pub fn draw_canvases(&mut self, on_screen: &dyn Fn(ViewId) -> bool) {
        let kernel = &self.kernel;
        let Some(c) = self.canvases.as_mut() else {
            return;
        };
        let retired = c.take_retired(&|v| kernel.node(v).is_some());
        if !retired.is_empty() {
            self.data.canvases_retired(&retired);
        }
        let ready = self.data.ready();
        let now = self.now_ms;
        let data = &mut self.data;
        let mut lines = Vec::new();
        let style = |view: ViewId| canvas_node_style(kernel, view);
        c.draw(
            ready,
            now,
            on_screen,
            &style,
            &mut |request, ctx| data.draw(request, ctx),
            &mut |line| lines.push(line),
        );
        for line in lines {
            self.log(line);
        }
    }

    /// A draw's reply (LLP 1056 D4). Applied, in order, only if its stamps
    /// are the canvas's live lifetime, generation and the draw in flight;
    /// otherwise discarded whole, with the recorder state it produced.
    /// A reply that fails the structural check starts a new generation, so
    /// the recorder and the host cannot diverge. Returns why a reply was
    /// discarded.
    pub fn canvas_reply(
        &mut self,
        lifetime: u64,
        generation: u32,
        seq: u64,
        reply: DrawReply,
    ) -> Result<(), String> {
        let now = self.now_ms;
        let Some(c) = self.canvases.as_mut() else {
            return Err(format!("canvas {lifetime} is gone"));
        };
        let mut lines = Vec::new();
        let result = c.reply(lifetime, generation, seq, reply, now, &mut |line| {
            lines.push(line)
        });
        for line in lines {
            self.log(line);
        }
        result
    }

    /// The stamped lists since the last take, for the host to apply in order.
    pub fn take_canvas_lists(&mut self) -> Vec<CanvasList> {
        self.canvases.as_mut().map_or_else(Vec::new, |c| c.take())
    }

    /// Whether `view` is a 2D canvas this runner draws.
    pub fn is_canvas_2d(&self, view: ViewId) -> bool {
        self.canvases.as_ref().is_some_and(|c| c.is_2d(view))
    }

    /// `state.canvas` (LLP 1056 §5): one record per 2D canvas.
    pub(crate) fn canvas_state(&self, s: &mut String) {
        match self.canvases.as_ref() {
            Some(c) => c.state(s),
            None => s.push_str("[]"),
        }
    }

    /// `layout <node>`'s readable form of a 2D canvas's last list (LLP 1056
    /// §5): at most 200 lines and 16 KiB, development builds only.
    pub fn canvas_describe(&self, view: ViewId) -> Option<Vec<String>> {
        self.canvases.as_ref()?.describe(view)
    }

    /// The host's text engine for Canvas 2D, callable on the executor's
    /// thread (LLP 1056 D8). Without one, text is measured by
    /// [`exact_canvas::font::Estimate`].
    pub fn set_canvas_text(&mut self, engine: Arc<dyn TextEngine>) {
        if let Some(c) = self.canvases.as_mut() {
            c.set_text(engine);
        }
    }

    /// The image handles a draw asked for since the last take: the host
    /// decodes each and answers [`Runner::canvas_image`] (LLP 1056 D9).
    pub fn take_canvas_image_requests(&mut self) -> Vec<String> {
        self.canvases
            .as_mut()
            .map_or_else(Vec::new, |c| c.take_image_requests())
    }

    /// Whether any requested image is still unanswered.
    pub fn canvas_images_pending(&self) -> bool {
        self.canvases.as_ref().is_some_and(|c| c.images_pending())
    }

    /// The host decoded `src` (`Ok` with its natural size in pixels) or
    /// could not (`Err` with why). Every canvas that asked for it, and
    /// `also` (lifetimes the host knows asked, on the web), draws again with
    /// cause `"image"`; a broken image draws nothing and `state` names it.
    pub fn canvas_image(&mut self, src: &str, result: Result<(u32, u32), String>, also: &[u64]) {
        if let Some(c) = self.canvases.as_mut() {
            c.image(src, result, also);
        }
    }

    /// A font finished loading (the web, LLP 1056 D8): every canvas whose
    /// last draw drew text draws again with cause `"font"`.
    pub fn canvas_fonts_loaded(&mut self) {
        if let Some(c) = self.canvases.as_mut() {
            c.fonts_loaded();
        }
    }
}

/// A canvas's node `color` and `direction` (LLP 1056 D3, D8):
/// `currentColor` and `direction = "inherit"` resolve to them.
fn canvas_node_style(kernel: &exact_kernel::Kernel, view: ViewId) -> (Option<Rgba>, bool) {
    let Some(node) = kernel.node(view) else {
        return (None, false);
    };
    let c = match node.text_color() {
        exact_kernel::ColorValue::Fixed(c) | exact_kernel::ColorValue::LightDark(c, _) => c.0,
    };
    let [r, g, b, a] = c.to_be_bytes();
    let rtl = node
        .computed_style(exact_kernel::StyleMask::INHERITED)
        .direction
        == exact_kernel::Direction::Rtl;
    (
        Some(Rgba {
            r,
            g,
            b,
            a: a as f64 / 255.0,
        }),
        rtl,
    )
}
