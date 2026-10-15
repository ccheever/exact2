//! Geometry and structural reads from actions, answered by the engine that
//! commits.
//!
//! @ref LLP 1051.000 D1 (what each read means), D2 (actions only; `measure`
//! answered before the action runs), D3 (the kernel natively), D4 (the page
//! on the web, through the artifact's link), D5 (provisional, unavailable)
//!
//! Both answer the box where the viewer sees it, as `getBoundingClientRect`
//! does: through every transform above it, with every scroll offset above
//! it applied, the page's included. The kernel lays out free of both, so
//! natively the read composes the transform rows, and the host tells the
//! runner where each scroller it presents stands ([`Scrolled`]) for the read
//! to subtract; the page's answer already has them.
//!
//! `elementFromPoint(x, y)` and `elementsFromPoint(x, y)` (LLP 1094 D10)
//! test the same boxes front to back: the former names the first hit by its
//! nearest `id`, the latter lists each hit's own `id`. Natively both use the
//! kernel's boxes, transform rows and [`Scrolled`]; the web uses its hit test.
//! `querySelectorAll` (LLP 1075.003 §9.1) lists nodes by a declared data word.
//!
//! An action runs against a read-only runner, so `frame` reads through a
//! borrowed kernel while the body runs, and every `measure` in the body is
//! answered just before, with the kernel's engine tree to lay out. The
//! compiler requires `measure`'s id to be a literal, which is what lets the
//! runner find them all first.

#[cfg(test)]
mod inline_tests;
mod intersection;
pub(crate) use intersection::intersection;

use exact_kernel::{
    Display, Frame, Kernel, NodeFlags, NodeKey, Overflow, PointerEvents, PropId, StyleId, ViewId,
    Visibility,
};
use exact_plan::{Opcode, Plan, Stdlib, StrId, Value};

/// One answer: a border box and its two flags (LLP 1051.000 D5).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct GeometryAnswer {
    /// The box's left edge in its root's space.
    pub x: f64,
    /// The box's top edge in its root's space.
    pub y: f64,
    /// The border box's width.
    pub width: f64,
    /// The border box's height.
    pub height: f64,
    /// An input the answer depends on has not settled (an image without its
    /// natural size, a paragraph whose height is an estimate).
    pub provisional: bool,
    /// The host could not answer: every number is zero.
    pub unavailable: bool,
}

impl GeometryAnswer {
    /// No answer: zeros, and `unavailable`.
    pub const UNAVAILABLE: GeometryAnswer = GeometryAnswer {
        x: 0.0,
        y: 0.0,
        width: 0.0,
        height: 0.0,
        provisional: false,
        unavailable: true,
    };

    /// A kernel frame as an answer, moved by `(left, top)` of scrolling.
    pub fn of(frame: Frame, provisional: bool, (left, top): (f64, f64)) -> GeometryAnswer {
        GeometryAnswer {
            x: f64::from(frame.x) - left,
            y: f64::from(frame.y) - top,
            width: f64::from(frame.width),
            height: f64::from(frame.height),
            provisional,
            unavailable: false,
        }
    }

    /// The `Geometry` record, its fields in the compiler's order.
    pub fn value(self) -> Value {
        Value::record(vec![
            Value::Number(self.x),
            Value::Number(self.y),
            Value::Number(self.width),
            Value::Number(self.height),
            Value::Bool(self.provisional),
            Value::Bool(self.unavailable),
        ])
    }
}

/// The `measure` answers found before an action's body runs, by the string
/// id of the literal each was asked with.
pub type Measured = Vec<(StrId, GeometryAnswer)>;

/// A native presenter's shaped inline fragments, read only when an action
/// asks for a run. The answer is their union in viewport coordinates.
pub trait InlineGeometry: std::fmt::Debug {
    /// The last presented run, or `UNAVAILABLE` when it is not mounted.
    fn frame(&self, view: ViewId) -> GeometryAnswer;
}

/// Where the host last showed each scroll container, and the page, in CSS
/// px (`scrollLeft`, `scrollTop`): what a native read subtracts from the
/// kernel's scroll-free box (kanban diary F4: an app hand-tracked every
/// column's offset to drop a card where the pointer was). A host notes an
/// offset whenever one changes, handler or not; a read sees the offsets as
/// last shown, as it sees the layout (LLP 1051.000 D1).
#[derive(Debug, Default, Clone)]
pub struct Scrolled {
    page: (f64, f64),
    boxes: Vec<(NodeKey, f64, f64)>,
    top_layer: Vec<(NodeKey, Option<[f64; 4]>)>,
    pub(crate) inline: Option<std::rc::Rc<dyn InlineGeometry>>,
}

