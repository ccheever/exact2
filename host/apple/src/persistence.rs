//! Failed store writes remain owed until persisted or superseded.
use super::*;

impl<D: DataSource> Host<D> {
    /// Persist after the runner commits. I/O failures belong in the journal;
    /// they cannot refuse work the runner has already committed.
    pub(super) fn persist(&mut self) {
        let mut changed = self.runner.data().take_storage_changed();
        if let Some(store) = &self.secrets {
            store.retry_failed();
        }
        for line in self.kept_failures() {
            self.store_failure(line);
        }
        let writes = self.runner.take_store_writes();
        if self.secrets.is_none() {
            if changed {
                crate::store::widget_changed(self.runner.data_ref().app_id());
            }
            return;
        }
        for write in writes {
            if let Some(pending) = self
                .pending_store_writes
                .iter_mut()
                .find(|w| w.name == write.name)
            {
                *pending = write;
            } else {
                self.pending_store_writes.push(write);
            }
        }
        for write in std::mem::take(&mut self.pending_store_writes) {
            if let Err(error) = self.secrets.as_ref().expect("checked above").write(&write) {
                self.store_failure(format!("store {} failed: {error}", write.name));
                self.pending_store_writes.push(write);
            } else {
                changed = true;
            }
        }
        if changed {
            crate::store::widget_changed(self.runner.data_ref().app_id());
        }
    }

    fn store_failure(&mut self, line: String) {
        self.runner.log(line);
    }
}
