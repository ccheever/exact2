use crate::{Data, DataError};
use std::any::Any;
use std::sync::Arc;

/// One game-authored `Data` file, used by the bake and the asset barrier alike.
/// JSON uses Data's record defaults and container replacement rules. The
/// authored file is `assets/<name>`, delivered as it is: an edit reaches a
/// running dev page as any asset does, without a build (`Sim::assets_changed`).
#[derive(Clone, Copy)]
pub struct Level {
    /// Asset name under the game's `assets/`; ends in `.level.json`.
    pub name: &'static str,
    /// Presentation data (`Level::shown`): only `Game::present` reads it, and
    /// saves and hashes never record it, so an edit moves no pin.
    pub shown: bool,
    decode: fn(&str) -> Result<LevelValue, DataError>,
}
impl Level {
    /// Simulation data (a level, a balance table): setup and ticks read it with
    /// `World::level` or `World::shared_level`. A save records its content
    /// identity, as it does a declared model's, so changed bytes refuse a
    /// restore by name.
    pub const fn of<T: Data + Send + Sync + 'static>(name: &'static str) -> Self {
        Self {
            name,
            shown: false,
            decode: decode::<T>,
        }
    }
    /// Simulation data whose type also checks what its shape cannot say
    /// (ranges, counts, rows that must agree): at bake, and at every delivery
    /// — a development reload's too, which then keeps the level the world runs on.
    pub const fn checked<T: Data + Checked + Send + Sync + 'static>(name: &'static str) -> Self {
        Self {
            name,
            shown: false,
            decode: decode_checked::<T>,
        }
    }
    /// Presentation data (a look's palette, lighting and camera): only
    /// `Present::shared_level` reads it. Awaited before setup like any
    /// declaration, but outside saves and hashes, so a changed file restores
    /// the same world and a development reload replaces it with no notice.
    pub const fn shown<T: Data + Send + Sync + 'static>(name: &'static str) -> Self {
        Self {
            shown: true,
            ..Self::of::<T>(name)
        }
    }
    /// Validate bytes using the author's derive; errors include the field path.
    pub fn decode(self, text: &str) -> Result<LevelValue, DataError> {
        if !super::asset_name(self.name) || !self.name.ends_with(".level.json") {
            return Err(DataError::new("expected a .level.json asset name").at(self.name));
        }
        let mut value = (self.decode)(text).map_err(|e| e.at(self.name))?;
        value.shown = self.shown;
        Ok(value)
    }
}
/// What a level's type requires beyond its shape; see `Level::checked`.
pub trait Checked {
    fn check(&self) -> Result<(), String>;
}
fn decode_checked<T: Data + Checked + Send + Sync + 'static>(
    text: &str,
) -> Result<LevelValue, DataError> {
    let value: T = crate::json::from_str(text)?;
    value.check().map_err(DataError::new)?;
    Ok(LevelValue::new(text, value))
}
fn decode<T: Data + Send + Sync + 'static>(text: &str) -> Result<LevelValue, DataError> {
    Ok(LevelValue::new(text, crate::json::from_str::<T>(text)?))
}
/// Validated level payload, decoded once; immutable and owned once by the
/// world asset store, so a tick may read a table every time instead of
/// copying it at setup, without parsing it again.
#[derive(Clone)]
pub struct LevelValue {
    pub(super) text: String,
    pub(super) ty: &'static str,
    value: Arc<dyn Any + Send + Sync>,
    shown: bool,
}
impl LevelValue {
    fn new<T: Data + Send + Sync + 'static>(text: &str, value: T) -> Self {
        Self {
            text: text.into(),
            ty: std::any::type_name::<T>(),
            value: Arc::new(value),
            shown: false,
        }
    }
    fn get<T: Data + Send + Sync + 'static>(&self, name: &str) -> Result<Arc<T>, DataError> {
        Arc::clone(&self.value)
            .downcast::<T>()
            .map_err(|_| DataError::new(format!("declared type is `{}`", self.ty)).at(name))
    }
}
impl crate::World {
    fn level_value(&self, name: &str) -> Result<&LevelValue, DataError> {
        self.assets
            .levels
            .get(name)
            .filter(|_| self.assets.declared.contains(name))
            .map(Arc::as_ref)
            .ok_or_else(|| DataError::new("declared level has not arrived").at(name))
    }
    /// A simulation level; presentation data (`Level::shown`) is refused,
    /// since only `Game::present` reads it.
    fn simulation_level(&self, name: &str) -> Result<&LevelValue, DataError> {
        let value = self.level_value(name)?;
        if value.shown {
            return Err(DataError::new(
                "presentation data (Level::shown): read it in Game::present with Present::shared_level",
            )
            .at(name));
        }
        Ok(value)
    }
    /// Read a declared simulation level after the barrier, as a value of the
    /// caller's own. The type must match its declaration.
    pub fn level<T: Data>(&self, name: &str) -> Result<T, DataError> {
        let value = self.simulation_level(name)?;
        if value.ty != std::any::type_name::<T>() {
            return Err(DataError::new(format!("declared type is `{}`", value.ty)).at(name));
        }
        crate::json::from_str(&value.text).map_err(|e| e.at(name))
    }
    /// The declared simulation level as decoded at delivery, shared rather
    /// than parsed again: a table a tick reads each time (LLP 1046.009 §3.1).
    /// After a development reload replaces the level, the next read sees the
    /// new one.
    pub fn shared_level<T: Data + Send + Sync + 'static>(
        &self,
        name: &str,
    ) -> Result<Arc<T>, DataError> {
        self.simulation_level(name)?.get(name)
    }
    /// A declared level of either kind, for `Present::shared_level`.
    pub(crate) fn shown_level<T: Data + Send + Sync + 'static>(
        &self,
        name: &str,
    ) -> Result<Arc<T>, DataError> {
        self.level_value(name)?.get(name)
    }
    /// Whether a development reload replaced this asset's content (a level, a
    /// sound, a model, a texture) since the last call, which consumes the
    /// notice. A game rebuilds what it derived from the asset at setup — a
    /// level's colliders — when it sees one. Saves and hashes never hold the
    /// notice; a fresh or restored world has none. Presentation data
    /// (`Level::shown`) needs none: `present` reads it each frame.
    pub fn take_replaced(&mut self, name: &str) -> bool {
        self.assets.replaced.contains(name) && self.assets.replaced.remove(name)
    }
}