impl Scrolled {
    /// `view`'s offset, or the page's for `None`. An id that names no live
    /// node, or a non-finite offset, is ignored; a node's entry goes with
    /// it, since its key is generation-checked.
    pub fn note(&mut self, kernel: &Kernel, view: Option<ViewId>, left: f64, top: f64) {
        if !(left.is_finite() && top.is_finite()) {
            return;
        }
        let Some(view) = view else {
            self.page = (left, top);
            return;
        };
        let arena = kernel.arena();
        self.boxes.retain(|(key, ..)| arena.resolve(*key).is_some());
        let Some(key) = arena.key_of(view) else {
            return;
        };
        match self.boxes.iter_mut().find(|(k, ..)| *k == key) {
            Some(entry) => *entry = (key, left, top),
            None => self.boxes.push((key, left, top)),
        }
    }

    /// One scroller's offset, zero for a node with none noted.
    fn offset(&self, key: NodeKey) -> (f64, f64) {
        self.boxes
            .iter()
            .find(|(k, ..)| *k == key)
            .map_or((0.0, 0.0), |(_, l, t)| (*l, *t))
    }

    /// The top-layer nodes the native presenter currently shows.
    pub fn top_layer(&mut self, kernel: &Kernel, views: &[ViewId]) {
        let arena = kernel.arena();
        self.top_layer = views
            .iter()
            .filter_map(|view| arena.key_of(*view).map(|key| (key, None)))
            .collect();
    }

    /// The presenter's front-to-back-independent stack order and viewport
    /// frames, from back to front. An open top-layer box is positioned by its
    /// presenter instead of its authored parent.
    pub fn top_layer_frames(&mut self, kernel: &Kernel, frames: &[(ViewId, [f64; 4])]) {
        let arena = kernel.arena();
        self.top_layer = frames
            .iter()
            .filter_map(|(view, rect)| {
                if rect.iter().all(|v| v.is_finite()) && rect[2] >= 0.0 && rect[3] >= 0.0 {
                    arena.key_of(*view).map(|key| (key, Some(*rect)))
                } else {
                    None
                }
            })
            .collect();
    }

    fn is_open(&self, key: NodeKey) -> bool {
        self.top_layer.iter().any(|(open, _)| *open == key)
    }

    fn top_frame(&self, key: NodeKey) -> Option<[f64; 4]> {
        self.top_layer
            .iter()
            .find(|(open, _)| *open == key)
            .and_then(|(_, rect)| *rect)
    }

    /// The scrolling above `view`: the page's and each ancestor
    /// scroller's, summed. A scroller's own offset moves its content, not
    /// its box.
    pub fn above(&self, kernel: &Kernel, view: ViewId) -> (f64, f64) {
        let arena = kernel.arena();
        let (mut left, mut top) = self.page;
        let mut at = arena.key_of(view).and_then(|key| arena.parent(key.index));
        while let Some(slot) = at {
            let key = arena.key(slot);
            if let Some((_, l, t)) = self.boxes.iter().find(|(k, ..)| *k == key) {
                left += l;
                top += t;
            }
            at = arena.parent(slot);
        }
        (left, top)
    }
}

/// How a runner answers geometry: `frame` from a borrowed kernel while an
/// action runs, `measure` from the kernel just before it (natively it lays
/// out the engine tree). The web's pair asks the page instead and never
/// reads the kernel, whose layout there is not the page's.
///
/// A host hands its reads to [`GeometryLinks::new`], and what every host
/// shares (naming a view by its id, finding an action's `measure`s, building
/// the record) rides along in the same value, so an artifact that links no
/// geometry carries none of it (LLP 1047 D3).
#[derive(Clone, Copy)]
pub struct GeometryLinks {
    frame: fn(&Kernel, &Scrolled, ViewId) -> GeometryAnswer,
    measure: fn(&mut Kernel, &Scrolled, ViewId) -> GeometryAnswer,
    point: fn(&Kernel, &Scrolled, f64, f64) -> Option<ViewId>,
    points: fn(&Kernel, &Scrolled, f64, f64) -> Vec<ViewId>,
    shown: fn(&Kernel, &Scrolled, ViewId) -> bool,
    ahead: fn(&Plan, &[u8], &mut Kernel, &Scrolled, &GeometryLinks) -> Measured,
    read: fn(&GeometryEnv<'_>, &Plan, Stdlib, &[Value]) -> Option<Value>,
}

impl GeometryLinks {
    /// A host's reads: `frame`, where layout put a view as last laid out,
    /// `measure`, the view's box as if its `height` were `auto`, and
    /// `point`, the front-most view whose box holds a viewport point, and
    /// `points`, every such view in paint order.
    pub const fn new(
        frame: fn(&Kernel, &Scrolled, ViewId) -> GeometryAnswer,
        measure: fn(&mut Kernel, &Scrolled, ViewId) -> GeometryAnswer,
        point: fn(&Kernel, &Scrolled, f64, f64) -> Option<ViewId>,
        points: fn(&Kernel, &Scrolled, f64, f64) -> Vec<ViewId>,
        shown: fn(&Kernel, &Scrolled, ViewId) -> bool,
    ) -> GeometryLinks {
        GeometryLinks {
            frame,
            measure,
            point,
            points,
            shown,
            ahead: measure_ahead,
            read,
        }
    }

