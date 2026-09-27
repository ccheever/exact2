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
use exact_canvas::{list, Causes, Context2d, Frame};
use exact_kernel::ViewId;
use exact_plan::Value;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::sync::atomic::{AtomicU64, Ordering};

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
}

/// Every 2D canvas the runner draws.
#[derive(Default)]
pub(crate) struct Canvases {
    records: BTreeMap<ViewId, Record>,
    roster: Option<Vec<(String, usize)>>,
    limits: Limits,
    out: Vec<CanvasList>,
    retired: Vec<(u64, u32)>,
    notes: Vec<String>,
}

/// Lists waiting for the host beyond this hold new draws back (LLP 1056 D4).
const QUEUED_BYTES: usize = 8 << 20;

impl<D: DataSource> Runner<D> {
    /// Route one commit's surface updates: a 2D surface's to its record
    /// (mount or arguments), the rest to the GPU module's side-output.
    pub(super) fn publish_surfaces(&mut self, updates: Vec<crate::SurfaceUpdate>) {
        if updates.is_empty() {
            return;
        }
        if self.canvases.roster.is_none() {
            self.canvases.roster = Some(self.data.canvas_surfaces());
        }
        let now = self.now_ms;
        for u in updates {
            let roster = self.canvases.roster.as_deref().unwrap_or(&[]);
            if !roster.iter().any(|(name, _)| *name == u.name) {
                self.surfaces.push(u);
                continue;
            }
            match self.canvases.records.get_mut(&u.view) {
                Some(r) if r.surface == u.name => {
                    r.args = u.values;
                    r.names = u.names;
                    r.causes = r.causes.with(Causes::ARGS);
                    // D4: an error stops frames until the next args.
                    r.error = None;
                }
                _ => {
                    if let Some(old) = self.canvases.records.remove(&u.view) {
                        self.canvases.retired.push((old.lifetime, old.generation));
                    }
                    self.canvases.records.insert(
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
                        },
                    );
                }
            }
        }
    }

    /// The size limits this host enforces; the web's by default.
    pub fn set_canvas_limits(&mut self, limits: Limits) {
        self.canvases.limits = limits;
    }

    /// The live 2D canvases, in tree order: the views whose geometry the
    /// host reports. Canvases whose node is gone are dropped here.
    pub fn canvas_views(&mut self) -> Vec<ViewId> {
        self.prune_canvases();
        self.canvases.records.keys().copied().collect()
    }

    fn prune_canvases(&mut self) {
        let kernel = &self.kernel;
        let gone: Vec<ViewId> = self
            .canvases
            .records
            .keys()
            .copied()
            .filter(|v| kernel.node(*v).is_none())
            .collect();
        for v in gone {
            let r = self.canvases.records.remove(&v).expect("listed");
            self.canvases.retired.push((r.lifetime, r.generation));
        }
    }

    /// The host's layout of one 2D canvas (LLP 1056 D4): natively the
    /// kernel's, on the web the browser's `ResizeObserver`. A change of
    /// backing size or scale starts a new generation; a refused size is
    /// reported through `state` and shows the box's background.
    pub fn set_canvas_geometry(&mut self, view: ViewId, geometry: Geometry) {
        let limits = self.canvases.limits;
        let others: u64 = self
            .canvases
            .records
            .iter()
            .filter(|(v, r)| **v != view && r.refused.is_none())
            .filter_map(|(_, r)| r.backing.map(|b| 2 * b.bytes()))
            .sum();
        let Some(r) = self.canvases.records.get_mut(&view) else {
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
            self.canvases.retired.push((r.lifetime, r.generation));
            r.generation += 1;
            r.causes = r.causes.with(Causes::SIZE);
        }
        r.fresh = true;
        r.ctx = Context2d::new();
        r.gradients.clear();
        r.in_flight = None;
        r.framed_at = None;
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
        for r in self.canvases.records.values_mut() {
            if r.wants_frame && r.framed_at != Some(now) {
                r.causes = r.causes.with(Causes::FRAME);
            }
        }
    }

    /// Whether any canvas asked for another frame: the host keeps its frame
    /// source running while this holds.
    pub fn canvas_wants_frame(&self) -> bool {
        self.canvases.records.values().any(|r| r.wants_frame)
    }

    /// Run every due draw, in tree order, after this turn's commits and
    /// geometry. `on_screen` is the host's judgement; a frame request waits
    /// for it, every other cause draws regardless (D4).
    pub fn draw_canvases(&mut self, on_screen: &dyn Fn(ViewId) -> bool) {
        self.prune_canvases();
        if !self.canvases.retired.is_empty() {
            let retired = std::mem::take(&mut self.canvases.retired);
            self.data.canvases_retired(&retired);
        }
        let views: Vec<ViewId> = self.canvases.records.keys().copied().collect();
        for view in views {
            let queued: usize = self
                .canvases
                .out
                .iter()
                .flat_map(|c| c.lists.iter().map(Vec::len))
                .sum();
            let now = self.now_ms;
            let r = self.canvases.records.get_mut(&view).expect("listed");
            if r.fresh {
                // The host learns the new generation even before a draw, so
                // an empty or refused store clears the old bitmap.
                r.fresh = false;
                let b = r.backing.expect("fresh has a backing");
                self.canvases.out.push(CanvasList {
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
                });
            }
            let Some(b) = r.backing else { continue };
            if b.empty() || r.in_flight.is_some() || queued > QUEUED_BYTES {
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
            let request = DrawRequest {
                canvas: lifetime,
                generation,
                seq,
                surface: &surface,
                args: &args,
                names: &names,
                frame,
            };
            match self.data.draw(&request, &ctx) {
                super::Drawn::Now(reply) => {
                    let _ = self.canvas_reply(lifetime, generation, seq, reply);
                }
                super::Drawn::Later => {}
            }
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
        let Some((&view, r)) = self
            .canvases
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
                    self.canvases.retired.push((r.lifetime, r.generation));
                    r.generation += 1;
                    r.fresh = true;
                    r.causes = r.causes.with(Causes::SIZE);
                    r.ctx = Context2d::new();
                    r.gradients.clear();
                    self.log(line);
                    return Err("malformed list".into());
                }
            }
        }
        r.gradients = gradients;
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
        };
        let surface = r.surface.clone();
        if !out.lists.is_empty() {
            self.canvases.out.push(out);
        }
        for n in reply.notes {
            self.canvases
                .notes
                .push(format!("canvas {view} ({surface}): {n}"));
        }
        if let Some(line) = line {
            self.log(line);
        }
        for n in std::mem::take(&mut self.canvases.notes) {
            self.log(n);
        }
        Ok(())
    }

    /// The stamped lists since the last take, for the host to apply in order.
    pub fn take_canvas_lists(&mut self) -> Vec<CanvasList> {
        std::mem::take(&mut self.canvases.out)
    }

    /// Whether `view` is a 2D canvas this runner draws.
    pub fn is_canvas_2d(&self, view: ViewId) -> bool {
        self.canvases.records.contains_key(&view)
    }

    /// `state.canvas` (LLP 1056 §5): one record per 2D canvas.
    pub(crate) fn canvas_state(&self, s: &mut String) {
        s.push('[');
        for (i, (view, r)) in self.canvases.records.iter().enumerate() {
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

    /// `layout <node>`'s readable form of a 2D canvas's last list (LLP 1056
    /// §5): at most 200 lines and 16 KiB, development builds only.
    pub fn canvas_describe(&self, view: ViewId) -> Option<Vec<String>> {
        let r = self.canvases.records.get(&view)?;
        if r.last_list.is_empty() {
            return Some(Vec::new());
        }
        Some(list::describe(&r.last_list, 200, 16 * 1024))
    }
}
