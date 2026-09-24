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

use exact_kernel::PropId;
use exact_plan::{BindingKind, Opcode, Plan, StrId};
use std::fmt;

/// A capability beyond the core, linked into an artifact only when its plan
/// uses it (LLP 1047 §4's roster, as each one moves behind its seam).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capability {
    /// `markup="markdown"` text: its source styled as pieces.
    Markdown,
}

impl Capability {
    /// Every capability, in bit order.
    pub const ALL: [Capability; 1] = [Capability::Markdown];

    /// The name an entry, a refusal and a report use.
    pub const fn name(self) -> &'static str {
        match self {
            Capability::Markdown => "markdown",
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
    for binding in &plan.bindings {
        if binding.kind == BindingKind::Prop
            && binding.id == PropId::Markup as u16
            && constant_str(plan, plan.code(binding.expr)).is_none_or(|v| v == "markdown")
        {
            uses = uses.with(Capability::Markdown);
        }
    }
    uses
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
