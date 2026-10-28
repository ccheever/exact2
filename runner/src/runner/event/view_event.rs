//! A package view's declared event (LLP 1133.021 §9 item 2): the
//! implementation sends a message whose text is `{"exact:event": name,
//! "args": [...]}`, and the lowering made each `name=action` a `message`
//! handler whose one bound argument is `exact:event:<name>`
//! (`contract_lower`'s `native_typed.rs`). The message runs the handler
//! that names it, with its arguments decoded by the action's parameter
//! types; a message that names no declared event runs the node's plain
//! `message` handler, if it has one.

use super::super::{DataSource, Runner, RunnerError};
use crate::vm::Frame;
use exact_kernel::CommitReceipt;
use exact_plan::{EventKind, HandlersRow, NodesId, Value};
use serde_json::Value as Json;

impl<D: DataSource> Runner<D> {
    /// `None` where node `node` has no declared event's handler, for the
    /// ordinary dispatch to take the message.
    pub(super) fn view_event(
        &mut self,
        node: NodesId,
        text: &str,
        frames: &[Frame],
        what: &mut String,
    ) -> Option<Result<CommitReceipt, RunnerError>> {
        let marker = "exact:event:";
        let mut marked: Vec<(String, HandlersRow)> = Vec::new();
        let mut plain = None;
        for h in self
            .plan
            .node(node)
            .handlers
            .iter()
            .map(|h| self.plan.handler(h).clone())
        {
            if h.event != EventKind::Message {
                continue;
            }
            let first = h.args.iter().next().map(|a| self.plan.arg(a).expr);
            let named = first
                .and_then(|code| self.eval(code, &[], frames).ok())
                .and_then(|v| {
                    v.as_str()
                        .and_then(|s| s.strip_prefix(marker))
                        .map(str::to_owned)
                });
            match named {
                Some(event) => marked.push((event, h)),
                None if plain.is_none() => plain = Some(h),
                None => {}
            }
        }
        if marked.is_empty() {
            return None;
        }
        let sent: Json = serde_json::from_str(text).unwrap_or_default();
        let Some(event) = sent["exact:event"].as_str() else {
            let h = plain?;
            let mut args = Vec::new();
            for a in h.args.iter() {
                match self.eval(self.plan.arg(a).expr, &[], frames) {
                    Ok(v) => args.push(v),
                    Err(e) => return Some(Err(e)),
                }
            }
            args.push(Value::str(text));
            return Some(self.run_action(h.action, args, frames));
        };
        let Some((_, h)) = marked.into_iter().find(|(e, _)| e == event) else {
            return Some(Ok(CommitReceipt::default()));
        };
        let action = self.plan.action(h.action).clone();
        let mut args = Vec::new();
        for (i, p) in action.params.iter().enumerate() {
            let ty = self.plan.param(p).ty;
            let json = sent["args"].get(i).cloned().unwrap_or(Json::Null);
            match self.event_arg(ty, &json) {
                Ok(v) => args.push(v),
                Err(why) => {
                    self.log(format!("{event}: refused: {why}"));
                    return Some(Ok(CommitReceipt::default()));
                }
            }
        }
        super::super::lines::ran(what, self.plan.str(action.name));
        Some(self.run_action(h.action, args, frames))
    }
}
