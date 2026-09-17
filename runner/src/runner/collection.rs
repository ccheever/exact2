//! One portable viewport feedback path; no data/resource settlement on scrolling.
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
    pub fn collection_feedback_bytes(
        &mut self,
        bytes: &[u8],
    ) -> Result<Option<CommitReceipt>, RunnerError> {
        let feedback = CollectionFeedback::decode(bytes).map_err(|_| {
            RunnerError::Instance(InstanceError::Collection(
                "malformed collection feedback".into(),
            ))
        })?;
        self.collection_feedback(feedback)
    }
    /// Update the addressed window and release transferred focus/interaction pins
    /// in other collections in the same commit. Stale feedback is ignored; no
    /// query, resource settlement, or all-N key validation runs here.
    pub fn collection_feedback(
        &mut self,
        feedback: CollectionFeedback,
    ) -> Result<Option<CommitReceipt>, RunnerError> {
        if self.poisoned {
            return Err(RunnerError::Poisoned);
        }
        feedback.validate().map_err(|_| {
            RunnerError::Instance(InstanceError::Collection(
                "invalid collection feedback".into(),
            ))
        })?;
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
        let (changed, ops, surfaces) = match result {
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
        if !changed && ops.is_empty() {
            return Ok(None);
        }
        match self.apply(ops) {
            Ok(receipt) => {
                self.surfaces.extend(surfaces);
                Ok(Some(receipt))
            }
            Err(error) => {
                self.poison();
                Err(error)
            }
        }
    }
}
