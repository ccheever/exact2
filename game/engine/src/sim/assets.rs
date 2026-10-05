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
        // Only a changed set of drawn model names reaches the requests below.
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
                        .1
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
}

/// The distinct model names `DrawnMesh` rows draw, sorted; true when they
/// differ from `names` (which then takes them). Reads the rows only when the
/// column changed since `names` was taken.
fn drawn_names_changed(w: &World, names: &mut ((u64, u64), Vec<String>)) -> bool {
    let at = (
        w.presentation_generation(),
        w.revision::<crate::DrawnMesh>(),
    );
    if std::mem::replace(&mut names.0, at) == at {
        return false;
    }
    let mut drawn = std::collections::BTreeSet::new();
    let mut rows = w.query::<&crate::DrawnMesh>();
    for (_, d) in rows.iter() {
        if let crate::Mesh::Asset(name) = &d.mesh {
            drawn.insert(name.as_str());
        }
    }
    if drawn.iter().copied().eq(names.1.iter().map(String::as_str)) {
        return false;
    }
    names.1 = drawn.into_iter().map(str::to_owned).collect();
    true
}
