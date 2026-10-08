//! Snapback4's client protocol, without I/O: one implementation for every
//! Exact host and for Rust and TypeScript callers alike.
//!
//! The client owns what Snapback's own TypeScript client (`snapback4/local`)
//! owns over a device: opening on the kept partition or the server's backend,
//! admitting writes with their predictions, the sync round (paging, store
//! identity, generation adoption), the outbox's at-most-once sends and their
//! receipts, and the change poll. It performs no I/O and reads no clock or
//! entropy. A round is a coroutine: each [`Step::Fetch`] is one HTTP exchange
//! for the driver to perform against the Snapback origin, and [`Client::deliver`]
//! hands its [`Reply`] back. Device calls are answered inline through [`Core`].
//! A TypeScript driver loops over `fetch`; a Rust source answers each step
//! with `Answer::Later`. Time arrives as arguments (`now`).

use serde_json::{json, Value as Json};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

/// What the client needs of a device: the device's JSON call, plus the host's
/// `open`/`close` (`{op:"open", path, origin, viewer, backend?}`). A host
/// configuration or authority failure is `Err`; a device refusal is
/// `{denied}` inside `Ok`, as the device returns it.
pub trait Core {
    fn call(&mut self, request: Json) -> Result<Json, String>;
}

/// One HTTP exchange the driver performs, relative to the Snapback origin.
/// The driver adds the origin, `content-type: application/json` when there is
/// a body, and its credentials (a bearer session, or a development persona).
#[derive(Clone, Debug, PartialEq)]
pub struct Fetch {
    pub method: &'static str,
    pub path: String,
    pub body: Option<Json>,
    /// Names this exchange: its reply is delivered with it, and a reply
    /// naming another (an earlier round's, a closed client's) is refused.
    pub exchange: String,
}

impl Fetch {
    pub(crate) fn get(path: String) -> Self {
        Self {
            method: "GET",
            path,
            body: None,
            exchange: String::new(),
        }
    }
    pub(crate) fn post(path: String, body: Json) -> Self {
        Self {
            method: "POST",
            path,
            body: Some(body),
            exchange: String::new(),
        }
    }
    pub fn to_json(&self) -> Json {
        let mut value =
            json!({"method": self.method, "path": self.path, "exchange": self.exchange});
        if let Some(body) = &self.body {
            value["body"] = body.clone();
        }
        value
    }
}

/// What came back from a [`Fetch`]: the status and the body parsed as JSON
/// (`Null` when it was not JSON), or a transport failure.
#[derive(Clone, Debug, PartialEq)]
pub enum Reply {
    Http { status: u16, body: Json },
    Failed(String),
}

impl Reply {
    /// `{status, body}` or `{error}`, the driver's JSON form.
    pub fn from_json(value: &Json) -> Result<Self, String> {
        if let Some(error) = value.get("error") {
            return Ok(Self::Failed(
                error.as_str().unwrap_or("the request failed").into(),
            ));
        }
        let status = value
            .get("status")
            .and_then(Json::as_u64)
            .filter(|status| (100..600).contains(status))
            .ok_or("a reply is {status, body} or {error}")?;
        Ok(Self::Http {
            status: status as u16,
            body: value.get("body").cloned().unwrap_or(Json::Null),
        })
    }
}

/// What driving a round produces next.
#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    /// Perform this exchange, then call [`Client::deliver`] with its reply.
    Fetch(Fetch),
    /// The round ended: `{ok:true}` when caught up, else `{ok:false, offline,
    /// retry, denied}`: `offline`, the server was not reached; `retry`, it
    /// asked to be asked again later; otherwise `denied` is its refusal.
    Done(Json),
}

impl Step {
    pub fn to_json(&self) -> Json {
        match self {
            Step::Fetch(fetch) => json!({"fetch": fetch.to_json()}),
            Step::Done(outcome) => json!({"done": outcome}),
        }
    }
}

/// Where a partition lives and whose it is.
#[derive(Clone, Debug)]
pub struct Config {
    /// `app:/data/<name>.sqlite`.
    pub path: String,
    /// The Snapback origin; part of the partition's identity.
    pub origin: String,
    /// The principal this device reads and writes as (`dev:alice`, `user:…`).
    pub viewer: String,
}

/// A refusal as Snapback words one: `{code, family, message, ...}`.
pub(crate) type Refusal = Json;

pub(crate) fn refusal(code: &str, family: &str, message: impl Into<String>) -> Refusal {
    json!({"code": code, "family": family, "message": message.into()})
}

