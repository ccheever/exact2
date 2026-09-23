//! Data-source boundary (LLP 1004 D4, LLP 1027 D4–D5).
use crate::request::{Answer, Dispatch, Outcome, Placement, Work};
use crate::store::Store;
use exact_plan::{Plan, Value};

/// Stops a source's running call from another thread (LLP 1048.000 D10: a
/// render's deadline). The call ends as a refusal, as one that threw does,
/// and its resource keeps what it showed. Clones stop the same source.
#[derive(Clone)]
pub struct Interrupt(std::sync::Arc<dyn Fn() + Send + Sync>);

impl Interrupt {
    /// A handle whose [`Interrupt::trigger`] runs `stop`, on the caller's
    /// thread.
    pub fn new(stop: impl Fn() + Send + Sync + 'static) -> Interrupt {
        Interrupt(std::sync::Arc::new(stop))
    }

    /// Stop the call running now, or the next one to start.
    pub fn trigger(&self) {
        (self.0)()
    }
}

/// What a request answers: a resource or a mutation, by its index in the
/// plan. The runner keeps at most one request in flight per target (LLP
/// 1016 D5), so an executor that parks a call until its reply comes keys
/// it by target: two targets may ask one source with equal arguments.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Target {
    /// `plan.resources[i]`.
    Resource(usize),
    /// `plan.mutations[i]`.
    Mutation(usize),
}

/// The app's data source: the one seam through which computation enters
/// (LLP 1004 D4). Implemented once, in Rust, by the app's data crate.
pub trait DataSource {
    /// Host-selected application directories. Configuration records paths only;
    /// implementations must defer opening storage until after first pixel.
    fn configure_storage(
        &mut self,
        data: std::path::PathBuf,
        cache: std::path::PathBuf,
        temporary: std::path::PathBuf,
    ) -> Result<(), DataError> {
        let _ = (data, cache, temporary);
        Ok(())
    }

    /// Take native work for an executor-local continuation exactly once. The
    /// closure owns its inputs and runs on the host worker, never the renderer.
    /// Missing or consumed tokens are refused by that executor.
    fn continuation(&mut self, token: u64) -> Option<Box<dyn FnOnce() -> Outcome + Send>> {
        let _ = token;
        None
    }

    /// Where this source's module instance runs (LLP 1027.002 D1). A
    /// composer orders every child that shares a `secret.keep` name with a
    /// worker child (D3); a host refuses a placement it cannot run.
    fn placement(&self) -> Placement {
        Placement::Main
    }

    /// The work behind continuation `token`, at dispatch: on the runner's
    /// thread, after the commit that handed the request out, with the store
    /// as committed then (LLP 1027.002 D3, change 1). A worker's proxy takes
    /// its scoped snapshot here. The default is `continuation`'s closure on
    /// the host's I/O worker, which is what every source did before
    /// placement existed.
    fn dispatch(&mut self, token: u64, store: &Store) -> Dispatch {
        let _ = store;
        match self.continuation(token) {
            Some(work) => Dispatch::Run(Work::Now(work)),
            None => Dispatch::Missing,
        }
    }

    /// Work `dispatch` answered `Held` that the commit just made releases,
    /// in order (LLP 1027.002 D3, change 2). A host asks after every commit
    /// and runs each as it would have at dispatch.
    fn release(&mut self, store: &Store) -> Vec<(u64, Dispatch)> {
        let _ = store;
        Vec::new()
    }

    /// A `Later` answer whose transaction was refused: its token is never
    /// dispatched. A source that recorded the call forgets it here (LLP
    /// 1027.002 D3, the cleanup of rolled-back calls).
    fn discard(&mut self, token: u64) {
        let _ = token;
    }

    /// Activate deferred logic after first pixel; binary-bound sources do nothing.
    fn activate(&mut self) -> Result<(), DataError> {
        Ok(())
    }

    /// Activate a disposable post-pixel validation candidate. Replaceable
    /// executors must withhold storage capabilities here: its runner may ask
    /// answers to validate carried state before all sessions accept the pair.
    /// Requests/effects stay with the uncommitted host and are discarded.
    fn activate_for_validation(&mut self) -> Result<(), DataError> {
        self.activate()
    }

    /// Prepare executable images after first pixel without blocking the caller.
    /// False means pending: retain the current generation and retry preparation.
    /// This may load code, but must not create app instances or release effects.
    fn preload(&self) -> Result<bool, DataError> {
        Ok(true)
    }

