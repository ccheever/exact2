//! Geometry reads from actions: `frame(id)` and `measure(id)`, each
//! answered by the engine that commits.
//!
//! @ref LLP 1051.000 D1 (what each read means), D2 (actions only; `measure`
//! answered before the action runs), D3 (the kernel natively), D4 (the page
//! on the web, through the artifact's link), D5 (provisional, unavailable)
//!
//! Both answer the box where the viewer sees it, as `getBoundingClientRect`
//! does: every scroll offset above it applied, the page's included. The
//! kernel lays out free of scrolling, so natively the host tells the runner
//! where each scroller it presents stands ([`Scrolled`]) and the read
//! subtracts them; the page's answer already has them.
//!
//! `elementFromPoint(x, y)` (LLP 1094 D10) tests the same boxes front to
//! back and names the hit by its nearest `id`: natively from the kernel's
//! boxes and [`Scrolled`], on the web from the page's.
//!
//! An action runs against a read-only runner, so `frame` reads through a
//! borrowed kernel while the body runs, and every `measure` in the body is
//! answered just before, with the kernel's engine tree to lay out. The
//! compiler requires `measure`'s id to be a literal, which is what lets the
//! runner find them all first.

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

    /// A scroller's own offset, `(0, 0)` for one never noted.
    fn own(&self, key: NodeKey) -> (f64, f64) {
        self.boxes
            .iter()
            .find(|(k, ..)| *k == key)
            .map_or((0.0, 0.0), |(_, l, t)| (*l, *t))
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
/// A host hands its two reads to [`GeometryLinks::new`], and what every host
/// shares (naming a view by its id, finding an action's `measure`s, building
/// the record) rides along in the same value, so an artifact that links no
/// geometry carries none of it (LLP 1047 D3).
#[derive(Clone, Copy)]
pub struct GeometryLinks {
    frame: fn(&Kernel, &Scrolled, ViewId) -> GeometryAnswer,
    measure: fn(&mut Kernel, &Scrolled, ViewId) -> GeometryAnswer,
    point: fn(&Kernel, &Scrolled, f64, f64) -> Option<ViewId>,
    ahead: fn(&Plan, &[u8], &mut Kernel, &Scrolled, &GeometryLinks) -> Measured,
    read: fn(&GeometryEnv<'_>, &Plan, Stdlib, &[Value]) -> Option<Value>,
}

impl GeometryLinks {
    /// A host's reads: `frame`, where layout put a view as last laid out,
    /// `measure`, the view's box as if its `height` were `auto`, and
    /// `point`, the front-most view whose box holds a viewport point.
    pub const fn new(
        frame: fn(&Kernel, &Scrolled, ViewId) -> GeometryAnswer,
        measure: fn(&mut Kernel, &Scrolled, ViewId) -> GeometryAnswer,
        point: fn(&Kernel, &Scrolled, f64, f64) -> Option<ViewId>,
    ) -> GeometryLinks {
        GeometryLinks {
            frame,
            measure,
            point,
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
pub static KERNEL: GeometryLinks = GeometryLinks::new(kernel_frame, kernel_measure, kernel_point);

fn kernel_frame(kernel: &Kernel, scrolled: &Scrolled, view: ViewId) -> GeometryAnswer {
    kernel
        .arena()
        .key_of(view)
        .and_then(|key| kernel.laid_out_frame(key))
        .map_or(GeometryAnswer::UNAVAILABLE, |(frame, provisional)| {
            GeometryAnswer::of(frame, provisional, scrolled.above(kernel, view))
        })
}

fn kernel_measure(kernel: &mut Kernel, scrolled: &Scrolled, view: ViewId) -> GeometryAnswer {
    let answer = kernel
        .arena()
        .key_of(view)
        .and_then(|key| kernel.measure_auto_height(key));
    answer.map_or(GeometryAnswer::UNAVAILABLE, |(frame, provisional)| {
        GeometryAnswer::of(frame, provisional, scrolled.above(kernel, view))
    })
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

/// A read's record: `measure` from the answers found before the body ran,
/// `frame` from the host now; `elementFromPoint`'s `option<string>`.
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
    let arena = kernel.arena();
    let mut at = arena.key_of(view).map(|key| key.index);
    while let Some(slot) = at {
        if let Some(id) = arena
            .props(slot)
            .str(PropId::Id)
            .filter(|id| !id.is_empty())
        {
            return Some(id.to_owned());
        }
        at = arena.parent(slot);
    }
    None
}

/// The front-most laid-out box at viewport `(x, y)` (LLP 1094 D10): every
/// root from the last, each node's children front to back by LLP 1083.000's
/// rank and then tree order, a child before its parent. Boxes are `frame`'s
/// (scrolls applied, transforms not); a scroller or a box that clips cuts
/// its descendants to its padding box; `pointer-events: none` and
/// `visibility` other than `visible` (both inherited) are passed over, and
/// `display: none` takes its subtree.
fn kernel_point(kernel: &Kernel, scrolled: &Scrolled, x: f64, y: f64) -> Option<ViewId> {
    let ranks: std::collections::HashMap<ViewId, i64> = kernel
        .paint_order()
        .into_iter()
        .map(|(view, placed)| (view, placed.rank))
        .collect();
    let walk = Point {
        kernel,
        scrolled,
        ranks,
        x,
        y,
    };
    let arena = kernel.arena();
    arena.roots().iter().rev().find_map(|&root| {
        walk.hit(
            root,
            scrolled.page,
            None,
            (PointerEvents::Auto, Visibility::Visible),
        )
    })
}

struct Point<'a> {
    kernel: &'a Kernel,
    scrolled: &'a Scrolled,
    ranks: std::collections::HashMap<ViewId, i64>,
    x: f64,
    y: f64,
}

impl Point<'_> {
    fn hit(
        &self,
        slot: u32,
        scroll: (f64, f64),
        clip: Option<[f64; 4]>,
        inherited: (PointerEvents, Visibility),
    ) -> Option<ViewId> {
        let arena = self.kernel.arena();
        let style = arena.style(slot);
        let flags = arena.flags(slot);
        if style.display == Display::None
            || flags.has(NodeFlags::HIDDEN)
            || flags.has(NodeFlags::CREATED)
        {
            return None;
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
        let (left, top) = (f64::from(f.x) - scroll.0, f64::from(f.y) - scroll.1);
        let (width, height) = (f64::from(f.width), f64::from(f.height));
        let own = self.scrolled.own(arena.key(slot));
        let inner = (scroll.0 + own.0, scroll.1 + own.1);
        let clips = style.overflow_x != Overflow::Visible || style.overflow_y != Overflow::Visible;
        let inner_clip = if clips {
            let padding = [
                left + f64::from(style.border_width_left),
                top + f64::from(style.border_width_top),
                left + width - f64::from(style.border_width_right),
                top + height - f64::from(style.border_width_bottom),
            ];
            Some(clip.map_or(padding, |c| {
                [
                    c[0].max(padding[0]),
                    c[1].max(padding[1]),
                    c[2].min(padding[2]),
                    c[3].min(padding[3]),
                ]
            }))
        } else {
            clip
        };
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
            if let Some(hit) = self.hit(child, inner, inner_clip, (pointer, visibility)) {
                return Some(hit);
            }
        }
        let inside =
            |b: [f64; 4]| b[0] <= self.x && self.x < b[2] && b[1] <= self.y && self.y < b[3];
        let hit = inside([left, top, left + width, top + height])
            && clip.is_none_or(inside)
            && pointer != PointerEvents::None
            && visibility == Visibility::Visible
            && !arena.is_inline_run(slot);
        hit.then(|| arena.local_id(slot))
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
