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

/// A host's wake for a pending [`DataSource::preload`]. A source holds at
/// most one at a time, so retries while an image loads add no waiters.
#[derive(Default)]
pub struct PreloadWake {
    wake: Option<std::sync::Arc<dyn Fn() + Send + Sync>>,
    asked: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl PreloadWake {
    /// Use `wake` for later asks. A source still holding the previous wake
    /// no longer counts as asked.
    pub fn set(&mut self, wake: std::sync::Arc<dyn Fn() + Send + Sync>) {
        self.wake = Some(wake);
        self.asked = Default::default();
    }

    /// Ask `source` to wake the host once it can answer, unless it already
    /// holds this wake.
    pub fn ask<D: DataSource + ?Sized>(&self, source: &D) {
        use std::sync::atomic::Ordering;
        let Some(wake) = self.wake.clone() else {
            return;
        };
        if self.asked.swap(true, Ordering::AcqRel) {
            return;
        }
        let asked = self.asked.clone();
        source.when_preloaded(Box::new(move || {
            asked.store(false, Ordering::Release);
            wake()
        }));
    }
}

/// Where a source's long native calls go (`native.later` in TypeScript):
/// the host hands each [`Request::is_native`] request's body and a [`Reply`]
/// to the handler, off the renderer and off the I/O workers, and the call
/// answers when the app's own code sends the reply. Empty until the source
/// activates a native module that takes such calls. Clones share one slot,
/// so a handle taken at construction reaches an instance built or moved to
/// another thread later, as [`Interrupt`] does.
#[derive(Clone, Default)]
pub struct Native(std::sync::Arc<NativeSlots>);

#[derive(Default)]
pub struct NativeSlots {
    handler: std::sync::Mutex<Option<NativeHandler>>,
    hosted: std::sync::Mutex<Option<NativeHandler>>,
    hosted_call: std::sync::Mutex<Option<NativeCall>>,
    announce: std::sync::Mutex<Option<Announce>>,
}

/// Where a host takes the device topics a native module announces
/// ([`Native::changed`]), from any thread (LLP 1016.002).
pub type Announce = std::sync::Arc<dyn Fn(&str) + Send + Sync>;

/// The host's app module answering one call now (`native.call`), on the
/// caller's thread: the request's JSON body in, the JSON reply out, or a
/// refusal message (LLP 1067.000 D9).
pub type NativeCall = std::sync::Arc<dyn Fn(&[u8]) -> Result<Vec<u8>, String> + Send + Sync>;

/// A native module's handler for calls that answer later: the request's
/// JSON body and the reply to send when it is done.
pub type NativeHandler = std::sync::Arc<dyn Fn(Vec<u8>, crate::Reply) + Send + Sync>;

impl Native {
    /// Fill the slot (activation) or empty it (unload).
    pub fn set(&self, handler: Option<NativeHandler>) {
        *self.0.handler.lock().unwrap_or_else(|e| e.into_inner()) = handler;
    }

    /// The handler now: the source's own, else the host's app module.
    pub fn handler(&self) -> Option<NativeHandler> {
        let own = self
            .0
            .handler
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        own.or_else(|| {
            self.0
                .hosted
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone()
        })
    }

    /// The session's app module, which the host owns and installs (LLP
    /// 1067.000 Q6): it outlives the source's activations, so unloading the
    /// source leaves it, and it answers when the source has no handler of
    /// its own. `None` removes it.
    pub fn host(&self, handler: Option<NativeHandler>) {
        *self.0.hosted.lock().unwrap_or_else(|e| e.into_inner()) = handler;
    }

    /// The app module's synchronous call, beside [`Native::host`]; `None`
    /// removes it.
    pub fn host_call(&self, call: Option<NativeCall>) {
        *self.0.hosted_call.lock().unwrap_or_else(|e| e.into_inner()) = call;
    }

