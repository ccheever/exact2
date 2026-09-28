//! Geometry reads from actions: `frame(id)` and `measure(id)`, each
//! answered by the engine that commits.
//!
//! @ref LLP 1051.000 D1 (what each read means), D2 (actions only; `measure`
//! answered before the action runs), D3 (the kernel natively), D4 (the page
//! on the web, through the artifact's link), D5 (provisional, unavailable)
//!
//! An action runs against a read-only runner, so `frame` reads through a
//! borrowed kernel while the body runs, and every `measure` in the body is
//! answered just before, with the kernel's engine tree to lay out. The
//! compiler requires `measure`'s id to be a literal, which is what lets the
//! runner find them all first.

use exact_kernel::{Frame, Kernel, ViewId};
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

    /// A kernel frame as an answer.
    pub fn of(frame: Frame, provisional: bool) -> GeometryAnswer {
        GeometryAnswer {
            x: f64::from(frame.x),
            y: f64::from(frame.y),
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
    frame: fn(&Kernel, ViewId) -> GeometryAnswer,
    measure: fn(&mut Kernel, ViewId) -> GeometryAnswer,
    ahead: fn(&Plan, &[u8], &mut Kernel, &GeometryLinks) -> Measured,
    read: fn(&GeometryEnv<'_>, &Plan, Stdlib, &[Value]) -> Option<Value>,
}

impl GeometryLinks {
    /// A host's reads: `frame`, where layout put a view as last laid out,
    /// and `measure`, the view's box as if its `height` were `auto`.
    pub const fn new(
        frame: fn(&Kernel, ViewId) -> GeometryAnswer,
        measure: fn(&mut Kernel, ViewId) -> GeometryAnswer,
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
    pub fn ahead(&self, plan: &Plan, code: &[u8], kernel: &mut Kernel) -> Measured {
        (self.ahead)(plan, code, kernel, self)
    }
}

/// The kernel's answers: a native host's kernel is its layout.
pub static KERNEL: GeometryLinks = GeometryLinks::new(kernel_frame, kernel_measure);

fn kernel_frame(kernel: &Kernel, view: ViewId) -> GeometryAnswer {
    kernel
        .arena()
        .key_of(view)
        .and_then(|key| kernel.laid_out_frame(key))
        .map_or(GeometryAnswer::UNAVAILABLE, |(frame, provisional)| {
            GeometryAnswer::of(frame, provisional)
        })
}

fn kernel_measure(kernel: &mut Kernel, view: ViewId) -> GeometryAnswer {
    kernel
        .arena()
        .key_of(view)
        .and_then(|key| kernel.measure_auto_height(key))
        .map_or(GeometryAnswer::UNAVAILABLE, |(frame, provisional)| {
            GeometryAnswer::of(frame, provisional)
        })
}

/// What an action's body reads geometry through (the VM's `Env`).
pub struct GeometryEnv<'a> {
    kernel: &'a Kernel,
    links: &'a GeometryLinks,
    measured: &'a [(StrId, GeometryAnswer)],
}

impl<'a> GeometryEnv<'a> {
    /// Reads for one action: `frame` through `links`, and the `measure`
    /// answers found before its body ran.
    pub fn new(
        kernel: &'a Kernel,
        links: &'a GeometryLinks,
        measured: &'a [(StrId, GeometryAnswer)],
    ) -> GeometryEnv<'a> {
        GeometryEnv {
            kernel,
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
            (env.links.frame)(env.kernel, view)
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
fn measure_ahead(plan: &Plan, code: &[u8], kernel: &mut Kernel, links: &GeometryLinks) -> Measured {
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
                                (links.measure)(kernel, view)
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
