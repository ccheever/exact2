//! `scrollIntoView` (LLP 1070.000): the command an action states, and the
//! agent's `tap … into`, both run here, never by a host.
use super::*;
use crate::instance::collection::{Align, IntoView, IntoViewStatus};

impl IntoView {
    /// The command's arguments as the compiler orders them: the list's id,
    /// the key, then `block`, `inline`, `behavior`, `row` and `offset`, each `none`
    /// where the author left the default.
    pub(super) fn from_command(args: &[Value]) -> Result<IntoView, String> {
        let text = |v: Option<&Value>| v.and_then(Value::as_str).map(str::to_string);
        let align = |v: Option<&Value>, default| match text(v) {
            None => Ok(default),
            Some(name) => Align::parse(&name).ok_or(format!("no alignment {name}")),
        };
        let list = text(args.first()).ok_or("the list is named by its id")?;
        let key = args.get(1).cloned().ok_or("a key")?;
        let offset = match args.get(6) {
            None | Some(Value::Option(None) | Value::Unit) => None,
            Some(Value::Number(n)) if n.is_finite() => Some(*n),
            _ => return Err("offset must be a finite number of CSS pixels".into()),
        };
        Ok(IntoView {
            list,
            key,
            offset,
            block: align(args.get(2), Align::Start)?,
            inline: align(args.get(3), Align::Nearest)?,
            row: args
                .get(5)
                .filter(|v| !matches!(v, Value::Option(None) | Value::Unit))
                .cloned(),
            view: None,
            smooth: text(args.get(4)).as_deref() == Some("smooth"),
        })
    }
}

impl<D: DataSource> Runner<D> {
    /// Materialize a logical row for a host's keyboard focus traversal, using
    /// the same nearest alignment as `scrollIntoView`. The host can inspect
    /// the mounted row, focus a descendant, then send its measurement/pin
    /// feedback. No unmounted records or keys cross the host seam.
    /// `None` means that the list or row no longer exists.
    pub fn reveal_collection_index(
        &mut self,
        view: ViewId,
        index: usize,
    ) -> Result<Option<CommitReceipt>, RunnerError> {
        if self.poisoned {
            return Err(RunnerError::Poisoned);
        }
        let Some(lists) = self.links.lists else {
            return Ok(None);
        };
        let mut found = false;
        let receipt = self.update_tree(false, |tree, u| {
            found = (lists.reveal_index)(tree, u, view, index)?;
            Ok(())
        })?;
        Ok(found.then_some(receipt))
    }

