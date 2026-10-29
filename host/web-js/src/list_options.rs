//! A virtualized list's options on the JS target, list.js's `vl`'s `o`.

use super::{style, value_js, Em};
use crate::code::{self, Scope};
use exact_kernel::{PropId, StyleId};
use exact_plan::{BindingKind, NodesId};

impl Em<'_> {
    /// What the runner reads when a virtualized list is created
    /// (`Collection::create`), evaluated there: its axis, its literal sizes,
    /// its row estimate, `scrollFollowEnd` (read again at each update),
    /// `scroll-start`, `scroll-restoration`, `reach-margin`, `keep-mounted`
    /// (read again at each update), its edges' handlers, and its plan site.
    pub(super) fn list_options(
        &mut self,
        i: u32,
        scope: &Scope,
        edges: &[String; 2],
    ) -> Result<String, String> {
        let plan = self.plan;
        let (mut ph, mut pw) = (Vec::new(), Vec::new());
        let (mut x, mut est, mut follow, mut manual) = (
            "!1".to_string(),
            "void 0".to_string(),
            "0".to_string(),
            "!1".to_string(),
        );
        let mut init = "void 0".to_string();
        let mut at_end = "!1".to_string();
        let mut reach = "void 0".to_string();
        let mut keep = "void 0".to_string();
        let bindings = plan.node(NodesId(i)).bindings;
        for b in bindings.iter().map(|b| plan.binding(b)) {
            let value = match style::literal(plan, plan.code(b.expr)) {
                Some(v) => value_js(&v),
                None => code::expression(plan, plan.code(b.expr), scope, &mut self.uses)
                    .map_err(|x| format!("node {i}: {x}"))?,
            };
            match b.kind {
                BindingKind::Style
                    if b.id == StyleId::Height as u16 || b.id == StyleId::MaxHeight as u16 =>
                {
                    ph.push(value)
                }
                BindingKind::Style
                    if b.id == StyleId::Width as u16 || b.id == StyleId::MaxWidth as u16 =>
                {
                    pw.push(value)
                }
                BindingKind::Style if b.id == StyleId::Display as u16 => {
                    x = format!("({value})===\"flex\"")
                }
                BindingKind::Prop
                    if b.id == PropId::EstimatedItemHeight as u16
                        || b.id == PropId::EstimatedItemWidth as u16 =>
                {
                    est = value
                }
                BindingKind::Prop if b.id == PropId::ScrollFollowEnd as u16 => {
                    follow = format!("()=>{value}")
                }
                BindingKind::Prop if b.id == PropId::InitialItemCount as u16 => init = value,
                BindingKind::Prop if b.id == PropId::ScrollStart as u16 => {
                    at_end = format!("({value})===\"end\"")
                }
                // @ref LLP 1145 D4 — a literal; list.js reads it as the runner does.
                BindingKind::Prop if b.id == PropId::ReachMargin as u16 => reach = value,
                // @ref LLP 1010 §6.11 — the keys of the rows it keeps mounted.
                BindingKind::Prop if b.id == PropId::KeepMounted as u16 => {
                    keep = format!("()=>{value}")
                }
                BindingKind::Prop if b.id == PropId::ScrollRestoration as u16 => {
                    manual = format!("({value})===\"manual\"")
                }
                _ => {}
            }
        }
        Ok(format!(
            "{{x:{x},ph:[{}],pw:[{}],est:{est},init:{init},atEnd:{at_end},follow:{follow},manual:{manual},reach:{reach},keep:{keep},start:{},end:{},site:\"{i}\"}}",
            ph.join(","),
            pw.join(","),
            edges[0],
            edges[1]
        ))
    }
}
