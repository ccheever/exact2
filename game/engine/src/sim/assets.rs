//! Asset delivery and setup barriers.
use super::*;

impl<G: Game> Sim<G> {
    /// Whether setup is waiting for declared model bytes.
    pub fn is_loading(&self) -> bool {
        self.setup_pending
    }
    /// Drain first-sight model requests. Nondeclared meshes may pop in after tick zero.
    pub fn take_assets(&mut self) -> Vec<String> {
        if self.setup_pending && !self.assets_pending() {
            return Vec::new();
        }
        for name in G::STREAMED {
            self.world.assets.request(name);
        }
        let revision = self.world.revision::<crate::Mesh>();
        let sprites_changed =
            crate::sprite::texture_names_changed(&self.world, &mut self.asset_sprite_names);
        if revision != self.asset_mesh_revision || sprites_changed {
            let names: Vec<_> = self
                .world
                .query::<&crate::Mesh>()
                .iter()
                .filter_map(|(_, mesh)| {
                    if let crate::Mesh::Asset(name) = mesh {
                        Some((name.clone(), ".model"))
                    } else {
                        None
                    }
                })
                .chain(
                    self.world
                        .query::<&crate::Sprite>()
                        .iter()
                        .map(|(_, s)| (s.texture.clone(), ".tex")),
                )
                .collect();
            let mut roots: std::collections::BTreeSet<_> =
                names.iter().map(|(n, _)| n.clone()).collect();
            // Declared and streamed assets stay resident: one that leaves the
            // screen and returns is still Loaded, so a save never refuses for it.
            // Undeclared cosmetics retire when unshown, bounding their memory.
            roots.extend(G::ASSETS.iter().map(|n| (*n).to_owned()));
            roots.extend(G::STREAMED.iter().map(|n| (*n).to_owned()));
            roots.extend(self.world.assets.declared.iter().cloned());
            if let Some(level) = G::LEVEL {
                roots.insert(level.name.into());
            }
            self.world.assets.retire(&roots);
            self.textures
                .retain(|n, _| self.world.assets.states.contains_key(n));
            for (name, suffix) in names {
                self.world.assets.request(&name);
                if !name.ends_with(suffix) {
                    self.asset_failed(&name, &format!("component requires a {suffix} name"));
                }
            }
            self.asset_mesh_revision = revision;
        }
        let assets = &mut *self.world.assets;
        let mut names: Vec<_> = assets
            .states
            .iter()
            .filter(|(n, s)| {
                (**s == crate::asset::AssetState::Pending || assets.redelivery.contains(*n))
                    && !assets.requested.contains(*n)
            })
            .map(|(n, _)| n.clone())
            .collect();
        // What setup and the first frame wait for goes first; streamed names last.
        names.sort_by_key(|n| {
            (
                G::STREAMED.contains(&n.as_str()),
                !assets.required.contains(n),
            )
        });
        assets.requested.extend(names.iter().cloned());
        names
    }
    /// Drain names whose last cosmetic mesh reference disappeared.
    pub fn take_retired_assets(&mut self) -> Vec<String> {
        std::mem::take(&mut self.world.assets.retired)
    }
    /// Device-backed surfaces retain arriving texture payloads until upload.
    pub fn defer_assets(&mut self, defer: bool) {
        self.defer_assets = defer;
    }
    /// Renderer-only model feed; games receive World, whose reads enforce declarations.
    pub fn presentation_models(&self) -> impl Iterator<Item = (&str, &crate::asset::Model)> {
        self.world
            .assets
            .models
            .iter()
            .filter(|(n, _)| self.world.assets.states.contains_key(n))
            .map(|(n, m)| (n.as_str(), m.model.as_ref()))
    }
    /// Content remains Loaded after loss; only device preparation is invalidated.
    pub fn invalidate_device_assets(&mut self) -> Vec<String> {
        let assets = &mut *self.world.assets;
        assets.prepared.clear();
        let mut retry = Vec::new();
        for (name, state) in &assets.states {
            if *state == crate::asset::AssetState::Pending {
                assets.requested.remove(name);
            }
            if name.ends_with(".tex") && *state == crate::asset::AssetState::Loaded {
                assets.requested.remove(name);
                assets.redelivery.insert(name.clone());
                retry.push(name.clone());
                // The module must reopen this answered name even if the replacement
                // attached and prepared assets before failing again.
                assets.retired.push(name.clone());
            }
        }
        retry
    }
    /// Names still awaiting bytes, including recovery uploads.
    pub fn assets_pending(&self) -> bool {
        if self.setup_pending
            && self.world.assets.required.iter().any(|n| {
                matches!(
                    self.world.assets.states.get(n),
                    Some(crate::asset::AssetState::Failed(_))
                )
            })
        {
            return false;
        }
        self.world
            .assets
            .states
            .values()
            .any(|s| *s == crate::asset::AssetState::Pending)
            || !self.world.assets.redelivery.is_empty()
    }
    /// Every retained model and dependency prepared for the current device.
    pub fn device_assets_ready(&self) -> bool {
        self.world.assets.states.iter().all(|(n, s)| {
            *s != crate::asset::AssetState::Loaded
                || n == crate::asset::TABLE_ASSET
                || n.ends_with(".level.json")
                || n.ends_with(".sound")
                || self.world.assets.prepared.contains(n)
        })
    }
    /// Move texture payloads to the renderer; no CPU mip copy survives upload.
    pub fn take_textures(
        &mut self,
    ) -> std::collections::BTreeMap<String, crate::asset::TextureData> {
        std::mem::take(&mut self.textures)
    }
    /// A named preparation/upload completed, or failed without poisoning the surface.
    pub fn asset_prepared(&mut self, name: &str, result: Result<(), String>) {
        use crate::asset::AssetState;
        match result {
            Ok(()) => {
                self.world.assets.prepared.insert(name.into());
            }
            Err(reason) => {
                self.world.assets.states.insert(
                    name.into(),
                    AssetState::Failed(format!("asset `{name}`: {reason}")),
                );
            }
        }
        self.finish_assets(name);
    }
    /// A model's state follows its textures': re-derive only the model
    /// `touched` (a delivered or prepared name) is, and the models naming it
    /// as a texture (`Assets::dependents`). Re-deriving every model on every
    /// delivery made a world of hundreds of models quadratic to load (garden's
    /// 202: ~4 s of a web load).
    fn finish_assets(&mut self, touched: &str) {
        use crate::asset::AssetState;
        let assets = &mut *self.world.assets;
        let mut models = assets.dependents.get(touched).cloned().unwrap_or_default();
        if assets.dependencies.contains_key(touched) {
            models.push(touched.into());
        }
        for name in models {
            let Some(textures) = assets.dependencies.get(&name) else {
                continue;
            };
            if matches!(assets.states.get(&name), None | Some(AssetState::Failed(_))) {
                continue;
            }
            let failed = textures.iter().find_map(|n| match assets.states.get(n) {
                Some(AssetState::Failed(e)) => Some(e.clone()),
                _ => None,
            });
            let next = if let Some(reason) = failed {
                AssetState::Failed(reason)
            } else if textures
                .iter()
                .all(|n| assets.states.get(n) == Some(&AssetState::Loaded))
            {
                AssetState::Loaded
            } else {
                continue;
            };
            // Write only a change: a rewrite moves the states' revision.
            if assets.states.get(&name) != Some(&next) {
                assets.states.insert(name, next);
            }
        }
        if self.setup_pending && self.required_ready() {
            self.world = Self::build(&self.args, self.world.assets.clone());
            self.setup_pending = false;
            self.asset_mesh_revision = u64::MAX;
        }
    }
    /// `Assets::ready` resumed from the last required name found not ready:
    /// the names before it were ready when passed, so a load costs one pass
    /// over the required set, not one per delivery. A state can move back
    /// (a retired, re-requested name), so the whole set is checked once more
    /// before setup.
    fn required_ready(&mut self) -> bool {
        use crate::asset::AssetState;
        let assets = &self.world.assets;
        let unready = |n: &&String| {
            assets
                .states
                .get(n)
                .is_some_and(|s| *s != AssetState::Loaded)
        };
        let from = self.ready_hint.take();
        let next = match &from {
            Some(hint) => assets
                .required
                .range::<String, _>(hint.clone()..)
                .find(unready),
            None => assets.required.iter().find(unready),
        };
        if let Some(name) = next {
            self.ready_hint = Some(name.clone());
            return false;
        }
        from.is_none() || assets.ready()
    }
    /// Transport failure after the host's bounded retries.
    pub fn asset_failed(&mut self, name: &str, reason: &str) {
        self.world.assets.request(name);
        self.world.assets.redelivery.remove(name);
        self.world.assets.requested.insert(name.into());
        self.asset_prepared(name, Err(reason.into()));
    }
    /// Install a validated content result. Decoding belongs to the model adapter.
    pub fn deliver_asset(
        &mut self,
        name: &str,
        result: Result<crate::asset::Content, String>,
    ) -> Result<(), String> {
        use crate::asset::{AssetState, Content};
        if name == crate::asset::TABLE_ASSET {
            if let Ok(Content::Tables(text)) = &result {
                self.deliver_tables(text)?;
            } else {
                return Err("invalid tuning transaction".into());
            }
            self.world.assets.requested.insert(name.into());
            self.world.assets.redelivery.remove(name);
            self.finish_assets(name);
            return Ok(());
        }
        self.world.assets.request(name);
        self.world.assets.requested.insert(name.into());
        self.world.assets.redelivery.remove(name);
        let result = result.and_then(|content| match content {
            Content::Level(text) => self.deliver_level(name, text).map(|()| None),
            Content::Sound(sound) => self.deliver_sound(name, sound).map(|()| None),
            other => Ok(Some(other)),
        });
        match result {
            Ok(None) => {}
            Ok(Some(Content::Level(_) | Content::Sound(_) | Content::Tables(_))) => unreachable!(),
            Ok(Some(Content::Texture(texture))) => {
                self.world
                    .assets
                    .states
                    .insert(name.into(), AssetState::Loaded);
                if self.defer_assets {
                    self.textures.insert(name.into(), texture);
                }
            }
            Ok(Some(Content::Model(model))) => {
                for texture in &model.textures {
                    self.world.assets.request(texture);
                    if self.world.assets.declared.contains(name) {
                        self.world.assets.required.insert(texture.clone());
                    }
                }
                self.world
                    .assets
                    .set_dependencies(name, model.textures.clone());
                self.world.assets.models.insert(name.into(), model.into());
            }
            Err(reason) => {
                self.asset_failed(name, &reason);
                return Err(format!("asset `{name}`: {reason}"));
            }
        }
        self.finish_assets(name);
        Ok(())
    }
}