    /// Every `measure` in an action's `code`, answered before it runs, by
    /// its literal id.
    pub fn ahead(
        &self,
        plan: &Plan,
        code: &[u8],
        kernel: &mut Kernel,
        scrolled: &Scrolled,
    ) -> Measured {
        (self.ahead)(plan, code, kernel, scrolled, self)
    }
}

/// The kernel's answers: a native host's kernel is its layout, and its
/// presenters' scroll offsets are noted in [`Scrolled`].
pub static KERNEL: GeometryLinks = GeometryLinks::new(
    kernel_frame,
    kernel_measure,
    kernel_point,
    kernel_points,
    kernel_shown,
);

fn kernel_frame(kernel: &Kernel, scrolled: &Scrolled, view: ViewId) -> GeometryAnswer {
    let arena = kernel.arena();
    if let Some(key) = arena
        .key_of(view)
        .filter(|key| arena.is_inline_run(key.index))
    {
        let mut owner = key.index;
        while arena.is_inline_run(owner) {
            let Some(parent) = arena.parent(owner) else {
                return GeometryAnswer::UNAVAILABLE;
            };
            owner = parent;
        }
        let Some((_, provisional)) = kernel.laid_out_frame(arena.key(owner)) else {
            return GeometryAnswer::UNAVAILABLE;
        };
        let mut answer = scrolled
            .inline
            .as_ref()
            .map_or(GeometryAnswer::UNAVAILABLE, |read| read.frame(view));
        if answer.unavailable
            || ![answer.x, answer.y, answer.width, answer.height]
                .iter()
                .all(|v| v.is_finite())
            || answer.width < 0.0
            || answer.height < 0.0
        {
            return GeometryAnswer::UNAVAILABLE;
        }
        answer.provisional |= provisional;
        return answer;
    }
    kernel
        .arena()
        .key_of(view)
        .and_then(|key| kernel.laid_out_frame(key))
        .map_or(GeometryAnswer::UNAVAILABLE, |(frame, provisional)| {
            client_rect(kernel, scrolled, view, frame, provisional)
        })
}

fn kernel_measure(kernel: &mut Kernel, scrolled: &Scrolled, view: ViewId) -> GeometryAnswer {
    let answer = kernel
        .arena()
        .key_of(view)
        .and_then(|key| kernel.measure_auto_height(key));
    answer.map_or(GeometryAnswer::UNAVAILABLE, |(frame, provisional)| {
        // At the origin `frame` answers, with the size as if `auto`.
        let placed = kernel_frame(kernel, scrolled, view);
        GeometryAnswer {
            width: f64::from(frame.width),
            height: f64::from(frame.height),
            provisional,
            ..placed
        }
    })
}

/// Where the viewer sees `view`'s laid-out border box, as
/// `getBoundingClientRect` places it: the box through its own transform and
/// every ancestor's (CSS's individual `translate`, its percentages of the
/// box included, `rotate` and `scale` about `transform-origin`), less every
/// scroll offset above it, the page's included; the bounding box of its
/// four corners (LLP 1051.000 D1, changed 2026-10-04: kanban2's drop aimed
/// at a translated card's laid-out place). The rows as committed: a
/// transition in flight counts at its end, which a presenter, not the
/// kernel, knows; a 3D rotation or a z translation counts by its 2D parts.
fn client_rect(
    kernel: &Kernel,
    scrolled: &Scrolled,
    view: ViewId,
    frame: Frame,
    provisional: bool,
) -> GeometryAnswer {
    let arena = kernel.arena();
    let Some(key) = arena.key_of(view) else {
        return GeometryAnswer::UNAVAILABLE;
    };
    // In f64, from the kernel's f32 boxes: the offsets telescope exactly,
    // so an untransformed box answers its frame less the scrolling, bit for
    // bit.
    let mut m = IDENTITY;
    let mut slot = key.index;
    let mut at = frame;
    let mut lifted = false;
    loop {
        m = mul(own_transform(arena.style(slot), at), m);
        if scrolled.is_open(arena.key(slot)) {
            let rect = scrolled.top_frame(arena.key(slot)).unwrap_or([
                f64::from(at.x),
                f64::from(at.y),
                f64::from(at.width),
                f64::from(at.height),
            ]);
            let sx = if at.width == 0.0 {
                1.0
            } else {
                rect[2] / f64::from(at.width)
            };
            let sy = if at.height == 0.0 {
                1.0
            } else {
                rect[3] / f64::from(at.height)
            };
            m = mul(
                shift(rect[0], rect[1]),
                mul([sx, 0.0, 0.0, sy, 0.0, 0.0], m),
            );
            lifted = true;
            break;
        }
        let parent = (!arena.is_root(slot)).then(|| arena.parent(slot)).flatten();
        let Some(parent) = parent else {
            m = mul(shift(f64::from(at.x), f64::from(at.y)), m);
            break;
        };
        let up = arena.frame(parent);
        let (left, top) = scrolled.offset(arena.key(parent));
        let dx = f64::from(at.x) - f64::from(up.x) - left;
        let dy = f64::from(at.y) - f64::from(up.y) - top;
        m = mul(shift(dx, dy), m);
        slot = parent;
        at = up;
    }
    let (w, h) = (f64::from(frame.width), f64::from(frame.height));
    let corners = [(0.0, 0.0), (w, 0.0), (0.0, h), (w, h)]
        .map(|(x, y)| (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]));
    let (x0, y0, x1, y1) = corners.iter().fold(
        (
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        ),
        |(a, b, c, d), (x, y)| (a.min(*x), b.min(*y), c.max(*x), d.max(*y)),
    );
    if !(x0.is_finite() && y0.is_finite() && x1.is_finite() && y1.is_finite()) {
        return GeometryAnswer::UNAVAILABLE;
    }
    let (left, top) = if lifted { (0.0, 0.0) } else { scrolled.page };
    GeometryAnswer {
        x: x0 - left,
        y: y0 - top,
        width: x1 - x0,
        height: y1 - y0,
        provisional,
        unavailable: false,
    }
}

