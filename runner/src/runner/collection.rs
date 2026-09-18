//! Portable viewport feedback and runner-owned geometric edge events.
use super::*;
use crate::instance::collection::{CollectionFeedback, CollectionSnapshot};

impl<D: DataSource> Runner<D> {
    /// Mounted collection metadata for a host's post-commit layout/measurement.
    pub fn collections(&self) -> Vec<CollectionSnapshot> {
        self.tree
            .as_ref()
            .map(Tree::collections)
            .unwrap_or_default()
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
    /// once. Start precedes end; a membership change defers end to fresh feedback.
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
            let mut update = Update {
                env: self.env(&[], &[]),
                ids: &mut ids,
                ops: Vec::new(),
                surfaces: Vec::new(),
                work: Default::default(),
            };
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
            for (position, event) in [edges.first, EventKind::Reachend].into_iter().enumerate() {
                if position == 1
                    && !edges.end_if_unchanged.as_ref().is_some_and(|membership| {
                        self.tree
                            .as_mut()
                            .expect("booted")
                            .take_collection_end(view, membership)
                    })
                {
                    break;
                }
                match self.dispatch_edge(view, event) {
                    Ok(receipt) => result.receipts.push(Timed {
                        at_ms: self.now_ms,
                        receipt,
                    }),
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
}
