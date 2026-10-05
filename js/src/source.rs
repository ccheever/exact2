//! `DataSource` for a TypeScript module: logs, storage, and the answer a
//! source gives now or later.
use super::*;

impl DataSource for Module {
    fn take_logs(&mut self) -> Vec<String> {
        Module::take_logs(self)
    }

    fn configure_storage(
        &mut self,
        data: std::path::PathBuf,
        cache: std::path::PathBuf,
        temporary: std::path::PathBuf,
    ) -> Result<(), DataError> {
        if self.is_loaded() {
            return Err(DataError::Unavailable(
                "configure storage before loading the module".into(),
            ));
        }
        self.directories = Some(storage::Directories {
            data,
            cache,
            temporary,
        });
        Ok(())
    }

    fn dispatch(&mut self, token: u64, store: &Store) -> Dispatch {
        let _ = store;
        let deferred = self
            .parked
            .iter()
            .any(|(_, p)| p.call == token && p.ticket == DEFERRED);
        if self
            .parked
            .iter()
            .any(|(_, p)| p.call == token && p.ticket == WAITING)
        {
            return self.wake(token).unwrap_or_else(|| {
                self.waiters.push(token);
                Dispatch::Held
            });
        }
        if !deferred {
            return match self.continuation(token) {
                Some(work) => Dispatch::Run(Work::Now(work)),
                None => Dispatch::Missing,
            };
        }
        if self.turn_open() {
            self.held.push_back(token);
            return Dispatch::Held;
        }
        Module::deferred_work()
    }

    fn release(&mut self, store: &Store) -> Vec<(u64, Dispatch)> {
        let _ = store;
        self.finish_retired();
        // Waiting answers whose wait may be over are asked again.
        let mut released = Vec::new();
        for token in std::mem::take(&mut self.waiters) {
            match self.wake(token) {
                Some(work) => released.push((token, work)),
                None if self.parked.iter().any(|(_, p)| p.call == token) => {
                    self.waiters.push(token)
                }
                None => {}
            }
        }
        // Once the open turn has ended, the oldest held answer begins; the
        // rest wait for the turn it may open in turn.
        while !self.turn_open() {
            let Some(token) = self.held.pop_front() else {
                break;
            };
            if self
                .parked
                .iter()
                .any(|(_, p)| p.call == token && p.ticket == DEFERRED)
            {
                released.push((token, Module::deferred_work()));
                break;
            }
        }
        released
    }

    fn continuation(&mut self, token: u64) -> Option<Box<dyn FnOnce() -> Outcome + Send>> {
        if self
            .parked
            .iter()
            .any(|(_, p)| p.call == token && p.ticket == DEFERRED)
        {
            // The owner-thread and test paths run turns in order already.
            return Some(Box::new(|| {
                Outcome::Response(Response {
                    status: 200,
                    headers: Vec::new(),
                    body: Vec::new(),
                })
            }));
        }
        // An owner thread runs one turn to its end, so another answer's
        // work cannot land while this one waits for it (LLP 1027.002).
        if self
            .parked
            .iter()
            .any(|(_, p)| p.call == token && p.ticket == WAITING)
        {
            return Some(Box::new(|| {
                Outcome::Failed {
                kind: exact_runner::FailureKind::Unsupported,
                message: "the answer awaits work another answer started (a shared fetch or storage queue), which a worker-placed source cannot wait for; make each answer's own fetch, or share the resolved value rather than the promise".into(),
            }
            }));
        }
        let (_, parked) = self
            .parked
            .iter_mut()
            .find(|(_, p)| p.call == token && p.ticket == 0 && !p.work_taken)?;
        let work = self.storage.as_ref()?.continuation();
        parked.work_taken = true;
        Some(work)
    }

    fn activate(&mut self) -> Result<(), DataError> {
        self.load().map_err(DataError::Unavailable)
    }

    fn activate_for_validation(&mut self) -> Result<(), DataError> {
        // A replacement may inherit directory paths from a direct consumer.
        // Validation gets neither those capabilities nor an existing adapter;
        // its disposable engine can only record ordinary host requests.
        self.unload();
        self.directories = None;
        self.activate()
    }

    fn replacement(&self, plan: &[u8], receipt: &str, module: Vec<u8>) -> Result<Self, DataError> {
        Paired::decode(receipt, plan, module, self.app_id(), self.grants())
            .map(|pair| {
                let mut module = pair.module;
                module.directories = self.directories.clone();
                module.native_factory = self.native_factory;
                module
            })
            .map_err(DataError::Unavailable)
    }

    fn app_id(&self) -> &str {
        &self.app_id
    }

    fn grants(&self) -> &str {
        &self.grants
    }

    fn revision(&self) -> Option<&str> {
        Some(
            self.revision
                .get_or_init(|| paired::revision_of(&self.bytecode)),
        )
    }

