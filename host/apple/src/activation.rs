//! Deferred data activation after first pixel. A source still loading its
//! executable image answers pending and wakes the session when it can answer.
use super::*;

const UNCHANGED: &str = "{\"ops\":[]}";

impl<D: DataSource> Host<D> {
    /// Load deferred app logic only after the presenter reports first pixel.
    pub fn activate_data(&mut self) -> String {
        if self.data_activated {
            return if self.store_refresh_pending {
                self.refresh_store()
            } else {
                UNCHANGED.into()
            };
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
        let mut receipts = Vec::new();
        let mut error = None;
        let mut changed = false;
        if self.store_refresh_pending {
            if let Some(store) = &self.secrets {
                let read = store.snapshot(self.runner.store().snapshot());
                match self.runner.adopt_store(read.values) {
                    Ok(adopted) => {
                        changed = adopted;
                        self.store_refresh_pending = !read.complete;
                        if !read.complete {
                            error = Some("store changed: incomplete read; will retry".into());
                        }
                    }
                    Err(refused) => error = Some(format!("store changed: {refused:?}")),
                }
            }
        }
        // A refused adoption must not swallow activation or its queued sends.
        match self.runner.data_ready_after_store(changed) {
            Ok(Some(receipt)) => receipts.push(Timed {
                at_ms: self.now_ms,
                receipt,
            }),
            Ok(None) => {}
            Err(refused) => {
                self.store_refresh_pending |= changed;
                self.store_activation_pending |= changed;
                let message = format!("data ready: {refused:?}");
                error = Some(error.map_or_else(|| message.clone(), |e| format!("{e}; {message}")));
            }
        }
        self.commit(&receipts, error)
    }

    /// Re-read the endowed store after a peer commits, without reloading logic.
    pub fn refresh_store(&mut self) -> String {
        let Some(store) = &self.secrets else {
            return UNCHANGED.into();
        };
        if !self.data_activated {
            self.store_refresh_pending = true;
            return UNCHANGED.into();
        }
        let read = store.snapshot(self.runner.store().snapshot());
        let retry = self.store_activation_pending;
        match self.runner.store_changed(read.values).and_then(|receipt| {
            if retry && receipt.is_none() {
                self.runner.data_ready_after_store(true)
            } else {
                Ok(receipt)
            }
        }) {
            Ok(receipt) => {
                self.store_refresh_pending = !read.complete;
                self.store_activation_pending = false;
                let error =
                    (!read.complete).then(|| "store changed: incomplete read; will retry".into());
                match receipt {
                    Some(receipt) => self.commit(
                        &[Timed {
                            at_ms: self.now_ms,
                            receipt,
                        }],
                        error,
                    ),
                    None if error.is_none() => UNCHANGED.into(),
                    None => self.commit(&[], error),
                }
            }
            Err(error) => {
                self.store_refresh_pending = true;
                self.commit(&[], Some(format!("store changed: {error:?}")))
            }
        }
    }
}
