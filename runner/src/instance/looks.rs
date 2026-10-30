//! A node's host-known state looks (LLP 1121 D8, stage 8 B): the plan's
//! `looks` rows for its site, each state's rows resolved as a static row is
//! (in the node's tuple, a token's name to its value) and sent whole as
//! `Op::SetLooks`, which each host composes over the base on its own
//! tracking. Resolved again when the node mounts, when its tuple moves (a
//! mode, a pin) and, through [`super::theme::ThemeIndex`], when a token a
//! look read takes a new value.

use super::{InstanceError, NodeInst, Update};
use crate::bridge;
use crate::vm::Frame;
use exact_kernel::{Op, StyleProps, ViewId};
use exact_plan::{Plan, Value};

/// A looks row: style id, text, written.
pub type Row = (u16, Value, bool);

/// A node's looks as resolved: each state's rows as written, and what they
/// read, to resolve again.
#[derive(Debug, Clone, PartialEq)]
pub struct Looks {
    /// The app tuple they were resolved in.
    pub tuple: u32,
    /// The tokens they read, as `theme::name_bit`s.
    pub tokens: u64,
    /// Each state and its rows.
    pub states: Vec<(u8, Vec<Row>)>,
}

impl Looks {
    /// Each state's rows resolved in the tuple now set, for `Op::SetLooks`.
    pub fn resolve(&self, plan: &Plan) -> Vec<(u8, StyleProps)> {
        self.states
            .iter()
            .map(|(state, rows)| (*state, patch(rows, plan)))
            .collect()
    }
}

/// `rows` resolved into one patch; a row that does not parse is left out,
/// as a static row's refusal is (its note is the build's).
fn patch(rows: &[Row], plan: &Plan) -> StyleProps {
    let mut patch = StyleProps::default();
    for (id, value, written) in rows {
        // An explicit clear (`unset`): written, no value, as a static row's.
        if bridge::unsets(*id, value) {
            if let Some(style) = exact_kernel::StyleId::from_bit(*id as u32).filter(|_| *written) {
                patch.written.set(style);
            }
            continue;
        }
        if let Ok(style) = bridge::set_plan_style(&mut patch, *id, value, plan) {
            match written {
                true => patch.written.set(style),
                false => patch.written.clear(style),
            }
        }
    }
    patch
}

/// `view`'s looks, resolved in `tuple`, as `Op::SetLooks` (empty: none),
/// and the tokens they read as resolved: a token's new value may read one
/// its old value didn't (a shadow over a base, LLP 1121 stage 12 B3).
pub(crate) fn op(view: ViewId, looks: &Looks, plan: &Plan) -> (Op, u64) {
    exact_kernel::style::theme::take_named();
    let _tuple = bridge::set_tuple(looks.tuple);
    let op = Op::SetLooks {
        id: view,
        looks: looks.resolve(plan),
    };
    bridge::set_tuple(_tuple);
    (op, exact_kernel::style::theme::take_named())
}

impl NodeInst {
    /// Send this node's looks when it mounts (`fresh`) or its tuple moved
    /// (`moved`); a node whose site has none sends nothing.
    pub(super) fn emit_looks(
        &mut self,
        u: &mut Update<'_>,
        frames: &[Frame],
        fresh: bool,
        moved: bool,
    ) -> Result<(), InstanceError> {
        let plan = u.env.plan;
        let at = plan.looks.partition_point(|l| l.node.0 < self.node.0);
        let rows = plan.looks[at..]
            .iter()
            .take_while(|l| l.node == self.node)
            .count();
        if rows == 0 || !(fresh || moved) {
            return Ok(());
        }
        let mut states = Vec::with_capacity(rows);
        for look in &plan.looks[at..at + rows] {
            let mut written = Vec::with_capacity(look.bindings.len as usize);
            for b in look.bindings.iter() {
                // A row that trapped is left out, as one that does not parse is.
                let binding = plan.binding(b);
                if let Some(value) = u.eval_row(self.view, self.node, b, frames)? {
                    written.push((binding.id, value, binding.written));
                }
            }
            states.push((look.state, written));
        }
        let mut looks = Looks {
            tuple: self.tuple,
            tokens: 0,
            states,
        };
        exact_kernel::style::theme::take_named();
        let _tuple = bridge::set_tuple(looks.tuple);
        let resolved = looks.resolve(plan);
        bridge::set_tuple(_tuple);
        looks.tokens = exact_kernel::style::theme::take_named();
        u.ops.push(Op::SetLooks {
            id: self.view,
            looks: resolved,
        });
        match looks.tokens {
            0 => {
                u.ids.theme.looks.remove(&self.view);
            }
            _ => {
                u.ids.theme.looks.insert(self.view, looks);
            }
        }
        Ok(())
    }
}
