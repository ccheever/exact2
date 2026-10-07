//! Portable viewport feedback and runner-owned geometric edge events.
use super::*;
use crate::compare::{equivalent, equivalent_all};
use crate::instance::collection::{CollectionFeedback, CollectionFill, CollectionSnapshot};

impl<D: DataSource> Runner<D> {
    /// Mounted collection metadata for a host's post-commit layout/measurement.
    pub fn collections(&self) -> Vec<CollectionSnapshot> {
        self.tree
            .as_ref()
            .map(Tree::collections)
            .unwrap_or_default()
    }
    /// Each mounted list's view, data generation and axis ([`Tree::collection_data`]).
    pub fn collection_data(&self) -> Vec<(ViewId, u64, bool)> {
        self.tree
            .as_ref()
            .map(Tree::collection_data)
            .unwrap_or_default()
    }
    /// One list's entry of [`Runner::collections`].
    pub fn collection(&self, view: ViewId) -> Option<CollectionSnapshot> {
        self.tree.as_ref().and_then(|tree| tree.collection(view))
    }
    /// [`Tree::collection_mounted`].
    pub fn collection_mounted(&self, view: ViewId, out: &mut Vec<(ViewId, u64)>) {
        out.clear();
        if let Some(tree) = &self.tree {
            tree.collection_mounted(view, out);
        }
    }
    /// [`Runner::collections`] with only each list's first mounted row
    /// ([`Tree::collections_shallow`]).
    pub fn collections_shallow(&self) -> Vec<CollectionSnapshot> {
        self.tree
            .as_ref()
            .map(Tree::collections_shallow)
            .unwrap_or_default()
    }
    /// Bound borrowed traversal and count all collections/rows before copying
    /// numeric host snapshots. No keys, records or action frames are captured.
    pub fn collections_bounded(
        &self,
        max_collections: usize,
        max_rows: usize,
        max_traversal: usize,
        max_json_bytes: usize,
    ) -> Result<Vec<CollectionSnapshot>, &'static str> {
        match &self.tree {
            Some(tree) => {
                tree.collections_bounded(max_collections, max_rows, max_traversal, max_json_bytes)
            }
            None => Ok(Vec::new()),
        }
    }
    /// Compact numeric metadata for existing JSON batch envelopes, without
    /// serde: `[]` where the artifact doesn't link lists (LLP 1047.000 §9).
    pub fn collections_json(&self) -> String {
        match (self.links.lists, &self.tree) {
            (Some(lists), Some(tree)) => (lists.collections_json)(tree),
            _ => "[]".to_string(),
        }
    }
    /// Every outer list's kept inner positions (LLP 1070 §4.2), for `state`.
    pub fn kept_positions_json(&self) -> String {
        match (self.links.lists, &self.tree) {
            (Some(lists), Some(tree)) => (lists.kept_json)(tree),
            _ => "[]".to_string(),
        }
    }
    /// Decode the common numeric LE protocol before touching state.
    pub fn collection_feedback_bytes(&mut self, bytes: &[u8]) -> Result<Advanced, RunnerError> {
        let (feedback, fill) = CollectionFeedback::decode_with_fill(bytes).map_err(|_| {
            RunnerError::Instance(InstanceError::Collection(
                "malformed collection feedback".into(),
            ))
        })?;
        self.collection_feedback_filled(feedback, fill)
    }
    /// [`Runner::collection_feedback_filled`] with no limit and no motion.
    pub fn collection_feedback(
        &mut self,
        feedback: CollectionFeedback,
    ) -> Result<Advanced, RunnerError> {
        self.collection_feedback_filled(feedback, CollectionFill::default())
    }
    /// Update the addressed window and release transferred focus/interaction pins
    /// in other collections in the same commit, then dispatch each edge at most
    /// once. Start precedes end; any action state change defers end until settled.
    /// Pre-commit errors return Err; an edge refusal accompanies the
    /// committed receipts in Advanced.error. Hosts must consume both. No timers
    /// advance. Without an edge handler, no resources or keys are evaluated.
    pub fn collection_feedback_filled(
        &mut self,
        feedback: CollectionFeedback,
        fill: CollectionFill,
    ) -> Result<Advanced, RunnerError> {
        if self.poisoned {
            return Err(RunnerError::Poisoned);
        }
        if feedback.validate().is_err() || !fill.velocity.is_finite() {
            return Err(RunnerError::Instance(InstanceError::Collection(
                "invalid collection feedback".into(),
            )));
        }
        let view = feedback.view;
        let mut tree = self.tree.take().expect("booted");
        // @ref LLP 1010 §6.9 — the list's end padding as its layout resolved
        // it (`env()`, a host's cover and a computed value included).
        let padding = self
            .kernel
            .node(view)
            .and_then(|node| self.kernel.resolved_padding(node.key))
            .map_or([0.0; 2], |(_, _, right, bottom)| {
                [bottom as f64, right as f64]
            });
        tree.set_collection_end_padding(view, padding);
        let mut ids = std::mem::take(&mut self.ids);
        let result = {
            let mut update = Update::new(self.env(&[], &[]), &self.sites, &mut ids);
            update.reuse = self.reuse;
            tree.update_collection(&mut update, feedback, fill)
                .map(|changed| {
                    (
                        changed,
                        update.ops,
                        update.surfaces,
                        update.notes,
                        update.renewed,
                        update.shown,
                    )
                })
        };
        self.tree = Some(tree);
        self.ids = ids;
        let ((changed, edge), ops, surfaces) = match result {
            Ok((changed, ops, surfaces, notes, renewed, shown)) => {
                self.notes = notes;
                self.renewed = renewed;
                self.shown.extend(shown);
                (changed, ops, surfaces)
            }
            Err(error) => {
                // This category is emitted only by pre-mutation geometry
                // validation. A bad host report must leave the runner usable.
                if !matches!(error, InstanceError::InvalidCollectionFeedback) {
                    self.poison();
                }
                return Err(error.into());
            }
        };
        let mut result = Advanced {
            receipts: Vec::new(),
            now_ms: self.now_ms,
            error: None,
        };
        // A row that showed with animations waiting on it is a commit of its
        // own when the report changed nothing else (LLP 1055 D13).
        let revealed = self
            .shown
            .revealed
            .iter()
            .any(|v| self.kernel.is_awaiting(*v));
        if changed || !ops.is_empty() || revealed {
            match self.apply(ops) {
                Ok(receipt) => {
                    self.publish_surfaces(surfaces);
                    result.receipts.push(Timed {
                        at_ms: self.now_ms,
                        receipt,
                    });
                }
                Err(error) => {
                    self.poison();
                    return Err(error);
                }
            }
        }
        // @ref LLP 1010 — a list on a route its stack keeps covered (a deep
        // link's root, a feed under a thread) is hidden and inert: its edge
        // waits, armed, for the route to show, so nothing is asked for a
        // screen the reader has not seen (`release_held_edges`).
        let edge = match edge {
            Some(edges) if self.inactive(view) => {
                let tree = self.tree.as_mut().expect("booted");
                tree.rearm_collection_edge(view, edges.first);
                if !self.held_edges.contains(&view) {
                    self.held_edges.push(view);
                    let name = if edges.first == EventKind::Reachstart {
                        "reachstart"
                    } else {
                        "reachend"
                    };
                    self.log(format!(
                        "{name} view {view} waits: its list is on a covered route; it is offered when the route shows"
                    ));
                }
                None
            }
            edge => edge,
        };
        if let Some(edges) = edge {
            let mut end_after_noop = edges.end_after_noop;
            for (position, event) in [edges.first, EventKind::Reachend].into_iter().enumerate() {
                if position == 1
                    && (!end_after_noop
                        || !self
                            .tree
                            .as_mut()
                            .expect("booted")
                            .take_collection_end(view))
                {
                    break;
                }
                // Geometry can keep arriving while an async answer is retained.
                // Do not let that old answer supersede the first edge's request.
                if event == EventKind::Reachend
                    && self.deferred_edges.iter().any(|(v, _)| *v == view)
                {
                    self.tree
                        .as_mut()
                        .expect("booted")
                        .rearm_collection_edge(view, event);
                    break;
                }
                let first_ticket = self.next_ticket;
                match self.dispatch_edge(view, event) {
                    Ok((receipt, changed)) => {
                        result.receipts.push(Timed {
                            at_ms: self.now_ms,
                            receipt,
                        });
                        if position == 0 && edges.end_after_noop && changed {
                            end_after_noop = false;
                            let targets = self
                                .pending
                                .iter()
                                .filter(|p| p.ticket >= first_ticket)
                                .map(|p| p.target)
                                .collect();
                            self.deferred_edges.retain(|(v, _)| *v != view);
                            self.deferred_edges.push((view, targets));
                            if let Err(error) = self.wake_deferred_edges() {
                                self.poison();
                                result.error = Some(error);
                                break;
                            }
                        }
                    }
                    Err(error) => {
                        if let Some(tree) = self.tree.as_mut() {
                            tree.rearm_collection_edge(view, event);
                        }
                        result.error = Some(error);
                        break;
                    }
                }
            }
        }
        Ok(result)
    }
    /// After an ordinary commit: a list whose edge waited under a covered
    /// route asks its host for a report once the route shows, which offers
    /// the edge it reaches then.
    pub(super) fn release_held_edges(&mut self) -> Result<(), RunnerError> {
        for view in std::mem::take(&mut self.held_edges) {
            if !self.tree.as_ref().expect("booted").has_collection(view) {
                continue;
            }
            if self.inactive(view) {
                self.held_edges.push(view);
            } else {
                self.tree
                    .as_mut()
                    .expect("booted")
                    .wake_collection_edge(view)?;
            }
        }
        Ok(())
    }
    /// Called once per ordinary commit, after settlement. Follow targets across
    /// continuation tickets, discard unmounted owners, and wake each ready edge
    /// once. Feedback-only commits never replenish this work.
    pub(super) fn wake_deferred_edges(&mut self) -> Result<(), RunnerError> {
        let tree = self.tree.as_mut().expect("booted");
        for (view, targets) in std::mem::take(&mut self.deferred_edges) {
            if !tree.has_collection(view) {
                continue;
            }
            if self.pending.iter().any(|p| targets.contains(&p.target)) {
                self.deferred_edges.push((view, targets));
            } else {
                tree.wake_collection_edge(view)?;
            }
        }
        Ok(())
    }
}

