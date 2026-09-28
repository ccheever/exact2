//! What a plan uses beyond the core, derived from its bytes.
//!
//! @ref LLP 1047 D1 (the core, linked and loaded capabilities)
//! @ref LLP 1047 D2 (the use-set is a pure function of the plan)
//! @ref LLP 1047 D6 (a host refuses a plan that uses what it doesn't link)
//!
//! No author declares a capability: a plan's rows say which it needs. This
//! lives in the runner, not the plan crate, because a use is a kernel prop or
//! style row, and the plan crate declares no kernel vocabulary (LLP 1004 D2).
//! A binding whose value the plan computes counts as a use of whatever it
//! might select, so the set is never smaller than what a run can reach.

use exact_kernel::{PropId, StyleId};
use exact_plan::{BindingKind, EventKind, Opcode, Plan, Stdlib, StrId};
use std::fmt;

/// A capability beyond the core, linked into an artifact only when its plan
/// uses it (LLP 1047 §4's roster, as each one moves behind its seam).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capability {
    /// `markup="markdown"` text: its source styled as pieces.
    Markdown,
    /// Springs and holds: a `transition` that can be a `spring()`, and the
    /// gestures that hold a value (a swipe, a height, transform or reorder
    /// drag). CSS plays every other transition.
    Motion,
    /// Lists the host windows: a `virtualized` list,
    /// or a handler for a list's edges.
    Collections,
    /// Drags a host tracks at every commit: height, transform and reorder
    /// handles. A drag holds a value, so it uses motion too.
    Drag,
    /// GPU canvas surfaces: a canvas with a surface, or a resource reading a
    /// surface's record; the host answers both (the GPU module is loaded).
    Surfaces,
    /// The router (LLP 1038): a plan that declares `routes`.
    Router,
    /// `formatDate` and `formatNumber` (LLP 1054.000.003 D8): a plan whose
    /// code calls one.
    Format,
    /// `backgroundMaterial` (LLP 1053.000 D4): the materials table, and on
    /// the web its approximation's CSS.
    Materials,
    /// `backdrop-filter` (LLP 1053.000 D1): its grammar and named refusals.
    Backdrop,
    /// `share(…)` (LLP 1069.003): a plan whose code runs the command.
    Share,
    /// `saveFile` and the three file pickers (LLP 1069.010): a plan whose
    /// code runs one.
    Documents,
    /// `input type="file"` and `showPicker` (LLP 1069.002): a plan with a
    /// file input, or whose code runs the command.
    Picker,
    /// Drag timelines (LLP 1057.003): `drag-timeline`, `animation-timeline`,
    /// `animation-range` and `timeline-scope`, their grammar, lowering and
    /// name lookup. A timeline follows a held value, so it uses motion too.
    Timelines,
    /// `text-transform` (LLP 1064 D5): its Unicode case mapping, for a plan
    /// that binds the row.
    TextTransform,
    /// `filter` and `clip-path` (LLP 1055.000 D10, D14): their grammars, for
    /// a plan that binds either row.
    Effects,
    /// CSS animations (LLP 1055 D5): the `animation` shorthand's and
    /// `@keyframes`' grammars, for a plan that declares keyframes or binds
    /// `animation` or `exit-animation`.
    Animations,
    /// `background-image`'s gradients (LLP 1066): its grammar, for a plan
    /// that binds the row.
    Gradients,
    /// `frame` and `measure` (LLP 1051.000): a plan whose actions read
    /// geometry. Native hosts answer from the kernel; the web links a
    /// synchronous import the page answers.
    Geometry,
}

impl Capability {
    /// Every capability, in bit order.
    pub const ALL: [Capability; 18] = [
        Capability::Markdown,
        Capability::Motion,
        Capability::Collections,
        Capability::Drag,
        Capability::Surfaces,
        Capability::Router,
        Capability::Format,
        Capability::Materials,
        Capability::Backdrop,
        Capability::Share,
        Capability::Documents,
        Capability::Picker,
        Capability::Timelines,
        Capability::TextTransform,
        Capability::Effects,
        Capability::Animations,
        Capability::Gradients,
        Capability::Geometry,
    ];

