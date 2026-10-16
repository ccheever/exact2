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
#[cfg(test)]
mod modal_tests;
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
    /// The index in `top_layer` of the frontmost modal presentation's
    /// first box, as its host reports it.
    modal: Option<usize>,
    placed: Vec<(NodeKey, [f64; 4], NodeKey)>,
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
        self.modal = None;
    }

    /// The presenter's front-to-back-independent stack order and viewport
    /// frames, from back to front. An open top-layer box is positioned by its
    /// presenter instead of its authored parent. `modal` is the index in
    /// `frames` of the frontmost modal presentation's first box
    /// (`modal_live`), none while no presentation is open.
    pub fn top_layer_frames(
        &mut self,
        kernel: &Kernel,
        frames: &[(ViewId, [f64; 4])],
        modal: Option<usize>,
    ) {
        let arena = kernel.arena();
        let entry = |(view, rect): &(ViewId, [f64; 4])| {
            arena
                .key_of(*view)
                .filter(|_| valid_rect(rect))
                .map(|key| (key, Some(*rect)))
        };
        self.top_layer = frames.iter().filter_map(entry).collect();
        // The boxes ignored before it are not counted
        self.modal = modal.map(|i| {
            frames[..i.min(frames.len())]
                .iter()
                .filter_map(entry)
                .count()
        });
    }

    /// Where the host shows the block of a subtree it lays out in a block of
    /// its own (LLP 1115.002 D8.2) without lifting it to the top layer: a
    /// stack's route, a tab panel, a split's pane (LLP 1123 D2). The
    /// rectangle is the block's, in the local coordinates of `within`, an
    /// ancestor of the root that holds the block in the host's hierarchy as
    /// in the document: its border box's, before its own transform, and
    /// after its scrolling, if the block is inside what it scrolls. That is
    /// the root's parent unless the host holds the block beside it, as
    /// UIKit's tab container holds a tab's route beside its panel; the
    /// root is then shown in `within` directly, as the host's hierarchy
    /// shows it, and the boxes between move nothing of it.
    /// `None` forgets it. The root keeps its place among its siblings, sits
    /// in the block by its own frame, and `within`'s and every ancestor's
    /// transform still apply above it. A non-finite or negative rectangle,
    /// or a `within` that names no live node, is ignored, and the placement
    /// it would replace kept.
    pub fn place(&mut self, kernel: &Kernel, view: ViewId, placement: Option<([f64; 4], ViewId)>) {
        let arena = kernel.arena();
        self.placed.retain(|(key, _, within)| {
            arena.resolve(*key).is_some() && arena.resolve(*within).is_some()
        });
        let Some(key) = arena.key_of(view) else {
            return;
        };
        let placement = match placement {
            Some((rect, within)) => {
                let Some(within) = arena.key_of(within).filter(|_| valid_rect(&rect)) else {
                    return;
                };
                Some((rect, within))
            }
            None => None,
        };
        self.placed.retain(|(k, ..)| *k != key);
        if let Some((rect, within)) = placement {
            self.placed.push((key, rect, within));
        }
    }

    /// A block root's placement: the box it is shown in, `within`, and the
    /// transform from its border box to that box's local coordinates
    /// (`place`). `None` for a node that is no block's root, was given no
    /// placement, or whose `within` is not above it.
    fn placement(&self, kernel: &Kernel, slot: u32, frame: Frame) -> Option<(u32, Affine)> {
        let arena = kernel.arena();
        let key = arena.key(slot);
        let &(_, rect, within) = self.placed.iter().find(|(k, ..)| *k == key)?;
        kernel.host_block(arena.local_id(slot))?;
        let within = arena.resolve(within)?;
        std::iter::successors(arena.parent(slot), |&p| arena.parent(p)).find(|&p| p == within)?;
        Some((within, shown_at(kernel, slot, frame, rect)))
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
    if let Some(frame) = kernel
        .arena()
        .slot_of(view)
        .and_then(|slot| native_frame(kernel, scrolled, slot))
    {
        return client_rect(kernel, scrolled, view, frame, false);
    }
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
    loop {
        m = mul(own_transform(arena.style(slot), at), m);
        match shown_in(kernel, scrolled, slot, at) {
            Shown::Viewport(to) => {
                m = mul(to, m);
                break;
            }
            Shown::Parent(parent, to) => {
                m = mul(to, m);
                slot = parent;
                at = arena.frame(parent);
            }
        }
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
    GeometryAnswer {
        x: x0,
        y: y0,
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

/// One step up a box's rendering ancestry (LLP 1123 D2): where `slot`,
/// laid out at `frame`, is shown, from its border box (its own transform
/// already applied) to the local coordinates of the box that shows it,
/// or to the viewport's when nothing above it moves it. An open top-layer
/// box (a dialog, a popover, a presented route) is where its host shows
/// it, in viewport points, and its logical ancestors neither move nor clip
/// it; a block's root its host places (a stack's route, a pane) is where
/// the host shows its block in the ancestor that holds it, its parent or
/// one above, the boxes between left out; any other box is at its frame in its
/// parent's, less the parent's scrolling, and a root at its frame less the
/// page's. Every reader of where a box is shown takes these steps:
/// `client_rect`, `elementFromPoint`'s walk, the intersection observer.
pub(crate) enum Shown {
    Parent(u32, Affine),
    Viewport(Affine),
}

pub(crate) fn shown_in(kernel: &Kernel, scrolled: &Scrolled, slot: u32, frame: Frame) -> Shown {
    let frame = native_frame(kernel, scrolled, slot).unwrap_or(frame);
    let arena = kernel.arena();
    let at = shift(f64::from(frame.x), f64::from(frame.y));
    if scrolled.is_open(arena.key(slot)) {
        let to = match scrolled.top_frame(arena.key(slot)) {
            Some(rect) => shown_at(kernel, slot, frame, rect),
            None => at,
        };
        return Shown::Viewport(to);
    }
    let parent = (!arena.is_root(slot)).then(|| arena.parent(slot)).flatten();
    let Some(parent) = parent else {
        let page = shift(-scrolled.page.0, -scrolled.page.1);
        return Shown::Viewport(mul(page, at));
    };
    if let Some((within, placed)) = scrolled.placement(kernel, slot, frame) {
        return Shown::Parent(within, placed);
    }
    let up = arena.frame(parent);
    let (left, top) = scrolled.offset(arena.key(parent));
    let to = shift(
        f64::from(frame.x) - f64::from(up.x) - left,
        f64::from(frame.y) - f64::from(up.y) - top,
    );
    Shown::Parent(parent, to)
}

/// Whether a host's rectangle is one: finite, of no negative size.
fn valid_rect(rect: &[f64; 4]) -> bool {
    rect.iter().all(|v| v.is_finite()) && rect[2] >= 0.0 && rect[3] >= 0.0
}

/// A block's rectangle in its root's border-box coordinates, when `slot`
/// is a block's root: the box its host shows the subtree in, which clips
/// it, as a sheet or a pane clips what it shows.
pub(crate) fn block_clip(kernel: &Kernel, slot: u32, frame: Frame) -> Option<[f64; 4]> {
    let (w, h) = kernel.host_block_size(kernel.arena().local_id(slot))?;
    let (x, y) = (f64::from(frame.x), f64::from(frame.y));
    Some([-x, -y, f64::from(w) - x, f64::from(h) - y])
}

/// A top-layer box replaced whole by its host has no kernel layout. Its
/// reported border box supplies its size; no geometry is inferred for the
/// authored children the native surface replaces.
fn native_frame(kernel: &Kernel, scrolled: &Scrolled, slot: u32) -> Option<Frame> {
    let arena = kernel.arena();
    if arena.cover(slot) != Some(exact_kernel::HostCover::Whole)
        || arena.flags(slot).has(NodeFlags::HIDDEN)
        || arena.flags(slot).has(NodeFlags::CREATED)
    {
        return None;
    }
    let rect = scrolled.top_frame(arena.key(slot))?;
    Some(Frame {
        width: rect[2] as f32,
        height: rect[3] as f32,
        ..arena.frame(slot)
    })
}

/// The transform from `slot`'s border box (`frame`) to the space `rect`
/// is in, where the host shows it. For a block's root, `rect` is the
/// block's: the root sits in it by its own frame, and the scale is the
/// rectangle's size over the block's used size, the host's own transform
/// (a sheet a detent scales). For any other box `rect` is its border box,
/// and the scale its size over the box's.
fn shown_at(kernel: &Kernel, slot: u32, frame: Frame, rect: [f64; 4]) -> Affine {
    let ratio = |to: f64, from: f32| {
        if from <= 0.0 {
            1.0
        } else {
            to / f64::from(from)
        }
    };
    let block = kernel.host_block_size(kernel.arena().local_id(slot));
    let (bw, bh, inner) = match block {
        Some((w, h)) => (w, h, shift(f64::from(frame.x), f64::from(frame.y))),
        None => (frame.width, frame.height, IDENTITY),
    };
    mul(
        shift(rect[0], rect[1]),
        mul(
            [ratio(rect[2], bw), 0.0, 0.0, ratio(rect[3], bh), 0.0, 0.0],
            inner,
        ),
    )
}

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
        let blocked =
            arena.style(slot).display == Display::None || arena.flags(slot).has(NodeFlags::HIDDEN);
        if blocked {
            continue;
        }
        if let (Some(id), Some(dataset)) = (props.str(PropId::Id), props.str(PropId::Dataset)) {
            if !id.is_empty()
                && crate::agent::field_str(dataset, word)
                    .is_some_and(|words| words.split_ascii_whitespace().any(|v| v == value))
                && props.bool(PropId::Inert) != Some(true)
                && inherited_hit_state(kernel, slot).is_some()
                && shown(kernel, scrolled, arena.local_id(slot))
            {
                out.push(id.to_owned());
            }
        }
        // An inert ancestor can contain a modal dialog that escapes it.
        stack.extend(arena.children(slot).iter().rev());
    }
    out
}

/// The live part of the top layer starts at the frontmost modal dialog or
/// the first box of the host's frontmost presentation (`top_layer_frames`):
/// whichever of its routes it shows, and what is open above it.
/// Everything else, below it in the top layer or in the document, is
/// behind it and inert to structural reads, as UIKit's dimming view and
/// the host's `inertBehind` make it to touch. None while no presentation
/// is open.
fn modal_live(kernel: &Kernel, scrolled: &Scrolled) -> Option<Vec<u32>> {
    let arena = kernel.arena();
    let dialog = scrolled.top_layer.iter().rposition(|&(key, _)| {
        arena
            .resolve(key)
            .is_some_and(|slot| modal_dialog(kernel, slot))
    });
    let from = scrolled.modal.into_iter().chain(dialog).max()?;
    Some(
        scrolled.top_layer[from.min(scrolled.top_layer.len())..]
            .iter()
            .filter_map(|&(key, _)| arena.resolve(key))
            .collect(),
    )
}

/// Whether `slot` is behind an open presentation (`modal_live`).
fn behind_modal(kernel: &Kernel, scrolled: &Scrolled, slot: u32) -> bool {
    let Some(live) = modal_live(kernel, scrolled) else {
        return false;
    };
    let arena = kernel.arena();
    !std::iter::successors(Some(slot), |&s| arena.parent(s)).any(|s| live.contains(&s))
}

fn kernel_shown(kernel: &Kernel, scrolled: &Scrolled, view: ViewId) -> bool {
    if crate::head::inactive(kernel, view) {
        return false;
    }
    let arena = kernel.arena();
    if arena
        .key_of(view)
        .is_some_and(|key| behind_modal(kernel, scrolled, key.index))
    {
        return false;
    }
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
    // Only the open presentation and what is open above it are read
    let live = modal_live(kernel, scrolled);
    for &(key, _) in scrolled.top_layer.iter().rev() {
        let Some(slot) = arena.resolve(key) else {
            continue;
        };
        if live.as_ref().is_some_and(|live| !live.contains(&slot)) {
            continue;
        }
        let Shown::Viewport(shown) = shown_in(kernel, scrolled, slot, arena.frame(slot)) else {
            continue;
        };
        let Some(inherited) = inherited_hit_state(kernel, slot) else {
            continue;
        };
        walk.hits(slot, shown, inherited, true, &mut Vec::new(), &mut hits);
        if first_only && !hits.is_empty() {
            return hits;
        }
    }
    if live.is_some() {
        return hits;
    }
    for &root in arena.roots().iter().rev() {
        let f = arena.frame(root);
        let placed = mul(page, shift(f64::from(f.x), f64::from(f.y)));
        walk.hits(
            root,
            placed,
            (PointerEvents::Auto, Visibility::Visible),
            false,
            &mut Vec::new(),
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
    let mut escaped = modal_dialog(kernel, slot);
    for p in std::iter::successors(arena.parent(slot), |&p| arena.parent(p)) {
        if !escaped && arena.props(p).bool(PropId::Inert) == Some(true) {
            return None;
        }
        escaped = escaped || modal_dialog(kernel, p);
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

fn modal_dialog(kernel: &Kernel, slot: u32) -> bool {
    let props = kernel.arena().props(slot);
    props.str(PropId::SemanticTag) == Some("dialog") && props.str(PropId::Popover).is_none()
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
    /// top left at the origin) to the viewport, and `above` the boxes the
    /// walk came through, each with the map from its local coordinates to
    /// the viewport, which a placed block's root shown in one of them
    /// (`shown_in`) starts from. Every clip above it holds the point, or the
    /// walk would not have come here.
    fn hits(
        &self,
        slot: u32,
        placed: Affine,
        inherited: (PointerEvents, Visibility),
        lifted: bool,
        above: &mut Vec<(u32, Affine)>,
        hits: &mut Vec<ViewId>,
    ) {
        let arena = self.kernel.arena();
        let style = arena.style(slot);
        let flags = arena.flags(slot);
        let props = arena.props(slot);
        let native = lifted
            .then(|| native_frame(self.kernel, self.scrolled, slot))
            .flatten();
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
        let f = native.unwrap_or_else(|| arena.frame(slot));
        // A block's root and its subtree are reached only inside the block
        // its host shows them in
        if let Some(b) = block_clip(self.kernel, slot, f) {
            let at = unmap(placed, self.at);
            if !at.is_some_and(|(x, y)| b[0] <= x && x < b[2] && b[1] <= y && y < b[3]) {
                return;
            }
        }
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
        if !clipped && native.is_none() {
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
            above.push((slot, m));
            for &(_, _, child) in children.iter().rev() {
                // Where the child is shown, in this box or the one above that
                // holds it; a top-layer child is tested in the top layer instead
                let Shown::Parent(holder, local) =
                    shown_in(self.kernel, self.scrolled, child, arena.frame(child))
                else {
                    continue;
                };
                let Some(&(_, from)) = above.iter().rev().find(|(s, _)| *s == holder) else {
                    continue;
                };
                let inherited = (pointer, visibility);
                self.hits(child, mul(from, local), inherited, false, above, hits);
                if self.first_only && !hits.is_empty() {
                    above.pop();
                    return;
                }
            }
            above.pop();
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