/// Capture immutable state around an edge action, never around offset-only
/// feedback. Shared values compare by identity first, so a no-op never scans
/// the contents of a resident answer. New values still compare semantically.
pub(super) struct EdgeState {
    slots: Vec<Value>,
    resources: Vec<Option<ResourceState>>,
    rows: Vec<(RowSlots, std::collections::BTreeMap<u32, Value>)>,
    pending: Vec<(Target, u64)>,
    ticket: u64,
    store: u64,
    commands: usize,
}
impl EdgeState {
    pub(super) fn capture<D: DataSource>(r: &Runner<D>, frames: &[Frame]) -> Self {
        Self {
            slots: r.slots.clone(),
            resources: r.resources.clone(),
            rows: frames
                .iter()
                .filter_map(|f| f.row.as_ref())
                .map(|row| (row.clone(), row.borrow().clone()))
                .collect(),
            pending: r.pending.iter().map(|p| (p.target, p.ticket)).collect(),
            ticket: r.next_ticket,
            store: r.store.revision(),
            commands: r.commands.len(),
        }
    }
    pub(super) fn changed<D: DataSource>(&self, r: &Runner<D>) -> bool {
        self.ticket != r.next_ticket
            || self.store != r.store.revision()
            || self.commands != r.commands.len()
            || !self
                .pending
                .iter()
                .copied()
                .eq(r.pending.iter().map(|p| (p.target, p.ticket)))
            || !equivalent_all(&self.slots, &r.slots)
            || self
                .resources
                .iter()
                .zip(&r.resources)
                .any(|(a, b)| match (a, b) {
                    (None, None) => false,
                    (Some(a), Some(b)) => {
                        !equivalent_all(&a.args, &b.args)
                            || !crate::held::Held::equivalent(&a.value, &b.value)
                    }
                    _ => true,
                })
            || self.rows.iter().any(|(row, old)| {
                let now = row.borrow();
                old.len() != now.len()
                    || old
                        .iter()
                        .zip(now.iter())
                        .any(|((ak, av), (bk, bv))| ak != bk || !equivalent(av, bv))
            })
    }
}

impl<D: DataSource> Runner<D> {
    /// After a batch applies (LLP 1055 D13): the views it renewed, taken.
    pub(super) fn settle_shown(
        &mut self,
        shown: crate::instance::collection::shown::RowsShown,
        receipt: &mut exact_kernel::CommitReceipt,
    ) -> Vec<ViewId> {
        // Rows that showed stop holding their animations, and rows mounted
        // out of their port start to, before a host hears the commit.
        let renewed = std::mem::take(&mut self.renewed);
        for view in shown.revealed {
            // A row bound again where it shows is heard as new (`renewed`),
            // which starts it: only a row that stayed is named revealed.
            if self.kernel.reveal(view) && !renewed.contains(&view) {
                receipt.revealed.extend(self.kernel.arena().key_of(view));
            }
        }
        for view in shown.awaiting {
            self.kernel.await_view(view);
        }
        renewed
    }
}
