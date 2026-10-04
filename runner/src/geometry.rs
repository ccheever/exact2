//! Geometry reads from actions: `frame(id)` and `measure(id)`, each
//! answered by the engine that commits.
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
//! An action runs against a read-only runner, so `frame` reads through a
//! borrowed kernel while the body runs, and every `measure` in the body is
//! answered just before, with the kernel's engine tree to lay out. The
//! compiler requires `measure`'s id to be a literal, which is what lets the
//! runner find them all first.

use exact_kernel::{Frame, Kernel, NodeKey, ViewId};
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

    /// One scroller's offset, zero for a node with none noted.
    fn offset(&self, key: NodeKey) -> (f64, f64) {
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
    ahead: fn(&Plan, &[u8], &mut Kernel, &Scrolled, &GeometryLinks) -> Measured,
    read: fn(&GeometryEnv<'_>, &Plan, Stdlib, &[Value]) -> Option<Value>,
}

impl GeometryLinks {
    /// A host's reads: `frame`, where layout put a view as last laid out,
    /// and `measure`, the view's box as if its `height` were `auto`.
    pub const fn new(
        frame: fn(&Kernel, &Scrolled, ViewId) -> GeometryAnswer,
        measure: fn(&mut Kernel, &Scrolled, ViewId) -> GeometryAnswer,
    ) -> GeometryLinks {
        GeometryLinks {
            frame,
            measure,
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
pub static KERNEL: GeometryLinks = GeometryLinks::new(kernel_frame, kernel_measure);

fn kernel_frame(kernel: &Kernel, scrolled: &Scrolled, view: ViewId) -> GeometryAnswer {
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
    use exact_kernel::svg::transform as tf;
    let arena = kernel.arena();
    let Some(key) = arena.key_of(view) else {
        return GeometryAnswer::UNAVAILABLE;
    };
    let mut m = tf::IDENTITY;
    let mut slot = key.index;
    let mut at = frame;
    loop {
        let s = arena.style(slot);
        let (w, h) = (at.width, at.height);
        let (ox, oy) = s.transform_origin.resolve(w, h);
        let turn = if s.rotate_axis.is_3d() { 0.0 } else { s.rotate };
        let own = tf::mul(
            tf::translate(
                ox + s.translate.x + s.translate_percent.x / 100.0 * w,
                oy + s.translate.y + s.translate_percent.y / 100.0 * h,
            ),
            tf::mul(
                tf::mul(tf::rotate(turn), tf::scale(s.scale, s.scale)),
                tf::translate(-ox, -oy),
            ),
        );
        m = tf::mul(own, m);
        let parent = (!arena.is_root(slot)).then(|| arena.parent(slot)).flatten();
        let Some(parent) = parent else {
            // The root's place: its own frame, in its root's space.
            m = tf::mul(tf::translate(at.x, at.y), m);
            break;
        };
        let up = arena.frame(parent);
        let (left, top) = scrolled.offset(arena.key(parent));
        m = tf::mul(
            tf::translate(at.x - up.x - left as f32, at.y - up.y - top as f32),
            m,
        );
        slot = parent;
        at = up;
    }
    let corners = [
        (0.0, 0.0),
        (frame.width, 0.0),
        (0.0, frame.height),
        (frame.width, frame.height),
    ]
    .map(|p| tf::apply(m, p));
    let (x0, y0, x1, y1) = corners.iter().fold(
        (
            f32::INFINITY,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::NEG_INFINITY,
        ),
        |(a, b, c, d), (x, y)| (a.min(*x), b.min(*y), c.max(*x), d.max(*y)),
    );
    if !(x0.is_finite() && y0.is_finite() && x1.is_finite() && y1.is_finite()) {
        return GeometryAnswer::UNAVAILABLE;
    }
    let (left, top) = scrolled.page;
    GeometryAnswer {
        x: f64::from(x0) - left,
        y: f64::from(y0) - top,
        width: f64::from(x1 - x0),
        height: f64::from(y1 - y0),
        provisional,
        unavailable: false,
    }
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
/// `frame` from the host now.
fn read(env: &GeometryEnv<'_>, plan: &Plan, f: Stdlib, args: &[Value]) -> Option<Value> {
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
