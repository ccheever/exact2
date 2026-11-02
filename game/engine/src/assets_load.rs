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

/// A `Sim` of the calling game with its `art/` baked in memory, for its hostless
/// tests: `let sim: Sim<MyGame> = exact_game::sim_with_art!(Options::default())?;`.
/// It evaluates to `Sim::with_assets(args, loader)`, so it returns
/// `Result<Sim<G>, String>` and refuses an asset the bake did not make by name.
///
/// The game is the nearest directory above the calling crate's
/// `CARGO_MANIFEST_DIR` that holds `logic/src/lib.rs`, so a generated
/// `.shells/logic` member and an authored `logic/Cargo.toml` both find it, from
/// any working directory, with no prior bake and no environment variable. The
/// payloads are those the GPU shell's bake writes to `assets/`, made once per call
/// site in a test binary. The calling crate needs `exact-game-bake` as a
/// dev-dependency; a generated member gets it when the game has `art/`.
#[macro_export]
macro_rules! sim_with_art {
    ($args:expr $(,)?) => {{
        static ART: ::std::sync::OnceLock<
            ::std::result::Result<
                ::std::collections::BTreeMap<::std::string::String, ::std::vec::Vec<u8>>,
                ::std::string::String,
            >,
        > = ::std::sync::OnceLock::new();
        match ART.get_or_init(|| ::exact_game_bake::game_art(env!("CARGO_MANIFEST_DIR"))) {
            ::std::result::Result::Ok(art) => $crate::Sim::with_assets($args, |name: &str| {
                art.get(name).cloned().ok_or_else(|| {
                    ::std::format!(
                        "`{name}` is not a payload of this game's art/ (a model `x.model` comes from art/x.glb or art/x.gltf)"
                    )
                })
            }),
            ::std::result::Result::Err(error) => ::std::result::Result::Err(error.clone()),
        }
    }};
}
