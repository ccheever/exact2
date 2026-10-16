//! Logical text and key lookups over virtualized lists. LLP 1010 §6.
use super::*;

/// The host's paragraph and `user-select: auto` rules for logical copies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListTextPolicy {
    /// DOM paragraphs, with the host stylesheet's unselectable button labels.
    Web,
    /// AppKit labels/chrome are unselectable; native titles have no paragraph.
    MacOS,
}

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
        policy: ListTextPolicy,
    ) -> Result<String, RunnerError> {
        if self.poisoned {
            return Err(RunnerError::Poisoned);
        }
        let mut ids = Ids::default();
        let mut u = Update::new(self.env(&[], &[]), &self.sites, &mut ids);
        u.fonts = Some(self.fonts.clone());
        Ok(self
            .tree
            .as_ref()
            .expect("booted")
            .list_text(&mut u, view, range, policy)?)
    }
    /// Re-evaluate every site and apply one batch. A failure here means the
    /// instance tree and the kernel may disagree; the runner is poisoned and
    /// the host restarts it — never a half-applied frame.
    pub(super) fn update(&mut self) -> Result<CommitReceipt, RunnerError> {
        let requests = std::mem::take(&mut self.into_view);
        let receipt = if requests.is_empty() {
            self.update_tree(true, |tree, u| tree.update(u))?
        } else {
            self.update_into_views(&requests, true)?
        };
        self.release_compiled();
        Ok(receipt)
    }

    pub(super) fn update_tree(
        &mut self,
        settled: bool,
        update: impl FnOnce(&mut Tree, &mut Update<'_>) -> Result<(), crate::instance::InstanceError>,
    ) -> Result<CommitReceipt, RunnerError> {
        let (ops, surfaces, _) = self.update_ops(settled, update)?;
        match self.apply(ops) {
            Ok(receipt) => {
                self.publish_surfaces(surfaces);
                if settled && !self.held_edges.is_empty() {
                    if let Err(error) = self.release_held_edges() {
                        self.poison();
                        return Err(error);
                    }
                }
                Ok(receipt)
            }
            Err(e) => {
                self.poison();
                Err(e)
            }
        }
    }
    /// Evaluate one phase without publishing it. Ordinary settlement runs once;
    /// a request phase only realizes the requested windows.
    pub(super) fn update_ops(
        &mut self,
        settled: bool,
        update: impl FnOnce(&mut Tree, &mut Update<'_>) -> Result<(), crate::instance::InstanceError>,
    ) -> Result<UpdateOps, RunnerError> {
        let mut tree = self.tree.take().expect("booted");
        let mut ids = std::mem::take(&mut self.ids);
        let rows = std::mem::take(&mut self.row_writes);
        let result = {
            let scroll_margin = |view| super::collection::scroll_margin(&self.kernel, view);
            let scroll_padding = |view| super::collection::scroll_padding(&self.kernel, view);
            let mut u = Update::new(self.env(&[], &[]), &self.sites, &mut ids);
            u.fonts = Some(self.fonts.clone());
            u.scroll_padding = Some(&scroll_padding);
            u.scroll_margin = Some(&scroll_margin);
            u.full = self.full;
            u.discard = self.kernel.is_detached();
            u.rows = rows;
            u.reuse = self.reuse;
            update(&mut tree, &mut u)
                .map(|_| (u.ops, u.surfaces, u.notes, u.renewed, u.shown, u.work))
        };
        self.ids = ids;
        self.tree = Some(tree);
        let (ops, surfaces, work) = match result {
            Ok((ops, surfaces, notes, renewed, shown, work)) => {
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
                (ops, surfaces, work)
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
        Ok((ops, surfaces, work))
    }
}

type UpdateOps = (
    Vec<exact_kernel::Op>,
    Vec<crate::SurfaceUpdate>,
    crate::instance::InstanceWork,
);
