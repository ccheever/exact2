use super::*;

impl<G: Game> Sim<G> {
    /// Why no save can be taken now: a shown asset still in flight.
    pub(crate) fn assets_unready(&self) -> Option<String> {
        {
            let assets = &self.world.assets;
            let mut meshes = self.world.query::<&crate::Mesh>();
            let mut sprites = self.world.query::<&crate::Sprite>();
            let mut needed = std::collections::BTreeSet::new();
            for (_, mesh) in meshes.iter() {
                if let crate::Mesh::Asset(name) = mesh {
                    needed.insert(name);
                }
            }
            needed.extend(sprites.iter().map(|(_, s)| &s.texture));
            for name in needed.clone() {
                if matches!(
                    assets.states.get(name),
                    Some(crate::asset::AssetState::Failed(_))
                ) {
                    continue;
                }
                if let Some(deps) = assets.dependencies.get(name) {
                    needed.extend(deps);
                }
            }
            let pending: Vec<_> = needed
                .iter()
                .filter(|name| {
                    !matches!(
                        assets.states.get(name),
                        Some(
                            crate::asset::AssetState::Loaded | crate::asset::AssetState::Failed(_)
                        )
                    )
                })
                .collect();
            (self.is_loading() || !pending.is_empty()).then(|| format!(
                "save refused: assets are not ready: {:?}; {}; inspect untargeted `state`: world[0].loading and world[0].assets before saving again",
                pending,
                assets.state_json()
            ))
        }
    }
    /// Save world time and relative pending input, independent of the host epoch.
    pub fn save(&self) -> Result<Vec<u8>, DataError> {
        if let Some(reason) = self.assets_unready() {
            return Err(DataError::new(reason));
        }
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
        // Restore clears tick-local edges; checkpoints keep continuation state.
        let mut input = self.input.clone();
        input.clear_edges();
        let saved = Saved {
            game: G::ID.into(),
            world: self.world.save(),
            args: self.args_json.clone(),
            input,
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
    /// Atomically restore dynamic state onto this binary's actions and a new epoch.
    /// Preflight extends the live type registry through `Game::register` for the chosen arguments.
    pub fn restore(&mut self, bytes: &[u8]) -> Result<(), DataError> {
        self.restore_into(bytes, false)
    }
    /// A surface retains the current app bindings, including setup arguments.
    pub fn restore_bound(&mut self, bytes: &[u8]) -> Result<(), DataError> {
        self.restore_into(bytes, true)
    }
    pub(super) fn restore_into(
        &mut self,
        bytes: &[u8],
        retain_args: bool,
    ) -> Result<(), DataError> {
        if bytes.starts_with(b"EXSIM\0\x06") {
            return Err(DataError::new("restore refused: an EXSIM v6 save predates v7's primary pointer press origin (`PointerState::press_origin`); no cross-version migration before 1.0: recreate with `screenshot checkpoint.world world save`; inspect `state`"));
        }
        let payload = bytes.strip_prefix(b"EXSIM\0\x07").ok_or_else(|| {
            DataError::new(format!(
                "restore refused: unsupported simulation save format (expected EXSIM v7; saw {:02x?}); no cross-version migration before 1.0: recreate with `screenshot checkpoint.world world save`; inspect `state`",
                &bytes[..bytes.len().min(8)]
            ))
        })?;
        let mut s: Saved = bin::from_slice(payload)?;
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
        if self.setup_pending {
            return Err(DataError::new("restore refused: EXSIM v7 awaits declared assets; inspect untargeted `state`: world[0].loading and world[0].assets; retry after delivery"));
        }
        let args = if retain_args {
            &self.args_json
        } else {
            &s.args
        };
        let bound: G::Args = crate::json::from_str(args)?;
        bound.check_scalars().map_err(DataError::new)?;
        G::validate(&bound).map_err(DataError::new)?;
        let mut input = Input::new(G::actions());
        input.validate_saved(&s.input).map_err(DataError::new)?;
        for event in &mut s.queue {
            input.validate(&event.event).map_err(DataError::new)?;
            if let Some(us) = &mut event.world_us {
                *us = us.checked_add(s.world_us).ok_or_else(|| DataError::new(
                    "restore refused: EXSIM v7 saved input stamp overflow; inspect state and create a fresh save"))?;
            }
        }
        let mut registry = self.world.registered_scratch();
        Self::register(&mut registry, &bound);
        let validated = registry.validate_saved(&s.world)?;
        let due = s.world_us as u128 * G::HZ as u128 / 1_000_000;
        if validated.hz() != G::HZ || validated.tick() as u128 != due {
            return Err(DataError::new("restore refused: EXSIM v7 saved world and clock disagree; no clock migration; inspect `state` and create a fresh save"));
        }
        input.restore_dynamic(s.input);
        let mut next =
            Self::from_world(bound, validated, input, s.queue.into()).map_err(DataError::new)?;
        next.world.assets = self.world.assets.clone();
        next.defer_assets = self.defer_assets;
        crate::scene::place_followers(&next.world);
        next.world.propagate();
        // The restored world is untrusted input: a present that fails on it is
        // a refused restore, not a crashed host (where unwinding is available).
        let presented = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Self::present(&mut next.world, &next.args)
        }));
        if let Err(panic) = presented {
            let reason = panic
                .downcast_ref::<String>()
                .map(String::as_str)
                .or_else(|| panic.downcast_ref::<&str>().copied())
                .unwrap_or("panicked");
            return Err(DataError::new(format!(
                "restore refused: Game::present failed on the restored world: {reason}"
            )));
        }
        next.world.restore_journal(s.journal, s.journal_next);
        next.world.restore_publications(s.published);
        next.world.published_pending.set(true);
        next.world_us = s.world_us;
        next.world.unobserve();
        next.restored_from = Some(s.args);
        next.overflow_logged = s.overflow_logged;
        next.rebase_queue = true;
        next.restored = true;
        next.world.presentation_generation = self
            .world
            .presentation_generation
            .checked_add(1)
            .expect("presentation generation exhausted");
        // A deferred restore may commit on the last texture's content delivery,
        // before the presenter has drained that payload into the current device.
        next.textures = std::mem::take(&mut self.textures);
        next.paranoid = self.paranoid;
        next.restarted = self.restarted;
        *self = next;
        Ok(())
    }
}
