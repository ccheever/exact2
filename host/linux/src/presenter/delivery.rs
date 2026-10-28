use super::*;
use crate::host::LET_GO_AFTER;
use std::time::Instant;

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

    /// A dev loop's status the boot already said: the poll says it again
    /// only once it changes.
    pub fn said_at_boot(&mut self, said: Option<String>) {
        if let (Some(dev), Some(said)) = (self.dev.as_mut(), said) {
            dev.says(&said);
        }
    }

    /// Connect the existing development URL loop; polling starts after first pixel.
    pub fn set_development(&mut self, url: Option<String>, identity: Option<String>) {
        self.dev = url.map(|url| crate::fetch::Poller::new(url, self.compat.clone(), identity));
        // Its answers wake the executor's fd, so a host that turns only when
        // woken (the Android reader's canvas host) takes them at once.
        let waker = self.executor.waker();
        if let Some(dev) = self.dev.as_mut() {
            *dev.waker.lock().unwrap_or_else(|e| e.into_inner()) = Some(waker);
        }
        // What the build channel's registration names (LLP 1122 D4).
        if let Some(link) = self.dev.as_mut().and_then(|dev| dev.link.as_mut()) {
            link.design = Some(self.host.runner().design());
            link.plan_digest = Some(self.host.runner().inspection_digest().to_string());
        }
    }

    /// Milliseconds from `now` until a turn is owed: the next timer, or the
    /// development poll's next tick when sooner (attached, and the first
    /// pixel out: it polls after). A host that turns only when woken asks
    /// this (the Android reader's canvas host).
    pub fn turn_due_ms(&self, now: f64) -> Option<f64> {
        let timer = self.timer_due_ms().map(|due| (due - now).max(0.0));
        // (A load to send, or a picture asking again for a failed download, as soon.)
        let timer = timer
            .into_iter()
            .chain(self.image_turn_due_in_ms())
            .reduce(f64::min);
        let dev = self.dev.as_ref().filter(|_| self.painted);
        let poll = dev
            .and_then(|d| d.due_in())
            .map(|d| d.as_secs_f64() * 1000.0);
        match (timer, poll) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }

    /// Apply a finished URL generation without blocking the presentation thread.
    pub fn poll_development(&mut self, data: impl FnOnce() -> D) {
        if !self.painted {
            return;
        }
        self.tick_development();
        // The build channel (LLP 1122 D4): the frames its tick took, the
        // commands dispatched before any generation is handled.
        self.serve_build_channel();
        self.apply_development(data);
    }

    /// The development poll's tick alone: its workers started when due, an
    /// answer taken (a generation kept for [`Self::poll_development`] to
    /// apply). A frame that must stay quick (a scroll step on the Android
    /// reader) still ticks, so a due poll is never left waiting on it.
    pub fn tick_development(&mut self) {
        if !self.painted {
            return;
        }
        match self.dev.as_mut().map(|dev| dev.poll()) {
            // A poll that began before the generation installed brings it again: not twice.
            Some(Ok(Some(generation)))
                if self.dev.as_ref().unwrap().identity.as_ref() == Some(&generation.identity) => {}
            Some(Ok(Some(generation))) => {
                // A newer generation waits its own second; the poll fetching
                // the held one again does not restart it.
                if self.pending_dev.as_ref().map(|g| &g.identity) != Some(&generation.identity) {
                    self.dev.as_mut().unwrap().held_since = None;
                }
                self.pending_dev = Some(generation);
            }
            // A dev loop's status (its producers starting) once, not each tick.
            Some(Err(error)) => self.dev.as_mut().unwrap().say(&error),
            _ => {}
        }
    }

    /// Install the generation a tick kept, when it may go.
    fn apply_development(&mut self, data: impl FnOnce() -> D) {
        if let Some(generation) = self.pending_dev.take() {
            let let_go = self
                .dev
                .as_ref()
                .and_then(|dev| dev.held_since)
                .is_some_and(|t| t.elapsed() >= LET_GO_AFTER);
            match self.reload_module(&generation.plan, data(), generation.module.clone(), let_go) {
                Ok(_) => {
                    let waker = self.executor.waker();
                    let dev = self.dev.as_mut().unwrap();
                    dev.identity = Some(generation.identity);
                    // A new executor, so its waker (and a new wake
                    // descriptor, which the Android reader watches again).
                    *dev.waker.lock().unwrap_or_else(|e| e.into_inner()) = Some(waker);
                    // The generation runs the design it was built with; one
                    // applied live while it downloaded is newer. The next
                    // tick's snapshot (or 204, the plan's own) converges.
                    if let Some(link) = dev.link.as_mut() {
                        link.snapshot = true;
                    }
                    self.repin();
                    self.rehold();
                    self.report_status();
                }
                Err(HostError::PreparingModule) => self.pending_dev = Some(generation),
                Err(HostError::RequestsPending) => {
                    self.dev
                        .as_mut()
                        .unwrap()
                        .held_since
                        .get_or_insert_with(Instant::now);
                    self.pending_dev = Some(generation);
                }
                Err(error) => eprintln!("exact url: candidate refused: {error}"),
            }
            if self.pending_dev.is_none() {
                self.dev.as_mut().unwrap().held_since = None;
            }
        }
        // The live table: the snapshot and the commands poll's batches.
        if self.pending_dev.is_none() {
            self.serve_live();
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn prepare_logic(
        &self,
        plan: &[u8],
        admitted: D,
        module: Option<&crate::delivery::Module>,
        text: &Shared,
        carried: &mut exact_runner::Carried,
        delivery: &exact_runner::Delivery,
        let_go: bool,
    ) -> Result<D, HostError> {
        let Some(module) = module else {
            return Ok(admitted);
        };
        if !let_go && self.host.runner().has_pending() {
            return Err(HostError::RequestsPending);
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
            let (mut host, error) = exact_runner::time::launching_with(
                exact_runner::time::Launch {
                    time: Some(self.host.runner().wall_time()),
                    place: Some(self.host.runner().place().clone()),
                    ..exact_runner::time::Launch::NONE
                },
                || {
                    Host::boot_with(
                        plan,
                        validation,
                        Box::new(Measurer(text.clone())),
                        self.viewport.0,
                        self.viewport.1,
                        Some(carried),
                        Some(delivery.clone()),
                    )
                },
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
    #[cfg(unix)]
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
        // No hatch runs before first pixel (LLP 1075.003.000.001 §2.1).
        self.connect_hatches();
    }

    /// First pixel has been acknowledged ([`Presenter::first_pixel`]).
    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    pub(crate) fn first_pixel_told(&self) -> bool {
        self.painted
    }

    /// The deferred data module has yet to activate, and has not failed to:
    /// the agent's `clock data` waits for it.
    pub fn data_activating(&self) -> bool {
        !self.activation_failed && self.host.data_pending()
    }

    /// A development generation or an update is being prepared: the display
    /// loop polls for it. A loading data source wakes the loop itself.
    pub fn delivery_pending(&self) -> bool {
        self.pending_dev.is_some() || self.pending_update
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
            false,
        )?;
        let (mut host, error) = exact_runner::time::launching_with(
            exact_runner::time::Launch {
                time: Some(self.host.runner().wall_time()),
                place: Some(self.host.runner().place().clone()),
                ..exact_runner::time::Launch::NONE
            },
            || {
                Host::boot_with(
                    &candidate.plan,
                    data,
                    Box::new(Measurer(text.clone())),
                    self.viewport.0,
                    self.viewport.1,
                    Some(&carried),
                    Some(delivery),
                )
            },
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
        let shaders = self
            .surfaces
            .prepare_shaders(&self.compat, &assets)
            .map_err(HostError::Asset)?;
        let updates = self.updates.as_mut().unwrap();
        self.surfaces
            .activate_shaders(shaders, || {
                updates.commit_activation(candidate.entry, candidate.seq)
            })
            .map_err(HostError::Asset)?;
        self.updates.as_mut().unwrap().boot_started();
        self.host = host;
        self.replaced();
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
        self.group = None;
        self.brush.lift = Default::default();
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::delivery::{Selection, Store};
    use exact_runner::{DataError, Value};
    use std::sync::Mutex;

    #[derive(Clone, Default)]
    struct FirstFacts(Arc<Mutex<Option<Vec<Value>>>>);

    impl DataSource for FirstFacts {
        fn query(&mut self, name: &str, args: &[Value]) -> Result<Value, DataError> {
            assert_eq!(name, "record");
            self.0.lock().unwrap().get_or_insert_with(|| args.to_vec());
            Ok(Value::Number(1.0))
        }

        fn replacement(&self, _: &[u8], _: &str, _: Vec<u8>) -> Result<Self, DataError> {
            Ok(self.clone())
        }
    }

    fn candidate_plan() -> Vec<u8> {
        crate::test_plan(
            r#"shape Time
  epochAtZero: number
  utcOffset: number
  locale: string
  formatLocale: string
  timeZone: string
  seed: number
component App
  resource time = exactTime() as shape Time
  resource first = record(time.epochAtZero, time.utcOffset, time.locale, time.formatLocale, time.timeZone, time.seed, performanceNow()) as shape number
  view
    text `${first}`
"#,
        )
    }

    fn dated_presenter() -> (Presenter<FirstFacts>, FirstFacts) {
        let facts = FirstFacts::default();
        let (mut p, error) = Presenter::boot_with(
            &crate::test_plan("component App\n  view\n    text \"old\"\n"),
            facts.clone(),
            (100., 100.),
            1.,
            PathBuf::new(),
            PainterChoice::Cpu,
        )
        .unwrap();
        assert!(error.is_none(), "{error:?}");
        assert!(p.set_time(1_790_000_000_000.0, -240.0).is_none());
        assert!(p
            .set_place(&exact_runner::time::Place {
                locale: "fr-CA".into(),
                format_locale: "en-GB".into(),
                time_zone: "America/Toronto".into(),
                seed: 123_456_789.0,
            })
            .is_none());
        assert!(p.clock(1250.0).1.is_none());
        (p, facts)
    }

    fn assert_first_facts(facts: &FirstFacts) {
        assert_eq!(
            facts.0.lock().unwrap().as_deref(),
            Some(
                [
                    Value::Number(1_790_000_000_000.0),
                    Value::Number(-240.0),
                    Value::str("fr-CA"),
                    Value::str("en-GB"),
                    Value::str("America/Toronto"),
                    Value::Number(123_456_789.0),
                    Value::Number(1250.0),
                ]
                .as_slice()
            )
        );
    }

    #[test]
    fn development_candidate_first_settle_reads_launch_facts() {
        let (mut p, facts) = dated_presenter();
        assert!(p
            .reload(&candidate_plan(), facts.clone())
            .unwrap()
            .is_none());
        assert_first_facts(&facts);
    }

    #[test]
    fn validation_candidate_first_settle_reads_launch_facts() {
        let (mut p, facts) = dated_presenter();
        p.painted = true;
        let mut carried = p.host.carry();
        p.prepare_logic(
            &candidate_plan(),
            facts.clone(),
            Some(&crate::delivery::Module {
                receipt: String::new(),
                bytes: Vec::new(),
            }),
            &p.text,
            &mut carried,
            p.host.runner().delivery(),
            false,
        )
        .unwrap();
        assert_first_facts(&facts);
    }

    #[test]
    fn update_candidate_first_settle_reads_launch_facts() {
        let (mut p, facts) = dated_presenter();
        p.set_updates(Some(Box::new(StagedPlan(candidate_plan().into()))));
        assert!(p.activate_update(facts.clone()).unwrap());
        assert_first_facts(&facts);
    }

    struct StagedPlan(Arc<[u8]>);

    impl Store for StagedPlan {
        fn prepare_selected(&mut self) -> Option<Selection> {
            None
        }
        fn boot_started(&mut self) {}
        fn entry_refused(&mut self, _: &str, _: &str) {}
        fn selection_corrupt(&mut self, _: &str) {}
        fn boot_succeeded(&mut self) {}
        fn take_note(&mut self) -> Option<String> {
            None
        }
        #[cfg(unix)]
        fn fd(&self) -> std::os::unix::io::RawFd {
            -1
        }
        fn check(&self) -> bool {
            false
        }
        fn take_line(&mut self) -> Option<String> {
            None
        }
        fn prepare_activation(&self) -> Result<Option<Selection>, String> {
            Ok(Some(Selection {
                entry: Some("new-paint-order".into()),
                seq: 1,
                plan: self.0.clone(),
                assets: Arc::new(|_| Ok(None)),
            }))
        }
        fn commit_activation(&mut self, entry: Option<String>, seq: u64) -> Result<(), String> {
            assert_eq!(entry.as_deref(), Some("new-paint-order"));
            assert_eq!(seq, 1);
            Ok(())
        }
        fn staged_stream_into(&self, _: &mut exact_runner::Delivery) {}
        fn status_into(&self, _: &mut exact_runner::Delivery) {}
    }

    #[test]
    fn delivered_update_recomputes_paint_ranks_at_the_same_kernel_epoch() {
        let source = r##"component Paint
  view
    box width=100 height=100
      box testId="red" position="absolute" z-index=0 width=100 height=100 background-color="#ff0000"
      box testId="blue" position="absolute" z-index=1 width=100 height=100 background-color="#0000ff"
"##;
        let old = contract::compile(source).unwrap().encode();
        let new = contract::compile(&source.replace("z-index=0", "z-index=2"))
            .unwrap()
            .encode();
        let (mut p, error) = Presenter::boot_with(
            &old,
            (),
            (100., 100.),
            1.,
            PathBuf::from(env!("CARGO_MANIFEST_DIR")),
            PainterChoice::Cpu,
        )
        .unwrap();
        assert!(error.is_none(), "{error:?}");
        let id = |p: &Presenter<()>, name| {
            let k = p.host.kernel();
            k.node_by_key(k.find_by_test_id(name)[0]).unwrap().id
        };
        let pixel = |p: &mut Presenter<()>| {
            let frame = p.frame();
            let c = frame.pixel(50, 50).unwrap().demultiply();
            (c.red(), c.green(), c.blue())
        };
        assert_eq!(pixel(&mut p), (0, 0, 255));
        assert_eq!(p.hit(50., 50.), Some(id(&p, "blue")));
        let epoch = p.host.kernel().epoch();
        let passes = p.brush.rank_passes;
        assert_eq!(p.brush.paint_epoch, Some(epoch));
        p.set_updates(Some(Box::new(StagedPlan(new.into()))));
        assert!(p.activate_update(()).unwrap());
        assert_eq!(
            p.host.kernel().epoch(),
            epoch,
            "replacement epochs must collide"
        );
        assert_eq!(
            pixel(&mut p),
            (255, 0, 0),
            "the delivered rank paints immediately"
        );
        assert_eq!(p.hit(50., 50.), Some(id(&p, "red")));
        assert_eq!(p.brush.rank_passes, passes + 1);
        // A host-only repaint at that epoch retains the new ranks.
        p.dirty = true;
        assert_eq!(pixel(&mut p), (255, 0, 0));
        assert_eq!(p.brush.rank_passes, passes + 1);
    }
}

#[cfg(test)]
#[path = "delivery_tests.rs"]
mod let_go_tests;
