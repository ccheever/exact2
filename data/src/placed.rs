//! A source on an owner of its own (LLP 1027.002 D1/D2). `Placed` runs its
//! source on a host-owned thread from activation on, answers every call
//! `Later`, runs each turn there against a scoped snapshot — staying on the
//! owner through the source's own continuation waits — and hands the turn's
//! envelope back for the runner to commit (D3). With `Placement::Main` it
//! is the source, inline, and costs nothing. The same proxy serves a Rust
//! source that can be moved to the thread and a TypeScript module that must
//! be built there.

use crate::envelope;
use exact_plan::{Plan, Value};
use exact_runner::{
    Answer, DataError, DataSource, Dispatch, Outcome, Placement, Reply, Request, Store, Work,
};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::mpsc::{channel, Sender};

fn unavailable(message: impl Into<String>) -> DataError {
    DataError::Unavailable(message.into())
}

/// How the owner thread obtains its instance: built there, or moved in.
pub type Obtain<D> = Box<dyn FnOnce() -> Result<D, DataError> + Send + 'static>;

/// One turn for the owner: a call, or a resumption after a yield. A plan
/// `Value` is not `Send`, so arguments cross as their canonical bytes.
enum Job {
    Answer {
        source: String,
        args: Vec<Vec<u8>>,
        snapshot: Vec<(String, String)>,
        reply: Reply,
    },
    Resume {
        source: String,
        args: Vec<Vec<u8>>,
        outcome: Outcome,
        snapshot: Vec<(String, String)>,
        reply: Reply,
    },
}

fn encode_args(args: &[Value]) -> Vec<Vec<u8>> {
    args.iter().map(Value::to_bytes).collect()
}

fn decode_args(args: &[Vec<u8>]) -> Result<Vec<Value>, DataError> {
    args.iter()
        .map(|bytes| {
            Value::from_bytes(bytes)
                .map_err(|e| unavailable(format!("an argument did not cross: {e:?}")))
        })
        .collect()
}

/// A call recorded at `answer` or `parse`, dispatched later.
enum Recorded {
    Answer {
        source: String,
        args: Vec<Value>,
    },
    Resume {
        source: String,
        args: Vec<Value>,
        outcome: Outcome,
    },
}

/// What a parked call is waiting for: its turn's envelope, or the host's
/// outcome for the request its turn yielded.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Stage {
    Turn,
    Yielded,
}

type Key = (String, Vec<u8>);

fn key(source: &str, args: &[Value]) -> Key {
    let mut bytes = Vec::new();
    for a in args {
        bytes.extend(a.to_bytes());
    }
    (source.to_string(), bytes)
}

/// How a worker instance comes to exist on its thread.
enum Spawn<D> {
    /// The instance itself moves there, activated: the function was
    /// instantiated by `new`, where `D: Send` holds.
    Move(fn(D) -> Obtain<D>),
    /// The instance is built there from what the template on this thread
    /// knows (a TypeScript module: bytecode, identity, directories, plan).
    Build(fn(&D) -> Obtain<D>),
}

/// A source with a placement.
pub struct Placed<D> {
    /// The source (`Main`), the template (`Worker`, built there), or nothing
    /// once moved to its owner (`Worker`, moved there).
    inner: Option<D>,
    placement: Placement,
    spawn: Spawn<D>,
    app_id: String,
    grants: String,
    revision: Option<String>,
    owner: Option<Sender<Job>>,
    recorded: BTreeMap<u64, Recorded>,
    stages: HashMap<Key, VecDeque<Stage>>,
    next: u64,
    validation: bool,
}

impl<D: DataSource + Send + 'static> Placed<D> {
    /// `source`, placed; on `Worker` it moves to its owner at activation.
    /// Nothing thread-affine crosses: `D: Send` says so.
    pub fn new(source: D, placement: Placement) -> Self {
        Self::with_spawn(source, placement, Spawn::Move(moved))
    }
}

/// The moved instance, as the closure the owner runs to obtain it.
fn moved<D: DataSource + Send + 'static>(source: D) -> Obtain<D> {
    Box::new(move || Ok(source))
}

