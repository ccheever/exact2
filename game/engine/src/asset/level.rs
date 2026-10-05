use crate::{Data, DataError};

/// One game-authored `Data` type, used by the bake and the asset barrier alike.
/// JSON uses Data's record defaults and container replacement rules.
#[derive(Clone, Copy)]
pub struct Level {
    /// Authored file relative to the game, delivered under this same asset name.
    pub name: &'static str,
    decode: fn(&str) -> Result<LevelValue, DataError>,
}
impl Level {
    /// Declare a JSON level without a second schema or a runtime type registry.
    pub const fn of<T: Data>(name: &'static str) -> Self {
        Self {
            name,
            decode: decode::<T>,
        }
    }
    /// Validate bytes using the author's derive; errors include the field path.
    pub fn decode(self, text: &str) -> Result<LevelValue, DataError> {
        if !super::asset_name(self.name) || !self.name.ends_with(".level.json") {
            return Err(DataError::new("expected a .level.json asset name").at(self.name));
        }
        (self.decode)(text).map_err(|e| e.at(self.name))
    }
}
fn decode<T: Data>(text: &str) -> Result<LevelValue, DataError> {
    let _: T = crate::json::from_str(text)?;
    Ok(LevelValue {
        text: text.into(),
        ty: std::any::type_name::<T>(),
    })
}
/// Validated level payload; immutable and owned once by the world asset store.
#[derive(Clone)]
pub struct LevelValue {
    pub(super) text: String,
    pub(super) ty: &'static str,
}
impl crate::World {
    /// Read the declared level after the barrier. The type must match its declaration.
    pub fn level<T: Data>(&self, name: &str) -> Result<T, DataError> {
        let value = self
            .assets
            .levels
            .get(name)
            .filter(|_| self.assets.declared.contains(name))
            .ok_or_else(|| DataError::new("declared level has not arrived").at(name))?;
        if value.ty != std::any::type_name::<T>() {
            return Err(DataError::new(format!("declared type is `{}`", value.ty)).at(name));
        }
        crate::json::from_str(&value.text).map_err(|e| e.at(name))
    }
}

impl<G: crate::Game> crate::Sim<G> {
    pub(crate) fn deliver_level(&mut self, name: &str, text: String) -> Result<(), String> {
        let level = G::LEVEL
            .filter(|level| level.name == name)
            .ok_or_else(|| format!("level `{name}` is not declared by Game::LEVEL"))?;
        let value = level.decode(&text).map_err(|e| e.to_string())?;
        let digest = crate::hash::of(&text);
        let assets = &mut self.world_mut().assets;
        if assets
            .identities
            .get(name)
            .is_some_and(|old| *old != digest)
        {
            return Err(format!(
                "level `{name}` cannot change after delivery; restart with the new level"
            ));
        }
        assets.identify(name, digest);
        assets
            .levels
            .insert(name.into(), std::sync::Arc::new(value));
        assets.states.insert(name.into(), super::AssetState::Loaded);
        Ok(())
    }
}

pub(crate) fn names<G: crate::Game>() -> impl Iterator<Item = &'static str> {
    G::ASSETS.iter().copied().chain(
        G::LEVEL
            .filter(|level| !G::ASSETS.contains(&level.name))
            .map(|level| level.name),
    )
}

/// The declaration naming `name`, for `AssetStore::declared_by`.
pub(crate) fn declared_by<G: crate::Game>(name: &str) -> Option<&'static str> {
    if G::ASSETS.contains(&name) {
        Some("Game::ASSETS")
    } else if G::STREAMED.contains(&name) {
        Some("Game::STREAMED")
    } else if G::LEVEL.is_some_and(|level| level.name == name) {
        Some("Game::LEVEL")
    } else {
        None
    }
}

pub(crate) fn validate_declaration<G: crate::Game>() -> Result<(), String> {
    if G::ASSETS.is_empty() && G::LEVEL.is_none() && G::STREAMED.is_empty() {
        return Ok(());
    }
    for name in names::<G>().chain(G::STREAMED.iter().copied()) {
        if !super::asset_name(name)
            || !(if G::LEVEL.is_some_and(|level| level.name == name) {
                name.ends_with(".level.json")
            } else {
                name.ends_with(".model") || name.ends_with(".tex") || name.ends_with(".sound")
            })
        {
            return Err(format!(
                "asset `{name}`: declaration requires a .model, .tex, .sound or declared .level.json name"
            ));
        }
    }
    for name in G::STREAMED {
        if name.ends_with(".sound") {
            return Err(format!(
                "streamed asset `{name}`: sounds are not streamed yet; declare it in Game::ASSETS"
            ));
        }
        if names::<G>().any(|declared| declared == *name) {
            return Err(format!(
                "asset `{name}` is in both Game::ASSETS and Game::STREAMED; setup either waits for it or does not"
            ));
        }
    }
    Ok(())
}