    /// A `Later` answer the runner dropped before handing it out — a refused
    /// pass, or a re-read whose reply's refresh asks again — is dropped here
    /// too. Left parked, a deferred call shares its key with the call still
    /// in flight, and `resume`, which finds a call by key, gave it that
    /// call's storage step: the read's turn never ended, and every answer
    /// held behind it waited forever (files diary F18: a mutation refreshing
    /// a folder's preview mid-walk, behind a composer, whose `forgotten`
    /// cannot name a dispatched call's token).
    fn discard(&mut self, token: u64) {
        let Some(at) = self.parked.iter().position(|(_, p)| p.call == token) else {
            return;
        };
        let (_, parked) = self.parked.remove(at);
        self.held.retain(|held| *held != token);
        self.waiters.retain(|waiter| *waiter != token);
        if parked.ticket != DEFERRED {
            self.forget_calls(vec![parked.call]);
        }
    }

    /// Calls whose requests the runner let go are dropped, here and in the
    /// prelude with the fetches they wait on (LLP 1016 D5).
    /// A re-ask with equal arguments (a `refresh`) shares its key with the
    /// call it replaced; only the continuation token tells them apart, so a
    /// call whose token is not the one in flight goes too (minesweeper F10:
    /// a read replaced by its own refresh kept its turn open forever, and the
    /// refresh, deferred behind it, never ran).
    fn forgotten(&mut self, store: &Store, in_flight: &[InFlight<'_>]) {
        self.retire_calls(store, in_flight);
    }

    /// Stops the running call, or the next one to start, from any thread:
    /// at module initialization too, and on a worker's owner thread.
    fn interrupt(&self) -> Option<Interrupt> {
        let watch = self.watch.clone();
        Some(Interrupt::new(move || watch.trigger()))
    }

    fn native(&self) -> Option<exact_runner::Native> {
        Some(self.native_slot.clone())
    }

    /// Not before the host loads it (LLP 1027 D4): the runner boots
    /// store-reading resources from their kept answers meanwhile.
    fn ready(&self) -> bool {
        self.is_loaded()
    }

    fn canvas_surfaces(&self) -> Vec<(String, usize)> {
        self.canvas_surfaces.clone()
    }

    /// Canvas 2D (LLP 1056 D1): the module's `draw` through the TypeScript
    /// recorder, synchronously in this turn (native `main` placement).
    fn draw(
        &mut self,
        request: &exact_runner::DrawRequest<'_>,
        ctx: &exact_runner::exact_canvas::Context2d,
    ) -> exact_runner::Drawn {
        self.host.canvas = Some(ctx.env());
        let reply = match self.engine.as_mut() {
            None => Err("the module is not loaded".to_string()),
            Some(engine) => engine.call("__exact_draw", [&request.json(), "", ""]),
        };
        self.host.canvas = None;
        exact_runner::Drawn::Now(match reply {
            Ok(json) => exact_runner::DrawReply::from_seam(&json),
            Err(e) => exact_runner::DrawReply {
                error: Some(e),
                ..Default::default()
            },
        })
    }

    fn canvases_retired(&mut self, retired: &[(u64, u32)]) {
        if let Some(engine) = self.engine.as_mut() {
            let json = serde_json::to_string(retired).unwrap_or_default();
            let _ = engine.call("__exact_retire", [&json, "", ""]);
        }
    }

    /// The seam's signatures, from the plan's `sources` table (LLP 1027 D2).
    fn bind(&mut self, plan: &Plan) {
        self.plan = Some(plan.clone());
        self.sigs.clear();
        for row in &plan.sources {
            let start = row.params.start as usize;
            let end = start + row.params.len as usize;
            let params = plan
                .source_params
                .get(start..end)
                .map(|rows| {
                    rows.iter()
                        .map(|p| Shape::from_plan(plan, p.ty))
                        .collect::<Result<Vec<_>, _>>()
                })
                .unwrap_or_else(|| Err("a source's parameters run past the table".into()));
            let result = Shape::from_plan(plan, row.ty);
            if let (Ok(params), Ok(result)) = (params, result) {
                self.sigs
                    .insert(plan.str(row.name).to_string(), Sig { params, result });
            }
        }
    }

    /// The bake's path and the in-process path: no store, and an answer that
    /// awaits a fetch cannot be given now.
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        match self.begin(None, None, source, args)? {
            Answer::Now(v) => Ok(v),
            Answer::Later(_) => {
                self.parked
                    .retain(|(k, _)| *k != Module::key(None, source, args));
                Err(DataError::Unavailable(format!(
                    "`{source}` fetches, and there is no host to run it here"
                )))
            }
        }
    }

    fn answer(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        self.begin(Some(store), None, source, args)
    }

    fn parse(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        self.resume(store, None, source, args, outcome)
    }

    fn answer_for(
        &mut self,
        target: Target,
        store: &mut Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        let refused = self.host.bake_refusals;
        let answer = self.begin(Some(store), Some(target), source, args);
        // A resource whose answer failed after the bake refused it storage
        // is the device's to answer, as one that fetches is: the bake shows
        // its placeholder and a launch asks it (kanban2 #5: an uncaught
        // `code: 'bake'` stopped the native build, where the web build's
        // check asks at launch). The token is never dispatched.
        match answer {
            Err(DataError::Unavailable(_))
                if self.host.bake_refusals > refused && matches!(target, Target::Resource(_)) =>
            {
                Ok(Answer::Later(Request::continuation(DEFERRED)))
            }
            answer => answer,
        }
    }

    fn parse_for(
        &mut self,
        target: Target,
        store: &mut Store,
        source: &str,
        args: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        self.resume(store, Some(target), source, args, outcome)
    }
}
