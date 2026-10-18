//! Deferred data activation after first pixel. A source still loading its
//! executable image answers pending and wakes the session when it can answer.
use super::*;
use exact_kernel::CommitReceipt;

const UNCHANGED: &str = "{\"ops\":[],\"unchanged\":true}";

impl<D: DataSource> Host<D> {
    /// Load deferred app logic only after the presenter reports first pixel.
    pub fn activate_data(&mut self) -> String {
        if self.data_activated {
            return self.refresh_store_after(true);
        }
        match self.runner.data_ref().preload() {
            Ok(false) => {
                self.preload_wake.ask(self.runner.data_ref());
                return "{\"ops\":[],\"pending\":true}".into();
            }
            Err(error) => return self.commit(&[], Some(format!("prepare data: {error:?}"))),
            Ok(true) => {}
        }
        if let Err(error) = Self::activate_source(self.runner.data()) {
            return self.commit(&[], Some(format!("activate data: {error:?}")));
        }
        self.data_activated = true;
        self.store_activation_pending = true;
        let mut changed = false;
        let mut error = None;
        if self.store_refresh_pending {
            if let Some(read) = self.read_shared_store(false) {
                match self.runner.adopt_store(read.values.clone()) {
                    Ok(adopted) => {
                        changed = adopted;
                        self.store_read_finished(read);
                    }
                    Err(refused) => error = Some(format!("store changed: {refused:?}")),
                }
            }
        }
        // Adoption never asks. Even a refused read must reach data_ready,
        // including the mutations queued before first pixel.
        let receipt = match self.runner.data_ready_after_store(changed) {
            Ok(receipt) => {
                self.store_activation_pending = !self.runner.data_ref().ready();
                receipt
            }
            Err(refused) => {
                error = Some(format!("data ready: {refused:?}"));
                None
            }
        };
        self.store_commit(receipt, error, false)
    }

    /// A moved counter always reads every endowed name, including own writes.
    pub fn refresh_store(&mut self) -> String {
        self.refresh_store_after(false)
    }

    /// An unchanged counter may retry just the kept names the cap omitted.
    pub(crate) fn refresh_store_after(&mut self, retry_only: bool) -> String {
        if self.secrets.is_none() {
            return UNCHANGED.into();
        }
        if !self.data_activated {
            self.store_refresh_pending = true;
            self.store_activation_pending = true;
            return self.store_commit(None, None, true);
        }
        let read = self.read_shared_store(retry_only).expect("endowed store");
        let result = if self.store_activation_pending {
            // A different store receipt cannot discharge the owed data_ready.
            self.runner
                .adopt_store(read.values.clone())
                .and_then(|_| self.runner.data_ready_after_store(true))
        } else {
            self.runner.store_changed(read.values.clone())
        };
        match result {
            Ok(receipt) => {
                self.store_read_finished(read);
                self.store_activation_pending = !self.runner.data_ref().ready();
                self.store_commit(receipt, None, true)
            }
            Err(error) => {
                self.store_refresh_pending = true;
                self.store_commit(None, Some(format!("store changed: {error:?}")), false)
            }
        }
    }

    fn read_shared_store(&self, retry_only: bool) -> Option<crate::store::StoreRead> {
        let store = self.secrets.as_ref()?;
        let previous = self.runner.store().snapshot();
        Some(if retry_only && !self.store_retry_names.is_empty() {
            store.retry_snapshot(previous, &self.store_retry_names)
        } else {
            store.snapshot(previous)
        })
    }

    fn store_read_finished(&mut self, read: crate::store::StoreRead) {
        self.store_refresh_pending = !read.complete || !read.omitted.is_empty();
        self.store_retry_names = if read.complete {
            read.omitted
        } else {
            Vec::new()
        };
        if self.store_refresh_pending {
            self.runner
                .log("store changed: incomplete read; will retry");
        }
    }

    fn store_commit(
        &mut self,
        receipt: Option<CommitReceipt>,
        error: Option<String>,
        allow_unchanged: bool,
    ) -> String {
        if allow_unchanged && receipt.is_none() && error.is_none() {
            return format!("{{\"ops\":[],\"unchanged\":true,\"storeReadPending\":{},\"storeActivationPending\":{}}}", self.store_refresh_pending, self.store_activation_pending);
        }
        let mut batch = Batch::new();
        batch.store_status = Some((self.store_refresh_pending, self.store_activation_pending));
        let receipts: Vec<_> = receipt
            .into_iter()
            .map(|receipt| Timed {
                at_ms: self.now_ms,
                receipt,
            })
            .collect();
        self.commit_into(&receipts, error, batch)
    }
}
