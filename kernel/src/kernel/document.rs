//! Document inheritance and atomic application of a producer batch.
use super::*;

impl Kernel {
    /// A kernel that keeps nothing: each batch is counted and answered with
    /// an empty receipt, and no node is created, checked or stored. For a
    /// render that writes its document from the runner's instance tree
    /// instead of from a kernel's nodes (LLP 1048.004), which checks what the
    /// kernel would have refused as it writes, and falls back to a kernel
    /// render when it finds one. Its runner reads no nodes back.
    pub fn detached() -> Self {
        let mut kernel = Self::with_monospace_on_demand();
        kernel.detached = true;
        kernel
    }

    /// Whether this kernel keeps nothing ([`Kernel::detached`]).
    pub fn is_detached(&self) -> bool {
        self.detached
    }

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
        if self.document_commit.is_some() {
            return Err(KernelError::DocumentCommit(
                "a document commit is already active",
            ));
        }
        if self.detached {
            return Ok(CommitReceipt {
                batch,
                root_id,
                epoch: self.epoch,
                ..CommitReceipt::default()
            });
        }
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
        Ok(self.publish_document_commit(receipt))
    }

    /// Begin one logical commit, resolving computed styles without publishing
    /// a receipt, epoch, region observation or timeline bindings. Callers may
    /// read computed nodes between phases, but must finish before presenting or
    /// laying out the tree. A rejected initial validation changes nothing.
    /// Only one commit may be active; ordinary apply calls are refused meanwhile.
    pub fn begin_document_commit(
        &mut self,
        root_id: u32,
        batch: u64,
        ops: &[Op],
        language: Option<(&str, crate::Direction)>,
    ) -> Result<(), KernelError> {
        if self.document_commit.is_some() {
            return Err(KernelError::DocumentCommit(
                "a document commit is already active",
            ));
        }
        let mut commit = Box::new(txn::DocumentCommit::new(batch, root_id, self.epoch + 1));
        self.apply_document_phase(ops, language, &mut commit)?;
        self.document_commit = Some(commit);
        Ok(())
    }

    /// Apply another phase and resolve its computed styles, preserving lifecycle
    /// identity from earlier phases. Each phase validates before its own writes.
    /// Any phase failure makes this commit unfinishable: earlier phases remain
    /// applied and no rollback is promised. The owner must poison its runner and
    /// reset/re-snapshot rather than present or continue the partial tree.
    pub fn continue_document_commit(&mut self, ops: &[Op]) -> Result<(), KernelError> {
        let mut commit = self
            .document_commit
            .take()
            .ok_or(KernelError::DocumentCommit("no document commit is active"))?;
        let result = if commit.failed {
            Err(KernelError::DocumentCommit(
                "the active document commit failed; reset required",
            ))
        } else {
            self.apply_document_phase(ops, None, &mut commit)
        };
        commit.failed |= result.is_err();
        self.document_commit = Some(commit);
        result
    }

    /// Finalize lifecycle facts once and publish one logical receipt and epoch.
    /// No intermediate phase enters receipt history, region observation or
    /// timeline resolution. Detached kernels retain their ordinary empty-receipt
    /// behavior, including unchanged epoch and no retained receipt history.
    pub fn finish_document_commit(&mut self) -> Result<CommitReceipt, KernelError> {
        let commit = self
            .document_commit
            .as_ref()
            .ok_or(KernelError::DocumentCommit("no document commit is active"))?;
        if commit.failed {
            return Err(KernelError::DocumentCommit(
                "the active document commit failed; reset required",
            ));
        }
        let commit = self
            .document_commit
            .take()
            .expect("active commit checked above");
        let mut receipt = txn::finish_document(&self.arena, *commit);
        if self.detached {
            receipt.epoch = self.epoch;
            return Ok(receipt);
        }
        Ok(self.publish_document_commit(receipt))
    }

    fn apply_document_phase(
        &mut self,
        ops: &[Op],
        language: Option<(&str, crate::Direction)>,
        commit: &mut txn::DocumentCommit,
    ) -> Result<(), KernelError> {
        if self.detached {
            return Ok(());
        }
        let changed_language =
            language.is_some_and(|(lang, _)| lang != self.arena.document_language);
        let mut unmirrored = Unmirrored;
        txn::continue_document(
            Target {
                arena: &mut self.arena,
                layout: match self.layout.as_deref_mut() {
                    Some(layout) => layout,
                    None => &mut unmirrored,
                },
                selectors: &mut self.selectors,
            },
            ops,
            language,
            commit,
        )?;
        if changed_language {
            self.measurer.set_language(&self.arena.document_language);
        }
        Ok(())
    }

    fn publish_document_commit(&mut self, mut receipt: CommitReceipt) -> CommitReceipt {
        let changed = !receipt.created.is_empty()
            || !receipt.destroyed.is_empty()
            || !receipt.touched.is_empty();
        if changed {
            self.epoch = receipt.epoch;
        }
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
        self.receipts.push_back(RetainedReceipt::new(&receipt));
        receipt
    }
}
