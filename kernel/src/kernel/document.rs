//! Document inheritance and atomic application of a producer batch.
use super::*;

impl Kernel {
    /// Decode one EXWF frame and apply it.
    pub fn apply_frame(&mut self, bytes: &[u8]) -> Result<CommitReceipt, KernelError> {
        let frame = wire::decode(bytes)?;
        self.apply(frame.root_id, frame.batch, &frame.ops)
    }

    /// Apply ops in-process. Validation covers the whole batch before any write;
    /// a rejection leaves everything untouched.
    pub fn apply(
        &mut self,
        root_id: u32,
        batch: u64,
        ops: &[Op],
    ) -> Result<CommitReceipt, KernelError> {
        self.apply_document(root_id, batch, ops, None)
    }

    /// Apply a batch with its document language and direction. Authored CSS
    /// direction still wins; otherwise nodes inherit the document's direction.
    pub fn apply_document(
        &mut self,
        root_id: u32,
        batch: u64,
        ops: &[Op],
        language: Option<(&str, crate::Direction)>,
    ) -> Result<CommitReceipt, KernelError> {
        let changed_language =
            language.is_some_and(|(lang, _)| lang != self.arena.document_language);
        let next_epoch = self.epoch + 1;
        let mut unmirrored = Unmirrored;
        let target = Target {
            arena: &mut self.arena,
            layout: match self.layout.as_deref_mut() {
                Some(layout) => layout,
                None => &mut unmirrored,
            },
            selectors: &mut self.selectors,
        };
        let receipt = match language {
            Some(language) => {
                txn::apply_document(target, ops, batch, root_id, next_epoch, Some(language))
            }
            None => txn::apply(target, ops, batch, root_id, next_epoch),
        }?;
        if changed_language {
            self.measurer.set_language(&self.arena.document_language);
        }
        let changed = !receipt.created.is_empty()
            || !receipt.destroyed.is_empty()
            || !receipt.touched.is_empty();
        if changed {
            self.epoch = next_epoch;
        }
        let mut receipt = receipt;
        receipt.epoch = self.epoch;
        // @ref LLP 1057.003 D4 — timeline names resolve against the tree
        // this commit left.
        receipt.timelines = crate::timeline::refresh(&mut self.arena, &receipt);
        if self.receipts.len() == RECEIPT_RING {
            self.receipts.pop_front();
        }
        if self
            .region
            .as_mut()
            .is_some_and(|r| !r.observe(&self.arena, &receipt))
        {
            self.region = None;
        }
        self.receipts.push_back(receipt.clone());
        Ok(receipt)
    }
}
