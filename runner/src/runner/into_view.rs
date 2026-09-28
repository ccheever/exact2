//! `scrollIntoView` (LLP 1070.000): the command an action states, and the
//! agent's `tap … into`, both run here, never by a host.
use super::*;
use crate::instance::collection::{Align, IntoView, IntoViewStatus};

impl IntoView {
    /// The command's arguments as the compiler orders them: the list's id,
    /// the key, then `block`, `inline`, `behavior` and `row`, each `none`
    /// where the author left the default.
    pub(super) fn from_command(args: &[Value]) -> Result<IntoView, String> {
        let text = |v: Option<&Value>| match v {
            Some(Value::Str(s)) => Some(s.to_string()),
            _ => None,
        };
        let align = |v: Option<&Value>, default| match text(v) {
            None => Ok(default),
            Some(name) => Align::parse(&name).ok_or(format!("no alignment {name}")),
        };
        let list = text(args.first()).ok_or("the list is named by its id")?;
        let key = args.get(1).cloned().ok_or("a key")?;
        Ok(IntoView {
            list,
            key,
            block: align(args.get(2), Align::Start)?,
            inline: align(args.get(3), Align::Nearest)?,
            row: args
                .get(5)
                .filter(|v| !matches!(v, Value::Option(None) | Value::Unit))
                .cloned(),
            view: None,
        })
    }
}

impl<D: DataSource> Runner<D> {
    /// Bring a virtualized list's row into view by key, as the agent's `tap
    /// <list> into <key>` asks (LLP 1070.000 §5): the same request an
    /// action's command makes, committed now. Its outcome is in `state`.
    pub fn scroll_into_view(&mut self, request: IntoView) -> Result<CommitReceipt, RunnerError> {
        if self.poisoned {
            return Err(RunnerError::Poisoned);
        }
        let mut status = None;
        let receipt = self.update_tree(false, |tree, u| {
            status = Some(tree.scroll_into_view(u, &request)?);
            Ok(())
        })?;
        self.record_into_view(&request, status);
        Ok(receipt)
    }

    /// A refusal is journaled and kept for `state` (a request that started
    /// is its list's to report).
    pub(super) fn record_into_view(&mut self, request: &IntoView, status: Option<IntoViewStatus>) {
        if let Some(IntoViewStatus::Refused(why)) = status {
            self.log(format!("scrollIntoView {} refused: {why}", request.list));
            let mut entry = String::from("{\"list\":");
            crate::agent::quote(&request.list, &mut entry);
            entry.push_str(",\"status\":");
            IntoViewStatus::Refused(why).json(&mut entry);
            entry.push('}');
            self.into_view_refused.push_back(entry);
            if self.into_view_refused.len() > 8 {
                self.into_view_refused.pop_front();
            }
        }
    }

    /// The agent's `tap <list> into <key>` (LLP 1070.000 §5): the mounted
    /// list by its view, the key as text, alignments by CSS's names.
    pub fn scroll_into_view_at(
        &mut self,
        view: ViewId,
        key: &str,
        block: &str,
        inline: &str,
    ) -> Result<CommitReceipt, RunnerError> {
        let bad = |name: &str| {
            RunnerError::Instance(crate::instance::InstanceError::Collection(format!(
                "no alignment {name}: start, center, end or nearest"
            )))
        };
        let request = IntoView {
            list: format!("#{view}"),
            key: Value::str(key),
            block: Align::parse(block).ok_or_else(|| bad(block))?,
            inline: Align::parse(inline).ok_or_else(|| bad(inline))?,
            row: None,
            view: Some(view),
        };
        self.scroll_into_view(request)
    }

    /// `state.scrollIntoView`: each list's latest request, then refusals.
    pub fn into_view_json(&self) -> String {
        let mut out = match (self.links.lists, &self.tree) {
            (Some(_), Some(tree)) => tree.into_view_json(),
            _ => "[]".to_string(),
        };
        out.pop();
        for entry in &self.into_view_refused {
            if out.len() > 1 {
                out.push(',');
            }
            out.push_str(entry);
        }
        out.push(']');
        out
    }
}
