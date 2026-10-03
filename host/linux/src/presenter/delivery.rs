use super::*;

impl<D: DataSource> Presenter<D> {
    /// Attach the update store (LLP 1026 D9): what it has to say reaches
    /// the runner now (`state.delivery`, the `delivery` resource) and after
    /// every check; a boot note it left goes to the journal.
    pub fn set_updates(&mut self, updates: Option<Box<dyn crate::delivery::Store>>) {
        self.updates = updates;
        if let Some(note) = self.updates.as_mut().and_then(|u| u.take_note()) {
            self.host.log(note);
        }
        self.sync_delivery();
    }

    /// Remember the accepted pair for future plan-only development reloads.
    pub fn set_module(&mut self, module: Option<crate::delivery::Module>) {
        self.module = module;
    }

    /// Connect the existing development URL loop; polling starts after first pixel.
    pub fn set_development(&mut self, url: Option<String>, identity: Option<String>) {
        self.dev = url.map(|url| crate::fetch::Poller::new(url, self.compat.clone(), identity));
    }

    /// Apply a finished URL generation without blocking the presentation thread.
    pub fn poll_development(&mut self, data: impl FnOnce() -> D) {
        if !self.painted {
            return;
        }
        match self.dev.as_mut().map(|dev| dev.poll()) {
            Some(Ok(Some(generation))) => self.pending_dev = Some(generation),
            Some(Err(error)) => eprintln!("exact url: {error}; keeping the running app"),
            _ => {}
        }
        if let Some(generation) = self.pending_dev.take() {
            match self.reload_module(&generation.plan, data(), generation.module.clone()) {
                Ok(_) => self.dev.as_mut().unwrap().identity = Some(generation.identity),
                Err(HostError::PreparingModule) => self.pending_dev = Some(generation),
                Err(error) => eprintln!("exact url: candidate refused: {error}"),
            }
        }
    }

    pub(super) fn prepare_logic(
        &self,
        plan: &[u8],
        admitted: D,
        module: Option<&crate::delivery::Module>,
        text: &Shared,
        carried: &mut exact_runner::Carried,
        delivery: &exact_runner::Delivery,
    ) -> Result<D, HostError> {
        let Some(module) = module else {
            return Ok(admitted);
        };
        if self.host.runner().has_pending() {
            return Err(HostError::Asset(
                "Rust replacement waits for pending requests; retry after they settle".into(),
            ));
        }
        let data = module
            .replacement(plan, &admitted)
            .map_err(HostError::Asset)?;
        if self.painted {
            if !data
                .preload()
                .map_err(|e| HostError::Asset(format!("candidate module: {e:?}")))?
            {
                return Err(HostError::PreparingModule);
            }
            let mut validation = module
                .replacement(plan, &admitted)
                .map_err(HostError::Asset)?;
            validation
                .activate_for_validation()
                .map_err(|e| HostError::Asset(format!("candidate module: {e:?}")))?;
            let (mut host, error) = Host::boot_with(
                plan,
                validation,
                Box::new(Measurer(text.clone())),
                self.viewport.0,
                self.viewport.1,
                Some(carried),
                Some(delivery.clone()),
            )?;
            if let Some(error) = error {
                return Err(HostError::Layout(error));
            }
            self.restore_time(&mut host)?;
            let mut validated = host.carry();
            validated.store = carried.store.clone();
            *carried = validated;
        }
        Ok(data)
    }

    /// The store's wake, for the display loop's poll set.
    pub fn update_fd(&self) -> Option<std::os::unix::io::RawFd> {
        self.updates.as_ref().map(|u| u.fd())
    }

    /// First pixel (LLP 1026 D11): the selection that booted is good.
    pub fn first_pixel(&mut self) {
        if self.display.blocked()
            || self.dirty
            || !self.last_frame_succeeded
            || self.activation_failed
        {
            return;
        }
        self.activate_first_pixel();
    }

    /// Wake an idle display while an executable image is loading off-thread.
    pub fn module_pending(&self) -> bool {
        (self.painted && !self.activation_failed && self.host.data_pending())
            || self.pending_dev.is_some()
            || self.pending_update
    }

    /// Check the stream's head now, on the store's thread; the outcome
    /// arrives through `poll_update`. `false` with no store, or a check
    /// already running.
    pub fn check_update(&mut self) -> bool {
        match &self.updates {
            Some(u) => u.check(),
            None => false,
        }
    }