    /// The name an entry, a refusal and a report use.
    pub const fn name(self) -> &'static str {
        match self {
            Capability::Markdown => "markdown",
            Capability::Motion => "motion",
            Capability::Collections => "collections",
            Capability::Drag => "drag",
            Capability::Surfaces => "surfaces",
            Capability::Router => "router",
            Capability::Format => "format",
            Capability::Materials => "materials",
            Capability::Backdrop => "backdrop",
            Capability::Share => "share",
            Capability::Documents => "documents",
            Capability::Picker => "picker",
            Capability::Timelines => "timelines",
            Capability::TextTransform => "text_transform",
            Capability::Effects => "effects",
            Capability::Animations => "animations",
            Capability::Gradients => "gradients",
            Capability::Geometry => "geometry",
        }
    }

    const fn bit(self) -> u32 {
        1 << self as u32
    }
}

/// A set of capabilities.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Uses(u32);

impl Uses {
    /// The core alone.
    pub const NONE: Uses = Uses(0);

    /// This set and `capability`.
    pub const fn with(self, capability: Capability) -> Uses {
        Uses(self.0 | capability.bit())
    }

    /// Whether the set holds `capability`.
    pub const fn has(self, capability: Capability) -> bool {
        self.0 & capability.bit() != 0
    }

    /// What this set holds that `linked` doesn't.
    pub const fn beyond(self, linked: Uses) -> Uses {
        Uses(self.0 & !linked.0)
    }

    /// Whether the set is the core alone.
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// The capabilities in the set, in bit order.
    pub fn iter(self) -> impl Iterator<Item = Capability> {
        Capability::ALL.into_iter().filter(move |c| self.has(*c))
    }
}

/// The names, comma-separated: `markdown, router`.
impl fmt::Display for Uses {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, capability) in self.iter().enumerate() {
            if i > 0 {
                f.write_str(", ")?;
            }
            f.write_str(capability.name())?;
        }
        Ok(())
    }
}

/// The capabilities `plan` uses (LLP 1047 D2).
pub fn uses(plan: &Plan) -> Uses {
    let mut uses = Uses::NONE;
    if !plan.keyframes.is_empty() {
        uses = uses.with(Capability::Animations);
    }
    if plan.router.is_some() {
        uses = uses.with(Capability::Router);
    }
    if !plan.surfaces.is_empty()
        || plan
            .resources
            .iter()
            .any(|r| plan.str(r.source) == crate::surface_record::SOURCE)
    {
        uses = uses.with(Capability::Surfaces);
    }
    let can_be = |binding: &exact_plan::BindingsRow, used: &dyn Fn(&str) -> bool| {
        constant_str(plan, plan.code(binding.expr)).is_none_or(used)
    };
    for binding in &plan.bindings {
        match binding.kind {
            BindingKind::Prop => match PropId::from_wire(binding.id) {
                Some(PropId::Markup) if can_be(binding, &|v| v == "markdown") => {
                    uses = uses.with(Capability::Markdown);
                }
                Some(PropId::HeightDragFor | PropId::TransformDragFor | PropId::ReorderFor) => {
                    uses = uses.with(Capability::Motion).with(Capability::Drag);
                }
                Some(PropId::Virtualized)
                    if constant_bool(plan.code(binding.expr)).is_none_or(|on| on) =>
                {
                    uses = uses.with(Capability::Collections);
                }
                Some(PropId::BackgroundMaterial) => uses = uses.with(Capability::Materials),
                Some(PropId::Type) if can_be(binding, &|v| v == "file") => {
                    uses = uses.with(Capability::Picker);
                }
                _ => {}
            },
            BindingKind::Style => {
                if StyleId::from_bit(u32::from(binding.id)) == Some(StyleId::BackdropBlur) {
                    uses = uses.with(Capability::Backdrop);
                }
                if matches!(
                    StyleId::from_bit(u32::from(binding.id)),
                    Some(
                        StyleId::DragTimeline
                            | StyleId::AnimationTimeline
                            | StyleId::AnimationRange
                            | StyleId::TimelineScope
                    )
                ) {
                    uses = uses.with(Capability::Timelines).with(Capability::Motion);
                }
                if StyleId::from_bit(u32::from(binding.id)) == Some(StyleId::TextTransform) {
                    uses = uses.with(Capability::TextTransform);
                }
                if matches!(
                    StyleId::from_bit(u32::from(binding.id)),
                    Some(StyleId::Filter | StyleId::ClipPath)
                ) {
                    uses = uses.with(Capability::Effects);
                }
                if matches!(
                    StyleId::from_bit(u32::from(binding.id)),
                    Some(StyleId::Animation | StyleId::ExitAnimation)
                ) {
                    uses = uses.with(Capability::Animations);
                }
                if StyleId::from_bit(u32::from(binding.id)) == Some(StyleId::BackgroundImage) {
                    uses = uses.with(Capability::Gradients);
                }
                if StyleId::from_bit(u32::from(binding.id)) == Some(StyleId::Transition)
                    && can_be(binding, &|v| v.contains("spring"))
                {
                    uses = uses.with(Capability::Motion);
                }
            }
        }
    }
    for handler in &plan.handlers {
        match handler.event {
            EventKind::Swiperight => uses = uses.with(Capability::Motion),
            EventKind::Heightrelease
            | EventKind::Transformgeometry
            | EventKind::Transformrelease
            | EventKind::Reorderdrop => {
                uses = uses.with(Capability::Motion).with(Capability::Drag);
            }
            _ => {}
        }
    }
    if plan
        .handlers
        .iter()
        .any(|h| matches!(h.event, EventKind::Reachstart | EventKind::Reachend))
    {
        uses = uses.with(Capability::Collections);
    }
    let (format, geometry) = stdlib_calls(plan);
    if format {
        uses = uses.with(Capability::Format);
    }
    if runs_command(plan, &["share"]) {
        uses = uses.with(Capability::Share);
    }
    if runs_command(
        plan,
        &[
            "saveFile",
            "showOpenFilePicker",
            "showDirectoryPicker",
            "showSaveFilePicker",
        ],
    ) {
        uses = uses.with(Capability::Documents);
    }
    if runs_command(plan, &["showPicker"]) {
        uses = uses.with(Capability::Picker);
    }
    if geometry {
        uses = uses.with(Capability::Geometry);
    }
    uses
}