/// A box's own transform in its border box's space, `at` its frame: CSS's
/// individual `translate` (its percentages of the box included), `rotate`
/// and `scale` about `transform-origin`, as
/// `T(origin + translate) · R · S · T(-origin)`. A 3D rotation counts by its
/// 2D part, none.
fn own_transform(s: &exact_kernel::StyleProps, at: Frame) -> Affine {
    let (w, h) = (f64::from(at.width), f64::from(at.height));
    let (ox, oy) = s.transform_origin.resolve(at.width, at.height);
    let (ox, oy) = (f64::from(ox), f64::from(oy));
    let turn = if s.rotate_axis.is_3d() { 0.0 } else { s.rotate };
    let k = f64::from(s.scale);
    let (sin, cos) = f64::from(turn).to_radians().sin_cos();
    let (tx, ty) = (
        f64::from(s.translate.x) + f64::from(s.translate_percent.x) / 100.0 * w,
        f64::from(s.translate.y) + f64::from(s.translate_percent.y) / 100.0 * h,
    );
    let (a, b, c, d) = (cos * k, sin * k, -sin * k, cos * k);
    [
        a,
        b,
        c,
        d,
        ox + tx - (a * ox + c * oy),
        oy + ty - (b * ox + d * oy),
    ]
}

/// A 2D affine `[a, b, c, d, e, f]`: `x' = a·x + c·y + e`, `y' = b·x + d·y + f`.
type Affine = [f64; 6];

const IDENTITY: Affine = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

fn shift(x: f64, y: f64) -> Affine {
    [1.0, 0.0, 0.0, 1.0, x, y]
}

