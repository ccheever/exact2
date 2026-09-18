use super::*;
impl<G: Game, P: Presentation> WorldSurface<G, P> {
    pub(super) fn cosmetic(&self, name: &str) -> bool {
        !G::ASSETS.contains(&name)
            && !G::assets().iter().any(|a| a.name == name)
            && !self.sim.as_ref().is_some_and(|s| {
                s.presentation_models()
                    .any(|(n, m)| G::ASSETS.contains(&n) && m.textures.iter().any(|t| t == name))
            })
    }
    pub(super) fn ready_reasons(&self) -> Vec<String> {
        let mut reasons = Vec::new();
        if let Some(sim) = &self.sim {
            reasons.extend(
                sim.world()
                    .loading()
                    .map(|n| format!("asset `{n}` Pending")),
            );
        } else {
            reasons.push("world is not bound".into());
        }
        if self.device_lost {
            reasons.push("device is lost".into());
        }
        if self.device {
            if self.render.is_none() || self.format.is_none() {
                reasons.push("renderer for current format is absent".into());
            }
            if !self.presented {
                reasons.push("first frame has not been presented".into());
            }
        } else if !self.presented {
            reasons.push("first headless feed has not completed".into());
        }
        if let Some(error) = &self.error {
            reasons.push(error.0.clone());
        }
        reasons
    }
    pub(super) fn mark_ready(&self) {
        if self.ready_reasons().is_empty() {
            self.audit.ready();
        }
    }
    pub(super) fn headless_feed(&mut self) {
        if self.device || self.device_lost || self.error.is_some() {
            return;
        }
        let Some(sim) = &self.sim else { return };
        let _scope = self.audit.enter();
        self.audit.tick(sim.world().tick());
        if self.recording.is_none() {
            let feed = match Feed::with_assets(G::assets()) {
                Ok(f) => f,
                Err(e) => {
                    self.error = Some(SurfaceError(e.to_string()));
                    return;
                }
            };
            self.recording = Some((crate::recording::Recording::new(self.audit.clone()), feed));
        }
        let (recording, feed) = self.recording.as_mut().unwrap();
        for (name, model) in sim.presentation_models() {
            let _name = self.audit.name(name, !G::ASSETS.contains(&name));
            recording.prepare(name, model);
        }
        if sim.world().loading().next().is_some() {
            return;
        }
        match recording.feed(feed, sim.world()) {
            Ok(()) => {
                self.presented = true;
                self.mark_ready();
            }
            Err(e) => self.error = Some(SurfaceError(e.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use exact_game::{Input, Material, Mesh, Transform};
    use serde_json::{json, Value as Json};
    fn state<G: Game>(s: &mut WorldSurface<G>) -> Json {
        serde_json::from_str::<Json>(&s.agent(r#"{"op":"state"}"#).unwrap()).unwrap()["world"]
            .clone()
    }
    struct Variant;
    impl Game for Variant {
        type Args = ();
        const ID: &'static str = "ready-negative-variant";
        fn setup(w: &mut World, _: &()) {
            w.spawn((Transform::default(), Mesh::cube(1.), Material::default()));
        }
        fn tick(w: &mut World, _: &Input, _: &()) {
            if w.tick() == 0 {
                w.spawn((Transform::default(), Mesh::sphere(1.), Material::default()));
            }
        }
    }
    #[test]
    fn never_seen_mesh_mid_play_trips_the_standard_zero_assertion() {
        let mut s = WorldSurface::<Variant>::default();
        assert!(!s.ready_reasons().is_empty());
        s.bind(&[]).unwrap();
        let first = state(&mut s);
        assert_eq!(first["ready"], true);
        assert_eq!(first["gpu"]["afterReady"]["violations"], 0);
        s.agent(r#"{"op":"clock","now":0}"#);
        s.agent(r#"{"op":"clock","now":17}"#);
        let later = state(&mut s);
        let gpu = &later["gpu"];
        assert!(
            gpu["afterReady"]["meshBytesUploaded"].as_u64().unwrap() > 0,
            "{later}"
        );
        assert!(gpu["afterReady"]["violations"].as_u64().unwrap() > 0);
        assert_eq!(gpu["lastAfterReady"]["name"], "Sphere");
        assert_eq!(gpu["lastAfterReady"]["tick"], 1);
        println!("negative variant: {gpu}");
        // Lifecycle recovery cannot move subsequent costs back before ready.
        s.device_ready();
        assert_eq!(state(&mut s)["ready"], false);
        s.device_lost();
        assert_eq!(state(&mut s)["ready"], false);
        assert_eq!(state(&mut s)["gpu"]["afterReady"], gpu["afterReady"]);
    }
    struct Growth;
    impl Game for Growth {
        type Args = ();
        const ID: &'static str = "ready-worst-growth";
        fn setup(w: &mut World, _: &()) {
            for _ in 0..256 {
                w.spawn((Transform::default(), Mesh::cube(1.), Material::default()));
            }
        }
        fn tick(w: &mut World, _: &Input, _: &()) {
            if w.tick() == 0 {
                // 10x the original entities; well past all initial feed capacities.
                for _ in 256..2560 {
                    w.spawn((Transform::default(), Mesh::cube(1.), Material::default()));
                }
            }
        }
    }
    #[test]
    fn tenfold_mid_play_growth_reports_real_buffer_and_slot_hitches() {
        let mut s = WorldSurface::<Growth>::default();
        s.bind(&[]).unwrap();
        assert_eq!(state(&mut s)["ready"], true);
        s.agent(r#"{"op":"clock","now":0}"#);
        s.agent(r#"{"op":"clock","now":17}"#);
        let later = state(&mut s);
        let counts = &later["gpu"]["afterReady"];
        assert_eq!(counts["bufferReallocations"], 4, "{later}");
        assert_eq!(counts["slotCapacityGrowth"], 2);
        assert_eq!(counts["meshBytesUploaded"], 0);
        assert!(counts["violations"].as_u64().unwrap() > 0);
        println!("10x growth: {}", later["gpu"]);
        // The recording feed refuses an explicit bound instead of allocating forever.
        use crate::world::Writes;
        let (recording, _) = s.recording.as_mut().unwrap();
        let error = recording.transforms(200_000, &[0.; 10], false).unwrap_err();
        assert_eq!(error.limit, 200_000);
    }
    struct Declared;
    impl Game for Declared {
        type Args = ();
        const ID: &'static str = "ready-assets";
        const ASSETS: &'static [&'static str] = &["crate.model"];
        fn setup(w: &mut World, _: &()) {
            w.spawn((Transform::default(), Mesh::asset("crate.model")));
        }
        fn tick(_: &mut World, _: &Input, _: &()) {}
    }
    #[test]
    fn pending_and_failed_readiness_uses_existing_asset_states() {
        let mut s = WorldSurface::<Declared>::default();
        s.bind(&[]).unwrap();
        let pending = state(&mut s);
        assert_eq!(pending["ready"], false);
        assert!(pending["readyReasons"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v.as_str().unwrap().contains("crate.model")));
        s.asset_failed("crate.model", "negative control: missing");
        let failed = state(&mut s);
        assert_eq!(failed["ready"], true, "{failed}");
        assert_eq!(failed["assets"][0]["state"], "Failed");
        assert!(failed["assets"][0]["reason"]
            .as_str()
            .unwrap()
            .contains("missing"));
    }
    struct Cosmetic;
    impl Game for Cosmetic {
        type Args = ();
        const ID: &'static str = "ready-cosmetic";
        fn setup(w: &mut World, _: &()) {
            w.spawn((Transform::default(), Mesh::asset("cosmetic.model")));
        }
        fn tick(_: &mut World, _: &Input, _: &()) {}
    }
    #[test]
    fn undeclared_asset_delivery_is_named_and_separate_from_violations() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../games/asset-fixture/art/crate.gltf");
        let (model, textures) = exact_game_bake::assets(&path).unwrap();
        let mut s = WorldSurface::<Cosmetic>::default();
        s.bind(&[]).unwrap();
        assert_eq!(s.assets(), ["cosmetic.model"]);
        s.asset("cosmetic.model", Some(&exact_game::bin::to_vec(&model)));
        for (name, data) in textures {
            s.asset(&name, Some(&exact_game::bin::to_vec(&data)));
        }
        let world = state(&mut s);
        let after = &world["gpu"]["afterReady"];
        assert_eq!(after["violations"], 0, "{world}");
        assert!(
            after["declaredExceptions"]["counts"]["meshBytesUploaded"]
                .as_u64()
                .unwrap()
                > 0
        );
        assert!(
            after["declaredExceptions"]["counts"]["textureBytesUploaded"]
                .as_u64()
                .unwrap()
                > 0
        );
        assert_ne!(after["declaredExceptions"]["last"]["name"], json!(null));
        assert_eq!(world["gpu"]["lastAfterReady"], json!(null));
    }
    #[test]
    fn declared_delivery_and_warm_feed_are_counted_before_ready() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../games/asset-fixture/art/crate.gltf");
        let (model, textures) = exact_game_bake::assets(&path).unwrap();
        let mut s = WorldSurface::<Declared>::default();
        s.bind(&[]).unwrap();
        s.asset("crate.model", Some(&exact_game::bin::to_vec(&model)));
        assert_eq!(state(&mut s)["ready"], false);
        for (name, data) in textures {
            s.asset(&name, Some(&exact_game::bin::to_vec(&data)));
        }
        let world = state(&mut s);
        assert_eq!(world["ready"], true);
        assert_eq!(world["gpu"]["afterReady"]["violations"], 0);
        assert!(
            world["gpu"]["beforeReady"]["meshBytesUploaded"]
                .as_u64()
                .unwrap()
                > 0
        );
        assert!(
            world["gpu"]["beforeReady"]["textureBytesUploaded"]
                .as_u64()
                .unwrap()
                > 0
        );
    }
}
