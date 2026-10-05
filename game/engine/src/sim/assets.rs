//! First-sight asset requests: which models and textures the host should
//! fetch next, and which unshown cosmetics retire.
use super::*;

impl<G: Game> Sim<G> {
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
        // Present rewrites DrawnMesh rows at every boundary: only a changed
        // set of drawn model names reaches the requests below.
        let drawn_changed = drawn_names_changed(&self.world, &mut self.asset_drawn_names);
        if revision != self.asset_mesh_revision || sprites_changed || drawn_changed {
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
                    self.asset_drawn_names
                        .iter()
                        .map(|name| (name.clone(), ".model")),
                )
                .chain(
                    self.world
                        .query::<&crate::Sprite>()
                        .iter()
                        .map(|(_, s)| (s.texture.clone(), ".tex")),
                )
                .collect();
            let mut roots: std::collections::BTreeSet<_> =
                names.iter().map(|(n, _)| n.clone()).collect();
            if self.world.assets.shown != roots {
                self.world.assets.shown = roots.clone();
            }
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
                    // A generated model is registered, never fetched: pending, it
                    // waits for its textures and is Loaded when they arrive.
                    && !(n.ends_with(".model") && assets.identities.contains_key(*n))
            })
            .map(|(n, _)| n.clone())
            .collect();
        // Streamed names nothing shows are asked for only once nothing else is
        // in flight: a host fetches in request order, so they never queue ahead
        // of what setup waits for, what is shown, or the textures those name.
        let unshown = |n: &str| G::STREAMED.contains(&n) && !assets.shown.contains(n);
        if assets
            .states
            .iter()
            .any(|(n, s)| *s == crate::asset::AssetState::Pending && !unshown(n))
        {
            names.retain(|n| !unshown(n));
        }
        // What setup and the first frame wait for goes first.
        names.sort_by_key(|n| (unshown(n), !assets.required.contains(n)));
        assets.requested.extend(names.iter().cloned());
        names
    }
    /// `Game::STREAMED` models delivered but not yet prepared for the device,
    /// those an entity shows first. A device-backed surface prepares none
    /// before its first drawn frame and a few per frame after it, so streamed
    /// content never delays or bloats that frame; until prepared, one draws
    /// as if still in flight.
    pub fn streamed_unprepared(&self) -> Vec<&str> {
        let assets = &self.world.assets;
        let mut names: Vec<&str> = G::STREAMED
            .iter()
            .copied()
            .filter(|n| {
                n.ends_with(".model")
                    && assets.models.contains_key(n)
                    && !assets.prepared.contains(*n)
                    && matches!(
                        assets.states.get(n),
                        Some(crate::asset::AssetState::Pending | crate::asset::AssetState::Loaded)
                    )
            })
            .collect();
        names.sort_by_key(|n| !assets.shown.contains(*n));
        names
    }
}

/// The distinct model names `DrawnMesh` rows draw, sorted; true when they
/// differ from `names` (which then takes them).
fn drawn_names_changed(w: &World, names: &mut Vec<String>) -> bool {
    let mut drawn = std::collections::BTreeSet::new();
    let mut rows = w.query::<&crate::DrawnMesh>();
    for (_, d) in rows.iter() {
        if let crate::Mesh::Asset(name) = &d.mesh {
            drawn.insert(name.as_str());
        }
    }
    if drawn.iter().copied().eq(names.iter().map(String::as_str)) {
        return false;
    }
    *names = drawn.into_iter().map(str::to_owned).collect();
    true
}