/// Where viewport `(x, y)` falls in the space `m` maps to the viewport;
/// `None` when `m` flattens it (a `scale` of 0).
fn unmap(m: Affine, (x, y): (f64, f64)) -> Option<(f64, f64)> {
    let det = m[0] * m[3] - m[1] * m[2];
    if det == 0.0 || !det.is_finite() {
        return None;
    }
    let (x, y) = (x - m[4], y - m[5]);
    Some(((m[3] * x - m[2] * y) / det, (m[0] * y - m[1] * x) / det))
}

/// `p · q`: `q` first.
fn mul(p: Affine, q: Affine) -> Affine {
    [
        p[0] * q[0] + p[2] * q[1],
        p[1] * q[0] + p[3] * q[1],
        p[0] * q[2] + p[2] * q[3],
        p[1] * q[2] + p[3] * q[3],
        p[0] * q[4] + p[2] * q[5] + p[4],
        p[1] * q[4] + p[3] * q[5] + p[5],
    ]
}

/// What an action's body reads geometry through (the VM's `Env`).
pub struct GeometryEnv<'a> {
    kernel: &'a Kernel,
    scrolled: &'a Scrolled,
    links: &'a GeometryLinks,
    measured: &'a [(StrId, GeometryAnswer)],
}

impl<'a> GeometryEnv<'a> {
    /// Reads for one action: `frame` through `links`, and the `measure`
    /// answers found before its body ran.
    pub fn new(
        kernel: &'a Kernel,
        scrolled: &'a Scrolled,
        links: &'a GeometryLinks,
        measured: &'a [(StrId, GeometryAnswer)],
    ) -> GeometryEnv<'a> {
        GeometryEnv {
            kernel,
            scrolled,
            links,
            measured,
        }
    }

    /// `frame(id)` or `measure(id)` (`f`, `args`, in `plan`), as the
    /// `Geometry` record.
    pub fn read(&self, plan: &Plan, f: Stdlib, args: &[Value]) -> Option<Value> {
        (self.links.read)(self, plan, f, args)
    }
}

/// A read's value: `measure` from answers found before the body ran,
/// `frame` from the host now, or a point/data-word structural result.
fn read(env: &GeometryEnv<'_>, plan: &Plan, f: Stdlib, args: &[Value]) -> Option<Value> {
    if f == Stdlib::ElementFromPoint {
        let (x, y) = (args.first()?.as_number()?, args.get(1)?.as_number()?);
        let hit = (x.is_finite() && y.is_finite())
            .then(|| (env.links.point)(env.kernel, env.scrolled, x, y))
            .flatten();
        return Some(
            hit.and_then(|view| named(env.kernel, view))
                .map_or(Value::NONE, |id| Value::some(Value::str(&id))),
        );
    }
    if f == Stdlib::ElementsFromPoint {
        let (x, y) = (args.first()?.as_number()?, args.get(1)?.as_number()?);
        let hits = if x.is_finite() && y.is_finite() {
            (env.links.points)(env.kernel, env.scrolled, x, y)
        } else {
            Vec::new()
        };
        let hit_views: std::collections::HashSet<ViewId> = hits.iter().copied().collect();
        let mut seen = std::collections::HashSet::new();
        return Some(Value::list(
            hits.into_iter()
                .filter_map(|view| {
                    if own_id(env.kernel, view).is_some() {
                        return Some(view);
                    }
                    let ancestor = named_view(env.kernel, view)?;
                    let box_at = (env.links.frame)(env.kernel, env.scrolled, ancestor);
                    (!hit_views.contains(&ancestor)
                        && !box_at.unavailable
                        && (x < box_at.x
                            || x >= box_at.x + box_at.width
                            || y < box_at.y
                            || y >= box_at.y + box_at.height))
                        .then_some(ancestor)
                })
                .filter(|view| seen.insert(*view))
                .filter_map(|view| own_id(env.kernel, view))
                .map(|id| Value::str(&id))
                .collect(),
        ));
    }
    if f == Stdlib::QuerySelectorAll {
        let selector = args.first()?.as_str()?;
        let hits = data_selector(selector).map_or_else(Vec::new, |(word, value)| {
            data_matches(env.kernel, env.scrolled, env.links.shown, word, value)
        });
        return Some(Value::list(
            hits.into_iter().map(|id| Value::str(&id)).collect(),
        ));
    }
    let id = args.first()?.as_str()?;
    let answer = if f == Stdlib::Measure {
        env.measured
            .iter()
            .find(|(named, _)| plan.str(*named) == id)
            .map_or(GeometryAnswer::UNAVAILABLE, |(_, answer)| *answer)
    } else {
        view_of(env.kernel, id).map_or(GeometryAnswer::UNAVAILABLE, |view| {
            (env.links.frame)(env.kernel, env.scrolled, view)
        })
    };
    Some(answer.value())
}

