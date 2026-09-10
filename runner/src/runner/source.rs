//! Data-source boundary (LLP 1004 D4, LLP 1027 D4–D5).
use crate::request::{Answer, Outcome};
use crate::store::Store;
use exact_plan::{Plan, Value};

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

    /// Answer one distinct runner call. `context` is fresh for each call,
    /// including equal-argument siblings and superseding requests; it remains
    /// the same across every subsequent `parse_scoped` continuation.
    /// Adapters with parked executor work use it as their call identity.
    fn answer_scoped(
        &mut self,
        context: u64,
        store: &mut Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        let _ = context;
        self.answer(store, source, args)
    }

    /// Parse a reply for the exact call that produced it. Chained `Later`
    /// answers keep this context while receiving a fresh host ticket.
    fn parse_scoped(
        &mut self,
        context: u64,
        store: &mut Store,
        source: &str,
        args: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        let _ = context;
        self.parse(store, source, args, outcome)
    }

    /// Release the executor state for a completed, superseded, unmounted,
    /// or refused call. Cancellation of previous calls waits for commit so
    /// a pre-walk refusal can restore the previous pending request.
    fn cancel_scoped(&mut self, context: u64) {
        let _ = context;
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

    /// Whether answers are available now. A TypeScript module before its
    /// host loads it is not (LLP 1027 D4): the runner then boots every
    /// store-reading resource from its kept answer or its compiled
    /// empty-store placeholder, and asks again at [`super::Runner::data_ready`].
    fn ready(&self) -> bool {
        true
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
