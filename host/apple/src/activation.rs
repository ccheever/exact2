//! Deferred data activation after first pixel. A source still loading its
//! executable image answers pending and wakes the session when it can answer.
use super::*;

impl<D: DataSource> Host<D> {
    /// Load deferred app logic only after the presenter reports first pixel.
    pub fn activate_data(&mut self) -> String {
        if self.data_activated {
            return self.commit(&[], None);
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
        if self.store_refresh_pending {
            self.store_refresh_pending = false;
            if let Some(store) = &self.secrets {
                match self.runner.store_changed(store.snapshot()) {
                    Ok(receipt) => receipts.push(Timed {
                        at_ms: self.now_ms,
                        receipt,
                    }),
                    Err(error) => {
                        return self.commit(&[], Some(format!("store changed: {error:?}")))
                    }
                }
            }
        }
        match self.runner.data_ready() {
            Ok(Some(receipt)) => receipts.push(Timed {
                at_ms: self.now_ms,
                receipt,
            }),
            Ok(None) => {}
            Err(error) => return self.commit(&receipts, Some(format!("data ready: {error:?}"))),
        }
        self.commit(&receipts, None)
    }

    /// Re-read the endowed store after a peer commits, without reloading logic.
    pub fn refresh_store(&mut self) -> String {
        if !self.data_activated {
            self.store_refresh_pending = true;
            return self.commit(&[], None);
        }
        let Some(store) = &self.secrets else {
            return self.commit(&[], None);
        };
        match self.runner.store_changed(store.snapshot()) {
            Ok(receipt) => self.commit(
                &[Timed {
                    at_ms: self.now_ms,
                    receipt,
                }],
                None,
            ),
            Err(error) => self.commit(&[], Some(format!("store changed: {error:?}"))),
        }
    }
}