/// The `id` of `view` or of its nearest ancestor with one: DOM's
/// `closest("[id]")?.id`. An empty `id` names nothing, as in HTML.
fn named(kernel: &Kernel, view: ViewId) -> Option<String> {
    named_view(kernel, view).and_then(|view| own_id(kernel, view))
}

fn named_view(kernel: &Kernel, view: ViewId) -> Option<ViewId> {
    let arena = kernel.arena();
    let mut at = arena.key_of(view).map(|key| key.index);
    while let Some(slot) = at {
        if arena
            .props(slot)
            .str(PropId::Id)
            .is_some_and(|id| !id.is_empty())
        {
            return Some(arena.local_id(slot));
        }
        at = arena.parent(slot);
    }
    None
}

fn own_id(kernel: &Kernel, view: ViewId) -> Option<String> {
    let arena = kernel.arena();
    arena
        .key_of(view)
        .and_then(|key| arena.props(key.index).str(PropId::Id))
        .filter(|id| !id.is_empty())
        .map(str::to_owned)
}

fn data_selector(selector: &str) -> Option<(&str, &str)> {
    let rest = selector.strip_prefix("[data-")?;
    let (word, value) = rest.split_once("~=\"")?;
    let value = value.strip_suffix("\"]")?;
    let valid = !word.is_empty()
        && word.split('-').all(|part| {
            let mut chars = part.chars();
            chars.next().is_some_and(|c| c.is_ascii_lowercase())
                && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        });
    (valid && !value.is_empty() && !value.chars().any(|c| matches!(c, '"' | '\\' | '\n' | '\r')))
        .then_some((word, value))
}

fn data_matches(
    kernel: &Kernel,
    scrolled: &Scrolled,
    shown: fn(&Kernel, &Scrolled, ViewId) -> bool,
    word: &str,
    value: &str,
) -> Vec<String> {
    let arena = kernel.arena();
    let mut stack: Vec<u32> = arena.roots().iter().rev().copied().collect();
    let mut out = Vec::new();
    while let Some(slot) = stack.pop() {
        let props = arena.props(slot);
        let blocked = arena.style(slot).display == Display::None
            || arena.flags(slot).has(NodeFlags::HIDDEN)
            || props.bool(PropId::Inert) == Some(true);
        if blocked {
            continue;
        }
        if let (Some(id), Some(dataset)) = (props.str(PropId::Id), props.str(PropId::Dataset)) {
            if !id.is_empty()
                && crate::agent::field_str(dataset, word)
                    .is_some_and(|words| words.split_ascii_whitespace().any(|v| v == value))
                && shown(kernel, scrolled, arena.local_id(slot))
            {
                out.push(id.to_owned());
            }
        }
        stack.extend(arena.children(slot).iter().rev());
    }
    out
}

fn kernel_shown(kernel: &Kernel, scrolled: &Scrolled, view: ViewId) -> bool {
    if crate::head::inactive(kernel, view) {
        return false;
    }
    let arena = kernel.arena();
    let mut at = arena.key_of(view).map(|key| key.index);
    while let Some(slot) = at {
        let props = arena.props(slot);
        if (props.str(PropId::Popover).is_some()
            || props.str(PropId::SemanticTag) == Some("dialog"))
            && !scrolled.is_open(arena.key(slot))
        {
            return false;
        }
        at = arena.parent(slot);
    }
    true
}

/// The front-most laid-out box at viewport `(x, y)` (LLP 1094 D10): every
/// root from the last, each node's children front to back by LLP 1083.000's
/// rank and then tree order, a child before its parent. Boxes are `frame`'s,
/// through every transform (LLP 1051.000 D1, changed 2026-10-04), each
/// tested in its own space, so a turned box answers its own shape, not its
/// bounding box, as DOM's hit test does; a scroller or a box that clips cuts
/// its descendants to its padding box; `pointer-events: none` and
/// `visibility` other than `visible` (both inherited) are passed over, and
/// `display: none` takes its subtree.
fn kernel_point(kernel: &Kernel, scrolled: &Scrolled, x: f64, y: f64) -> Option<ViewId> {
    kernel_hits(kernel, scrolled, x, y, true).into_iter().next()
}

fn kernel_points(kernel: &Kernel, scrolled: &Scrolled, x: f64, y: f64) -> Vec<ViewId> {
    kernel_hits(kernel, scrolled, x, y, false)
}

