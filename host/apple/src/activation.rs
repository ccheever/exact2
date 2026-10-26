//! Deferred data activation after first pixel. A source still loading its
//! executable image answers pending and wakes the session when it can answer.
use super::*;
use exact_kernel::CommitReceipt;

const UNCHANGED: &str = "{\"ops\":[],\"unchanged\":true}";

impl<D: DataSource> Host<D> {
    /// The common post-pixel activation order for committed sources.
    pub(crate) fn activate_source(source: &mut D) -> Result<(), exact_runner::DataError> {
        Self::configure_storage(source)?;
        source.activate()
    }

    /// Activate the source (a TypeScript module evaluates its top level
    /// here) without committing, so the presenter can run this on the owner
    /// thread while its own thread stays free, then commit with
    /// `activate_data`. Does nothing once activated, or while the source is
    /// still loading its image: `activate_data` answers pending then.
    pub fn prepare_data(&mut self) {
        if self.data_activated || self.source_prepared.is_some() {
            return;
        }
        if !matches!(self.runner.data_ref().preload(), Ok(true)) {
            return;
        }
        self.source_prepared =
            Some(Self::activate_source(self.runner.data()).map_err(|e| format!("{e:?}")));
    }

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
        let activated = match self.source_prepared.take() {
            Some(prepared) => prepared,
            None => Self::activate_source(self.runner.data()).map_err(|e| format!("{e:?}")),
        };
        if let Err(error) = activated {
            return self.commit(&[], Some(format!("activate data: {error}")));
        }
        self.data_activated = true;
        self.store_activation_pending = true;
        let mut changed = exact_runner::StoreChange::default();
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
                .and_then(|change| self.runner.data_ready_after_store(change))
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
        // An accepted peer value supersedes the local value still owed to disk.
        self.pending_store_writes.retain(|write| {
            read.values
                .iter()
                .find(|(name, _)| name == &write.name)
                .map(|(_, value)| value.as_slice())
                == write.value.as_deref()
        });
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

/// The process's app-wide module state, which every session shares: an
/// embedder's sessions are one app (LLP 1133.004 §7, `"scope": "app"`).
pub(super) fn app_state() -> exact_runner::AppState {
    static STATE: std::sync::OnceLock<exact_runner::AppState> = std::sync::OnceLock::new();
    STATE.get_or_init(Default::default).clone()
}
