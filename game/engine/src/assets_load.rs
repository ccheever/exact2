use crate::{DataError, Game, Sim};

impl<G: Game> Sim<G> {
    /// Synchronously load requested assets and their dependencies for a headless
    /// simulation. Named failures and a still-pending gate return an error.
    /// Async hosts continue to use request/delivery; this helper does not advance time.
    pub fn load_assets<E: std::fmt::Display>(
        &mut self,
        mut read: impl FnMut(&str) -> Result<Vec<u8>, E>,
    ) -> Result<(), DataError> {
        let mut failures = Vec::new();
        for _ in 0..16 {
            let names = self.take_assets();
            if names.is_empty() {
                break;
            }
            for name in names {
                match read(&name) {
                    Ok(bytes) => {
                        if let Err(error) = self.asset(&name, Some(&bytes)) {
                            failures.push(error);
                        }
                    }
                    Err(error) => {
                        self.asset_failed(&name, &error.to_string());
                        failures.push(format!("asset `{name}`: {error}"));
                    }
                }
            }
        }
        if self.is_loading()
            || !failures.is_empty()
            || self.world.assets.refusal.is_some()
            || self
                .world
                .assets
                .states
                .values()
                .any(|s| *s != crate::asset::AssetState::Loaded)
        {
            Err(DataError::new(format!(
                "assets not ready: {}; {}",
                self.world.assets.state_json(),
                failures.join("; ")
            )))
        } else {
            Ok(())
        }
    }
}
