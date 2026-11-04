//! Long calls own the partition off the source thread. Cheap calls try the
//! same mutex without waiting, so they cannot spend a source's turn on it.

use super::Module;
use exact_js::{LaterHandler, NativeModule};
use serde_json::Value;
use std::{path::PathBuf, sync::Arc};

impl NativeModule for Module {
    fn configure_storage(
        &mut self,
        data: PathBuf,
        cache: PathBuf,
        temporary: PathBuf,
    ) -> Result<(), String> {
        Module::configure_storage(self, data, cache, temporary)
    }

    fn call(&mut self, request: &Value) -> Result<Value, String> {
        Module::call(self, request)
    }

    fn later(&mut self) -> Option<LaterHandler> {
        let state = Arc::clone(&self.state);
        Some(Arc::new(move |request, reply| {
            let state = Arc::clone(&state);
            std::thread::spawn(move || {
                let result = state
                    .lock()
                    .map_err(|_| "Snapback4 device owner panicked; reopen the module".into())
                    .and_then(|mut state| state.call(&request));
                // Unlock before settling: a continuation may call synchronously.
                reply.send(result);
            });
        }))
    }
}

#[cfg(test)]
mod tests;