/// What the round and the client share between steps.
#[derive(Default)]
pub(crate) struct Shared {
    pub opened: bool,
    pub online: Option<bool>,
    pub denied: Option<Refusal>,
    /// Recent terminal outcomes with their results, newest last.
    pub outcomes: Vec<(String, Json)>,
    /// Terminal outcomes the server gave that the device has not yet kept
    /// and settled (a failed save, or a round cut off between the two).
    /// Every round persists these before anything else (`persistPending`).
    pub unpersisted: Vec<Unpersisted>,
    /// Increments whenever the partition's rows or outbox may have changed:
    /// what a screen compares to know it must read again.
    pub revision: u64,
}

/// A receipt held in memory until the device keeps and settles it.
#[derive(Clone)]
pub(crate) struct Unpersisted {
    pub id: String,
    pub outcome: Json,
    pub revalidated: Option<Json>,
    pub kept: bool,
}

const OUTCOMES: usize = 256;

/// The last submission number a write took, refused ones included (the
/// device counts only what it admitted).
const SUBMITTED: &str = "exact:submission";

impl Shared {
    pub fn finish(&mut self, id: &str, outcome: Json) {
        self.outcomes.retain(|(known, _)| known != id);
        self.outcomes.push((id.into(), outcome));
        if self.outcomes.len() > OUTCOMES {
            self.outcomes.remove(0);
        }
    }
}

enum Yield {
    Device(Json),
    Fetch(Fetch),
}

enum Back {
    Device(Result<Json, String>),
    Fetch(Reply),
}

#[derive(Default)]
struct Channel {
    out: Option<Yield>,
    back: Option<Back>,
}

/// The round's side of the coroutine: each await hands one request out.
#[derive(Clone)]
pub(crate) struct Io(Arc<Mutex<Channel>>);

struct Suspend(bool);

impl Future for Suspend {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<()> {
        if self.0 {
            Poll::Ready(())
        } else {
            self.0 = true;
            Poll::Pending
        }
    }
}

impl Io {
    fn channel(&self) -> std::sync::MutexGuard<'_, Channel> {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// What the round asked for at its last await (the lock ends here).
    fn take_out(&self) -> Option<Yield> {
        self.channel().out.take()
    }

    /// A raw device call: the host's `Err`, or the device's `{ok}`/`{denied}`.
    pub async fn raw(&self, request: Json) -> Result<Json, String> {
        self.channel().out = Some(Yield::Device(request));
        Suspend(false).await;
        match self.channel().back.take() {
            Some(Back::Device(answer)) => answer,
            _ => Err("the Snapback4 client lost its device answer".into()),
        }
    }

    /// A device call's `ok` value; a host failure or the device's `denied`
    /// is the round's refusal.
    pub async fn device(&self, request: Json) -> Result<Json, Refusal> {
        let answer = self
            .raw(request)
            .await
            .map_err(|e| refusal("E_STORE", "store", e))?;
        match answer.get("denied") {
            Some(denied) => Err(denied.clone()),
            None => Ok(answer.get("ok").cloned().unwrap_or(Json::Null)),
        }
    }

    pub async fn fetch(&self, fetch: Fetch) -> Reply {
        self.channel().out = Some(Yield::Fetch(fetch));
        Suspend(false).await;
        match self.channel().back.take() {
            Some(Back::Fetch(reply)) => reply,
            _ => Reply::Failed("the Snapback4 client lost its reply".into()),
        }
    }
}

type Round = Pin<Box<dyn Future<Output = Json> + Send>>;

struct Running {
    future: Round,
    io: Io,
    /// The exchange whose reply the round awaits, if it awaits one.
    awaiting: Option<String>,
}

/// One partition's client. See the module documentation.
pub struct Client {
    config: Arc<Config>,
    shared: Arc<Mutex<Shared>>,
    round: Option<Running>,
    /// This client among every client in the process: exchange tokens carry it.
    incarnation: u64,
    exchanges: u64,
    /// The change poll awaiting its reply: a newer poll supersedes it, and a
    /// reply is consumed when delivered.
    poll: Option<String>,
    /// The session refresh awaiting its reply, as `poll` is for the poll.
    refresh: Option<String>,
}

static INCARNATIONS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

fn lock(shared: &Mutex<Shared>) -> std::sync::MutexGuard<'_, Shared> {
    shared
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn ok(answer: Result<Json, String>) -> Result<Json, String> {
    let answer = answer?;
    match answer.get("denied") {
        Some(denied) => Err(format!(
            "{}: {}",
            denied["code"].as_str().unwrap_or("E_STORE"),
            denied["message"]
                .as_str()
                .unwrap_or("Snapback refused the operation")
        )),
        None => Ok(answer.get("ok").cloned().unwrap_or(Json::Null)),
    }
}

/// The open request a partition makes; `backend` is the server's `/schema`.
pub(crate) fn open_request(config: &Config, backend: Option<Json>) -> Json {
    json!({"op": "open", "path": config.path, "origin": config.origin,
        "viewer": config.viewer, "backend": backend})
}

