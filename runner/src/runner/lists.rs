//! Logical text and key lookups over virtualized lists. LLP 1010 §6.
use super::*;
use crate::instance::collection::IntoViewStatus;

/// A text position that survives retirement of a list row's native views.
#[derive(Debug, Clone, Copy)]
pub struct ListTextPosition<'a> {
    /// The opaque key published on the row wrapper.
    pub key: &'a str,
    /// Zero-based paragraph within the row.
    pub paragraph: usize,
    /// UTF-16 offset within that paragraph, as on the DOM and Apple text APIs.
    pub offset: usize,
}

impl<D: DataSource> Runner<D> {
    /// Resolve an opaque row key without materializing that row.
    pub fn list_index(&self, view: ViewId, key: &str) -> Option<usize> {
        self.tree.as_ref()?.list_index(view, key)
    }

    /// Copy all logical text, or the range between two stable endpoints.
    /// At most one unmounted row is realized at a time; its operations are
    /// never committed. No data query, clock advance, or native view is created.
    pub fn list_text(
        &self,
        view: ViewId,
        range: Option<(ListTextPosition<'_>, ListTextPosition<'_>)>,
    ) -> Result<String, RunnerError> {
        if self.poisoned {
            return Err(RunnerError::Poisoned);
        }
        let mut ids = Ids::default();
        let mut u = Update::new(self.env(&[], &[]), &self.sites, &mut ids);
        Ok(self
            .tree
            .as_ref()
            .expect("booted")
            .list_text(&mut u, view, range)?)
    }
    /// Re-evaluate every site and apply one batch. A failure here means the
    /// instance tree and the kernel may disagree; the runner is poisoned and
    /// the host restarts it — never a half-applied frame.
    pub(super) fn update(&mut self) -> Result<CommitReceipt, RunnerError> {
        let requests = std::mem::take(&mut self.into_view);
        let into_view = self.links.lists.map(|lists| lists.into_view);
        let mut statuses = Vec::new();
        let receipt = self.update_tree(true, |tree, u| {
            tree.update(u)?;
            for request in &requests {
                statuses.push(match into_view {
                    Some(into_view) => into_view(tree, u, request)?,
                    None => IntoViewStatus::Refused("this artifact doesn't link lists".into()),
                });
            }
            Ok(())
        })?;
        for (request, status) in requests.iter().zip(statuses) {
            self.record_into_view(request, Some(status));
        }
        self.release_compiled();
        Ok(receipt)
    }

    pub(super) fn update_tree(
        &mut self,
        settled: bool,
        update: impl FnOnce(&mut Tree, &mut Update<'_>) -> Result<(), crate::instance::InstanceError>,
    ) -> Result<CommitReceipt, RunnerError> {
        let mut tree = self.tree.take().expect("booted");
        let mut ids = std::mem::take(&mut self.ids);
        let rows = std::mem::take(&mut self.row_writes);
        let result = {
            let mut u = Update::new(self.env(&[], &[]), &self.sites, &mut ids);
            u.full = self.full;
            u.discard = self.kernel.is_detached();
            u.rows = rows;
            u.reuse = self.reuse;
            update(&mut tree, &mut u).map(|_| (u.ops, u.surfaces, u.notes, u.renewed, u.shown))
        };
        self.ids = ids;
        self.tree = Some(tree);
        let (ops, surfaces) = match result {
            Ok((ops, surfaces, notes, renewed, shown)) => {
                self.notes = notes;
                self.renewed = renewed;
                self.shown.extend(shown);
                if settled {
                    let tree = self.tree.as_mut().expect("booted");
                    tree.last_work.rows_scanned += self.lookup_rows.take();
                    tree.last_work.derives_evaluated = self.derives_evaluated;
                    tree.last_work.store_bytes_copied =
                        self.store.copied_bytes() - self.copied_at_checkpoint;
                }
                (ops, surfaces)
            }
            Err(e) => {
                if settled || e != crate::instance::InstanceError::InvalidCollectionFeedback {
                    self.poison();
                }
                return Err(e.into());
            }
        };
        // Only ordinary settlement wakes deferred collection edges; geometry-only
        // feedback must not replenish this work.
        if settled {
            if let Err(error) = self.wake_deferred_edges() {
                self.poison();
                return Err(error);
            }
        }
        match self.apply(ops) {
            Ok(receipt) => {
                self.publish_surfaces(surfaces);
                Ok(receipt)
            }
            Err(e) => {
                self.poison();
                Err(e)
            }
        }
    }
}
