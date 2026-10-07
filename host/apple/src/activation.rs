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
        match self.runner.data_ready() {
            Ok(Some(receipt)) => self.commit(
                &[Timed {
                    at_ms: self.now_ms,
                    receipt,
                }],
                None,
            ),
            Ok(None) => self.commit(&[], None),
            Err(error) => self.commit(&[], Some(format!("data ready: {error:?}"))),
        }
    }
}