/// What the host says when a partition has never held a backend.
pub const NEEDS_BACKEND: &str = "this Snapback4 partition needs the server's backend once";

impl Client {
    pub fn new(config: Config) -> Self {
        Self {
            config: Arc::new(config),
            shared: Arc::default(),
            round: None,
            incarnation: INCARNATIONS.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            exchanges: 0,
            poll: None,
            refresh: None,
        }
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Open on what the device kept, with no network. `Ok(false)`: this
    /// partition has never synced, and the first round opens it on the
    /// server's backend.
    pub fn open(&mut self, core: &mut dyn Core) -> Result<bool, String> {
        if lock(&self.shared).opened {
            return Ok(true);
        }
        match ok(core.call(open_request(&self.config, None))) {
            Ok(_) => {
                lock(&self.shared).opened = true;
                Ok(true)
            }
            Err(error) if error == NEEDS_BACKEND => Ok(false),
            Err(error) => Err(error),
        }
    }

    pub fn is_open(&self) -> bool {
        lock(&self.shared).opened
    }

    fn require_open(&mut self, core: &mut dyn Core) -> Result<(), String> {
        if self.open(core)? {
            Ok(())
        } else {
            Err("E_OFFLINE: this device has never synced; connect once to open it".into())
        }
    }

    /// A named query over the partition: the device's card as it reads it
    /// (`data`, `complete`, `next`, `loading`, `pending` rows, or `denied`).
    pub fn read(
        &mut self,
        core: &mut dyn Core,
        name: &str,
        args: Json,
        now: i64,
    ) -> Result<Json, String> {
        self.require_open(core)?;
        ok(core.call(
            json!({"op": "query", "name": name, "viewer": self.config.viewer,
            "args": args, "now": now}),
        ))
    }

    /// Whether the device answers the query `name` from its partition: every
    /// table it or its rules read is synced here. A query over an `online
    /// only` table or view is the server's to answer (`POST /q/<name>`); the
    /// device would answer it with unknown coverage, an empty page that only
    /// looks like an answer.
    pub fn answers_on_device(&mut self, core: &mut dyn Core, name: &str) -> Result<bool, String> {
        self.require_open(core)?;
        let backend = ok(core.call(json!({"op": "backend"})))?;
        Ok(crate::backend::predictable(&backend, name))
    }

    /// Admit a write: kept in the outbox and predicted in one device commit.
    /// Returns `{id, state:"pending", newIds, result?}`, which the next round
    /// sends, or the refusal as `{id, state:"failed", why}`: a write the
    /// device refuses (its prediction refused it, as the server would on the
    /// same rows) is never sent, and `outcome` and `refusals` answer it as
    /// they answer the server's refusals.
    ///
    /// With a `key` (an idempotency key: the app's name for this intent, such
    /// as the draft version a post publishes) the write's id is derived from
    /// it ([`Client::write_id`]): asking again for the same key admits
    /// nothing new and answers what became of the first. A prediction does
    /// not refuse a keyed write: it queues without a row, for the server,
    /// which may hold its receipt. A keyed write the device cannot admit at
    /// all (the same key with other input, `E_WRITE_ID_REUSE`) is refused in
    /// this answer only: the id names the first write, or none yet.
    pub fn write(
        &mut self,
        core: &mut dyn Core,
        op: &str,
        args: Json,
        now: i64,
        key: Option<&str>,
    ) -> Result<Json, String> {
        self.require_open(core)?;
        let backend = ok(core.call(json!({"op": "backend"})))?;
        let device = self.device(core)?;
        // A refused write consumed its number too (`SUBMITTED`), which the
        // device does not count: the next write must not take its id.
        let submitted = ok(core.call(json!({"op": "meta", "key": SUBMITTED})))?;
        let seq = ok(core.call(json!({"op": "next_submission"})))?
            .as_u64()
            .ok_or("next_submission is an integer")?
            .max(
                submitted
                    .as_str()
                    .and_then(|n| n.parse::<u64>().ok())
                    .map_or(0, |n| n + 1),
            );
        let (id, new_ids): (String, Vec<String>) = match key {
            Some(key) => {
                let id = keyed_id(&device, key, None);
                // A key names one intent: the same key with other input is
                // refused, as Snapback's own client refuses a reused id, for
                // as long as the device knows what the first carried.
                if let Some(earlier) = self.input_of(core, &id)? {
                    if earlier != input_key(op, &args) {
                        return Ok(json!({"id": id, "state": "failed", "why": {
                            "code": "E_WRITE_ID_REUSE", "family": "input",
                            "message": "this key already names a write with other input"}}));
                    }
                }
                let known = self.outcome(core, &id)?;
                if known["state"] != "unknown" {
                    return Ok(known);
                }
                (
                    id,
                    (0..8).map(|i| keyed_id(&device, key, Some(i))).collect(),
                )
            }
            None => (
                write_id(&device, seq, None),
                (0..8).map(|i| write_id(&device, seq, Some(i))).collect(),
            ),
        };
        let predictable = crate::backend::predictable(&backend, op);
        // A keyed write whose prediction refuses queues without a row so the
        // server, which may hold its receipt, decides; a fresh id's refusal
        // is final here (Snapback's `mutate`, a caller-supplied id).
        let entry = json!({"id": id, "seq": seq, "op": op, "args": args, "viewer": self.config.viewer,
            "now": now, "new_ids": new_ids, "predictable": predictable, "predicted": [],
            "replay_candidate": key.is_some()});
        // Ephemeral writes are not in this client yet: refuse them here, as a
        // terminal failure the app sees, rather than queue what never lands.
        let unsupported = crate::backend::touches_ephemeral(&backend, op).then(|| {
            json!({"denied": {"code": "E_CLIENT_UNSUPPORTED", "family": "client",
                "message": format!("{op} writes an ephemeral table, which this client does not send yet (snapback4/README.md); it was not sent")}})
        });
        let admitted = match unsupported {
            Some(refused) => refused,
            None => core.call(json!({"op": "admit", "entry": entry}))?,
        };
        if let Some(why) = admitted.get("denied") {
            // A keyed write the device could not admit at all (a reused key,
            // a device that cannot write) took nothing: its key names the
            // first write, or none yet. Any other refusal is this write's.
            if key.is_none() {
                ok(core
                    .call(json!({"op": "set_meta", "key": SUBMITTED, "value": seq.to_string()})))?;
                self.refused_here(core, &entry, why)?;
            }
            return Ok(json!({"id": id, "state": "failed", "why": why}));
        }
        lock(&self.shared).revision += 1;
        let mut reply = json!({"id": id, "state": "pending", "newIds": new_ids});
        if let Some(result) = admitted["ok"].get("result") {
            reply["result"] = result.clone();
        }
        Ok(reply)
    }

    /// A write the device refused at admission (its prediction refused it,
    /// as the server would on the same rows, or the device could not admit
    /// it) is terminal, as in Snapback's own client (`mutate`'s local
    /// failure): it is never sent, and [`Client::outcome`] and
    /// [`Client::refusals`] answer it as they answer the server's refusals.
    fn refused_here(
        &mut self,
        core: &mut dyn Core,
        entry: &Json,
        why: &Json,
    ) -> Result<(), String> {
        let id = entry["id"].as_str().unwrap_or_default();
        let failed = json!({"id": id, "state": "failed", "why": why});
        lock(&self.shared).finish(id, failed.clone());
        ok(core.call(json!({"op": "keep_write", "value": failed.to_string()})))?;
        let refused = crate::journal::entry(entry, why);
        for (key, value) in crate::journal::append(&self.journal(core)?, refused) {
            ok(core.call(json!({"op": "set_meta", "key": key, "value": value})))?;
        }
        lock(&self.shared).revision += 1;
        Ok(())
    }

    /// The digest of the operation and input a write carried ([`input_key`]),
    /// while the device still knows it: queued in the outbox, refused in the
    /// journal, or answered in a kept receipt.
    fn input_of(&mut self, core: &mut dyn Core, id: &str) -> Result<Option<String>, String> {
        let queued = ok(core.call(json!({"op": "queued"})))?;
        if let Some(entry) = queued
            .as_array()
            .into_iter()
            .flatten()
            .find(|e| e["id"] == id)
        {
            return Ok(Some(input_key(
                entry["op"].as_str().unwrap_or_default(),
                &entry["args"],
            )));
        }
        let refused = self.refusals(core)?;
        if let Some(entry) = refused
            .as_array()
            .into_iter()
            .flatten()
            .find(|e| e["id"] == id)
        {
            return Ok(entry["input"].as_str().map(str::to_owned));
        }
        let history = ok(core.call(json!({"op": "meta", "key": crate::round::HISTORY})))?;
        Ok(crate::round::history_input(&history, id))
    }

    /// The id a write with this idempotency `key` has (or would have) on this
    /// device: for asking what became of it ([`Client::outcome`]) before, or
    /// instead of, writing.
    pub fn write_id(&mut self, core: &mut dyn Core, key: &str) -> Result<String, String> {
        self.require_open(core)?;
        Ok(keyed_id(&self.device(core)?, key, None))
    }

    fn device(&self, core: &mut dyn Core) -> Result<String, String> {
        let device = ok(core.call(json!({"op": "meta", "key": "exact:device"})))?;
        device
            .as_str()
            .filter(|d| !d.is_empty())
            .map(str::to_owned)
            .ok_or_else(|| "Snapback4 device has no identity".into())
    }

    /// Save the outcomes this client holds now, without the network (a host
    /// that just handed them over with [`Client::hold`] must not wait for a
    /// round). `{ok:true}`, or `{ok:false, denied}` if the device refused.
    pub fn persist(&mut self, core: &mut dyn Core) -> Json {
        if self.round.is_some() {
            return json!({"ok": false, "busy": true});
        }
        let io = Io(Arc::default());
        let future = Box::pin(crate::round::persist_round(io.clone(), self.shared.clone()));
        self.round = Some(Running {
            future,
            io,
            awaiting: None,
        });
        match self.advance(core, None) {
            Step::Done(done) => done,
            Step::Fetch(_) => {
                self.round = None;
                json!({"ok": false, "denied": refusal("E_STORE", "store", "saving held outcomes asked for the network")})
            }
        }
    }

    /// A write's state: `pending` while queued, `sent` or `failed` once the
    /// server answered (with its result while this client remembers it), or
    /// `unknown`. A receipt outranks the outbox: a write the server answered
    /// whose entry has not yet been retired reads as answered.
    pub fn outcome(&mut self, core: &mut dyn Core, id: &str) -> Result<Json, String> {
        {
            let shared = lock(&self.shared);
            if let Some((_, outcome)) = shared.outcomes.iter().find(|(known, _)| known == id) {
                return Ok(outcome.clone());
            }
            if let Some(held) = shared.unpersisted.iter().find(|held| held.id == id) {
                return Ok(held.outcome.clone());
            }
        }
        self.require_open(core)?;
        let queued = ok(core.call(json!({"op": "queued"})))?;
        let entry = queued
            .as_array()
            .into_iter()
            .flatten()
            .find(|entry| entry["id"] == id)
            .cloned();
        if let Some(seq) = entry.as_ref().and_then(crate::round::success_sequence) {
            return Ok(json!({"id": id, "state": "sent", "seq": seq, "replayed": true}));
        }
        let history = ok(core.call(json!({"op": "meta", "key": crate::round::HISTORY})))?;
        if let Some(known) = crate::round::history_receipt(&history, id) {
            return Ok(known);
        }
        if entry.is_some() {
            return Ok(json!({"id": id, "state": "pending"}));
        }
        // A refusal outlives its receipt: the journal keeps it until dismissed.
        let refused = self.refusals(core)?;
        if let Some(refused) = refused
            .as_array()
            .into_iter()
            .flatten()
            .find(|r| r["id"] == id)
        {
            return Ok(json!({"id": id, "state": "failed", "why": refused["why"]}));
        }
        Ok(json!({"id": id, "state": "unknown"}))
    }

    /// Every server outcome this client knows, for a host that must rebuild
    /// the device (the web, after a failed save) to hand to the new client
    /// with [`Client::hold`]. Not only the unsaved ones: the device may have
    /// kept and settled an outcome in memory that its host then failed to
    /// make durable. Keeping a receipt again is harmless; settling happens
    /// only where the rebuilt device still has the write queued.
    pub fn held(&self) -> Json {
        let shared = lock(&self.shared);
        let mut held: Vec<Json> = shared
            .unpersisted
            .iter()
            .map(|held| json!({"id": held.id, "outcome": held.outcome, "revalidated": held.revalidated}))
            .collect();
        for (id, outcome) in &shared.outcomes {
            if !shared.unpersisted.iter().any(|held| &held.id == id) {
                held.push(json!({"id": id, "outcome": outcome,
                    "revalidated": outcome.get("revalidated_store").cloned().unwrap_or(Json::Null)}));
            }
        }
        Json::Array(held)
    }

    /// Take over outcomes another client held ([`Client::held`]); the next
    /// round keeps and settles them before anything else.
    pub fn hold(&mut self, held: &Json) {
        let mut shared = lock(&self.shared);
        for item in held.as_array().into_iter().flatten() {
            let Some(id) = item["id"].as_str() else {
                continue;
            };
            if shared.unpersisted.iter().any(|known| known.id == id) {
                continue;
            }
            shared.finish(id, item["outcome"].clone());
            shared.unpersisted.push(Unpersisted {
                id: id.into(),
                outcome: item["outcome"].clone(),
                revalidated: item.get("revalidated").filter(|v| !v.is_null()).cloned(),
                kept: false,
            });
        }
    }

    /// Writes the server refused, oldest first, each with its `id`, `op`, the
    /// `args` it carried, the refusal (`why`) and the clock it was written
    /// at: kept in the partition, across restarts, until [`Client::dismiss`].
    pub fn refusals(&mut self, core: &mut dyn Core) -> Result<Json, String> {
        self.require_open(core)?;
        Ok(Json::Array(
            self.journal(core)?
                .into_iter()
                .flat_map(|(_, list)| list)
                .collect(),
        ))
    }

    /// The journal's segments, in order, each with its refusals.
    fn journal(&mut self, core: &mut dyn Core) -> Result<crate::journal::Segments, String> {
        use crate::journal::{index, segment, segment_key, INDEX};
        let mut meta = |key: String| ok(core.call(json!({"op": "meta", "key": key})));
        index(&meta(INDEX.into())?)
            .into_iter()
            .map(|n| Ok((n, segment(&meta(segment_key(n))?))))
            .collect()
    }

    /// Forget refusals the app has shown (by id; `None`: all of them).
    pub fn dismiss(&mut self, core: &mut dyn Core, ids: Option<&[String]>) -> Result<(), String> {
        use crate::journal::{index_text, segment_key, INDEX};
        self.require_open(core)?;
        let keep = |refused: &Json| {
            ids.is_some_and(|ids| !ids.iter().any(|id| refused["id"] == id.as_str()))
        };
        let journal = self.journal(core)?;
        let mut segments = Vec::new();
        for (n, list) in &journal {
            let rest: Vec<Json> = list
                .iter()
                .filter(|refused| keep(refused))
                .cloned()
                .collect();
            if rest.len() != list.len() {
                ok(core.call(json!({"op": "set_meta", "key": segment_key(*n),
                    "value": Json::Array(rest.clone()).to_string()})))?;
            }
            if !rest.is_empty() {
                segments.push(*n);
            }
        }
        if segments.len() != journal.len() {
            ok(core.call(json!({"op": "set_meta", "key": INDEX, "value": index_text(&segments)})))?;
        }
        lock(&self.shared).revision += 1;
        Ok(())
    }

    /// What a screen shows of the link: `{open, online, denied, watermark,
    /// acquired, queued:[{id, op}]}`.
    pub fn status(&mut self, core: &mut dyn Core) -> Result<Json, String> {
        let (online, denied) = {
            let shared = lock(&self.shared);
            (shared.online, shared.denied.clone())
        };
        let revision = lock(&self.shared).revision;
        let mut status = json!({"open": self.is_open(), "online": online, "denied": denied,
            "syncing": self.round.is_some(), "revision": revision});
        if self.is_open() {
            let state = ok(core.call(json!({"op": "sync_state", "capture": false})))?;
            status["watermark"] = state["watermark"].clone();
            status["acquired"] = state["acquired"].clone();
            let queued = ok(core.call(json!({"op": "queued"})))?;
            status["queued"] = queued
                .as_array()
                .into_iter()
                .flatten()
                .map(|entry| json!({"id": entry["id"], "op": entry["op"], "args": entry["args"]}))
                .collect();
        }
        Ok(status)
    }

    /// Start a round — open if needed, sync, send the outbox, sync again —
    /// and run it to its first exchange. A round already running is resumed
    /// only by [`Client::deliver`]; asking again reports it busy.
    pub fn sync(&mut self, core: &mut dyn Core) -> Step {
        if self.round.is_some() {
            return Step::Done(json!({"ok": false, "busy": true}));
        }
        let io = Io(Arc::default());
        let future = Box::pin(crate::round::round(
            io.clone(),
            self.config.clone(),
            self.shared.clone(),
        ));
        self.round = Some(Running {
            future,
            io,
            awaiting: None,
        });
        self.advance(core, None)
    }

    /// Continue the running round with the reply to its last [`Step::Fetch`],
    /// named by that fetch's `exchange`. A reply for any other exchange (a
    /// cancelled round's, a closed client's) is refused and changes nothing.
    pub fn deliver(&mut self, core: &mut dyn Core, exchange: &str, reply: Reply) -> Step {
        match &self.round {
            Some(running) if running.awaiting.as_deref() == Some(exchange) => {
                self.advance(core, Some(Back::Fetch(reply)))
            }
            _ => Step::Done(json!({"ok": false, "stale": true,
                "denied": refusal("E_STALE", "input", "no round awaits this exchange's reply")})),
        }
    }

    /// Abandon the round awaiting `exchange` (its driver went away). Another
    /// round is untouched. Device state stays consistent: every device call
    /// commits on its own, and a receipt the server gave is persisted first
    /// by the next round.
    pub fn cancel(&mut self, exchange: &str) -> bool {
        let owns = self
            .round
            .as_ref()
            .is_some_and(|running| running.awaiting.as_deref() == Some(exchange));
        if owns {
            self.round = None;
        }
        // A refresh whose reply will not come is let go the same way.
        if self.refresh.as_deref() == Some(exchange) {
            self.refresh = None;
            return true;
        }
        owns
    }

    fn advance(&mut self, core: &mut dyn Core, mut back: Option<Back>) -> Step {
        let Some(running) = self.round.as_mut() else {
            return Step::Done(json!({"ok": false}));
        };
        running.awaiting = None;
        loop {
            if let Some(back) = back.take() {
                running.io.channel().back = Some(back);
            }
            let mut context = Context::from_waker(Waker::noop());
            match running.future.as_mut().poll(&mut context) {
                Poll::Ready(outcome) => {
                    self.round = None;
                    return Step::Done(outcome);
                }
                Poll::Pending => match running.io.take_out() {
                    Some(Yield::Device(request)) => back = Some(Back::Device(core.call(request))),
                    Some(Yield::Fetch(mut fetch)) => {
                        self.exchanges += 1;
                        fetch.exchange = format!("{}.{}", self.incarnation, self.exchanges);
                        running.awaiting = Some(fetch.exchange.clone());
                        return Step::Fetch(fetch);
                    }
                    None => {
                        self.round = None;
                        return Step::Done(
                            json!({"ok": false, "denied": refusal("E_STORE", "store", "the round stopped without a request")}),
                        );
                    }
                },
            }
        }
    }

    /// Ask for a replacement session (`POST /auth/refresh`), sent with the
    /// host's current bearer like every exchange. Deliver the reply to
    /// [`Client::refreshed`]. The server retires the presented token before
    /// it answers, so the host keeps the session that comes back before it
    /// uses the link again; a reply lost on the way leaves the member to sign
    /// in again, as Snapback's own client does. One at a time: a second
    /// would present the token the first retires, so it is refused until the
    /// first is delivered or cancelled.
    pub fn refresh(&mut self) -> Result<Fetch, String> {
        if self.refresh.is_some() {
            return Err(
                "E_BUSY: a session refresh is in flight; deliver or cancel it first".into(),
            );
        }
        let mut fetch = Fetch {
            method: "POST",
            path: "/auth/refresh".into(),
            body: None,
            exchange: String::new(),
        };
        self.exchanges += 1;
        fetch.exchange = format!("{}.refresh{}", self.incarnation, self.exchanges);
        self.refresh = Some(fetch.exchange.clone());
        Ok(fetch)
    }

    /// The refresh's answer: `{ok:true, session:{principal, kind, token,
    /// expiresAt}}`, or `{ok:false, denied}` — `E_AUTH` when the session
    /// cannot be refreshed (the member signs in again), `offline: true` when
    /// the server was not reached. A session for another principal, or one
    /// already expired at `now`, is refused.
    pub fn refreshed(&mut self, exchange: &str, reply: Reply, now: i64) -> Json {
        if self.refresh.as_deref() != Some(exchange) {
            return json!({"ok": false, "denied": refusal("E_STALE", "client", "this refresh was superseded or already answered")});
        }
        self.refresh = None;
        let (status, body) = match reply {
            Reply::Failed(error) => {
                lock(&self.shared).online = Some(false);
                return json!({"ok": false, "offline": true, "denied": refusal("E_TRANSPORT", "link", error)});
            }
            Reply::Http { status, body } => (status, body),
        };
        if let Some(why) = body.get("denied").filter(|why| why.is_object()) {
            return json!({"ok": false, "denied": why});
        }
        let session = &body["session"];
        let valid = (200..300).contains(&status)
            && session["token"].as_str().is_some_and(|t| !t.is_empty())
            && session["kind"].is_string()
            && session["expiresAt"]
                .as_f64()
                .is_some_and(|at| at > now as f64);
        if !valid {
            return json!({"ok": false, "denied": refusal("E_HTTP_RESPONSE", "link",
                format!("/auth/refresh: HTTP {status} without a live session; sign in again if this persists"))});
        }
        if session["principal"] != self.config.viewer.as_str() {
            return json!({"ok": false, "denied": refusal("E_AUTH", "auth", "a refreshed session changed principal")});
        }
        json!({"ok": true, "session": session})
    }

    /// The change poll: `GET /changes` from the partition's watermark. Deliver
    /// its reply to [`Client::changed`] with its `exchange`; `true` means start
    /// a round. Asking again supersedes an unanswered poll.
    pub fn changes(&mut self, core: &mut dyn Core, wait_seconds: u32) -> Result<Fetch, String> {
        self.require_open(core)?;
        let state = ok(core.call(json!({"op": "sync_state", "capture": false})))?;
        let mut path = format!(
            "/changes?since={}&wait={}",
            state["watermark"].as_u64().unwrap_or(0),
            wait_seconds.min(55)
        );
        if let Some(store) = state["store_id"].as_str() {
            path.push_str("&store_id=");
            path.push_str(&encode(store));
        }
        let mut fetch = Fetch::get(path);
        self.exchanges += 1;
        fetch.exchange = format!("{}.poll{}", self.incarnation, self.exchanges);
        self.poll = Some(fetch.exchange.clone());
        Ok(fetch)
    }

    /// Whether a change reply calls for a round: the head moved, the store
    /// was replaced, or the backend's generation changed. A refusal or a
    /// transport failure is `Err`, and the poll's caller waits before asking
    /// again.
    pub fn changed(
        &mut self,
        core: &mut dyn Core,
        exchange: &str,
        reply: Reply,
    ) -> Result<bool, String> {
        if self.poll.as_deref() != Some(exchange) {
            return Err("E_STALE: this change poll was superseded or already answered".into());
        }
        self.poll = None;
        let body = match reply {
            Reply::Failed(error) => {
                lock(&self.shared).online = Some(false);
                return Err(error);
            }
            Reply::Http { status, body }
                if (200..300).contains(&status) && body.get("denied").is_none() =>
            {
                body
            }
            Reply::Http { status, body } => {
                let why = body
                    .get("denied")
                    .cloned()
                    .unwrap_or(json!({"message": format!("changes: HTTP {status}")}));
                return Err(why["message"].as_str().unwrap_or("changes refused").into());
            }
        };
        lock(&self.shared).online = Some(true);
        let state = ok(core.call(json!({"op": "sync_state", "capture": false})))?;
        let generation = ok(core.call(json!({"op": "state"})))?["generation"].clone();
        let replaced = body
            .get("store_id")
            .is_some_and(|id| *id != state["store_id"]);
        let moved = body["seq"].as_u64() != state["watermark"].as_u64();
        let regenerated = body
            .get("generation")
            .is_some_and(|g| !g.is_null() && *g != generation);
        Ok(replaced || moved || regenerated)
    }
}

/// A keyed write's id, or its `index`th new row id, from the device's
/// identity and the app's idempotency key: the same intent, the same id.
/// A digest of a write's operation and input, its keys in sorted order: what
/// a key is bound to, kept with its receipt and its refusal.
pub(crate) fn input_key(op: &str, args: &Json) -> String {
    use sha2::{Digest, Sha256};
    fn canonical(value: &Json, out: &mut String) {
        match value {
            Json::Object(fields) => {
                let mut keys: Vec<&String> = fields.keys().collect();
                keys.sort();
                out.push('{');
                for (i, key) in keys.into_iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    out.push_str(&Json::String(key.clone()).to_string());
                    out.push(':');
                    canonical(&fields[key], out);
                }
                out.push('}');
            }
            Json::Array(items) => {
                out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    canonical(item, out);
                }
                out.push(']');
            }
            other => out.push_str(&other.to_string()),
        }
    }
    let mut text = String::new();
    canonical(args, &mut text);
    let mut hash = Sha256::new();
    hash.update(b"exact-snapback4 write input\0");
    hash.update(op.as_bytes());
    hash.update([0]);
    hash.update(text.as_bytes());
    crockford(&hash.finalize())
}

