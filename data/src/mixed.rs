//! Explicit source ownership across two executors, without depending on either engine.
//! @ref LLP 1027 D8 / LLP 1027.001 / LLP 1029.000 — paired replacement.
//! @ref LLP 1027.002 D3 — the ordered set: every child that shares a
//! `secret.keep` name with a worker child takes its turns one at a time,
//! each against the store as committed at dispatch.

use crate::envelope;
use exact_plan::{Plan, Value};
use exact_runner::{
    Answer, DataError, DataSource, Dispatch, Interrupt, Outcome, Placement, Request, Store, Target,
    Work,
};
use serde_json::Value as Json;
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};

type Retain<R> = fn(&R) -> Result<R, DataError>;

/// A call an ordered member recorded, to run at dispatch.
enum Recorded {
    Answer {
        rust: bool,
        target: Option<Target>,
        source: String,
        args: Vec<Value>,
    },
    Resume {
        rust: bool,
        target: Option<Target>,
        source: String,
        args: Vec<Value>,
        outcome: Outcome,
    },
}

/// What a parked ordered call waits for: its turn's envelope, or the host's
/// outcome for the request its turn yielded.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Stage {
    Turn,
    Yielded,
}

/// A call's stages are keyed by the runner's target when it named one,
/// then by owner, source and arguments: two targets asking one source with
/// equal arguments are two calls.
type Key = (bool, Option<Target>, String, Vec<u8>);

fn key(rust: bool, target: Option<Target>, source: &str, args: &[Value]) -> Key {
    let mut bytes = Vec::new();
    for a in args {
        bytes.extend(a.to_bytes());
    }
    (rust, target, source.to_string(), bytes)
}

/// One ordered set (LLP 1027.002 D3, change 2): its members, the turn it
/// has reserved, and the calls waiting behind it, in order.
struct Set {
    busy: bool,
    held: VecDeque<u64>,
}

/// Two data executors with disjoint, declared source names and one app identity.
/// Each child retains its own grants; hosts receive their union. Construction
/// never probes an answer to discover ownership, so writes cannot happen twice.
pub struct Mixed<J, R> {
    javascript: J,
    rust: R,
    sources: BTreeMap<String, bool>,
    app_id: String,
    grants: String,
    revision: String,
    retain_rust: Option<Retain<R>>,
    continuations: BTreeMap<u64, (bool, u64)>,
    next_continuation: u64,
    /// The ordered sets a worker child makes (none when both children are
    /// on `main`), and which set each child belongs to.
    sets: Vec<Set>,
    set_of: [Option<usize>; 2],
    recorded: BTreeMap<u64, (usize, Recorded)>,
    stages: HashMap<Key, VecDeque<Stage>>,
    logs: Vec<String>,
}

fn unavailable(message: impl Into<String>) -> DataError {
    DataError::Unavailable(message.into())
}

fn kept_names(grants: &str) -> BTreeSet<String> {
    Store::new(grants, []).granted().iter().cloned().collect()
}

impl<J: DataSource, R: DataSource> Mixed<J, R> {
    #[cfg(test)]
    pub(crate) fn staged_keys(&self) -> usize {
        self.stages.len()
    }

    /// Require a complete JavaScript/Rust pair for every replacement.
    pub fn new(
        javascript: J,
        rust: R,
        javascript_sources: &[&str],
        rust_sources: &[&str],
    ) -> Result<Self, DataError> {
        let mut sources = BTreeMap::new();
        for (names, owner) in [(javascript_sources, false), (rust_sources, true)] {
            for name in names {
                if name.is_empty() || sources.insert((*name).into(), owner).is_some() {
                    return Err(unavailable(format!(
                        "duplicate or empty data source: {name}"
                    )));
                }
            }
        }
        Self::construct(javascript, rust, sources, None)
    }

    /// Disable Rust replacement and explicitly preserve its state when replacing
    /// JavaScript. `retain` must make an isolated candidate (for example a clone);
    /// it must not reset live state or share mutable state with validation.
    pub fn with_embedded_rust(mut self, retain: Retain<R>) -> Self {
        self.retain_rust = Some(retain);
        self
    }