impl<D: DataSource + 'static> Placed<D> {
    /// `template`, placed; on `Worker` the owner thread builds its own
    /// instance from `build(&template)` at activation, and the template stays
    /// here for identity, configuration and replacement. For a source that
    /// cannot be sent — one that owns a VM.
    pub fn built(template: D, placement: Placement, build: fn(&D) -> Obtain<D>) -> Self {
        Self::with_spawn(template, placement, Spawn::Build(build))
    }

    fn with_spawn(source: D, placement: Placement, spawn: Spawn<D>) -> Self {
        Self {
            app_id: source.app_id().to_string(),
            grants: source.grants().to_string(),
            revision: source.revision().map(str::to_string),
            inner: Some(source),
            placement,
            spawn,
            owner: None,
            recorded: BTreeMap::new(),
            stages: HashMap::new(),
            next: 0,
            validation: false,
        }
    }

    #[cfg(test)]
    pub(crate) fn staged_keys(&self) -> usize {
        self.stages.len()
    }

    /// The placement this was given (what `placement()` reports outside
    /// validation).
    pub fn given_placement(&self) -> Placement {
        self.placement
    }

    fn here(&mut self) -> Result<&mut D, DataError> {
        self.inner
            .as_mut()
            .ok_or_else(|| unavailable("the source is on its owner thread"))
    }

    fn inline(&self) -> bool {
        self.owner.is_none()
    }

    fn token(&mut self) -> Result<u64, DataError> {
        self.next = self
            .next
            .checked_add(1)
            .ok_or_else(|| unavailable("placed tokens exhausted"))?;
        Ok(self.next)
    }

    fn record(&mut self, k: Key, recorded: Recorded) -> Result<Answer, DataError> {
        let token = self.token()?;
        self.recorded.insert(token, recorded);
        self.stages.entry(k).or_default().push_back(Stage::Turn);
        Ok(Answer::Later(Request::continuation(token)))
    }

    /// Start the owner (LLP 1027.002 D2): it obtains its instance, then runs
    /// turns in order until this proxy drops its end of the channel.
    fn spawn_owner(&mut self, obtain: Obtain<D>) -> Result<(), DataError> {
        let (jobs, rx) = channel::<Job>();
        let grants = self.grants.clone();
        std::thread::Builder::new()
            .name("exact-owner".into())
            .spawn(move || {
                let mut source = obtain();
                for job in rx {
                    let (name, args, outcome, snapshot, reply) = match job {
                        Job::Answer {
                            source,
                            args,
                            snapshot,
                            reply,
                        } => (source, args, None, snapshot, reply),
                        Job::Resume {
                            source,
                            args,
                            outcome,
                            snapshot,
                            reply,
                        } => (source, args, Some(outcome), snapshot, reply),
                    };
                    let mut local = Store::new(&grants, snapshot);
                    let result = match &mut source {
                        Err(e) => Err(e.clone()),
                        Ok(source) => decode_args(&args)
                            .and_then(|args| turn(source, &mut local, &name, &args, outcome)),
                    };
                    reply.send(envelope::encode(result, &mut local, Vec::new()));
                }
                // Retired on its owner, after its last turn.
                drop(source);
            })
            .map_err(|e| unavailable(format!("the owner thread could not start: {e}")))?;
        self.owner = Some(jobs);
        Ok(())
    }
}

/// One turn (LLP 1027.002 D3, change 3): begin or resume; stay on the owner
/// through every wait the source runs itself (its storage, its own worker
/// closures); end at an answer, an error, or a request only the host can run
/// (`fetch`, the portable storage protocol) — a yield.
fn turn<D: DataSource>(
    source: &mut D,
    local: &mut Store,
    name: &str,
    args: &[Value],
    outcome: Option<Outcome>,
) -> Result<Answer, DataError> {
    let mut result = match outcome {
        None => source.answer(local, name, args),
        Some(outcome) => source.parse(local, name, args, outcome),
    };
    loop {
        match result {
            Ok(Answer::Later(request)) if request.continuation.is_some() => {
                let token = request.continuation.expect("checked");
                let Some(work) = source.continuation(token) else {
                    return Err(unavailable(format!(
                        "`{name}`: a continuation with no work on its owner"
                    )));
                };
                let outcome = work();
                result = source.parse(local, name, args, outcome);
            }
            other => return other,
        }
    }
}