    /// The app module's synchronous call, if the host installed one.
    pub fn hosted_call(&self) -> Option<NativeCall> {
        self.0
            .hosted_call
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Whether the host installed an app module.
    pub fn hosted(&self) -> bool {
        self.0
            .hosted
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_some()
    }

    /// Where the host takes announced topics; `None` stops taking them.
    pub fn on_changed(&self, announce: Option<Announce>) {
        *self.0.announce.lock().unwrap_or_else(|e| e.into_inner()) = announce;
    }

    /// The module says `topic` changed; the host asks the resources that
    /// watch it again ([`super::Runner::changed`]). Dropped when no host listens.
    pub fn changed(&self, topic: &str) {
        let announce = self
            .0
            .announce
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        if let Some(announce) = announce {
            announce(topic);
        }
    }
}

/// The continuation token of a module's background work (LLP 1097 D5):
/// reserved, as an executor's own tokens are, and passed through every
/// composer unchanged, so a forwarder never consumes a round's token and a
/// [`DataSource::forgotten`] never prunes it.
pub const BACKGROUND: u64 = u64::MAX - 2;

/// A module's storage (LLP 1097 D8): `state.background`, and the
/// `background` count a `clock` reply carries. Its operations are counted
/// module-wide, an answer's and the background's alike, as the JS target,
/// which has no owners, counts them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BackgroundState {
    /// Operations waiting behind the one in flight.
    pub queued: u64,
    /// Operations in flight: 0 or 1.
    pub in_flight: u64,
    /// Operations that landed, since load.
    pub done: u64,
    /// Operations that failed, since load.
    pub failed: u64,
    /// The last failure's journal line.
    pub last: Option<String>,
}

/// A request still in flight, as [`DataSource::forgotten`] names it.
#[derive(Debug, Clone, Copy)]
pub struct InFlight<'a> {
    /// What it answers.
    pub target: Target,
    /// The source it asks.
    pub source: &'a str,
    /// The arguments it asks with.
    pub args: &'a [Value],
    /// The continuation token the source handed out for it, when it is one
    /// (`None` for a fetch or storage request, or where a forwarder can no
    /// longer tell): what tells two calls with equal arguments apart.
    pub continuation: Option<u64>,
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

impl Target {
    /// `Resource(3)`, `Mutation(0)`: what `{:?}` writes, without `core::fmt`
    /// (a module call's key carries it, on a first press's path).
    pub fn text(self) -> String {
        match self {
            Target::Resource(i) => exact_num::text!("Resource({})", i),
            Target::Mutation(i) => exact_num::text!("Mutation({})", i),
        }
    }
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

    /// Work the module started that no answer waits for (LLP 1097 D5): the
    /// next round, a continuation under [`BACKGROUND`], or `None`. The
    /// runner polls it whenever no round is out; it arms only when the
    /// module's storage operation in flight is the background's, and
    /// returns `None` while a round is out. A source that forwards
    /// `dispatch` forwards this, `background_landed`, `background_state`
    /// and `take_logs` too.
    fn background(&mut self, store: &Store) -> Option<crate::request::Request> {
        let _ = store;
        None
    }

    /// A background round's outcome: the next round, or `None` when the
    /// operation in flight is no longer the background's or nothing was
    /// delivered. It makes no commit: no Contract `then` runs, no resource
    /// is asked again (D4).
    fn background_landed(
        &mut self,
        store: &Store,
        outcome: Outcome,
    ) -> Result<Option<crate::request::Request>, DataError> {
        let _ = (store, outcome);
        Ok(None)
    }

    /// The module's background storage now, or `None` for a source that
    /// has none (LLP 1097 D8).
    fn background_state(&self) -> Option<BackgroundState> {
        None
    }