pub(crate) fn keyed_id(device: &str, key: &str, index: Option<u8>) -> String {
    use sha2::{Digest, Sha256};
    let mut hash = Sha256::new();
    hash.update(b"exact-snapback4 keyed write\0");
    hash.update(device.as_bytes());
    hash.update([0]);
    hash.update(key.as_bytes());
    hash.update([index.map_or(0, |i| i + 1)]);
    crockford(&hash.finalize())
}

fn crockford(digest: &[u8]) -> String {
    let mut bits = u128::from_be_bytes(digest[..16].try_into().unwrap());
    const DIGITS: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
    let mut out = [0u8; 26];
    for slot in out.iter_mut().rev() {
        *slot = DIGITS[(bits & 31) as usize];
        bits >>= 5;
    }
    String::from_utf8(out.to_vec()).unwrap()
}

/// A write's id, or its `index`th new row id: 26 Crockford base32 digits, as
/// Snapback mints them, derived from the device's random identity and its
/// submission number. Unique per device without a clock or a fresh draw.
pub(crate) fn write_id(device: &str, seq: u64, index: Option<u8>) -> String {
    use sha2::{Digest, Sha256};
    let mut hash = Sha256::new();
    hash.update(b"exact-snapback4 write id\0");
    hash.update(device.as_bytes());
    hash.update(seq.to_be_bytes());
    hash.update([index.map_or(0, |i| i + 1)]);
    crockford(&hash.finalize())
}

pub(crate) fn encode(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}
