//! Portable viewport feedback and runner-owned geometric edge events.
use super::*;
use crate::compare::{equivalent, equivalent_all};
use crate::instance::collection::{CollectionFeedback, CollectionSnapshot};

impl<D: DataSource> Runner<D> {
    /// Mounted collection metadata for a host's post-commit layout/measurement.
    pub fn collections(&self) -> Vec<CollectionSnapshot> {
        self.tree
            .as_ref()
            .map(Tree::collections)
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
    /// Compact numeric metadata for existing JSON batch envelopes, without serde.
    pub fn collections_json(&self) -> String {
        crate::instance::collection::snapshots_json(&self.collections())
    }
    /// Decode the common numeric LE protocol before touching state.
    pub fn collection_feedback_bytes(&mut self, bytes: &[u8]) -> Result<Advanced, RunnerError> {
        let feedback = CollectionFeedback::decode(bytes).map_err(|_| {
            RunnerError::Instance(InstanceError::Collection(
                "malformed collection feedback".into(),
            ))
        })?;
        self.collection_feedback(feedback)
    }
    /// Update the addressed window and release transferred focus/interaction pins
    /// in other collections in the same commit, then dispatch each edge at most
    /// once. Start precedes end; any action state change defers end until settled.
    /// Pre-commit errors return Err; an edge refusal accompanies the
    /// committed receipts in Advanced.error. Hosts must consume both. No timers
    /// advance. Without an edge handler, no resources or keys are evaluated.
    pub fn collection_feedback(
        &mut self,
        feedback: CollectionFeedback,
    ) -> Result<Advanced, RunnerError> {
        if self.poisoned {
            return Err(RunnerError::Poisoned);
        }
        feedback.validate().map_err(|_| {
            RunnerError::Instance(InstanceError::Collection(
                "invalid collection feedback".into(),
            ))
        })?;
        let view = feedback.view;
        let mut tree = self.tree.take().expect("booted");
        let mut ids = std::mem::take(&mut self.ids);
        let result = {
            let mut update = Update::new(self.env(&[], &[]), &self.sites, &mut ids);
            tree.update_collection(&mut update, feedback)
                .map(|changed| (changed, update.ops, update.surfaces))
        };
        self.tree = Some(tree);
        self.ids = ids;
        let ((changed, edge), ops, surfaces) = match result {
            Ok(result) => result,
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
        if changed || !ops.is_empty() {
            match self.apply(ops) {
                Ok(receipt) => {
                    self.surfaces.extend(surfaces);
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
                        !equivalent_all(&a.args, &b.args) || !equivalent(&a.value, &b.value)
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
