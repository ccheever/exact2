//! What a press would mount, found without running it.
//!
//! A press's response is late the first time it opens a screen: the screen's
//! text is shaped then. A host asks [`Runner::foresee`] at the press's touch
//! down, lays the answer out in a scratch kernel over its text engine, and
//! the release finds the paragraphs made. Nothing here changes the runner:
//! the action's body is evaluated and its outcome dropped (no write lands,
//! no command or send leaves, nothing is journaled), and the tree is read
//! ([`crate::instance::Tree::foresee`]).

use super::*;
use crate::instance::{Ids, Mount, Update};

/// What [`Runner::foresee`] found.
#[derive(Debug, Default)]
pub struct Foreseen {
    /// The arms the press would show, each under a live node.
    pub mounts: Vec<Mount>,
    /// The ops that create them, for a scratch kernel: none names a live
    /// view, and none belongs in this runner's kernel.
    pub ops: Vec<exact_kernel::Op>,
}

impl<D: DataSource> Runner<D> {
    /// The views a press on `view` would mount, by what its action writes to
    /// the app's slots alone; `None` when it would mount none, or when that
    /// cannot be told without running it (an action that reads geometry, a
    /// handler that takes the event's record, a poisoned runner).
    pub fn foresee(&self, view: ViewId) -> Option<Foreseen> {
        if self.poisoned {
            return None;
        }
        let tree = self.tree.as_ref()?;
        let (node, frames) = self.find(view)?;
        let handler = self
            .plan
            .node(node)
            .handlers
            .iter()
            .map(|h| self.plan.handler(h))
            .find(|h| h.event == EventKind::Press)?;
        let action = self.plan.action(handler.action);
        if handler.args.len != action.params.len {
            return None;
        }
        let mut args = Vec::with_capacity(handler.args.len as usize);
        for a in handler.args.iter() {
            args.push(self.eval(self.plan.arg(a).expr, &[], &frames).ok()?);
        }
        let allowed: Vec<u32> = action
            .writes
            .iter()
            .map(|w| self.plan.write(w).slot.0)
            .collect();
        let outcome = vm::eval(
            self.plan.code(action.body),
            &self.env(&args, &frames),
            &allowed,
        )
        .ok()?;
        if outcome.writes.is_empty() {
            return None;
        }
        let mut slots = self.slots.clone();
        for (slot, value) in outcome.writes {
            *slots.get_mut(slot as usize)? = value;
        }
        let mut ids = Ids::after(&self.ids);
        let env = vm::Env {
            slots: &slots,
            ..self.env(&[], &[])
        };
        let mut u = Update::new(env, &self.sites, &mut ids);
        let mounts = tree.foresee(&mut u).ok()?;
        (!mounts.is_empty()).then(|| Foreseen {
            mounts,
            ops: std::mem::take(&mut u.ops),
        })
    }
}
