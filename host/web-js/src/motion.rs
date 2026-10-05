//! The kernel's motion seam, as the compiler writes it (rt.js `mo`): which
//! nodes the motion engine follows, and what it is told about each.

use super::{value_js, Em};
use crate::code::Scope;
use crate::style;
use exact_kernel::{PropId, StyleId};
use exact_plan::{BindingKind, EventKind, Value};
use std::fmt::Write as _;

impl Em<'_> {
    /// Whether the motion engine follows node `i`: one a swipe holds, a
    /// swipe's indicator, or one whose `transition` can be a spring (css.rs
    /// leaves springs to the engine).
    pub(crate) fn is_motion_node(&self, i: u32) -> bool {
        let plan = self.plan;
        let row = &plan.nodes[i as usize];
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
        let target = self.transforms.values().any(|(t, _)| *t == i);
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
        let row = &plan.nodes[i as usize];
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
                    None => format!("({})()", self.f(b.expr, scope)?),
                },
            });
        }
        let mo = self.uses.rt("mo");
        let _ = write!(self.out, "{mo}({e},()=>[{}]);", values.join(","));
        Ok(())
    }

    /// Each `heightDragFor` handle's owner: the unique strict ancestor whose
    /// `id` it names (kernel `height_drag_target`), read from the plan, whose
    /// literal ids the kernel's are. A handle that names none drags nothing.
    pub(super) fn height_targets(&self) -> std::collections::BTreeMap<u32, u32> {
        self.drag_targets(PropId::HeightDragFor)
    }

    /// Each `transformDragFor` handle's target and its clip, the target's
    /// parent (kernel `transform_drag_binding`).
    pub(super) fn transform_targets(&self) -> std::collections::BTreeMap<u32, (u32, u32)> {
        self.drag_targets(PropId::TransformDragFor)
            .into_iter()
            .filter_map(|(h, t)| Some((h, (t, self.parent_of(t)?))))
            .collect()
    }

    /// Each `reorderFor` grip's list: the unique strict ancestor with that
    /// `id` (runner `reorder_binding`; the list, its handler and its rows
    /// are checked at run time).
    pub(super) fn reorder_lists(&self) -> std::collections::BTreeMap<u32, u32> {
        self.drag_targets(PropId::ReorderFor)
    }

    /// A grip whose `reorderFor` is not a literal: its list is the strict
    /// ancestor its value names at run time (reorder.js `reorderBinding`).
    pub(super) fn computed_reorder(&self, i: u32) -> bool {
        let plan = self.plan;
        plan.nodes[i as usize]
            .bindings
            .iter()
            .map(|b| plan.binding(b))
            .any(|b| b.kind == BindingKind::Prop && b.id == PropId::ReorderFor as u16)
    }

    fn drag_targets(&self, prop: PropId) -> std::collections::BTreeMap<u32, u32> {
        let plan = self.plan;
        let literal = |i: u32, prop: PropId| {
            plan.nodes[i as usize]
                .bindings
                .iter()
                .map(|b| plan.binding(b))
                .find(|b| b.kind == BindingKind::Prop && b.id == prop as u16)
                .and_then(|b| style::literal(plan, plan.code(b.expr)))
                .and_then(|v| v.as_str().map(str::to_string))
        };
        let mut out = std::collections::BTreeMap::new();
        for i in 0..plan.nodes.len() as u32 {
            let Some(name) = literal(i, prop).filter(|n| !n.is_empty()) else {
                continue;
            };
            let mut found = Vec::new();
            let mut at = self.parent_of(i);
            while let Some(p) = at {
                if literal(p, PropId::Id).as_deref() == Some(name.as_str()) {
                    found.push(p);
                }
                at = self.parent_of(p);
            }
            if let [t] = found[..] {
                out.insert(i, t);
            }
        }
        out
    }

    /// A height drag's owner (rt.js `mh`): its numeric height and its
    /// `transition`, which a released drag springs by.
    pub(crate) fn height_owner(&mut self, i: u32, e: &str, scope: &Scope) -> Result<(), String> {
        if !self.heights.values().any(|t| *t == i) {
            return Ok(());
        }
        self.motion = true;
        let plan = self.plan;
        let mut values = Vec::new();
        for (id, initial) in [(StyleId::Height, "null"), (StyleId::Transition, "\"\"")] {
            let b = plan.nodes[i as usize]
                .bindings
                .iter()
                .map(|b| plan.binding(b))
                .find(|b| b.kind == BindingKind::Style && b.id == id as u16);
            values.push(match b {
                None => initial.to_string(),
                Some(b) => match style::literal(plan, plan.code(b.expr)) {
                    Some(v) => value_js(&v),
                    None => format!("({})()", self.f(b.expr, scope)?),
                },
            });
        }
        let mh = self.uses.rt("mh");
        let _ = write!(self.out, "{mh}({e},()=>[{}]);", values.join(","));
        Ok(())
    }
}