    fn construct(
        javascript: J,
        rust: R,
        sources: BTreeMap<String, bool>,
        retain_rust: Option<Retain<R>>,
    ) -> Result<Self, DataError> {
        if javascript.app_id() != rust.app_id() {
            return Err(unavailable(
                "mixed executors must declare the same app identity",
            ));
        }
        let app_id = javascript.app_id().into();
        let grants = [javascript.grants(), rust.grants()]
            .into_iter()
            .flat_map(str::lines)
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
            .join("\n");
        // If Rust adds no capabilities, retain the existing JS receipt spelling.
        // Otherwise the union is deterministic; each child keeps its own spelling.
        let grants = if grants.lines().all(|line| {
            javascript
                .grants()
                .lines()
                .map(str::trim)
                .any(|grant| grant == line)
        }) {
            javascript.grants().to_owned()
        } else {
            grants
        };
        let revision = format!(
            "mixed:{}",
            serde_json::to_string(&(javascript.revision(), rust.revision())).unwrap()
        );
        // The ordered sets (LLP 1027.002 D3, change 2): a worker child is a
        // set; a main child joins it when their `secret.keep` names overlap,
        // since the Store is the only state the ordering protects. Two
        // workers with disjoint names are two sets that run concurrently.
        let workers = [
            javascript.placement() == Placement::Worker,
            rust.placement() == Placement::Worker,
        ];
        let overlap = !kept_names(javascript.grants()).is_disjoint(&kept_names(rust.grants()));
        let mut sets = Vec::new();
        let mut set_of = [None, None];
        if workers.iter().any(|w| *w) {
            if overlap || (workers[0] && workers[1]) {
                sets.push(Set {
                    busy: false,
                    held: VecDeque::new(),
                });
                set_of = [Some(0), Some(0)];
            } else {
                for (i, worker) in workers.iter().enumerate() {
                    if *worker {
                        set_of[i] = Some(sets.len());
                        sets.push(Set {
                            busy: false,
                            held: VecDeque::new(),
                        });
                    }
                }
            }
        }
        Ok(Self {
            javascript,
            rust,
            sources,
            app_id,
            grants,
            revision,
            retain_rust,
            continuations: BTreeMap::new(),
            next_continuation: 1,
            sets,
            set_of,
            recorded: BTreeMap::new(),
            stages: HashMap::new(),
            logs: Vec::new(),
        })
    }

    fn owner(&self, source: &str) -> Result<bool, DataError> {
        self.sources
            .get(source)
            .copied()
            .ok_or_else(|| DataError::UnknownSource(source.into()))
    }

    fn token(&mut self) -> Result<u64, DataError> {
        let token = self.next_continuation;
        self.next_continuation = token
            .checked_add(1)
            .ok_or_else(|| unavailable("mixed continuation tokens exhausted"))?;
        Ok(token)
    }

    fn child_grants(&self, rust: bool) -> String {
        if rust {
            self.rust.grants().to_owned()
        } else {
            self.javascript.grants().to_owned()
        }
    }

    fn child_placement(&self, rust: bool) -> Placement {
        if rust {
            self.rust.placement()
        } else {
            self.javascript.placement()
        }
    }

    fn route_answer(&mut self, rust: bool, mut answer: Answer) -> Result<Answer, DataError> {
        if let Answer::Later(request) = &mut answer {
            let grants = if rust {
                self.rust.grants()
            } else {
                self.javascript.grants()
            };
            if let Some(scope) = &request.grants {
                if scope.lines().map(str::trim).any(|line| {
                    !line.is_empty() && !grants.lines().map(str::trim).any(|grant| grant == line)
                }) {
                    return Err(unavailable("nested request exceeds its executor's grants"));
                }
            } else {
                request.grants = Some(grants.into());
            }
            if let Some(child) = request.continuation {
                let token = self.token()?;
                self.continuations.insert(token, (rust, child));
                request.continuation = Some(token);
            }
        }
        Ok(answer)
    }

    /// Record an ordered member's call; it runs at dispatch, one turn at a
    /// time per set, against the store as committed then.
    fn record(&mut self, set: usize, k: Key, recorded: Recorded) -> Result<Answer, DataError> {
        let token = self.token()?;
        self.recorded.insert(token, (set, recorded));
        self.stages.entry(k).or_default().push_back(Stage::Turn);
        Ok(Answer::Later(Request::continuation(token)))
    }

