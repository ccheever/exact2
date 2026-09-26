//! A render's environment (LLP 1048 D3, 1048.000 D9): the app's own data
//! source, holding nothing private. Its grants are the app's `net.fetch`
//! lines and nothing else — no kept secrets (the store is empty and can't
//! be written), no files, no SQLite, no key-value storage — so a render
//! answers what a new anonymous device would see. A render never configures
//! storage and never replaces the logic it booted with.
use exact_plan::{Plan, Value};
use exact_runner::{
    Answer, DataError, DataSource, Dispatch, InFlight, Interrupt, Outcome, Placement, Store, Target,
};

/// The app's data source in a render's environment.
pub struct Anonymous<D> {
    inner: D,
    grants: String,
}

impl<D: DataSource> Anonymous<D> {
    /// Wrap `inner`; its grants narrow to what a render holds.
    pub fn new(inner: D) -> Self {
        let grants = inner
            .grants()
            .lines()
            .map(str::trim)
            .filter(|line| line.split_whitespace().next() == Some("net.fetch"))
            .collect::<Vec<_>>()
            .join("\n");
        Self { inner, grants }
    }
}

impl<D: DataSource> DataSource for Anonymous<D> {
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        self.inner.query(source, args)
    }
    fn app_id(&self) -> &str {
        self.inner.app_id()
    }
    fn answer(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        self.inner.answer(store, source, args)
    }
    fn parse(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        self.inner.parse(store, source, args, outcome)
    }
    fn answer_for(
        &mut self,
        target: Target,
        store: &mut Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        self.inner.answer_for(target, store, source, args)
    }
    fn parse_for(
        &mut self,
        target: Target,
        store: &mut Store,
        source: &str,
        args: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        self.inner.parse_for(target, store, source, args, outcome)
    }
    fn grants(&self) -> &str {
        &self.grants
    }
    fn revision(&self) -> Option<&str> {
        self.inner.revision()
    }
    fn bind(&mut self, plan: &Plan) {
        self.inner.bind(plan);
    }
    fn adopt(&mut self, source: &str, args: &[Value], value: &Value) {
        self.inner.adopt(source, args, value);
    }
    fn ready(&self) -> bool {
        self.inner.ready()
    }
    fn preload(&self) -> Result<bool, DataError> {
        self.inner.preload()
    }
    fn activate(&mut self) -> Result<(), DataError> {
        self.inner.activate()
    }
    fn activate_for_validation(&mut self) -> Result<(), DataError> {
        self.inner.activate_for_validation()
    }
    fn continuation(&mut self, token: u64) -> Option<Box<dyn FnOnce() -> Outcome + Send>> {
        self.inner.continuation(token)
    }
    fn placement(&self) -> Placement {
        self.inner.placement()
    }
    fn interrupt(&self) -> Option<Interrupt> {
        self.inner.interrupt()
    }
    fn forgotten(&mut self, in_flight: &[InFlight<'_>]) {
        self.inner.forgotten(in_flight);
    }
    fn dispatch(&mut self, token: u64, store: &Store) -> Dispatch {
        self.inner.dispatch(token, store)
    }
    fn release(&mut self, store: &Store) -> Vec<(u64, Dispatch)> {
        self.inner.release(store)
    }
    fn discard(&mut self, token: u64) {
        self.inner.discard(token);
    }
}