    /// A finished check, if one landed: its line to stderr and the journal
    /// (`exact update: …`), the store's facts into the runner.
    pub fn poll_update(&mut self) -> bool {
        let Some(line) = self.updates.as_mut().and_then(|u| u.take_line()) else {
            return false;
        };
        eprintln!("exact update: {line}");
        self.host.log(format!("exact update: {line}"));
        self.sync_delivery();
        true
    }

    /// Apply the staged bundle now with carry (`deliveryActivate`, LLP 1030
    /// D7): its assets stand in for the root's by name, its plan restarts
    /// the app. `Ok(false)` when nothing is staged.
    pub fn activate_update(&mut self, data: D) -> Result<bool, HostError> {
        if self.content_registration.is_some() {
            return Err(HostError::Layout(
                "content-region trial requires session retirement before update".into(),
            ));
        }
        let Some(updates) = self.updates.as_ref() else {
            return Ok(false);
        };
        let Some(candidate) = updates.prepare_activation().map_err(HostError::Asset)? else {
            return Ok(false);
        };
        let assets = Assets::selected(self.assets.root().to_path_buf(), candidate.assets.clone());
        let decoded = Plan::decode(&candidate.plan).map_err(HostError::Plan)?;
        let text = TextEngine::shared_for_assets(&decoded, &assets);
        let mut delivery = self.host.runner().delivery().with_compat(&self.compat);
        updates.status_into(&mut delivery);
        updates.staged_stream_into(&mut delivery);
        delivery.seq = candidate.seq;
        delivery.staged = false;
        let module =
            crate::delivery::Module::resolve(&candidate.assets).map_err(HostError::Asset)?;
        let kept = self.focus_place();
        let mut carried = self.host.carry();
        let data = self.prepare_logic(
            &candidate.plan,
            data,
            module.as_ref(),
            &text,
            &mut carried,
            &delivery,
        )?;
        let (mut host, error) = Host::boot_with(
            &candidate.plan,
            data,
            Box::new(Measurer(text.clone())),
            self.viewport.0,
            self.viewport.1,
            Some(&carried),
            Some(delivery),
        )?;
        if let Some(error) = error {
            return Err(HostError::Layout(error));
        }
        self.restore_time(&mut host)?;
        let mut images = self.images.candidate(assets.clone());
        if !images.prepare_metadata(host.kernel(), &host.preorder(), Duration::from_secs(1)) {
            return Err(HostError::Layout(
                "selected image metadata did not finish preparing".into(),
            ));
        }
        if let Some(reason) = assets.take_refusal() {
            return Err(HostError::Asset(reason));
        }
        self.updates
            .as_mut()
            .unwrap()
            .commit_activation(candidate.entry, candidate.seq)
            .map_err(HostError::Asset)?;
        self.updates.as_mut().unwrap().boot_started();
        self.host = host;
        if self.display.new_session() {
            self.painted = false;
        }
        self.activation_failed = false;
        self.module = module;
        self.text = text.clone();
        self.brush.text = text;
        self.assets = assets;
        images.enable_decode();
        images.fit(self.viewport, self.brush.scale);
        self.images = images;
        self.executor = self.host.executor();
        self.parked.clear();
        self.scroll.clear();
        self.collection = collection::State::default();
        self.contact = None;
        self.retained_motion = None;
        self.transform_geometry = Default::default();
        self.arrange = None;
        self.brush.arrange_lift = None;
        self.page = (0.0, 0.0);
        self.restore_focus(kept);
        self.pointer = None;
        self.commands.clear();
        self.host.log("exact update: activated the staged bundle");
        self.sync_delivery();
        self.after_commit();
        Ok(true)
    }

    /// Commit changed store facts to the delivery resource (LLP 1030 D7).
    pub(super) fn sync_delivery(&mut self) {
        let Some(u) = &self.updates else {
            return;
        };
        let mut delivery = self.host.runner().delivery().clone();
        u.status_into(&mut delivery);
        if delivery == *self.host.runner().delivery() {
            return;
        }
        if let Some(e) = self.host.set_delivery(delivery) {
            eprintln!("exact: {e}");
        }
        if let Some(e) = self.after_commit() {
            eprintln!("exact: {e}");
        }
    }
}
