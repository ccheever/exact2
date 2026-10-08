//! Renew retained carriers before their new row's authored deltas are emitted.
use super::{Batch, Host};
use exact_runner::{DataSource, Timed};
use std::collections::BTreeSet;

impl<D: DataSource> Host<D> {
    /// Opt into the runner's bounded, conservative row rebinding. The caller's
    /// presenter must handle `renew` with fresh interaction and async ownership.
    pub fn set_row_reuse(&mut self, on: bool) {
        self.runner.set_row_reuse(on);
    }

    pub(super) fn renew_rows(&mut self, receipts: &[Timed], batch: &mut Batch) {
        let renewed: BTreeSet<_> = receipts
            .iter()
            .flat_map(|t| t.receipt.renewed.iter())
            .filter_map(|key| self.keys.get(key).copied())
            .filter(|id| self.runner.kernel().node(*id).is_some())
            .collect();
        let mut carriers = Vec::new();
        for id in renewed {
            // Inline runs have no platform carrier, but any old animated color
            // must disappear before the new paragraph is assembled.
            self.paint.runs.remove(&id);
            // Natural dimensions belong to the image resource, so an unchanged
            // source can keep them. Other per-item measurements start fresh.
            let same_image = self.runner.kernel().node(id).is_some_and(|node| {
                node.node_type == exact_kernel::NodeType::Image
                    && node
                        .props
                        .str(exact_kernel::PropId::ImageSource)
                        .is_some_and(|source| {
                            self.mirror
                                .get(&id)
                                .and_then(|m| m.props.get("imageSource"))
                                .is_some_and(|old| old == source)
                        })
            });
            if !same_image {
                self.runner
                    .kernel_mut()
                    .set_intrinsic_size(id, None)
                    .expect("a surviving carrier can forget its previous measurement");
            }
            if let Some((owner, _)) = self.inline_runs.get(&id) {
                self.dirty_paragraphs.insert(*owner);
            }
            if self.mirror.contains_key(&id) {
                carriers.push(id);
                // A retained mirror can contain presented paint. Compare its
                // authored target again even if unchanged bindings made no op.
                self.update(id, batch);
            }
        }
        // Newly created arms are absent from the old mirror and already fresh.
        // Reset goes first even if earlier code staged other batch operations.
        batch.renew(&carriers);
    }
}

#[cfg(test)]
#[path = "row_reuse_tests.rs"]
mod tests;