    /// Start a recorded turn: a worker child records and dispatches its own
    /// job; a main child computes here, now, against a snapshot, and its
    /// envelope goes round through the host like any other reply.
    fn start(&mut self, set: usize, token: u64, store: &Store) -> Dispatch {
        let Some((_, recorded)) = self.recorded.remove(&token) else {
            return Dispatch::Missing;
        };
        let rust = match &recorded {
            Recorded::Answer { rust, .. } | Recorded::Resume { rust, .. } => *rust,
        };
        self.sets[set].busy = true;
        let grants = self.child_grants(rust);
        if self.child_placement(rust) == Placement::Worker {
            let mut scratch = Store::new(&grants, []);
            let answer = match recorded {
                Recorded::Answer {
                    target,
                    source,
                    args,
                    ..
                } => self.child_answer(rust, target, &mut scratch, &source, &args),
                Recorded::Resume {
                    target,
                    source,
                    args,
                    outcome,
                    ..
                } => self.child_parse(rust, target, &mut scratch, &source, &args, outcome),
            };
            return match answer {
                Ok(Answer::Later(request)) if request.continuation.is_some() => {
                    let child = request.continuation.expect("checked");
                    let dispatch = if rust {
                        self.rust.dispatch(child, store)
                    } else {
                        self.javascript.dispatch(child, store)
                    };
                    if matches!(dispatch, Dispatch::Missing | Dispatch::Held) {
                        self.sets[set].busy = false;
                    }
                    dispatch
                }
                // A worker child that answered at once (or refused): its
                // reply still goes round as an envelope, so the turn ends
                // where every turn ends.
                other => {
                    let outcome = envelope::encode(other, &mut scratch, Vec::new());
                    Dispatch::Run(Work::Now(Box::new(move || outcome)))
                }
            };
        }
        let mut local = Store::new(&grants, envelope::snapshot(store, &grants));
        let result = match recorded {
            Recorded::Answer {
                target,
                source,
                args,
                ..
            } => self.child_answer(rust, target, &mut local, &source, &args),
            Recorded::Resume {
                target,
                source,
                args,
                outcome,
                ..
            } => self.child_parse(rust, target, &mut local, &source, &args, outcome),
        };
        let outcome = envelope::encode(result, &mut local, Vec::new());
        Dispatch::Run(Work::Now(Box::new(move || outcome)))
    }

    fn child_answer(
        &mut self,
        rust: bool,
        target: Option<Target>,
        store: &mut Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        if rust {
            crate::answer(&mut self.rust, target, store, source, args)
        } else {
            crate::answer(&mut self.javascript, target, store, source, args)
        }
    }

    fn child_parse(
        &mut self,
        rust: bool,
        target: Option<Target>,
        store: &mut Store,
        source: &str,
        args: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        if rust {
            crate::parse(&mut self.rust, target, store, source, args, outcome)
        } else {
            crate::parse(&mut self.javascript, target, store, source, args, outcome)
        }
    }

    fn answer_with(
        &mut self,
        target: Option<Target>,
        store: &mut Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        let rust = self.owner(source)?;
        if let Some(set) = self.set_of[rust as usize] {
            return self.record(
                set,
                key(rust, target, source, args),
                Recorded::Answer {
                    rust,
                    target,
                    source: source.to_string(),
                    args: args.to_vec(),
                },
            );
        }
        let grants = self.child_grants(rust);
        let answer = store.with_grants(&grants, |store| {
            self.child_answer(rust, target, store, source, args)
        })?;
        self.route_answer(rust, answer)
    }

    fn parse_with(
        &mut self,
        target: Option<Target>,
        store: &mut Store,
        source: &str,
        args: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        let rust = self.owner(source)?;
        let grants = self.child_grants(rust);
        if let Some(set) = self.set_of[rust as usize] {
            let k = key(rust, target, source, args);
            let stage = self.stages.get_mut(&k).and_then(VecDeque::pop_front);
            if self.stages.get(&k).is_some_and(VecDeque::is_empty) {
                self.stages.remove(&k);
            }
            return match stage {
                Some(Stage::Turn) => {
                    // The reserved turn ended, however it ended.
                    self.sets[set].busy = false;
                    let answer = if self.child_placement(rust) == Placement::Worker {
                        store.with_grants(&grants, |store| {
                            self.child_parse(rust, target, store, source, args, outcome)
                        })
                    } else {
                        let mut logs = Vec::new();
                        let answer = store.with_grants(&grants, |store| {
                            envelope::apply(outcome, store, &mut logs)
                        });
                        self.logs.extend(logs);
                        answer
                    };
                    match answer {
                        Ok(Answer::Later(request)) => {
                            self.stages.entry(k).or_default().push_back(Stage::Yielded);
                            self.route_answer(rust, Answer::Later(request))
                        }
                        other => other,
                    }
                }
                Some(Stage::Yielded) => self.record(
                    set,
                    k,
                    Recorded::Resume {
                        rust,
                        target,
                        source: source.to_string(),
                        args: args.to_vec(),
                        outcome,
                    },
                ),
                None => Err(unavailable(format!(
                    "`{source}`: a reply for an answer not in flight"
                ))),
            };
        }
        let answer = store.with_grants(&grants, |store| {
            self.child_parse(rust, target, store, source, args, outcome)
        })?;
        self.route_answer(rust, answer)
    }

