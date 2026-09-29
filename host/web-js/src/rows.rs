//! A node's dynamic style rows as the compiler writes them (rt.js `S`,
//! `Sm`), and where a node's static rows must be inline too.

use super::Em;
use crate::style;
use exact_kernel::PropId;
use exact_kernel::{NodeType, StyleId};
use exact_plan::{BindingKind, BindingsRow};
use exact_web::host::template::Parts;
use std::fmt::Write as _;

impl Em<'_> {
    /// One dynamic style binding `b` of node `i` (element `e`, value `f`).
    pub(super) fn style_row(
        &mut self,
        i: u32,
        b: &BindingsRow,
        parts: &Parts,
        e: &str,
        f: &str,
    ) -> Result<(), String> {
        let plan = self.plan;
        let row = &plan.nodes[i as usize];
        let press = parts.css.contains("--exact-press:");
        let timeline = row.bindings.iter().any(|b| {
            let b = plan.binding(b);
            b.kind == BindingKind::Style && b.id == StyleId::AnimationTimeline as u16
        });
        // An id a reference names is scoped per instance by the
        // kernel (LLP 1055.000 D3); a dynamic one is not resolved.
        if style::can_be(plan, plan.code(b.expr), &|v| v.contains("url(")) {
            return Err(format!(
                "node {i}: a dynamic `{}` that can reference an element (`url(#…)`) is not in the JS target",
                StyleId::from_bit(b.id as u32).map_or("style", |r| r.name())
            ));
        }
        let writes =
            style::style_writes(b.id, press, timeline).map_err(|x| format!("node {i}: {x}"))?;
        let s = self
            .uses
            .rt(if self.is_motion_node(i) { "Sm" } else { "S" });
        for w in writes {
            let (name, unit) = (w.name, w.unit);
            match w.map {
                Some(m) => {
                    let _ = write!(
                        self.out,
                        "{s}({e},\"{name}\",\"{unit}\",()=>({m})(({f})()));"
                    );
                }
                None => {
                    let _ = write!(self.out, "{s}({e},\"{name}\",\"{unit}\",{f});");
                }
            }
        }
        Ok(())
    }

    /// A Markdown text field is the web host's own editor (LLP 1045 D5:
    /// markup-editor.js over its wasm), which replaces it once loaded.
    pub(super) fn editor(&mut self, i: u32, element: &str, e: &str) -> Result<(), String> {
        let plan = self.plan;
        let row = &plan.nodes[i as usize];
        let markup = row
            .bindings
            .iter()
            .map(|b| plan.binding(b))
            .find(|b| b.kind == BindingKind::Prop && b.id == PropId::Markup as u16);
        if element == "textarea" {
            match markup.map(|b| style::literal(plan, plan.code(b.expr))) {
                Some(Some(v)) if v.as_str() == Some("markdown") => {
                    self.editor = true;
                    let mde = self.uses.rt("mde");
                    let _ = write!(self.out, "{mde}({e});");
                }
                Some(None) => {
                    return Err(format!(
                        "node {i}: a dynamic `markup` on a text field is not in the JS target"
                    ))
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Whether node `i` is a `symbol` or inside one (through regions' arms).
    pub(super) fn in_symbol(&self, i: u32) -> bool {
        let plan = self.plan;
        let mut at = Some(i);
        while let Some(n) = at {
            let row = &plan.nodes[n as usize];
            if NodeType::from_wire(row.node_type) == Some(NodeType::SvgSymbol) {
                return true;
            }
            at = match (row.parent, row.arm) {
                (Some(p), _) => Some(p.0),
                (None, Some(a)) => {
                    let mut region = plan.arms[a.0 as usize].region;
                    loop {
                        let r = &plan.regions[region.0 as usize];
                        match (r.parent, r.arm) {
                            (Some(p), _) => break Some(p.0),
                            (None, Some(a)) => region = plan.arms[a.0 as usize].region,
                            (None, None) => break None,
                        }
                    }
                }
                (None, None) => None,
            };
        }
        false
    }
}
