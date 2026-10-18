use super::*;

impl<D: DataSource> Runner<D> {
    pub(super) fn apply(
        &mut self,
        mut ops: Vec<exact_kernel::Op>,
    ) -> Result<CommitReceipt, RunnerError> {
        let _roles = self.bind_roles(); // Its own (LLP 1121 stage 5).
                                        // The queued patches' colours stay kept until this commit marks them.
        let (mut prefix, _queued) = self.take_reorder_ops();
        if !prefix.is_empty() {
            prefix.append(&mut ops);
            ops = prefix;
        }
        let change = self.router_change()?;
        let live_undo = self.prepare_live_times(&mut ops);
        self.batch += 1;
        let (language, direction) = self.document_facts();
        let shown = std::mem::take(&mut self.shown);
        let mut receipt =
            match self
                .kernel
                .apply_document(0, self.batch, &ops, Some((&language, direction)))
            {
                Ok(receipt) => receipt,
                Err(e) => {
                    live_undo.restore(&mut self.live_times);
                    self.notes.clear();
                    return Err(e.into());
                }
            };
        let renewed = self.settle_shown(shown, &mut receipt);
        self.tally(&ops, &receipt.touched);
        for note in std::mem::take(&mut self.notes) {
            self.log(note);
        }
        let arena = self.kernel.arena();
        receipt.renewed = renewed
            .into_iter()
            .filter_map(|view| arena.key_of(view))
            .collect();
        // Forget destroyed views once they outnumber the live ones (a
        // detached kernel holds none, and its runner asks for none).
        if !self.kernel.is_detached() && self.ids.remembered() > 2 * self.kernel.live_count() + 256
        {
            let kernel = &self.kernel;
            self.ids.retain(|view| kernel.node(view).is_some());
        }
        let cleanup = self.reconcile_reorder()?;
        if !cleanup.is_empty() {
            self.batch += 1;
            let tail = self.kernel.apply(0, self.batch, &cleanup)?;
            self.tally(&cleanup, &tail.touched);
            receipt.batch = tail.batch;
            receipt.epoch = tail.epoch;
            for key in tail.touched {
                if !receipt.created.contains(&key) && !receipt.touched.contains(&key) {
                    receipt.touched.push(key);
                }
            }
            receipt.layout_invalidated |= tail.layout_invalidated;
        }
        self.commit_router(change);
        self.retire_holds();
        Ok(receipt)
    }
}