impl<G: crate::Game> crate::Sim<G> {
    /// `replacing`: a development reload announced new bytes under this name
    /// (`Sim::assets_changed`), so a changed digest replaces the level rather
    /// than being refused. The world's saves then carry the new identity.
    /// Presentation data (`Level::shown`) is in no save: new bytes always
    /// replace the old.
    pub(crate) fn deliver_level(
        &mut self,
        name: &str,
        text: String,
        replacing: bool,
    ) -> Result<(), String> {
        let level = G::LEVELS
            .iter()
            .find(|level| level.name == name)
            .ok_or_else(|| format!("level `{name}` is not declared by Game::LEVELS"))?;
        let value = level.decode(&text).map_err(|e| e.to_string())?;
        let assets = &mut self.world_mut().assets;
        if !level.shown {
            let digest = crate::hash::of(&text);
            if !replacing
                && assets
                    .identities
                    .get(name)
                    .is_some_and(|old| *old != digest)
            {
                return Err(format!(
                    "level `{name}` cannot change after delivery; restart with the new level"
                ));
            }
            assets.identify(name, digest);
        }
        assets.levels.insert(name.into(), Arc::new(value));
        assets.states.insert(name.into(), super::AssetState::Loaded);
        Ok(())
    }
}

pub(crate) fn names<G: crate::Game>() -> impl Iterator<Item = &'static str> {
    G::ASSETS.iter().copied().chain(
        G::LEVELS
            .iter()
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
    } else if G::LEVELS.iter().any(|level| level.name == name) {
        Some("Game::LEVELS")
    } else {
        None
    }
}

pub(crate) fn validate_declaration<G: crate::Game>() -> Result<(), String> {
    if G::ASSETS.is_empty() && G::LEVELS.is_empty() && G::STREAMED.is_empty() {
        return Ok(());
    }
    let level = |name: &str| G::LEVELS.iter().any(|level| level.name == name);
    for name in names::<G>().chain(G::STREAMED.iter().copied()) {
        if !super::asset_name(name)
            || !(if level(name) {
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
    for (i, level) in G::LEVELS.iter().enumerate() {
        if G::LEVELS[..i].iter().any(|other| other.name == level.name) {
            return Err(format!("level `{}` is declared twice", level.name));
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