/// Whether any code range calls a `format` entry, and whether any reads
/// geometry (`frame`, `measure`): each validated body walked whole, so no
/// call a run can reach is missed.
fn stdlib_calls(plan: &Plan) -> (bool, bool) {
    let (mut format, mut geometry) = (false, false);
    plan.each_code(&mut |code| {
        for i in crate::vm::instructions(plan.code(code)).flatten() {
            if i.op == Opcode::Call {
                match Stdlib::from_wire(i.args[0] as u8) {
                    Some(Stdlib::FormatDate | Stdlib::FormatNumber) => format = true,
                    Some(Stdlib::Frame | Stdlib::Measure) => geometry = true,
                    _ => {}
                }
            }
        }
    });
    (format, geometry)
}

/// Whether any code range runs a host command named one of `names`.
fn runs_command(plan: &Plan, names: &[&str]) -> bool {
    let mut runs = false;
    plan.each_code(&mut |code| {
        runs = runs
            || crate::vm::instructions(plan.code(code)).any(|i| {
                i.is_ok_and(|i| {
                    i.op == Opcode::Command
                        && plan
                            .strings
                            .get(i.args[0] as usize)
                            .is_some_and(|_| names.contains(&plan.str(StrId(i.args[0] as u32))))
                })
            });
    });
    runs
}

/// The boolean a binding always evaluates to, when its code is one constant:
/// `Bool`, then `Return`.
fn constant_bool(code: &[u8]) -> Option<bool> {
    match code {
        [op, value, ret] if *op == Opcode::Bool as u8 && *ret == Opcode::Return as u8 => {
            Some(*value != 0)
        }
        _ => None,
    }
}

/// The string a binding always evaluates to, when its code is one constant:
/// `Str`, then `Return`.
fn constant_str<'a>(plan: &'a Plan, code: &[u8]) -> Option<&'a str> {
    match code {
        [op, a, b, c, d, ret] if *op == Opcode::Str as u8 && *ret == Opcode::Return as u8 => {
            let id = u32::from_le_bytes([*a, *b, *c, *d]);
            plan.strings.get(id as usize).map(|_| plan.str(StrId(id)))
        }
        _ => None,
    }
}
