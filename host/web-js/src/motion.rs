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
        swiped || indicator || spring
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
}
