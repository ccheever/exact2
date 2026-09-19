use super::*;
impl<G: Game> Sim<G> {
    /// Device state at the host boundary, including events waiting for a tick.
    pub(crate) fn host_input(&self) -> Input {
        let mut input = self.input.clone();
        for queued in &self.queue {
            input.apply_paused(queued.event.clone());
        }
        input
    }
    /// Named contacts, including queued presses and releases at the host boundary.
    pub fn held_controls(&self) -> Vec<String> {
        self.host_input().held_controls()
    }
    pub(crate) fn capture_queue(&self) -> Vec<u8> {
        bin::to_vec(&(
            self.relative_queue(),
            self.queue.iter().map(|e| e.delivered).collect::<Vec<_>>(),
        ))
    }
    pub(super) fn relative_queue(&self) -> Vec<Queued> {
        let world_us = self.exact_world_us();
        let mut queue: Vec<_> = self.queue.iter().cloned().collect();
        for e in &mut queue {
            if let Some(us) = &mut e.world_us {
                *us -= world_us;
                e.host_us = 0;
            } else {
                e.host_us = e.host_us.saturating_sub(self.last_us.unwrap_or(0));
            }
            e.event.set_at_ms(0.0); // the queue owns the stamp; no absolute host time in a save
        }
        queue
    }
    /// Save world time and relative pending input, independent of the host epoch.
    pub fn save(&self) -> Result<Vec<u8>, DataError> {
        let assets = &self.world.assets;
        let mut needed = std::collections::BTreeSet::new();
        for (_, mesh) in self.world.query::<&crate::Mesh>().iter() {
            if let crate::Mesh::Asset(name) = mesh {
                needed.insert(name.clone());
            }
        }
        needed.extend(
            self.world
                .query::<&crate::Sprite>()
                .iter()
                .map(|(_, s)| s.texture.clone()),
        );
        for name in needed.clone() {
            if matches!(
                assets.states.get(&name),
                Some(crate::asset::AssetState::Failed(_))
            ) {
                continue;
            }
            if let Some(deps) = assets.dependencies.get(&name) {
                needed.extend(deps.iter().cloned());
            }
        }
        let pending: Vec<_> = needed
            .iter()
            .filter(|name| {
                !matches!(
                    assets.states.get(name),
                    Some(crate::asset::AssetState::Loaded | crate::asset::AssetState::Failed(_))
                )
            })
            .collect();
        if self.is_loading() || !pending.is_empty() {
            return Err(DataError::new(format!(
                "save refused: assets are not ready: {:?}; {}; inspect untargeted `state`: world[0].loading and world[0].assets before saving again",
                pending,
                assets.state_json()
            )));
        }
        let world_us = self.exact_world_us();
        let queue = self.relative_queue();
        let saved = Saved {
            game: G::ID.into(),
            world: self.world.save(),
            base: self.base.clone(),
            base_args: if self.base_args == self.args_json {
                String::new()
            } else {
                self.base_args.clone()
            },
            args: self.args_json.clone(),
            input: self.input.clone(),
            queue,
            world_us,
            published: self.world.publications(),
            journal: self.world.journal(),
            journal_next: self.world.journal_next(),
            overflow_logged: self.overflow_logged,
        };
        let mut w = bin::Encoder::prefixed(b"EXSIM\0\x07");
        saved.write(&mut w);
        Ok(w.finish())
    }
    /// Restore atomically; controlled clocks retain their anchor, live clocks exclude the loading gap.
    pub fn restore(&mut self, bytes: &[u8]) -> Result<(), DataError> {
        self.restore_into(bytes, None, None, true)
    }
    /// A surface retains the current app bindings, including setup arguments.
    pub fn restore_bound(&mut self, bytes: &[u8]) -> Result<(), DataError> {
        let args = crate::json::to_string(&self.args)?;
        self.restore_into(bytes, Some(&args), None, true)
    }
    /// Restore saved declarations exactly while retaining all current app bindings.
    pub fn open_bound(&mut self, bytes: &[u8]) -> Result<(), DataError> {
        let args = crate::json::to_string(&self.args)?;
        self.restore_into(bytes, Some(&args), None, false)
    }
    pub(super) fn restore_into(
        &mut self,
        bytes: &[u8],
        args: Option<&str>,
        budget: Option<&LoadBudget>,
        carry: bool,
    ) -> Result<(), DataError> {
        if self.setup_pending {
            return Err(DataError::new("restore refused: EXSIM v7 awaits declared assets; inspect untargeted `state`: world[0].loading and world[0].assets; retry after delivery"));
        }
        let mut next = Self::restore_candidate(
            bytes,
            args,
            self.world.assets.clone(),
            budget,
            carry,
            args.map(|_| (self.base.as_slice(), self.base_args.as_str())),
            Some(self.world.registered_scratch()),
        )?;
        next.defer_assets = self.defer_assets;
        next.world.presentation_generation = self
            .world
            .presentation_generation
            .checked_add(1)
            .expect("presentation generation exhausted");
        if let Some(now) = self.last_us.filter(|_| self.agent_owned) {
            next.rebase(now as f64 / 1000.0, false)
                .map_err(DataError::new)?;
        }
        self.capture_fail("world restored during recording; start a new capture window");
        next.recorder = self.recorder.take();
        next.agent_owned = self.agent_owned;
        next.contamination = self.contamination;
        next.source_tagged = self.source_tagged;
        // Content can arrive before the presenter drains the last texture.
        next.textures = std::mem::take(&mut self.textures);
        next.paranoid = self.paranoid;
        next.restarted = self.restarted;
        *self = next;
        Ok(())
    }
    /// Decode through `Game::register` before constructing once from saved arguments.
    pub fn from_save(bytes: &[u8]) -> Result<Self, DataError> {
        Self::restore_candidate(bytes, None, Default::default(), None, false, None, None)
    }
    pub(crate) fn from_save_in(
        bytes: &[u8],
        assets: crate::asset::AssetStore,
        budget: Option<&LoadBudget>,
    ) -> Result<Self, DataError> {
        Self::restore_candidate(bytes, None, assets, budget, false, None, None)
    }
    pub(super) fn restore_candidate(
        bytes: &[u8],
        args: Option<&str>,
        assets: crate::asset::AssetStore,
        budget: Option<&LoadBudget>,
        carry: bool,
        authored: Option<(&[u8], &str)>,
        registry: Option<World>,
    ) -> Result<Self, DataError> {
        let payload = bytes.strip_prefix(b"EXSIM\0\x07").ok_or_else(|| {
            DataError::new(format!(
                "restore refused: unsupported simulation save format (expected EXSIM v7; saw {:02x?}); no cross-version migration before 1.0: recreate with `screenshot checkpoint.world world save`; inspect `state`",
                &bytes[..bytes.len().min(8)]
            ))
        })?;
        let s: Saved = bin::from_slice_in(payload, budget)?;
        if s.game != G::ID {
            return Err(DataError::new(format!(
                "restore refused: EXSIM v7 save belongs to `{}`, expected `{}`; game IDs must match, no cross-game migration; inspect `state`",
                s.game,
                G::ID
            )));
        }
        if s.world_us < 0 || s.queue.len() > QUEUE_LIMIT {
            return Err(DataError::new("restore refused: EXSIM v7 has invalid saved clock or input queue; no repair migration; inspect `state` and create a fresh save"));
        }
        World::saved_payload(&s.world)?;
        let bound: G::Args = crate::json::from_str_in(args.unwrap_or(&s.args), budget)?;
        bound.check_scalars().map_err(DataError::new)?;
        G::validate(&bound).map_err(DataError::new)?;
        let initial: G::Args = if args.is_none() && !s.base_args.is_empty() {
            crate::json::from_str_in(&s.base_args, budget)?
        } else {
            crate::json::from_str_in(args.unwrap_or(&s.args), budget)?
        };
        initial.check_scalars().map_err(DataError::new)?;
        G::validate(&initial).map_err(DataError::new)?;
        // The entire typed saved world, including hierarchy and clock, must
        // decode before any setup or authored initializer can run.
        let mut selected = World::new(G::HZ, 0);
        G::register(&mut selected, &initial);
        G::register(&mut selected, &bound);
        if let Some(live) = registry {
            selected.inherit_registry(&live);
        }
        let validated = selected.validate_saved_in(&s.world, budget)?;
        let due = s.world_us as u128 * G::HZ as u128 / 1_000_000;
        if validated.hz() != G::HZ || validated.tick() as u128 != due {
            return Err(DataError::new("restore refused: EXSIM v7 saved world and clock disagree; no clock migration; inspect `state` and create a fresh save"));
        }
        drop(validated);
        let input = Input::new(G::actions());
        input
            .validate_saved(&s.input)
            .map_err(|e| DataError::new(format!("restore refused: {e}")))?;
        for event in &s.queue {
            input
                .validate(&event.event)
                .map_err(|e| DataError::new(format!("restore refused: {e}")))?;
            if let Some(us) = event.world_us {
                us.checked_add(s.world_us).ok_or_else(|| {
                    DataError::new("restore refused: EXSIM v7 saved input stamp overflow")
                })?;
            }
        }
        let mut next = Self::with_store(initial, assets, 0).map_err(DataError::new)?;
        next.args_json = crate::json::to_string(&bound)?;
        next.args = bound;
        if next.setup_pending {
            return Err(DataError::new("restore refused: EXSIM v7 awaits declared assets; inspect untargeted `state`: world[0].loading and world[0].assets; retry after delivery"));
        }
        next.world.load_in(&s.world, budget)?;
        let due = s.world_us as u128 * G::HZ as u128 / 1_000_000;
        if next.world.hz() != G::HZ || next.world.tick() as u128 != due {
            return Err(DataError::new("restore refused: EXSIM v7 saved world and clock disagree; no clock migration; inspect `state` and create a fresh save"));
        }
        // Typed scratch decode precedes setup; authored merging follows both.
        if carry {
            if let Some((base, base_args)) = authored {
                next.base = base.to_vec();
                next.base_args = base_args.to_owned();
            }
            next.reload = next.world.merge_initializer(&s.base, &next.base, budget)?;
            // Independently valid authored/runtime edges can form a cycle when
            // combined. Refuse the candidate rather than repair a carried edge.
            next.world
                .validate_hierarchy(&mut bin::Decoder::for_load(&[], budget))?;
        } else {
            next.base = s.base;
            next.base_args = if s.base_args.is_empty() {
                s.args.clone()
            } else {
                s.base_args
            };
        }
        crate::scene::place_followers(&next.world);
        next.world.propagate();
        next.world.restore_journal(s.journal, s.journal_next);
        next.reload.log(&next.world);
        next.world.restore_publications(s.published);
        next.world.published_pending.set(true);
        next.input
            .validate_saved(&s.input)
            .map_err(|e| DataError::new(format!("restore refused: {e}")))?;
        for event in &s.queue {
            next.input
                .validate(&event.event)
                .map_err(|e| DataError::new(format!("restore refused: {e}")))?;
        }
        next.input.restore_dynamic(s.input);
        next.queue = s.queue.into();
        for e in &mut next.queue {
            if let Some(us) = &mut e.world_us {
                *us = us
                    .checked_add(s.world_us)
                    .ok_or_else(|| DataError::new("restore refused: EXSIM v7 saved input stamp overflow; no clock migration; inspect `state` and create a fresh save"))?;
            }
        }
        next.world_us = s.world_us;
        next.world.unobserve();
        next.restored_from = Some(s.args);
        next.overflow_logged = s.overflow_logged;
        next.rebase_queue = true;
        next.restored = true;
        Ok(next)
    }
}

