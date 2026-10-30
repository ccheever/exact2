//! The kernel's motion seam, as the compiler writes it (rt.js `mo`): which
//! nodes the motion engine follows, and what it is told about each.

use super::{value_js, Em};
use crate::code::Scope;
use crate::style;
use exact_kernel::{PropId, StyleId};
use exact_plan::{BindingKind, EventKind, NodesId, Value};
use std::fmt::Write as _;

impl Em<'_> {
    /// Whether the motion engine follows node `i`: one a swipe holds, a
    /// swipe's indicator, or one whose `transition` can be a spring (css.rs
    /// leaves springs to the engine).
    pub(crate) fn is_motion_node(&self, i: u32) -> bool {
        let plan = self.plan;
        let row = &plan.node(NodesId(i));
        let binding = |id: u16, kind: BindingKind| {
            row.bindings
                .iter()
                .map(|b| plan.binding(b))
                .find(|b| b.kind == kind && b.id == id)
        };
        let swiped = row
            .handlers
            .iter()
            .any(|h| plan.handler(h).event == EventKind::Swiperight);
        let indicator =
            binding(PropId::SwipeIndicator as u16, BindingKind::Prop).is_some_and(|b| {
                !matches!(
                    style::literal(plan, plan.code(b.expr)),
                    Some(Value::Bool(false))
                )
            });
        let spring = binding(StyleId::Transition as u16, BindingKind::Style).is_some_and(|b| {
            match style::literal(plan, plan.code(b.expr)) {
                Some(v) => v.as_str().is_some_and(|t| t.contains("spring")),
                None => style::can_be(plan, plan.code(b.expr), &|v| v.contains("spring")),
            }
        });
        let target = self.transforms.contains(&i);
        swiped || indicator || spring || target
    }

    /// A node the motion engine follows (rt.js `mo`, the kernel's motion
    /// seam): one a swipe holds, a swipe's indicator, or one whose
    /// `transition` can be a spring (css.rs leaves springs to the engine).
    pub(crate) fn motion_node(&mut self, i: u32, e: &str, scope: &Scope) -> Result<(), String> {
        if !self.is_motion_node(i) {
            return Ok(());
        }
        let plan = self.plan;
        let row = &plan.node(NodesId(i));
        let binding = |id: u16, kind: BindingKind| {
            row.bindings
                .iter()
                .map(|b| plan.binding(b))
                .find(|b| b.kind == kind && b.id == id)
        };
        self.motion = true;
        let mut values = Vec::new();
        for (id, initial) in [
            (StyleId::Translate, "0"),
            (StyleId::Scale, "1"),
            (StyleId::Rotate, "0"),
            (StyleId::Opacity, "1"),
            (StyleId::Transition, "\"\""),
        ] {
            values.push(match binding(id as u16, BindingKind::Style) {
                None => initial.to_string(),
                Some(b) => match style::literal(plan, plan.code(b.expr)) {
                    Some(v) => value_js(&v),
                    None => format!("(({})()??{initial})", self.bound(i, e, &b, scope)?),
                },
            });
        }
        let mo = self.uses.rt("mo");
        let _ = write!(self.out, "{mo}({e},()=>[{}]);", values.join(","));
        Ok(())
    }

    /// Possible owners need their authored motion values registered before a
    /// drag starts. The live binding is resolved from the DOM after each commit;
    /// only a literal mismatch can rule an ancestor out here.
    pub(super) fn drag_targets(&self, prop: PropId) -> std::collections::BTreeSet<u32> {
        let plan = self.plan;
        let binding = |i: u32, prop: PropId| {
            plan.node(NodesId(i))
                .bindings
                .iter()
                .map(|b| plan.binding(b))
                .find(|b| b.kind == BindingKind::Prop && b.id == prop as u16)
        };
        let mut out = std::collections::BTreeSet::new();
        for i in 0..plan.nodes.len() as u32 {
            let Some(b) = binding(i, prop) else {
                continue;
            };
            let name = style::literal(plan, plan.code(b.expr));
            if name
                .as_ref()
                .is_some_and(|v| v.as_str().is_none_or(str::is_empty))
            {
                continue;
            }
            let mut at = self.parent_of(i);
            while let Some(p) = at {
                if let Some(id) = binding(p, PropId::Id) {
                    let id = style::literal(plan, plan.code(id.expr));
                    let possible = match (&name, &id) {
                        (_, Some(id)) if id.as_str().is_none_or(str::is_empty) => false,
                        (Some(name), Some(id)) => name.as_str() == id.as_str(),
                        _ => true,
                    };
                    if possible {
                        out.insert(p);
                    }
                }
                at = self.parent_of(p);
            }
        }
        out
    }

    pub(super) fn reorder_handle(&self, i: u32) -> bool {
        let plan = self.plan;
        plan.node(NodesId(i))
            .bindings
            .iter()
            .map(|b| plan.binding(b))
            .any(|b| b.kind == BindingKind::Prop && b.id == PropId::ReorderFor as u16)
    }

    /// A height drag's owner (rt.js `mh`): its numeric height and its
    /// `transition`, which a released drag springs by.
    pub(crate) fn height_owner(&mut self, i: u32, e: &str, scope: &Scope) -> Result<(), String> {
        if !self.heights.contains(&i) {
            return Ok(());
        }
        self.motion = true;
        let plan = self.plan;
        let mut values = Vec::new();
        for (id, initial) in [(StyleId::Height, "null"), (StyleId::Transition, "\"\"")] {
            let b = plan
                .node(NodesId(i))
                .bindings
                .iter()
                .map(|b| plan.binding(b))
                .find(|b| b.kind == BindingKind::Style && b.id == id as u16);
            values.push(match b {
                None => initial.to_string(),
                Some(b) => match style::literal(plan, plan.code(b.expr)) {
                    Some(v) => value_js(&v),
                    None => format!("(({})()??{initial})", self.bound(i, e, &b, scope)?),
                },
            });
        }
        let mh = self.uses.rt("mh");
        let _ = write!(self.out, "{mh}({e},()=>[{}]);", values.join(","));
        Ok(())
    }
}