    /// Pair candidate logic with a plan, preserving this binary's admitted
    /// app identity and grants. Does not execute candidate code.
    fn replacement(&self, plan: &[u8], receipt: &str, module: Vec<u8>) -> Result<Self, DataError>
    where
        Self: Sized,
    {
        let _ = (plan, receipt, module);
        Err(DataError::Unavailable(
            "this client has binary-bound logic; rebuild it".into(),
        ))
    }

    /// Answer a resource's or a mutation's request now. `args` are the
    /// resource's argument expressions evaluated against current state, or
    /// a `send`'s arguments. Bake and every in-process source use this.
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError>;

    /// The app identity, reverse-DNS (LLP 1023 D5) — the one declaration:
    /// bake writes it into the plan header, and boot refuses a plan whose
    /// header names a different app. Empty is unnamed — a fixture or a
    /// stand-in — and unnamed matches anything.
    fn app_id(&self) -> &str {
        ""
    }

    /// Answer now, or hand back a request the host will run (LLP 1016 D1:
    /// the runner never does I/O). The default answers `query` now; a source
    /// that reaches outside the process overrides this and [`parse`]. The
    /// [`Store`] is the app's durable state (LLP 1018 D1) — the host's
    /// snapshot, read here synchronously; a write rides the commit out.
    ///
    /// [`parse`]: DataSource::parse
    fn answer(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        let _ = store;
        self.query(source, args).map(Answer::Now)
    }

    /// The value of a resource or mutation from what the host brought back
    /// for a request `answer` handed out, in the shape the declaration
    /// names — or one more request (LLP 1027 D1a: a TypeScript `answer`
    /// that awaits a second `fetch` is pending again, on the same target,
    /// with the same arguments). No I/O, no host — the store is the one
    /// thing it may write (a token from a reply, LLP 1018 D5). A failure on
    /// the wire is an `Outcome` too — what the app sees is the source's to
    /// decide (D4). A source that never answers later need not implement it.
    fn parse(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        let _ = (store, args, outcome);
        Err(DataError::UnknownSource(source.to_string()))
    }

    /// [`answer`] for `target`, which is how the runner asks. A source that
    /// parks a call until its reply keys it by target; one that forwards to
    /// another source forwards this and [`parse_for`] too. The default
    /// forgets the target.
    ///
    /// [`answer`]: DataSource::answer
    /// [`parse_for`]: DataSource::parse_for
    fn answer_for(
        &mut self,
        target: Target,
        store: &mut Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        let _ = target;
        self.answer(store, source, args)
    }

    /// [`parse`] for the reply to `target`'s request; see [`answer_for`].
    ///
    /// [`parse`]: DataSource::parse
    /// [`answer_for`]: DataSource::answer_for
    fn parse_for(
        &mut self,
        target: Target,
        store: &mut Store,
        source: &str,
        args: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        let _ = target;
        self.parse(store, source, args, outcome)
    }

    /// What the app may reach and keep (LLP 1016 D6, LLP 1018 D3; ibex LLP
    /// 0067): one grant per line — `net.fetch <url prefix>`, `secret.keep
    /// <name>`. A request outside them fails as `Refused` on every host
    /// before any executor sees it; a secret outside them reads as absent
    /// and refuses a write. Empty: nothing.
    fn grants(&self) -> &str {
        ""
    }

    /// Identity of replaceable logic, if any. A changed identity invalidates
    /// carried resource answers, but not slots, the clock, or app secrets.
    /// Rust sources remain binary-bound and return `None`.
    fn revision(&self) -> Option<&str> {
        None
    }

    /// The plan this source answers for — once, at boot, after the identity
    /// gate and before any answer (LLP 1027 D2). An executor that marshals
    /// by the plan's declared shapes reads the `sources` table here; a Rust
    /// crate has nothing to learn and ignores it.
    fn bind(&mut self, plan: &Plan) {
        let _ = plan;
    }

    /// A handle another thread may trigger to stop this source's running
    /// call (LLP 1048.000 D10), or `None` when a call always returns on its
    /// own, as a Rust source's does. A source that forwards to another
    /// forwards this too.
    fn interrupt(&self) -> Option<Interrupt> {
        None
    }

    /// Whether answers are available now. A TypeScript module before its
    /// host loads it is not (LLP 1027 D4): the runner then boots resources
    /// from compiled placeholders, or matching kept store-reader answers,
    /// and asks again at [`super::Runner::data_ready`] (LLP 1038 D5).
    fn ready(&self) -> bool {
        true
    }
}

/// A Contract app without application data has no sources to answer.
impl DataSource for () {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

/// Why a data source could not answer.
#[allow(missing_docs)]
#[derive(Debug, Clone, PartialEq)]
pub enum DataError {
    UnknownSource(String),
    BadArguments(String),
    Unavailable(String),
}