#[cfg(test)]
mod reload_atomic_tests {
    use super::*;
    use crate::{Transform, Vec3};
    struct Probe<const EDITED: bool>;
    impl<const EDITED: bool> Game for Probe<EDITED> {
        const ID: &'static str = "typed-then-authored";
        type Args = ();
        fn setup(w: &mut World, _: &()) {
            w.spawn_named(
                "probe",
                Transform {
                    scale: Vec3::splat(if EDITED { 2. } else { 1. }),
                    ..Default::default()
                },
            );
        }
        fn tick(_: &mut World, _: &Input, _: &()) {}
    }
    fn encode(saved: &Saved) -> Vec<u8> {
        let mut w = bin::Encoder::prefixed(b"EXSIM\0\x07");
        saved.write(&mut w);
        w.finish()
    }
    #[test]
    fn clock_disagreement_and_invalid_queue_refuse_without_applying_authored_changes() {
        let mut old = Sim::<Probe<false>>::new(()).unwrap();
        old.run(17.);
        let bytes = old.save().unwrap();
        let mut target = Sim::<Probe<true>>::new(()).unwrap();
        let before = target.save().unwrap();
        let report = target.reload.json();
        let mut saved: Saved = bin::from_slice(&bytes[7..]).unwrap();
        saved.world_us = 0; // tick=1 is not accepted as due+1.
        let error = target.restore_bound(&encode(&saved)).unwrap_err();
        assert!(error.message.contains("clock disagree"), "{error}");
        assert_eq!(target.save().unwrap(), before);
        assert_eq!(target.reload.json(), report);
        saved = bin::from_slice(&bytes[7..]).unwrap();
        saved.queue.push(Queued {
            event: InputEvent::Control {
                name: "missing".into(),
                id: 1,
                phase: crate::PointerPhase::Down,
                x: 0.,
                y: 0.,
                at_ms: 0.,
            },
            ..Default::default()
        });
        let error = target.restore_bound(&encode(&saved)).unwrap_err();
        assert!(error.message.contains("missing"), "{error}");
        assert_eq!(target.save().unwrap(), before);
        assert_eq!(target.reload.json(), report);
        // Negative control: the same complete typed world with valid input merges.
        target.restore_bound(&bytes).unwrap();
        assert_eq!(
            target.world().require::<Transform>("probe").scale,
            Vec3::splat(2.)
        );
        assert_eq!(target.world().tick(), 1);
        assert_ne!(target.reload.json(), report);
    }
}