impl<D: DataSource + 'static> DataSource for Placed<D> {
    fn placement(&self) -> Placement {
        if self.validation {
            Placement::Main
        } else {
            self.placement
        }
    }

    fn app_id(&self) -> &str {
        &self.app_id
    }

    fn grants(&self) -> &str {
        &self.grants
    }

    fn revision(&self) -> Option<&str> {
        self.revision.as_deref()
    }

    fn ready(&self) -> bool {
        if self.owner.is_some() {
            return true;
        }
        self.inner.as_ref().is_some_and(D::ready)
    }

    fn preload(&self) -> Result<bool, DataError> {
        match &self.inner {
            Some(inner) => inner.preload(),
            None => Ok(true),
        }
    }

    fn bind(&mut self, plan: &Plan) {
        if let Some(inner) = self.inner.as_mut() {
            inner.bind(plan);
        }
    }

    fn configure_storage(
        &mut self,
        data: std::path::PathBuf,
        cache: std::path::PathBuf,
        temporary: std::path::PathBuf,
    ) -> Result<(), DataError> {
        self.here()?.configure_storage(data, cache, temporary)
    }

    fn activate(&mut self) -> Result<(), DataError> {
        self.validation = false;
        if self.placement != Placement::Worker {
            return self.here()?.activate();
        }
        if self.owner.is_some() {
            return Ok(());
        }
        if cfg!(target_arch = "wasm32") {
            return Err(unavailable(
                "a worker placement is unavailable on this host",
            ));
        }
        let obtain: Obtain<D> = match self.spawn {
            Spawn::Build(build) => build(self.here()?),
            Spawn::Move(moved) => {
                let mut source = self
                    .inner
                    .take()
                    .ok_or_else(|| unavailable("the source is on its owner thread"))?;
                if let Err(e) = source.activate() {
                    self.inner = Some(source);
                    return Err(e);
                }
                moved(source)
            }
        };
        self.spawn_owner(obtain)
    }

    /// Validation is disposable and runs inline, on this thread: a candidate
    /// is judged by its answers, not by where it would run. A built worker's
    /// template is what gets validated.
    fn activate_for_validation(&mut self) -> Result<(), DataError> {
        self.validation = true;
        self.owner = None;
        self.here()?.activate_for_validation()
    }

    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        self.here()?.query(source, args)
    }

    fn answer(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        if self.inline() {
            return self.here()?.answer(store, source, args);
        }
        self.record(
            key(source, args),
            Recorded::Answer {
                source: source.to_string(),
                args: args.to_vec(),
            },
        )
    }

    fn parse(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        if self.inline() {
            return self.here()?.parse(store, source, args, outcome);
        }
        let k = key(source, args);
        let stage = self.stages.get_mut(&k).and_then(VecDeque::pop_front);
        if self.stages.get(&k).is_some_and(VecDeque::is_empty) {
            self.stages.remove(&k);
        }
        match stage {
            Some(Stage::Turn) => {
                let mut logs = Vec::new();
                let answer = envelope::apply(outcome, store, &mut logs);
                if let Ok(Answer::Later(_)) = &answer {
                    self.stages.entry(k).or_default().push_back(Stage::Yielded);
                }
                answer
            }
            Some(Stage::Yielded) => self.record(
                k,
                Recorded::Resume {
                    source: source.to_string(),
                    args: args.to_vec(),
                    outcome,
                },
            ),
            None => Err(unavailable(format!(
                "`{source}`: a reply for an answer not in flight"
            ))),
        }
    }

    fn dispatch(&mut self, token: u64, store: &Store) -> Dispatch {
        let Some(recorded) = self.recorded.remove(&token) else {
            return match self.inner.as_mut() {
                Some(inner) if self.owner.is_none() => inner.dispatch(token, store),
                _ => Dispatch::Missing,
            };
        };
        let Some(owner) = self.owner.clone() else {
            return Dispatch::Missing;
        };
        let snapshot = envelope::snapshot(store, &self.grants);
        // Everything the closure carries is `Send`: names, bytes, the
        // outcome, the snapshot. The reply arrives on the host's I/O worker.
        let (source, args, outcome) = match recorded {
            Recorded::Answer { source, args } => (source, encode_args(&args), None),
            Recorded::Resume {
                source,
                args,
                outcome,
            } => (source, encode_args(&args), Some(outcome)),
        };
        Dispatch::Run(Work::Later(Box::new(move |reply| {
            let job = match outcome {
                None => Job::Answer {
                    source,
                    args,
                    snapshot,
                    reply,
                },
                Some(outcome) => Job::Resume {
                    source,
                    args,
                    outcome,
                    snapshot,
                    reply,
                },
            };
            // A closed channel drops the reply, which reports `Aborted`.
            let _ = owner.send(job);
        })))
    }

    fn release(&mut self, store: &Store) -> Vec<(u64, Dispatch)> {
        match self.inner.as_mut() {
            Some(inner) if self.owner.is_none() => inner.release(store),
            _ => Vec::new(),
        }
    }

    fn discard(&mut self, token: u64) {
        match self.recorded.remove(&token) {
            Some(Recorded::Answer { source, args } | Recorded::Resume { source, args, .. }) => {
                let k = key(&source, &args);
                if let Some(stages) = self.stages.get_mut(&k) {
                    stages.pop_back();
                    if stages.is_empty() {
                        self.stages.remove(&k);
                    }
                }
            }
            None => {
                if let Some(inner) = self.inner.as_mut() {
                    inner.discard(token);
                }
            }
        }
    }

    fn continuation(&mut self, token: u64) -> Option<Box<dyn FnOnce() -> Outcome + Send>> {
        if self.owner.is_some() {
            return None;
        }
        self.inner.as_mut()?.continuation(token)
    }

    /// A replacement keeps its placement (LLP 1027.002 §6). A moved source
    /// has no `Self` left here to pair; a built one is replaced through its
    /// template.
    fn replacement(&self, plan: &[u8], receipt: &str, module: Vec<u8>) -> Result<Self, DataError> {
        match (&self.inner, &self.spawn) {
            (Some(inner), Spawn::Build(build)) => Ok(Self::with_spawn(
                inner.replacement(plan, receipt, module)?,
                self.placement,
                Spawn::Build(*build),
            )),
            (Some(inner), Spawn::Move(moved)) => Ok(Self::with_spawn(
                inner.replacement(plan, receipt, module)?,
                self.placement,
                Spawn::Move(*moved),
            )),
            (None, _) => Err(unavailable(
                "a placed source is binary-bound once on its owner; rebuild it",
            )),
        }
    }
}

impl<D> Drop for Placed<D> {
    fn drop(&mut self) {
        // Closing the channel ends the owner after its current turn; the
        // runner's thread never waits on it (LLP 1027.002 D5).
        self.owner.take();
    }
}