fn kernel_hits(
    kernel: &Kernel,
    scrolled: &Scrolled,
    x: f64,
    y: f64,
    first_only: bool,
) -> Vec<ViewId> {
    let ranks: std::collections::HashMap<ViewId, i64> = kernel
        .paint_order()
        .into_iter()
        .map(|(view, placed)| (view, placed.rank))
        .collect();
    let walk = Point {
        kernel,
        scrolled,
        ranks,
        at: (x, y),
        first_only,
    };
    let arena = kernel.arena();
    let page = shift(-scrolled.page.0, -scrolled.page.1);
    let mut hits = Vec::new();
    for &(key, rect) in scrolled.top_layer.iter().rev() {
        let Some(slot) = arena.resolve(key) else {
            continue;
        };
        let f = arena.frame(slot);
        let rect = rect.unwrap_or([
            f64::from(f.x),
            f64::from(f.y),
            f64::from(f.width),
            f64::from(f.height),
        ]);
        let sx = if f.width == 0.0 {
            1.0
        } else {
            rect[2] / f64::from(f.width)
        };
        let sy = if f.height == 0.0 {
            1.0
        } else {
            rect[3] / f64::from(f.height)
        };
        let Some(inherited) = inherited_hit_state(kernel, slot) else {
            continue;
        };
        walk.hits(
            slot,
            mul(shift(rect[0], rect[1]), [sx, 0.0, 0.0, sy, 0.0, 0.0]),
            inherited,
            true,
            &mut hits,
        );
        if first_only && !hits.is_empty() {
            return hits;
        }
    }
    for &root in arena.roots().iter().rev() {
        let f = arena.frame(root);
        let placed = mul(page, shift(f64::from(f.x), f64::from(f.y)));
        walk.hits(
            root,
            placed,
            (PointerEvents::Auto, Visibility::Visible),
            false,
            &mut hits,
        );
        if first_only && !hits.is_empty() {
            break;
        }
    }
    hits
}

/// What an open top-layer box inherits for hit testing from its ancestors,
/// whose clips and transforms it escapes but not their `inert` (HTML's inert
/// subtrees), `pointer-events` or `visibility` (CSS inheritance follows the
/// tree, not the top layer): none when an ancestor is inert, else the
/// nearest ancestor's value of each, `auto` and `visible` above them all. A
/// modal `dialog` escapes the inertness of what is above it, for itself and
/// what opens inside it (HTML's modal dialogs); its own `inert` holds. Every
/// `dialog` that is not a popover counts as modal without asking whether it
/// is open: Exact opens a dialog only modally (`showModal`, `command="show-modal"`;
/// `open` is refused), and the host stays the source of truth for what is
/// open, which iOS does not report for a dialog's sheet. A popover open in a
/// closed dialog so escapes too, though the dialog's subtree is hidden.
fn inherited_hit_state(kernel: &Kernel, slot: u32) -> Option<(PointerEvents, Visibility)> {
    let arena = kernel.arena();
    let (mut pointer, mut visibility) = (None, None);
    let dialog = |s: u32| {
        let props = arena.props(s);
        props.str(PropId::SemanticTag) == Some("dialog") && props.str(PropId::Popover).is_none()
    };
    let mut escaped = dialog(slot);
    for p in std::iter::successors(arena.parent(slot), |&p| arena.parent(p)) {
        if !escaped && arena.props(p).bool(PropId::Inert) == Some(true) {
            return None;
        }
        escaped = escaped || dialog(p);
        let style = arena.style(p);
        if pointer.is_none() && style.mask.has(StyleId::PointerEvents) {
            pointer = Some(style.pointer_events);
        }
        if visibility.is_none() && style.mask.has(StyleId::Visibility) {
            visibility = Some(style.visibility);
        }
    }
    Some((
        pointer.unwrap_or(PointerEvents::Auto),
        visibility.unwrap_or(Visibility::Visible),
    ))
}

struct Point<'a> {
    kernel: &'a Kernel,
    scrolled: &'a Scrolled,
    ranks: std::collections::HashMap<ViewId, i64>,
    at: (f64, f64),
    first_only: bool,
}

