//! Failed store writes remain owed until persisted or superseded.
use super::*;

impl<D: DataSource> Host<D> {
    /// Persist after the runner commits. The batch reports failures through
    /// its existing error channel; the committed in-memory value stays live.
    pub(super) fn persist(&mut self) {
        let mut changed = self.runner.data().take_storage_changed();
        for line in self.kept_failures() {
            self.store_failure(line);
        }
        let writes = self.runner.take_store_writes();
        if self.secrets.is_none() {
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
        self.runner.log(line.clone());
        if let Some(error) = &mut self.store_error {
            error.push_str("; ");
            error.push_str(&line);
        } else {
            self.store_error = Some(line);
        }
    }
}
