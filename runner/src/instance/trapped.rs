//! A prop or style binding that traps (LLP 1017.003 D3): invalid at
//! computed-value time, as a value its row's grammar refuses is (CSS Custom
//! Properties §3.1). Its row is unset, one journal line names the binding's
//! site and the trap, and the update goes on. The trap stays inside the one
//! evaluation, so nothing else needs undoing. A trap in a region's subject,
//! a key or a row initializer still fails the update.
use super::*;
use exact_plan::{BindingsId, BindingsRow};

impl Update<'_> {
    /// Binding `b` of node `node` (view `view`), or `None` when it trapped,
    /// journaled. `Pending` is not the binding's: the settlement retries it.
    pub(super) fn eval_row(
        &mut self,
        view: ViewId,
        node: NodesId,
        b: BindingsId,
        frames: &[Frame],
    ) -> Result<Option<Value>, InstanceError> {
        let binding = self.env.plan.binding(b);
        match self.eval_binding(b, binding.expr, frames) {
            Ok(value) => Ok(Some(value)),
            Err(trap @ Trap::Pending { .. }) => Err(InstanceError::Trap(trap)),
            Err(trap) => {
                let row = row_name(&binding);
                self.notes.push(format!(
                    "view {view} (site {}): {row} trapped Trap({trap:?}); unset",
                    node.0
                ));
                Ok(None)
            }
        }
    }
}

impl NodeInst {
    /// Unset binding `i`'s row, which trapped.
    pub(super) fn unset_trapped(&mut self, u: &mut Update<'_>, i: usize, binding: &BindingsRow) {
        let id = self.view;
        match binding.kind {
            BindingKind::Prop => {
                if let Some(prop) = exact_kernel::PropId::from_wire(binding.id) {
                    u.ops.push(Op::ClearProp { id, prop });
                }
            }
            BindingKind::Style => {
                if let Some(style) = exact_kernel::StyleId::from_bit(binding.id as u32) {
                    let mut mask = exact_kernel::StyleMask::default();
                    mask.set(style);
                    u.ops.push(Op::ClearStyle {
                        id,
                        mask,
                        written: binding.written,
                    });
                    self.unthemed(u, i, style);
                }
            }
        }
    }
}

/// A binding's row as the journal names it: a prop's name, a style's CSS
/// property.
fn row_name(binding: &BindingsRow) -> String {
    match binding.kind {
        BindingKind::Prop => exact_kernel::PropId::from_wire(binding.id)
            .map_or_else(|| format!("prop {}", binding.id), |p| p.name().to_string()),
        BindingKind::Style => exact_kernel::StyleId::from_bit(binding.id as u32).map_or_else(
            || format!("style {}", binding.id),
            |s| s.name().replace('_', "-"),
        ),
    }
}
