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
            if self.setup_pending {
                roots.extend(G::ASSETS.iter().map(|n| (*n).to_owned()));
            }
            self.world.assets.retire(&roots);
            self.textures
                .retain(|n, _| self.world.assets.states.contains_key(n));
            for (name, suffix) in names {
                if !self.world.assets.request(&name) {
                    continue;
                }
                if !name.ends_with(suffix) {
                    self.asset_failed(&name, &format!("component requires a {suffix} name"));
                }
            }
            self.asset_mesh_revision = revision;
        }
        let assets = &mut *self.world.assets;
        let names: Vec<_> = assets
            .states
            .iter()
            .filter(|(n, s)| {
                (**s == crate::asset::AssetState::Pending || assets.redelivery.contains(*n))
                    && !assets.requested.contains(*n)
            })
            .map(|(n, _)| n.clone())
            .collect();
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
            *s != crate::asset::AssetState::Loaded || self.world.assets.prepared.contains(n)
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
        self.finish_assets();
    }
    pub(super) fn finish_assets(&mut self) {
        use crate::asset::AssetState;
        let assets = &mut *self.world.assets;
        for (name, textures) in &assets.dependencies {
            if !assets.states.contains_key(name) {
                continue;
            }
            if matches!(assets.states.get(name), Some(AssetState::Failed(_))) {
                continue;
            }
            let failed = textures.iter().find_map(|n| match assets.states.get(n) {
                Some(AssetState::Failed(e)) => Some(e.clone()),
                _ => None,
            });
            if let Some(reason) = failed {
                assets
                    .states
                    .insert(name.clone(), AssetState::Failed(reason));
            } else if textures
                .iter()
                .all(|n| assets.states.get(n) == Some(&AssetState::Loaded))
            {
                assets.states.insert(name.clone(), AssetState::Loaded);
            }
        }
        if self.setup_pending && assets.ready() {
            self.world = Self::build(&self.args, self.world.assets.clone());
            self.base = self
                .world
                .initializer()
                .expect("deferred authored initializer exceeds its work bound");
            self.base_args = self.args_json.clone();
            self.base_authored = true;
            self.reload = Default::default();
            self.setup_pending = false;
            self.asset_mesh_revision = u64::MAX;
        }
    }
    /// Transport failure after the host's bounded retries.
    pub fn asset_failed(&mut self, name: &str, reason: &str) {
        if !self.world.assets.request(name) {
            return;
        }
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
        if !self.world.assets.request(name) {
            return Err(format!("asset `{name}`: surface limit is 256 names"));
        }
        self.world.assets.requested.insert(name.into());
        self.world.assets.redelivery.remove(name);
        match result {
            Ok(Content::Texture(texture)) => {
                self.world
                    .assets
                    .states
                    .insert(name.into(), AssetState::Loaded);
                if self.defer_assets {
                    self.textures.insert(name.into(), texture);
                }
            }
            Ok(Content::Model(model)) => {
                let extra = model
                    .textures
                    .iter()
                    .filter(|n| !self.world.assets.states.contains_key(n))
                    .count();
                if self.world.assets.states.len() + extra > 256 {
                    self.asset_failed(name, "dependencies exceed surface limit of 256 names");
                    return Err(format!(
                        "asset `{name}`: dependencies exceed surface limit of 256 names"
                    ));
                }
                for texture in &model.textures {
                    self.world.assets.request(texture);
                    if self.world.assets.declared.contains(name) {
                        self.world.assets.required.insert(texture.clone());
                    }
                }
                self.world
                    .assets
                    .dependencies
                    .insert(name.into(), model.textures.clone());
                self.world.assets.models.insert(name.into(), model.into());
            }
            Err(reason) => {
                self.asset_failed(name, &reason);
                return Err(format!("asset `{name}`: {reason}"));
            }
        }
        self.finish_assets();
        Ok(())
    }
    pub(super) fn decode_args(values: &[Value]) -> Result<G::Args, String> {
        crate::args::arity(values, G::Args::FIELDS)?;
        let mut complete = G::Args::default().values();
        complete[..values.len()].clone_from_slice(values);
        G::Args::decode(&complete)
    }
}