impl Point<'_> {
    /// `slot`'s hit, `placed` mapping its untransformed border box (its
    /// top left at the origin) to the viewport. Every clip above it holds
    /// the point, or the walk would not have come here.
    fn hits(
        &self,
        slot: u32,
        placed: Affine,
        inherited: (PointerEvents, Visibility),
        lifted: bool,
        hits: &mut Vec<ViewId>,
    ) {
        let arena = self.kernel.arena();
        let style = arena.style(slot);
        let flags = arena.flags(slot);
        let props = arena.props(slot);
        if style.display == Display::None
            || flags.has(NodeFlags::HIDDEN)
            || flags.has(NodeFlags::CREATED)
            || props.bool(PropId::Inert) == Some(true)
            || (self.scrolled.is_open(arena.key(slot)) && !lifted)
            || ((props.str(PropId::Popover).is_some()
                || props.str(PropId::SemanticTag) == Some("dialog"))
                && !self.scrolled.is_open(arena.key(slot)))
            || ((props.str(PropId::NavigationKey).is_some()
                || props.str(PropId::AccessibilityRole) == Some("tabpanel"))
                && crate::head::inactive(self.kernel, arena.local_id(slot)))
        {
            return;
        }
        let pointer = if style.mask.has(StyleId::PointerEvents) {
            style.pointer_events
        } else {
            inherited.0
        };
        let visibility = if style.mask.has(StyleId::Visibility) {
            style.visibility
        } else {
            inherited.1
        };
        let f = arena.frame(slot);
        let m = mul(placed, own_transform(style, f));
        let local = unmap(m, self.at);
        let (width, height) = (f64::from(f.width), f64::from(f.height));
        let inside = |b: [f64; 4]| {
            local.is_some_and(|(x, y)| b[0] <= x && x < b[2] && b[1] <= y && y < b[3])
        };
        let clips = style.overflow_x != Overflow::Visible || style.overflow_y != Overflow::Visible;
        let clipped = clips
            && !inside([
                f64::from(style.border_width_left),
                f64::from(style.border_width_top),
                width - f64::from(style.border_width_right),
                height - f64::from(style.border_width_bottom),
            ]);
        if !clipped {
            let (left, top) = self.scrolled.offset(arena.key(slot));
            let mut children: Vec<(i64, usize, u32)> = arena
                .children(slot)
                .iter()
                .enumerate()
                .map(|(i, &c)| {
                    let rank = self.ranks.get(&arena.local_id(c)).copied().unwrap_or(0);
                    (rank, i, c)
                })
                .collect();
            children.sort_unstable();
            for &(_, _, child) in children.iter().rev() {
                let cf = arena.frame(child);
                let dx = f64::from(cf.x) - f64::from(f.x) - left;
                let dy = f64::from(cf.y) - f64::from(f.y) - top;
                self.hits(
                    child,
                    mul(m, shift(dx, dy)),
                    (pointer, visibility),
                    false,
                    hits,
                );
                if self.first_only && !hits.is_empty() {
                    return;
                }
            }
        }
        let hit = inside([0.0, 0.0, width, height])
            && pointer != PointerEvents::None
            && visibility == Visibility::Visible
            && !arena.is_inline_run(slot);
        if hit {
            hits.push(arena.local_id(slot));
        }
    }
}

/// The view an id names: the first live node whose `id` prop it is, as
/// `showPicker` and `scrollIntoView` resolve names.
fn view_of(kernel: &Kernel, id: &str) -> Option<ViewId> {
    kernel
        .find_by_id(id)
        .first()
        .map(|key| kernel.arena().local_id(key.index))
}

/// Answer every `measure` in `code` before it runs: each literal id once,
/// in the order the body names them. The compiler writes a literal id as the
/// `Str` push just before the call.
fn measure_ahead(
    plan: &Plan,
    code: &[u8],
    kernel: &mut Kernel,
    scrolled: &Scrolled,
    links: &GeometryLinks,
) -> Measured {
    let mut answers: Measured = Vec::new();
    let mut literal = None;
    for instruction in crate::vm::instructions(code).map_while(Result::ok) {
        match instruction.op {
            Opcode::Str => literal = Some(instruction.args[0] as u32),
            Opcode::Call
                if Stdlib::from_wire(instruction.args[0] as u8) == Some(Stdlib::Measure) =>
            {
                if let Some(at) = literal.take().map(StrId) {
                    if !answers.iter().any(|(named, _)| *named == at) {
                        let answer = view_of(kernel, plan.str(at))
                            .map_or(GeometryAnswer::UNAVAILABLE, |view| {
                                (links.measure)(kernel, scrolled, view)
                            });
                        answers.push((at, answer));
                    }
                }
            }
            _ => literal = None,
        }
    }
    answers
}
