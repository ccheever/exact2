//! A prop or style binding that traps (LLP 1017.003 D3): invalid at
//! computed-value time, as a value its row's grammar refuses is (CSS Custom
//! Properties §3.1). Its row is unset, one journal line names the binding's
//! site and the trap, and the update goes on. The trap stays inside the one
//! evaluation, so nothing else needs undoing. The traps that still fail the
//! update are LLP 1017.003 D3's.
use super::*;
use exact_plan::{BindingsId, BindingsRow};

/// A binding's evaluation as its row takes it.
pub(super) enum Evaluated {
    Value(Value),
    /// It trapped. `first`: its last evaluation did not, so the trap was
    /// journaled now and its row is to be unset; a binding that goes on
    /// trapping is neither journaled nor unset again until it evaluates to
    /// a value.
    Trapped {
        first: bool,
    },
}

impl NodeInst {
    /// Binding `b` of this node. `Pending` is not the binding's: the
    /// settlement retries it.
    pub(super) fn eval_row(
        &mut self,
        u: &mut Update<'_>,
        b: BindingsId,
        frames: &[Frame],
    ) -> Result<Evaluated, InstanceError> {
        let binding = u.env.plan.binding(b);
        match u.eval_binding(b, binding.expr, frames) {
            Ok(value) => {
                if let Some(rare) = self.rare.as_mut() {
                    rare.trapped.retain(|t| *t != b.0);
                }
                Ok(Evaluated::Value(value))
            }
            Err(trap @ Trap::Pending { .. }) => Err(InstanceError::Trap(trap)),
            Err(trap) => {
                let trapped = &mut self.rare().trapped;
                let first = !trapped.contains(&b.0);
                if first {
                    trapped.push(b.0);
                    u.notes.push(format!(
                        "view {} (site {}): {} trapped Trap({trap:?}); unset",
                        self.view,
                        self.node.0,
                        row_name(&binding)
                    ));
                }
                Ok(Evaluated::Trapped { first })
            }
        }
    }

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