    /// Bring a virtualized list's row into view by key, as the agent's `tap
    /// <list> into <key>` asks (LLP 1070.000 §5): the same request an
    /// action's command makes, committed now. Its outcome is in `state`.
    pub fn scroll_into_view(&mut self, request: IntoView) -> Result<CommitReceipt, RunnerError> {
        if self.poisoned {
            return Err(RunnerError::Poisoned);
        }
        let Some(_) = self.links.lists else {
            let why = "this artifact doesn't link lists".to_string();
            self.record_into_view(&request, Some(IntoViewStatus::Refused(why)));
            return Ok(CommitReceipt::default());
        };
        self.update_into_views(&[request], false)
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
            smooth: false,
            offset: None,
        };
        self.scroll_into_view(request)
    }

    /// `state.scrollIntoView`: each list's latest request, then refusals.
    pub fn into_view_json(&self) -> String {
        let mut out = match (self.links.lists, &self.tree) {
            (Some(lists), Some(tree)) => (lists.into_view_json)(tree),
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

impl<D: DataSource> Runner<D> {
    /// Compute styles between requested scrollers, but publish one logical
    /// commit. In particular, an outer seek can create the inner list whose
    /// scroll-padding the next step needs. Ordinary updates keep the one-phase
    /// path; only explicit requests pay for continuation state.
    pub(super) fn update_into_views(
        &mut self,
        requests: &[IntoView],
        settled: bool,
    ) -> Result<CommitReceipt, RunnerError> {
        let result = self.update_into_views_inner(requests, settled);
        if result.is_err() {
            self.poison();
        }
        result
    }

    fn update_into_views_inner(
        &mut self,
        requests: &[IntoView],
        settled: bool,
    ) -> Result<CommitReceipt, RunnerError> {
        let (mut ops, mut surfaces, _) =
            self.update_ops(
                settled,
                |tree, u| {
                    if settled {
                        tree.update(u)
                    } else {
                        Ok(())
                    }
                },
            )?;
        if !self.reorder_ops.is_empty() {
            let mut prefix = std::mem::take(&mut self.reorder_ops);
            prefix.append(&mut ops);
            ops = prefix;
        }
        let change = self.router_change()?;
        self.batch += 1;
        let language = self.resolved_locale().to_owned();
        let direction = if self.direction() == "rtl" {
            exact_kernel::Direction::Rtl
        } else {
            exact_kernel::Direction::Ltr
        };
        self.kernel
            .begin_document_commit(0, self.batch, &ops, Some((&language, direction)))?;
        let mut effects = CommitReceipt::default();
        self.settle_into_view_phase(&mut effects);
        let mut notes = std::mem::take(&mut self.notes);
        let into_view = self.links.lists.map(|lists| lists.into_view);
        let mut statuses = Vec::with_capacity(requests.len());
        if !settled {
            self.tree.as_mut().expect("booted").last_work = Default::default();
        }
        for request in requests {
            let mut step = Some(request.clone());
            let mut status = IntoViewStatus::Pending;
            // At most the outer and inner scroller of this request, in order.
            // This is a continuation of the command, not a convergence loop.
            while let Some(request) = step.take() {
                let (phase, writes, work) = self.update_ops(false, |tree, u| {
                    status = match into_view {
                        Some(run) => run(tree, u, &request, &mut step)?,
                        None => IntoViewStatus::Refused("this artifact doesn't link lists".into()),
                    };
                    Ok(())
                })?;
                self.kernel.continue_document_commit(&phase)?;
                self.settle_into_view_phase(&mut effects);
                notes.extend(std::mem::take(&mut self.notes));
                surfaces.extend(writes);
                ops.extend(phase);
                let total = &mut self.tree.as_mut().expect("booted").last_work;
                total.nodes_visited += work.nodes_visited;
                total.rows_keyed += work.rows_keyed;
                total.rows_reused += work.rows_reused;
                total.regions_skipped += work.regions_skipped;
                total.bindings_evaluated += work.bindings_evaluated;
                total.rows_scanned += work.rows_scanned;
                total.derives_evaluated += work.derives_evaluated;
                total.store_bytes_copied += work.store_bytes_copied;
                total.rows_rebound += work.rows_rebound;
            }
            statuses.push(status);
        }
        let cleanup = self.reconcile_reorder()?;
        if !cleanup.is_empty() {
            self.kernel.continue_document_commit(&cleanup)?;
            ops.extend(cleanup);
        }
        let mut receipt = self.kernel.finish_document_commit()?;
        let live_unique = |keys: &mut Vec<exact_kernel::NodeKey>| {
            keys.retain(|key| self.kernel.node_by_key(*key).is_some());
            keys.sort_unstable_by_key(|key| (key.index, key.generation));
            keys.dedup();
        };
        live_unique(&mut effects.renewed);
        live_unique(&mut effects.revealed);
        receipt.renewed = effects.renewed;
        receipt.revealed = effects.revealed;
        self.tally(&ops, &receipt.touched);
        for note in notes {
            self.log(note);
        }
        if !self.kernel.is_detached() && self.ids.remembered() > 2 * self.kernel.live_count() + 256
        {
            let kernel = &self.kernel;
            self.ids.retain(|view| kernel.node(view).is_some());
        }
        self.commit_router(change);
        self.retire_holds();
        self.publish_surfaces(surfaces);
        if settled && !self.held_edges.is_empty() {
            self.release_held_edges()?;
        }
        for (request, status) in requests.iter().zip(statuses) {
            self.record_into_view(request, Some(status));
        }
        Ok(receipt)
    }

    /// Apply awaiting/revealing in phase order: a row awaited by settlement
    /// and revealed by a seek must finish revealed. Only live generations are
    /// published when the logical commit finishes.
    fn settle_into_view_phase(&mut self, effects: &mut CommitReceipt) {
        let shown = std::mem::take(&mut self.shown);
        let renewed = self.settle_shown(shown, effects);
        effects.renewed.extend(
            renewed
                .into_iter()
                .filter_map(|view| self.kernel.arena().key_of(view)),
        );
    }
}