    /// The module's journal lines since the last take (LLP 1012, LLP 1097
    /// D8): console output, including a refused answer's turn, plus failed
    /// storage and unhandled rejections. The runner writes them to the
    /// journal after each answer and reply, and after a background round.
    fn take_logs(&mut self) -> Vec<String> {
        Vec::new()
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

    /// Call `wake`, from any thread, once a pending [`DataSource::preload`]
    /// would no longer answer false. A source whose `preload` can answer
    /// false overrides this; the default wakes at once, so a wrapper that
    /// forwards `preload` forwards this too, or its host retries unpaced.
    fn when_preloaded(&self, wake: Box<dyn FnOnce() + Send>) {
        wake()
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
    /// 0067): one grant per line — `net.fetch <origin>` (matched whole, or `scheme://*.domain` for every host under one domain), `secret.keep
    /// <name>`. A request outside them fails as `Refused` on every host
    /// before any executor sees it; a secret outside them reads as absent
    /// and refuses a write. Empty: nothing. The set is read whole
    /// ([`crate::grants`]): one line that does not parse refuses the bake,
    /// and on a device grants nothing.
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

    /// A value the runner holds for `source(args)` that this instance did
    /// not answer: the plan's compiled value, which a resource takes at
    /// boot instead of asking (LLP 1027 D11). It is the bake's answer to
    /// the same query, so a source that keeps its own copy of what it
    /// answers may keep this one — cloning shares its allocations — instead
    /// of building it again. Told once per resource that takes it, after
    /// the settlement that published it. Nothing the runner does depends on
    /// whether a source adopts; the default forgets it.
    fn adopt(&mut self, source: &str, args: &[Value], value: &Value) {
        let _ = (source, args, value);
    }

    /// After a commit that let requests go, the ones still in flight. A
    /// request is let go when newer arguments replace it (LLP 1016 D5), when
    /// its target is answered now or assigned, when the runner is poisoned,
    /// or when a refused commit puts back what it had and drops what it
    /// asked; so is one a re-ask with equal arguments replaced (a `refresh`),
    /// which only its continuation token tells apart. Its reply is never
    /// parsed, so a source that parks calls until replies come drops every
    /// call it parked for anything no longer in flight. A call parked without
    /// a target (`answer`) is its own to keep. A source that forwards
    /// `answer_for` forwards this too, translating any continuation token it
    /// remapped back to the one its child handed out.
    /// `store` supplies read context for external work that must finish.
    /// Its reply and any Store writes are discarded, never committed here.
    fn forgotten(&mut self, store: &Store, in_flight: &[InFlight<'_>]) {
        let _ = (store, in_flight);
    }

    /// A handle another thread may trigger to stop this source's running
    /// call (LLP 1048.000 D10), or `None` when a call always returns on its
    /// own, as a Rust source's does. A source that forwards to another
    /// forwards this too.
    fn interrupt(&self) -> Option<Interrupt> {
        None
    }

    /// Where the host sends this source's long native calls, taken at
    /// construction; `None` for a source that makes none. A source that
    /// forwards to another forwards this too.
    fn native(&self) -> Option<Native> {
        None
    }

    /// Whether answers are available now. A TypeScript module before its
    /// host loads it is not (LLP 1027 D4): the runner then boots resources
    /// from compiled placeholders, or matching kept store-reader answers,
    /// and asks again at [`super::Runner::data_ready`] (LLP 1038 D5).
    fn ready(&self) -> bool {
        true
    }

    /// The Canvas 2D surfaces this source draws, name and arity (LLP 1056
    /// D1): known without running app code — a Rust crate's is a constant,
    /// a TypeScript module's is what the bake read. A `canvas surface=`
    /// naming one of these is a 2D canvas the runner draws; any other is the
    /// GPU module's (LLP 1009). A source that forwards forwards this too.
    fn canvas_surfaces(&self) -> Vec<(String, usize)> {
        Vec::new()
    }

    /// Draw one 2D surface with the Rust recorder `ctx` (LLP 1056 D1):
    /// `Ok(true)` asks for another frame. A throw keeps every call made
    /// before it (D4, r3). The default draws nothing, by name.
    fn draw_2d(
        &mut self,
        surface: &str,
        args: &[Value],
        ctx: &exact_canvas::Context2d,
        frame: &exact_canvas::Frame,
    ) -> Result<bool, exact_canvas::DrawError> {
        let _ = (args, ctx, frame);
        Err(exact_canvas::DrawError::Message(format!(
            "this source draws no 2D surface `{surface}`"
        )))
    }

    /// Run one draw request (LLP 1056 D4). The runner keeps one Rust
    /// recorder per canvas generation and passes it as `ctx`; the default
    /// draws with [`DataSource::draw_2d`] into it and answers now. A source
    /// with its own recorders (TypeScript) records with its own, keyed by
    /// the request's canvas and generation, and may answer
    /// [`super::Drawn::Later`], delivering through
    /// [`super::Runner::canvas_reply`]. A source that forwards forwards this
    /// too.
    fn draw(
        &mut self,
        request: &super::DrawRequest<'_>,
        ctx: &exact_canvas::Context2d,
    ) -> super::Drawn {
        let result = self.draw_2d(request.surface, request.args, ctx, &request.frame);
        super::Drawn::Now(super::DrawReply {
            lists: ctx.take_lists(),
            wants_frame: matches!(result, Ok(true)),
            error: result.err().map(|e| e.to_string()),
            notes: ctx.take_notes(),
        })
    }

    /// The canvas generations a source keeps recorders for are gone: a
    /// source with its own recorders drops them. The runner calls this for
    /// canvases unmounted or superseded.
    fn canvases_retired(&mut self, retired: &[(u64, u32)]) {
        let _ = retired;
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
    /// Build-time storage cannot answer; use the resource placeholder until activation.
    DeferredAtBake(String),
    /// The seam to a loaded Rust module could not carry the call (an
    /// answer over its bound, a trap): the source is there but cannot
    /// answer now. A resource asked this fails; it does not refuse the
    /// commit (LLP 1071 §7, Charlie's ruling of 2026-09-29).
    Interface(String),
}

#[cfg(test)]
mod target_tests {
    use super::Target;

    #[test]
    fn a_target_text_is_its_debug_text() {
        for i in [0, 1, 9, 10, 4_294_967_295, usize::MAX] {
            for target in [Target::Resource(i), Target::Mutation(i)] {
                assert_eq!(target.text(), format!("{target:?}"));
            }
        }
    }
}
