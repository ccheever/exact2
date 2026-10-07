//! Deferred data activation after first pixel. A source still loading its
//! executable image answers pending and wakes the executor when it can answer.
use super::*;

impl<D: DataSource> Host<D> {
    fn configure_storage(&mut self) -> Result<(), exact_runner::DataError> {
        let app_id = self.runner.data().app_id().to_string();
        let Some(([data, cache, temporary], _)) = crate::picker::app_dirs(&app_id)? else {
            return Ok(());
        };
        // An authored test's store starts empty every run (`agent --test`):
        // emptied at the boot that read it, so this is a no-op unless that failed.
        crate::picker::empty_fresh_tree(&app_id)?;
        // What `app:/` names for the picker and an image's source (LLP
        // 1069.002 D4, D7); the last launch's picks go.
        crate::picker::set_roots(data.clone(), cache.clone(), temporary.clone());
        self.runner.data().configure_storage(data, cache, temporary)
    }

    /// Activate deferred data only after the presenter has produced first pixel.
    /// Returns whether a data-ready commit needs presenting and dispatching.
    pub fn activate_data(&mut self) -> Result<bool, String> {
        if self.data_activated {
            return Ok(false);
        }
        if !self
            .runner
            .data_ref()
            .preload()
            .map_err(|e| format!("prepare data: {e:?}"))?
        {
            self.preload_wake.ask(self.runner.data_ref());
            return Ok(false);
        }
        if let Err(error) = self
            .configure_storage()
            .and_then(|()| self.runner.data().activate())
        {
            return Err(format!("activate data: {error:?}"));
        }
        self.data_activated = true;
        match self.runner.data_ready() {
            Ok(Some(receipt)) => match self.commit(
                &[Timed {
                    at_ms: self.now_ms,
                    receipt,
                }],
                None,
            ) {
                Some(error) => Err(error),
                None => Ok(true),
            },
            Ok(None) => Ok(false),
            Err(error) => Err(format!("data ready: {error:?}")),
        }
    }

    /// Deferred image preparation needs another turn after first pixel.
    pub fn data_pending(&self) -> bool {
        !self.data_activated
    }
}
