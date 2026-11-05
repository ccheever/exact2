//! A prop or style binding that traps (LLP 1017.003 D3): its row is unset,
//! one journal line names its plan site and the trap, and the commit goes
//! on, as the runner's `instance/trapped.rs` has it (budget.js `tg`). The
//! guard keeps its state on the element, so the several writes one binding
//! feeds (a painted flag, a motion value) journal it once.

use super::Em;
use crate::code::Scope;
use exact_kernel::{PropId, StyleId};
use exact_plan::{BindingKind, BindingsRow};

impl Em<'_> {
    /// Binding `b` of node `i` (element `e`) as a function. A body that
    /// names none of budget.js's cannot trap and is left as it is.
    pub(super) fn bound(
        &mut self,
        i: u32,
        e: &str,
        b: &BindingsRow,
        scope: &Scope,
    ) -> Result<String, String> {
        let before = self.uses.metered;
        let mut f = self.f(b.expr, scope)?;
        let metered = self.uses.metered != before;
        if let Some(owners) = (b.kind == BindingKind::Prop)
            .then(|| self.answers.status_binding(i, b.id))
            .flatten()
        {
            let owners = super::answers::status_owners(owners);
            let guard = self.uses.rt("statusSubject");
            f = format!("(globalThis.__exactRender?{guard}({e},{f},{owners}):{f})");
        }
        if !metered {
            return Ok(f);
        }
        let row = match b.kind {
            BindingKind::Prop => PropId::from_wire(b.id)
                .map_or_else(|| format!("prop {}", b.id), |p| p.name().to_string()),
            BindingKind::Style => StyleId::from_bit(b.id as u32)
                .map_or_else(|| format!("style {}", b.id), |s| s.name().replace('_', "-")),
        };
        let tg = self.uses.budget("tg");
        Ok(format!(
            "{tg}({e},{i},{},{f})",
            serde_json::to_string(&row).unwrap()
        ))
    }
}