    /// The turn's `console` lines a main member produced, for the host's
    /// logs; a worker child keeps its own.
    pub fn take_logs(&mut self) -> Vec<String> {
        std::mem::take(&mut self.logs)
    }
}

impl<J: DataSource, R: DataSource> DataSource for Mixed<J, R> {
    fn placement(&self) -> Placement {
        if self.sets.is_empty() {
            Placement::Main
        } else {
            Placement::Worker
        }
    }

    fn preload(&self) -> Result<bool, DataError> {
        let javascript = self.javascript.preload()?;
        let rust = self.rust.preload()?;
        Ok(javascript && rust)
    }
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        if self.owner(source)? {
            self.rust.query(source, args)
        } else {
            self.javascript.query(source, args)
        }
    }

    fn answer(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        self.answer_with(None, store, source, args)
    }

    fn answer_for(
        &mut self,
        target: Target,
        store: &mut Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        self.answer_with(Some(target), store, source, args)
    }

    fn parse(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        self.parse_with(None, store, source, args, outcome)
    }

    fn parse_for(
        &mut self,
        target: Target,
        store: &mut Store,
        source: &str,
        args: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        self.parse_with(Some(target), store, source, args, outcome)
    }

    fn dispatch(&mut self, token: u64, store: &Store) -> Dispatch {
        if let Some((set, _)) = self.recorded.get(&token) {
            let set = *set;
            if self.sets[set].busy {
                self.sets[set].held.push_back(token);
                return Dispatch::Held;
            }
            return self.start(set, token, store);
        }
        let Some((rust, child)) = self.continuations.get(&token).copied() else {
            return Dispatch::Missing;
        };
        let dispatch = if rust {
            self.rust.dispatch(child, store)
        } else {
            self.javascript.dispatch(child, store)
        };
        if !matches!(dispatch, Dispatch::Held) {
            self.continuations.remove(&token);
        }
        dispatch
    }

    fn release(&mut self, store: &Store) -> Vec<(u64, Dispatch)> {
        let mut released = Vec::new();
        for set in 0..self.sets.len() {
            if self.sets[set].busy {
                continue;
            }
            if let Some(token) = self.sets[set].held.pop_front() {
                let dispatch = self.start(set, token, store);
                released.push((token, dispatch));
            }
        }
        // A child's own held work, under the tokens this composer handed out.
        for rust in [false, true] {
            let inner = if rust {
                self.rust.release(store)
            } else {
                self.javascript.release(store)
            };
            for (child, dispatch) in inner {
                let outer = self
                    .continuations
                    .iter()
                    .find(|(_, (r, c))| *r == rust && *c == child)
                    .map(|(outer, _)| *outer);
                if let Some(outer) = outer {
                    if !matches!(dispatch, Dispatch::Held) {
                        self.continuations.remove(&outer);
                    }
                    released.push((outer, dispatch));
                }
            }
        }
        released
    }

    fn discard(&mut self, token: u64) {
        match self.recorded.remove(&token) {
            Some((
                _,
                Recorded::Answer {
                    rust,
                    target,
                    source,
                    args,
                }
                | Recorded::Resume {
                    rust,
                    target,
                    source,
                    args,
                    ..
                },
            )) => {
                let k = key(rust, target, &source, &args);
                if let Some(stages) = self.stages.get_mut(&k) {
                    stages.pop_back();
                    if stages.is_empty() {
                        self.stages.remove(&k);
                    }
                }
            }
            None => {
                if let Some((rust, child)) = self.continuations.remove(&token) {
                    if rust {
                        self.rust.discard(child);
                    } else {
                        self.javascript.discard(child);
                    }
                }
            }
        }
    }

    fn app_id(&self) -> &str {
        &self.app_id
    }
    fn grants(&self) -> &str {
        &self.grants
    }
    fn revision(&self) -> Option<&str> {
        Some(&self.revision)
    }
    fn ready(&self) -> bool {
        self.javascript.ready() && self.rust.ready()
    }

    /// Stops whichever child is running a call.
    fn interrupt(&self) -> Option<Interrupt> {
        match (self.javascript.interrupt(), self.rust.interrupt()) {
            (Some(javascript), Some(rust)) => Some(Interrupt::new(move || {
                javascript.trigger();
                rust.trigger();
            })),
            (javascript, rust) => javascript.or(rust),
        }
    }

    fn bind(&mut self, plan: &Plan) {
        self.javascript.bind(plan);
        self.rust.bind(plan);
    }

    fn activate(&mut self) -> Result<(), DataError> {
        self.javascript.activate()?;
        self.rust.activate()
    }

    fn activate_for_validation(&mut self) -> Result<(), DataError> {
        self.javascript.activate_for_validation()?;
        self.rust.activate_for_validation()
    }

    fn configure_storage(
        &mut self,
        data: std::path::PathBuf,
        cache: std::path::PathBuf,
        temporary: std::path::PathBuf,
    ) -> Result<(), DataError> {
        self.javascript
            .configure_storage(data.clone(), cache.clone(), temporary.clone())?;
        self.rust.configure_storage(data, cache, temporary)
    }

    fn continuation(&mut self, token: u64) -> Option<Box<dyn FnOnce() -> Outcome + Send>> {
        let (rust, child) = self.continuations.remove(&token)?;
        if rust {
            self.rust.continuation(child)
        } else {
            self.javascript.continuation(child)
        }
    }

    fn replacement(&self, plan: &[u8], receipt: &str, module: Vec<u8>) -> Result<Self, DataError> {
        if receipt.len() > 1024 * 1024
            || module.len() > 32 * 1024 * 1024
            || plan.len() > 32 * 1024 * 1024
        {
            return Err(unavailable("mixed replacement exceeds its byte limit"));
        }
        let metadata: Json = serde_json::from_str(receipt)
            .map_err(|error| unavailable(format!("invalid mixed receipt: {error}")))?;
        let (javascript, rust) = if metadata["kind"] != "mixed" {
            let retain = self.retain_rust.ok_or_else(|| {
                unavailable("mixed replacement requires a paired JavaScript and Rust generation")
            })?;
            if metadata["kind"] == "rust" {
                return Err(unavailable("Rust replacement is disabled for this host"));
            }
            (
                self.javascript.replacement(plan, receipt, module)?,
                retain(&self.rust)?,
            )
        } else {
            if self.retain_rust.is_some() {
                return Err(unavailable("Rust replacement is disabled for this host"));
            }
            if metadata["version"] != 1
                || !metadata["javascript"].is_object()
                || !metadata["rust"].is_object()
                || metadata["rust"]["kind"] != "rust"
            {
                return Err(unavailable("invalid paired mixed receipt"));
            }
            let split = metadata["javascriptBytes"]
                .as_u64()
                .and_then(|length| usize::try_from(length).ok())
                .filter(|length| *length > 0 && *length < module.len())
                .ok_or_else(|| unavailable("invalid paired mixed module boundary"))?;
            (
                self.javascript.replacement(
                    plan,
                    &metadata["javascript"].to_string(),
                    module[..split].to_vec(),
                )?,
                self.rust.replacement(
                    plan,
                    &metadata["rust"].to_string(),
                    module[split..].to_vec(),
                )?,
            )
        };
        if javascript.app_id() != self.javascript.app_id()
            || rust.app_id() != self.rust.app_id()
            || javascript.grants().trim() != self.javascript.grants().trim()
            || rust.grants().trim() != self.rust.grants().trim()
        {
            return Err(unavailable(
                "mixed replacement changes admitted identity or grants",
            ));
        }
        // Placement is carried, never changed by a replacement (LLP
        // 1027.002 §6): the children's placements decide the sets again.
        Self::construct(javascript, rust, self.sources.clone(), self.retain_rust)
    }
}
